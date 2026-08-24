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

//! The `fastwarc.v1.WarcService` implementation.

mod batch;
mod channel_reader;
mod parser;

use std::io;

use fastwarc::stream_io::bufread::RawReaderAdapter;
use prost::bytes::Bytes;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};

use self::batch::BatchEmitter;
use self::channel_reader::ChannelReader;
use self::parser::{parse_into, record_error};
use crate::convert;
use crate::defaults::{DEFAULT_INPUT_BUFFER_SIZE, MAX_HEADER_LEN, MAX_INPUT_BUFFER_SIZE, MAX_PAYLOAD_CHUNK_SIZE};
use crate::proto::fastwarc::v1 as pb;

const CHUNK_CHANNEL_BOUND: usize = 8;
const RESPONSE_CHANNEL_BOUND: usize = 8;

type ResponseSender = mpsc::Sender<Result<pb::ParseWarcResponse, Status>>;

enum ParserInput {
    Chunks(mpsc::Receiver<Bytes>),
    LocalFile,
}

/// The `fastwarc.v1.WarcService` gRPC service.
#[derive(Default)]
pub struct WarcParser {
    allow_local_files: bool,
}

impl WarcParser {
    /// A parser that rejects server-side archive paths.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A parser that permits server-side paths. Only use this when every
    /// client is trusted with read access to the server's files.
    #[must_use]
    pub fn with_local_files() -> Self {
        Self {
            allow_local_files: true,
        }
    }
}

#[tonic::async_trait]
impl pb::warc_service_server::WarcService for WarcParser {
    type ParseWarcStream = ReceiverStream<Result<pb::ParseWarcResponse, Status>>;

    async fn parse_warc(
        &self,
        request: Request<Streaming<pb::ParseWarcRequest>>,
    ) -> Result<Response<Self::ParseWarcStream>, Status> {
        let mut stream = request.into_inner();
        let config = read_config(&mut stream).await?;
        validate_config(&config)?;
        if !config.archive_path.is_empty() && !self.allow_local_files {
            return Err(Status::permission_denied(
                "archive_path is disabled on this server; stream the archive as chunks instead",
            ));
        }

        let (response_tx, response_rx) = mpsc::channel(RESPONSE_CHANNEL_BOUND);
        if config.archive_path.is_empty() {
            let (chunk_tx, chunk_rx) = mpsc::channel::<Bytes>(CHUNK_CHANNEL_BOUND);
            spawn_parser(ParserInput::Chunks(chunk_rx), response_tx.clone(), config);
            tokio::spawn(forward_chunks(stream, chunk_tx, response_tx));
        } else {
            spawn_parser(ParserInput::LocalFile, response_tx, config);
        }
        Ok(Response::new(ReceiverStream::new(response_rx)))
    }

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
            Ok(response) => Ok(Response::new(response)),
            Err(error) if error.is_panic() => Err(Status::internal("WARC parser task panicked")),
            Err(_) => Err(Status::cancelled("WARC parser task cancelled")),
        }
    }
}

async fn read_config(stream: &mut Streaming<pb::ParseWarcRequest>) -> Result<pb::ParseWarcConfig, Status> {
    match stream.message().await? {
        Some(pb::ParseWarcRequest {
            kind: Some(pb::parse_warc_request::Kind::Config(config)),
        }) => Ok(config),
        Some(_) => Err(Status::invalid_argument("first ParseWarc request message must set `config`")),
        None => Err(Status::invalid_argument("empty ParseWarc request stream; first message must set `config`")),
    }
}

fn spawn_parser(input: ParserInput, response_tx: ResponseSender, config: pb::ParseWarcConfig) {
    let error_tx = response_tx.clone();
    // Report parser task failures as terminal gRPC errors.
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

async fn forward_chunks(
    mut stream: Streaming<pb::ParseWarcRequest>,
    chunk_tx: mpsc::Sender<Bytes>,
    response_tx: ResponseSender,
) {
    loop {
        match stream.message().await {
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

fn validate_limit(name: &str, value: u32, max: usize) -> Result<(), Status> {
    if usize::try_from(value).unwrap_or(usize::MAX) > max {
        Err(Status::invalid_argument(format!("{name} must not exceed {max} bytes")))
    } else {
        Ok(())
    }
}

fn run_parser(input: ParserInput, response_tx: &ResponseSender, config: &pb::ParseWarcConfig) {
    let mut emitter = BatchEmitter::new(response_tx, convert::response_batch_size(config));
    let mut emit = |response| emitter.emit(response);
    match input {
        ParserInput::Chunks(chunk_rx) => {
            parse_into(RawReaderAdapter::new(ChannelReader::new(chunk_rx)), config, &mut emit);
        }
        ParserInput::LocalFile => {
            let input_buffer_size = if config.input_buffer_size == 0 {
                DEFAULT_INPUT_BUFFER_SIZE
            } else {
                config.input_buffer_size as usize
            };
            match std::fs::File::open(&config.archive_path) {
                Ok(file) => parse_into(io::BufReader::with_capacity(input_buffer_size, file), config, &mut emit),
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

fn collect_archive(
    reader: impl fastwarc::stream_io::traits::IntoWarcReader,
    config: &pb::ParseWarcConfig,
) -> pb::ParseArchiveResponse {
    let mut records = Vec::new();
    let mut errors = Vec::new();
    let mut open: Option<pb::ParsedRecord> = None;
    let mut emit = |response| {
        fold_response(response, &mut open, &mut records, &mut errors);
        true
    };
    parse_into(reader, config, &mut emit);
    pb::ParseArchiveResponse { records, errors }
}

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
