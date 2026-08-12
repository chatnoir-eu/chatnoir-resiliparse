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
//!
//! Both RPCs run the same blocking parse pipeline against an emit callback.
//! For `ParseWarc`, request chunks feed a private `ChannelReader` via `mpsc`
//! and emitted messages return on a second bounded channel
//! (`ReceiverStream`). For the unary `ParseArchive`, the request bytes are
//! parsed from an in-memory cursor and the emitted messages are folded into
//! a single response.

use std::io::{self, Read, Seek, SeekFrom};

use fastwarc::stream_io::traits::IntoWarcReader;
use fastwarc::warc::iter::{ArchiveIterator, ArchiveIteratorOptions};
use fastwarc::warc::record::WarcRecord;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};

use crate::convert;
use crate::proto::fastwarc::v1 as pb;

/// Default maximum WARC/HTTP header block length (matches the crate default).
const DEFAULT_MAX_HEADER_LEN: usize = 32 << 10;
/// Default payload chunk size for `payload_chunk` messages.
const DEFAULT_PAYLOAD_CHUNK_SIZE: usize = 64 << 10;
/// Default buffer capacity of the byte stream fed into the parser.
const DEFAULT_INPUT_BUFFER_SIZE: usize = 64 << 10;
/// Bound of the request-chunk channel into the parser thread.
const CHUNK_CHANNEL_BOUND: usize = 8;
/// Bound of the response channel back to the client.
const RESPONSE_CHANNEL_BOUND: usize = 32;

type ResponseSender = mpsc::Sender<Result<pb::ParseWarcResponse, Status>>;

/// Consumer of parse protocol messages; returns `false` when the consumer is
/// gone and the parse should stop.
type EmitFn<'a> = &'a mut dyn FnMut(pb::ParseWarcResponse) -> bool;

/// The `fastwarc.v1.WarcService` gRPC service (stateless).
pub struct WarcParser;

#[tonic::async_trait]
impl pb::warc_service_server::WarcService for WarcParser {
    type ParseWarcStream = ReceiverStream<Result<pb::ParseWarcResponse, Status>>;

    async fn parse_warc(
        &self,
        request: Request<Streaming<pb::ParseWarcRequest>>,
    ) -> Result<Response<Self::ParseWarcStream>, Status> {
        let mut stream = request.into_inner();

        // First message must carry config.
        let config = match stream.message().await {
            Ok(Some(msg)) => match msg.kind {
                Some(pb::parse_warc_request::Kind::Config(config)) => config,
                _ => {
                    return Err(Status::invalid_argument("first ParseWarc request message must set `config`"));
                }
            },
            Ok(None) => {
                return Err(Status::invalid_argument(
                    "empty ParseWarc request stream; first message must set `config`",
                ));
            }
            Err(e) => return Err(e),
        };

        let (chunk_tx, chunk_rx) = mpsc::channel::<Vec<u8>>(CHUNK_CHANNEL_BOUND);
        let (resp_tx, resp_rx) = mpsc::channel(RESPONSE_CHANNEL_BOUND);
        let forwarder_tx = resp_tx.clone();
        let panic_tx = resp_tx.clone();

        // Map JoinError (panic/cancel) to a gRPC status; bare spawn_blocking
        // would drop the sender and look like a clean EOF.
        tokio::spawn(async move {
            match tokio::task::spawn_blocking(move || {
                run_parser(chunk_rx, &resp_tx, &config);
            })
            .await
            {
                Ok(()) => {}
                Err(e) if e.is_panic() => {
                    let _ = panic_tx.send(Err(Status::internal("WARC parser task panicked"))).await;
                }
                Err(_) => {
                    let _ = panic_tx
                        .send(Err(Status::cancelled("WARC parser task cancelled")))
                        .await;
                }
            }
        });

        // Forward archive bytes into the parser thread.
        tokio::spawn(async move {
            loop {
                match stream.message().await {
                    Ok(Some(msg)) => match msg.kind {
                        Some(pb::parse_warc_request::Kind::Chunk(chunk)) => {
                            if chunk_tx.send(chunk).await.is_err() {
                                break;
                            }
                        }
                        Some(pb::parse_warc_request::Kind::Config(_)) => {
                            let _ = forwarder_tx
                                .send(Err(Status::invalid_argument(
                                    "`config` may only be set on the first request message",
                                )))
                                .await;
                            break;
                        }
                        None => {
                            let _ = forwarder_tx
                                .send(Err(Status::invalid_argument(
                                    "ParseWarc request message must set `config` or `chunk`",
                                )))
                                .await;
                            break;
                        }
                    },
                    Ok(None) => break,
                    Err(e) => {
                        let _ = forwarder_tx.send(Err(e)).await;
                        break;
                    }
                }
            }
            // Dropping chunk_tx signals EOF to ChannelReader.
        });

        Ok(Response::new(ReceiverStream::new(resp_rx)))
    }

    async fn parse_archive(
        &self,
        request: Request<pb::ParseArchiveRequest>,
    ) -> Result<Response<pb::ParseArchiveResponse>, Status> {
        let request = request.into_inner();
        let Some(config) = request.config else {
            return Err(Status::invalid_argument("ParseArchive request must set `config`"));
        };
        let archive = request.archive;

        let joined = tokio::task::spawn_blocking(move || {
            let mut records = Vec::new();
            let mut errors = Vec::new();
            let mut open: Option<pb::ParsedRecord> = None;
            let mut emit = |resp: pb::ParseWarcResponse| {
                fold_response(resp, &mut open, &mut records, &mut errors);
                true
            };
            parse_into(io::Cursor::new(archive), &config, &mut emit);
            pb::ParseArchiveResponse { records, errors }
        })
        .await;
        match joined {
            Ok(response) => Ok(Response::new(response)),
            Err(e) if e.is_panic() => Err(Status::internal("WARC parser task panicked")),
            Err(_) => Err(Status::cancelled("WARC parser task cancelled")),
        }
    }
}

/// Fold one parse protocol message into the unary response accumulators.
///
/// The pipeline guarantees well-formed sequences (`record_start` before
/// chunks and `record_end`, errors outside record sequences), so the fold
/// does not re-validate them.
fn fold_response(
    resp: pb::ParseWarcResponse,
    open: &mut Option<pb::ParsedRecord>,
    records: &mut Vec<pb::ParsedRecord>,
    errors: &mut Vec<pb::RecordError>,
) {
    match resp.kind {
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
        Some(pb::parse_warc_response::Kind::RecordEnd(end)) => {
            if let Some(mut record) = open.take() {
                record.block_digest_status = end.block_digest_status;
                record.payload_digest_status = end.payload_digest_status;
                record.digest_detail = end.digest_detail;
                records.push(record);
            }
        }
        Some(pb::parse_warc_response::Kind::RecordError(error)) => errors.push(error),
        None => {}
    }
}

fn response(kind: pb::parse_warc_response::Kind) -> pb::ParseWarcResponse {
    pb::ParseWarcResponse { kind: Some(kind) }
}

fn record_error(stream_pos: u64, recoverable: bool, message: String) -> pb::ParseWarcResponse {
    response(pb::parse_warc_response::Kind::RecordError(pb::RecordError {
        stream_pos,
        recoverable,
        message,
    }))
}

/// Blocking parse entry point for the streaming RPC: reads archive bytes
/// from `chunk_rx` and emits protocol messages on `resp_tx`.
fn run_parser(chunk_rx: mpsc::Receiver<Vec<u8>>, resp_tx: &ResponseSender, config: &pb::ParseWarcConfig) {
    let input_buffer_size = if config.input_buffer_size == 0 {
        DEFAULT_INPUT_BUFFER_SIZE
    } else {
        config.input_buffer_size as usize
    };
    let reader = io::BufReader::with_capacity(input_buffer_size, ChannelReader::new(chunk_rx));
    let mut emit = |resp: pb::ParseWarcResponse| resp_tx.blocking_send(Ok(resp)).is_ok();
    parse_into(reader, config, &mut emit);
}

/// Core parse loop shared by both RPCs: `record_start` / `payload_chunk`* /
/// `record_end` per kept record, or `record_error` on failure, delivered to
/// `emit` in stream order.
///
/// Iterator-level errors (invalid WARC framing) end the parse: the crate
/// detaches the reader on those failures and subsequent `next()` calls loop
/// on `"No reader set"`. HTTP-header failures inside an already-framed record
/// are recoverable; the iterator consumes the remainder on the next step.
fn parse_into(reader: impl IntoWarcReader, config: &pb::ParseWarcConfig, emit: EmitFn<'_>) {
    let chunk_size = if config.payload_chunk_size == 0 {
        DEFAULT_PAYLOAD_CHUNK_SIZE
    } else {
        config.payload_chunk_size as usize
    };
    let max_header_len = if config.max_header_len == 0 {
        DEFAULT_MAX_HEADER_LEN
    } else {
        config.max_header_len as usize
    };
    // Unset optional defaults to true (Python/Rust ArchiveIterator default).
    let stream_detect = config.stream_detect.unwrap_or(true);
    let options = ArchiveIteratorOptions {
        stream_detect,
        // HTTP parse is manual after block-digest verify: WARC-Block-Digest
        // covers the raw block and must run before HTTP advances the stream.
        parse_http: false,
        decode_http_payload: convert::auto_decode(config.decode_http_payload),
        // Iterator-level verify skips mismatches; we verify per record so
        // results can be reported on the wire.
        verify_digests: false,
        quirks_mode: config.quirks_mode,
        max_header_len,
        inplace: false,
    };
    let iterator = ArchiveIterator::with_options(reader, options);

    let mut record_index = 0u64;
    for item in iterator {
        let record = match item {
            Ok(record) => record,
            Err(e) => {
                // Framing failure: the crate has lost the reader; do not continue.
                let _ = emit(record_error(0, false, e.to_string()));
                return;
            }
        };

        {
            let mut borrowed = record.borrow_mut();
            if !convert::record_passes_filters(&mut borrowed, config) {
                // Skip without emitting; next() consumes any unread payload.
                continue;
            }
        }

        match process_record(&record, record_index, config, max_header_len, chunk_size, emit) {
            ProcessOutcome::Stop => return,
            // Advance for both successful emits and recoverable record_errors so
            // indexes stay aligned with framed, non-filtered records (skips do not
            // consume an index).
            ProcessOutcome::Emitted | ProcessOutcome::Continue => record_index += 1,
        }
    }
}

/// Result of attempting to emit one framed record.
enum ProcessOutcome {
    /// `record_start` / chunks / `record_end` were sent; advance `record_index`.
    Emitted,
    /// Recoverable per-record failure (`record_error`); parse continues, index unchanged.
    Continue,
    /// Fatal: consumer gone or non-recoverable payload failure.
    Stop,
}

/// One record: block digest, optional HTTP parse, payload digest, then
/// `record_start` / `payload_chunk`* / `record_end`.
///
/// Digests run before payload streaming because verification rewinds a frozen
/// record, not a live stream.
fn process_record(
    shared: &std::rc::Rc<std::cell::RefCell<WarcRecord>>,
    record_index: u64,
    config: &pb::ParseWarcConfig,
    max_header_len: usize,
    chunk_size: usize,
    emit: EmitFn<'_>,
) -> ProcessOutcome {
    let mut record = shared.borrow_mut();

    let (block_digest_status, block_detail) = if config.verify_digests {
        convert::digest_status(record.verify_block_digest(false))
    } else {
        (pb::DigestStatus::Unspecified, None)
    };

    if config.parse_http
        && let Err(e) = record.parse_http_with_opts(
            convert::auto_decode(config.decode_http_payload),
            max_header_len,
            config.quirks_mode,
        )
    {
        // Already-framed record: the iterator will consume the remainder on
        // the next step, so this is recoverable.
        let stream_pos = record.stream_pos();
        return if emit(record_error(stream_pos, true, format!("failed to parse HTTP headers: {e}"))) {
            ProcessOutcome::Continue
        } else {
            ProcessOutcome::Stop
        };
    }

    let (payload_digest_status, payload_detail) = if config.verify_digests {
        convert::digest_status(record.verify_payload_digest(false))
    } else {
        (pb::DigestStatus::Unspecified, None)
    };

    let metadata = convert::record_metadata(&record, record_index);
    if !emit(response(pb::parse_warc_response::Kind::RecordStart(pb::RecordStart {
        metadata: Some(metadata),
    }))) {
        return ProcessOutcome::Stop;
    }

    let payload_length = match stream_payload(&mut record, record_index, chunk_size, emit) {
        Ok(len) => len,
        Err(e) => {
            let stream_pos = record.stream_pos();
            let _ = emit(record_error(stream_pos, false, format!("failed to read record payload: {e}")));
            return ProcessOutcome::Stop;
        }
    };

    let details: Vec<String> = [block_detail, payload_detail].into_iter().flatten().collect();

    if emit(response(pb::parse_warc_response::Kind::RecordEnd(pb::RecordEnd {
        record_index,
        payload_length,
        block_digest_status: block_digest_status.into(),
        payload_digest_status: payload_digest_status.into(),
        digest_detail: if details.is_empty() {
            None
        } else {
            Some(details.join("; "))
        },
    }))) {
        ProcessOutcome::Emitted
    } else {
        ProcessOutcome::Stop
    }
}

/// Emit remaining payload as `payload_chunk` messages; return total bytes.
fn stream_payload(record: &mut WarcRecord, record_index: u64, chunk_size: usize, emit: EmitFn<'_>) -> io::Result<u64> {
    let Some(reader) = record.reader_mut() else {
        return Ok(0);
    };
    let mut buf = vec![0u8; chunk_size];
    let mut offset = 0u64;
    loop {
        match reader.read(&mut buf) {
            Ok(0) => return Ok(offset),
            Ok(n) => {
                if !emit(response(pb::parse_warc_response::Kind::PayloadChunk(pb::PayloadChunk {
                    record_index,
                    offset,
                    data: buf[..n].to_vec(),
                }))) {
                    return Err(io::Error::new(io::ErrorKind::BrokenPipe, "consumer gone"));
                }
                offset += n as u64;
            }
            Err(e) => return Err(e),
        }
    }
}

/// `Read` + `Seek` over streamed request chunks.
///
/// Blocks until the next chunk or EOF. Only current-position seeks succeed;
/// the parser only queries position during linear reads. Digest verification
/// freezes the record into an in-memory `Cursor` before seeking, so identity
/// seeks on this adapter are sufficient for the linear parse path.
struct ChannelReader {
    rx: mpsc::Receiver<Vec<u8>>,
    current: Option<(Vec<u8>, usize)>,
    pos: u64,
}

impl ChannelReader {
    fn new(rx: mpsc::Receiver<Vec<u8>>) -> Self {
        Self {
            rx,
            current: None,
            pos: 0,
        }
    }
}

impl Read for ChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            if let Some((chunk, consumed)) = &mut self.current {
                if *consumed < chunk.len() {
                    let n = (chunk.len() - *consumed).min(buf.len());
                    buf[..n].copy_from_slice(&chunk[*consumed..*consumed + n]);
                    *consumed += n;
                    self.pos += n as u64;
                    return Ok(n);
                }
                self.current = None;
            }
            match self.rx.blocking_recv() {
                Some(chunk) if chunk.is_empty() => {}
                Some(chunk) => self.current = Some((chunk, 0)),
                None => return Ok(0),
            }
        }
    }
}

impl Seek for ChannelReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Current(0) => Ok(self.pos),
            SeekFrom::Start(p) if p == self.pos => Ok(self.pos),
            _ => Err(io::Error::new(io::ErrorKind::Unsupported, "streamed WARC input does not support repositioning")),
        }
    }
}
