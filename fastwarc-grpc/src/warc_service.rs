// Copyright 2026 Kristian Rickert
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Streaming and unary RPCs backed by blocking parser tasks.
//! Input chunks pass through `channel_reader`; `parser` produces record
//! events, and `batch` groups them for the streaming response.

mod batch;
mod channel_reader;
mod parser;

use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use cap_std::fs::Dir;
use fastwarc::stream_io::bufread::RawReaderAdapter;
use prost::bytes::Bytes;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};

use self::batch::BatchEmitter;
use self::channel_reader::ChannelReader;
use self::parser::{parse_into, record_error};
use crate::convert;
use crate::defaults::{
    DEFAULT_INPUT_BUFFER_SIZE, MAX_HEADER_LEN, MAX_INPUT_BUFFER_SIZE, MAX_PAYLOAD_CHUNK_SIZE, MAX_UNARY_RESPONSE_SIZE,
};
use crate::proto::fastwarc::v1 as pb;

/// Maximum queued input chunks.
const CHUNK_CHANNEL_BOUND: usize = 8;
/// Maximum queued response messages.
const RESPONSE_CHANNEL_BOUND: usize = 8;

/// Response channel shared by the parser and request forwarder.
type ResponseSender = mpsc::Sender<Result<pb::ParseWarcResponse, Status>>;

/// Where the parser task reads archive bytes from.
enum ParserInput {
    /// Chunks streamed by the client, delivered through a bounded channel.
    Chunks(mpsc::Receiver<Bytes>),
    /// A path opened relative to the configured directory handle.
    LocalFile(Arc<LocalFileRoot>, PathBuf),
}

/// The `fastwarc.v1.WarcService` gRPC service.
///
/// Use [`Self::new`] for uploaded archives or [`Self::with_local_files`] to
/// enable files under a configured directory.
#[derive(Default)]
pub struct WarcParser {
    /// Directory that `archive_path` requests are confined to. `None`
    /// rejects every server-side path.
    local_file_root: Option<Arc<LocalFileRoot>>,
}

/// The configured directory, held open so requests cannot redirect its path.
struct LocalFileRoot {
    /// Canonical name used to accept absolute paths under the root.
    path: PathBuf,
    /// Directory handle used for confined opens.
    dir: Dir,
}

impl WarcParser {
    /// A parser that rejects server-side archive paths.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A parser that permits server-side paths confined to the `root`
    /// directory.
    ///
    /// Relative paths are opened beneath a retained directory handle.
    /// Absolute paths must start with the canonical root; `..` and symlinks
    /// that escape it are rejected. Symlinks must use relative targets.
    /// Clients must be trusted with read access to everything under `root`.
    ///
    /// # Errors
    ///
    /// Fails if `root` cannot be canonicalized or is not a directory.
    pub fn with_local_files(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = root.as_ref().canonicalize()?;
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("local file root {} is not a directory", root.display()),
            ));
        }
        let dir = Dir::open_ambient_dir(&root, cap_std::ambient_authority())?;
        Ok(Self {
            local_file_root: Some(Arc::new(LocalFileRoot { path: root, dir })),
        })
    }
}

/// Converts an archive path to a relative path without parent traversal.
/// Symlink confinement is enforced when the directory handle opens the file.
fn relative_archive_path(root: &Path, requested: &str) -> Result<PathBuf, Status> {
    let denied = || Status::permission_denied("archive_path must stay inside the configured local file root");
    let path = Path::new(requested);
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(denied());
    }
    if path.has_root() || path.components().any(|c| matches!(c, Component::Prefix(_))) {
        path.strip_prefix(root).map(Path::to_path_buf).map_err(|_| denied())
    } else {
        Ok(path.to_path_buf())
    }
}

#[tonic::async_trait]
impl pb::warc_service_server::WarcService for WarcParser {
    type ParseWarcStream = ReceiverStream<Result<pb::ParseWarcResponse, Status>>;

    /// Validates the configuration and starts the parser and request-forwarding tasks.
    async fn parse_warc(
        &self,
        request: Request<Streaming<pb::ParseWarcRequest>>,
    ) -> Result<Response<Self::ParseWarcStream>, Status> {
        let mut stream = request.into_inner();
        let config = read_config(&mut stream).await?;
        validate_config(&config)?;
        let local_path = if config.archive_path.is_empty() {
            None
        } else if let Some(root) = &self.local_file_root {
            Some((Arc::clone(root), relative_archive_path(&root.path, &config.archive_path)?))
        } else {
            return Err(Status::permission_denied(
                "archive_path is disabled on this server; stream the archive as chunks instead",
            ));
        };

        let (response_tx, response_rx) = mpsc::channel(RESPONSE_CHANNEL_BOUND);
        if let Some((root, path)) = local_path {
            spawn_parser(ParserInput::LocalFile(root, path), response_tx, config);
        } else {
            let (chunk_tx, chunk_rx) = mpsc::channel::<Bytes>(CHUNK_CHANNEL_BOUND);
            spawn_parser(ParserInput::Chunks(chunk_rx), response_tx.clone(), config);
            tokio::spawn(forward_chunks(stream, chunk_tx, response_tx));
        }
        Ok(Response::new(ReceiverStream::new(response_rx)))
    }

    /// Collects an uploaded archive on a blocking task. Local paths are rejected.
    async fn parse_archive(
        &self,
        request: Request<pb::ParseArchiveRequest>,
    ) -> Result<Response<pb::ParseArchiveResponse>, Status> {
        let request = request.into_inner();
        let Some(config) = request.config else {
            return Err(Status::invalid_argument("ParseArchive request must set `config`"));
        };
        validate_config(&config)?;
        if !config.archive_path.is_empty() {
            return Err(Status::invalid_argument("archive_path is only supported by ParseWarc"));
        }

        let archive = request.archive;
        let joined = tokio::task::spawn_blocking(move || collect_archive(io::Cursor::new(archive), &config)).await;
        match joined {
            Ok(response) => Ok(Response::new(response?)),
            Err(error) if error.is_panic() => Err(Status::internal("WARC parser task panicked")),
            Err(_) => Err(Status::cancelled("WARC parser task cancelled")),
        }
    }
}

/// Reads the required first configuration message, rejecting other message kinds.
async fn read_config(stream: &mut Streaming<pb::ParseWarcRequest>) -> Result<pb::ParseWarcConfig, Status> {
    match stream.message().await? {
        Some(pb::ParseWarcRequest {
            kind: Some(pb::parse_warc_request::Kind::Config(config)),
        }) => Ok(config),
        Some(_) => Err(Status::invalid_argument("first ParseWarc request message must set `config`")),
        None => Err(Status::invalid_argument("empty ParseWarc request stream; first message must set `config`")),
    }
}

/// Runs the parser on a blocking task and reports task failures as gRPC errors.
fn spawn_parser(input: ParserInput, response_tx: ResponseSender, config: pb::ParseWarcConfig) {
    let error_tx = response_tx.clone();
    tokio::spawn(async move {
        let joined = tokio::task::spawn_blocking(move || run_parser(input, &response_tx, &config)).await;
        if let Err(error) = joined {
            let status = if error.is_panic() {
                Status::internal("WARC parser task panicked")
            } else {
                Status::cancelled("WARC parser task cancelled")
            };
            let _ = error_tx.send(Err(status)).await;
        }
    });
}

/// Forwards chunks until input ends or the parser stops. Duplicate configuration
/// messages and missing message kinds terminate the RPC with `InvalidArgument`.
async fn forward_chunks(
    mut stream: Streaming<pb::ParseWarcRequest>,
    chunk_tx: mpsc::Sender<Bytes>,
    response_tx: ResponseSender,
) {
    loop {
        // Release the response sender when the parser stops, even if upload stays open.
        let message = tokio::select! {
            biased;
            () = chunk_tx.closed() => return,
            message = stream.message() => message,
        };
        match message {
            Ok(Some(pb::ParseWarcRequest {
                kind: Some(pb::parse_warc_request::Kind::Chunk(chunk)),
            })) => {
                if chunk_tx.send(chunk).await.is_err() {
                    return;
                }
            }
            Ok(Some(pb::ParseWarcRequest {
                kind: Some(pb::parse_warc_request::Kind::Config(_)),
            })) => {
                // Already queued parser events may precede this terminal status.
                let _ = response_tx
                    .send(Err(Status::invalid_argument("`config` may only be set on the first request message")))
                    .await;
                return;
            }
            Ok(Some(pb::ParseWarcRequest { kind: None })) => {
                let _ = response_tx
                    .send(Err(Status::invalid_argument("ParseWarc request message must set `config` or `chunk`")))
                    .await;
                return;
            }
            Ok(None) => return,
            Err(error) => {
                let _ = response_tx.send(Err(error)).await;
                return;
            }
        }
    }
}

/// Rejects sizes above their limits and inverted content-length bounds.
fn validate_config(config: &pb::ParseWarcConfig) -> Result<(), Status> {
    validate_limit("max_header_len", config.max_header_len, MAX_HEADER_LEN)?;
    validate_limit("payload_chunk_size", config.payload_chunk_size, MAX_PAYLOAD_CHUNK_SIZE)?;
    validate_limit("input_buffer_size", config.input_buffer_size, MAX_INPUT_BUFFER_SIZE)?;
    if let (Some(min), Some(max)) = (config.min_content_length, config.max_content_length)
        && min > max
    {
        return Err(Status::invalid_argument("min_content_length must not exceed max_content_length"));
    }
    Ok(())
}

/// Rejects values above `max`; zero selects the default.
fn validate_limit(name: &str, value: u32, max: usize) -> Result<(), Status> {
    if usize::try_from(value).unwrap_or(usize::MAX) > max {
        Err(Status::invalid_argument(format!("{name} must not exceed {max} bytes")))
    } else {
        Ok(())
    }
}

/// Parses the selected input and flushes any pending response batch.
fn run_parser(input: ParserInput, response_tx: &ResponseSender, config: &pb::ParseWarcConfig) {
    let mut emitter = BatchEmitter::new(response_tx, convert::response_batch_size(config));
    let mut emit = |response| emitter.emit(response);
    match input {
        ParserInput::Chunks(chunk_rx) => {
            parse_into(RawReaderAdapter::new(ChannelReader::new(chunk_rx)), config, &mut emit);
        }
        ParserInput::LocalFile(root, path) => {
            let input_buffer_size = if config.input_buffer_size == 0 {
                DEFAULT_INPUT_BUFFER_SIZE
            } else {
                config.input_buffer_size as usize
            };
            match root.dir.open(&path) {
                Ok(file) => {
                    parse_into(io::BufReader::with_capacity(input_buffer_size, file.into_std()), config, &mut emit);
                }
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    let _ = response_tx.blocking_send(Err(Status::permission_denied(
                        "archive_path cannot be opened inside the configured local file root",
                    )));
                }
                Err(error) => {
                    emit(record_error(
                        0,
                        false,
                        format!("failed to open archive_path {}: {error}", config.archive_path),
                    ));
                }
            }
        }
    }
    emitter.flush();
}

/// Collects records for the unary RPC, stopping when the response budget is exceeded.
fn collect_archive(
    reader: impl fastwarc::stream_io::traits::IntoWarcReader,
    config: &pb::ParseWarcConfig,
) -> Result<pb::ParseArchiveResponse, Status> {
    let mut records = Vec::new();
    let mut errors = Vec::new();
    let mut open: Option<pb::ParsedRecord> = None;
    // Compressed request size does not bound the decoded response.
    let mut collected: usize = 0;
    let mut emit = |response: pb::ParseWarcResponse| {
        collected = collected.saturating_add(event_size(&response));
        if collected > MAX_UNARY_RESPONSE_SIZE {
            return false;
        }
        fold_response(response, &mut open, &mut records, &mut errors);
        true
    };
    parse_into(reader, config, &mut emit);
    if collected > MAX_UNARY_RESPONSE_SIZE {
        return Err(Status::resource_exhausted(format!(
            "archive decodes past the {MAX_UNARY_RESPONSE_SIZE}-byte ParseArchive response budget; \
             use the streaming ParseWarc RPC for large archives"
        )));
    }
    Ok(pb::ParseArchiveResponse { records, errors })
}

/// Conservative size of the data and protobuf framing added to a unary response.
fn event_size(response: &pb::ParseWarcResponse) -> usize {
    use prost::Message;
    match &response.kind {
        Some(pb::parse_warc_response::Kind::RecordStart(start)) => {
            // Reserve maximum prefixes for the enclosing record and its payload.
            let prefix = 1 + prost::length_delimiter_len(MAX_UNARY_RESPONSE_SIZE);
            start.metadata.as_ref().map_or(0, |metadata| {
                let size = metadata.encoded_len();
                1 + prost::length_delimiter_len(size) + size
            }) + 2 * prefix
        }
        Some(pb::parse_warc_response::Kind::PayloadChunk(chunk)) => chunk.data.len(),
        Some(pb::parse_warc_response::Kind::RecordError(error)) => {
            let size = error.encoded_len();
            1 + prost::length_delimiter_len(size) + size
        }
        Some(pb::parse_warc_response::Kind::Batch(batch)) => {
            batch.items.iter().map(event_size).fold(0, usize::saturating_add)
        }
        Some(pb::parse_warc_response::Kind::RecordEnd(_)) | None => 0,
    }
}

/// Assembles payloads between start/end events and discards records with errors.
fn fold_response(
    response: pb::ParseWarcResponse,
    open: &mut Option<pb::ParsedRecord>,
    records: &mut Vec<pb::ParsedRecord>,
    errors: &mut Vec<pb::RecordError>,
) {
    match response.kind {
        Some(pb::parse_warc_response::Kind::RecordStart(start)) => {
            *open = Some(pb::ParsedRecord {
                metadata: start.metadata,
                ..Default::default()
            });
        }
        Some(pb::parse_warc_response::Kind::PayloadChunk(chunk)) => {
            if let Some(record) = open.as_mut() {
                record.payload.extend_from_slice(&chunk.data);
            }
        }
        Some(pb::parse_warc_response::Kind::RecordEnd(_)) => {
            if let Some(record) = open.take() {
                records.push(record);
            }
        }
        Some(pb::parse_warc_response::Kind::RecordError(error)) => {
            open.take();
            errors.push(error);
        }
        Some(pb::parse_warc_response::Kind::Batch(batch)) => {
            for item in batch.items {
                fold_response(item, open, records, errors);
            }
        }
        None => {}
    }
}

#[cfg(test)]
#[path = "warc_service_test.rs"]
mod warc_service_test;
