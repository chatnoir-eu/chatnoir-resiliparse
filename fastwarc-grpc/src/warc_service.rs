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

use std::io::{self, BufRead, Read, Seek, SeekFrom};

use fastwarc::stream_io::bufread::RawReaderAdapter;
use fastwarc::stream_io::traits::IntoWarcReader;
use fastwarc::warc::iter::{ArchiveIterator, ArchiveIteratorOptions};
use fastwarc::warc::record::WarcRecord;
use prost::bytes::{Buf, Bytes, BytesMut};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};

use crate::convert;
use crate::proto::fastwarc::v1 as pb;

/// Default maximum WARC/HTTP header block length (matches the crate default).
const DEFAULT_MAX_HEADER_LEN: usize = 32 << 10;
/// Largest accepted WARC/HTTP header block. A `HeaderBlock` carries both its
/// parsed fields and raw bytes, so this leaves room for both WARC and HTTP
/// headers within the 16 MiB response-message limit.
const MAX_HEADER_LEN: usize = 2 << 20;
/// Default payload chunk size for `payload_chunk` messages.
const DEFAULT_PAYLOAD_CHUNK_SIZE: usize = 64 << 10;
/// Default buffer capacity of the byte stream fed into the parser.
const DEFAULT_INPUT_BUFFER_SIZE: usize = 64 << 10;
/// Cap unread archive bytes queued into the parser thread. Slot count
/// follows `input_buffer_size`.
const CHUNK_CHANNEL_BYTES: usize = 32 * 1024 * 1024;
const CHUNK_CHANNEL_BOUND_MAX: usize = 256;

fn chunk_channel_bound(input_buffer_size: u32) -> usize {
    let hint = if input_buffer_size == 0 {
        DEFAULT_INPUT_BUFFER_SIZE
    } else {
        input_buffer_size as usize
    };
    (CHUNK_CHANNEL_BYTES / hint.max(1)).clamp(2, CHUNK_CHANNEL_BOUND_MAX)
}

/// Bound of the response channel back to the client.
const RESPONSE_CHANNEL_BOUND: usize = 1024;
/// Flush a batch before it approaches the gRPC message-size cap.
const MAX_BATCH_BYTES: usize = 2 << 20;

type ResponseSender = mpsc::Sender<Result<pb::ParseWarcResponse, Status>>;

/// Consumer of parse protocol messages; returns `false` when the consumer is
/// gone and the parse should stop.
type EmitFn<'a> = &'a mut dyn FnMut(pb::ParseWarcResponse) -> bool;

/// The `fastwarc.v1.WarcService` gRPC service (stateless).
///
/// By default `ParseWarcConfig.archive_path` is rejected with
/// `PermissionDenied`: letting remote clients name server-side files is a
/// separate security domain from parsing bytes the client supplied, so it
/// must be an explicit operator decision (see [`WarcParser::with_local_files`]).
#[derive(Default)]
pub struct WarcParser {
    allow_local_files: bool,
}

impl WarcParser {
    /// A parser that only parses client-supplied bytes; `archive_path`
    /// requests are rejected with `PermissionDenied`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A parser that additionally allows `ParseWarcConfig.archive_path` to
    /// open files on the server's filesystem. Only enable this when every
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
        if !config.archive_path.is_empty() && !self.allow_local_files {
            return Err(Status::permission_denied(
                "archive_path is disabled on this server; stream the archive as chunks instead",
            ));
        }
        validate_config(&config)?;

        let (chunk_tx, chunk_rx) = mpsc::channel::<Bytes>(chunk_channel_bound(config.input_buffer_size));
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
        validate_config(&config)?;
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
        Some(pb::parse_warc_response::Kind::RecordEnd(_)) => {
            if let Some(record) = open.take() {
                records.push(record);
            }
        }
        Some(pb::parse_warc_response::Kind::RecordError(error)) => errors.push(error),
        Some(pb::parse_warc_response::Kind::Batch(batch)) => {
            for item in batch.items {
                fold_response(item, open, records, errors);
            }
        }
        None => {}
    }
}

fn validate_config(config: &pb::ParseWarcConfig) -> Result<(), Status> {
    if usize::try_from(config.max_header_len).unwrap_or(usize::MAX) > MAX_HEADER_LEN {
        return Err(Status::invalid_argument(format!("max_header_len must not exceed {MAX_HEADER_LEN} bytes")));
    }
    Ok(())
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
fn run_parser(chunk_rx: mpsc::Receiver<Bytes>, resp_tx: &ResponseSender, config: &pb::ParseWarcConfig) {
    let input_buffer_size = if config.input_buffer_size == 0 {
        DEFAULT_INPUT_BUFFER_SIZE
    } else {
        config.input_buffer_size as usize
    };
    let mut emitter = BatchEmitter::new(resp_tx, convert::response_batch_size(config));
    let mut emit = |resp: pb::ParseWarcResponse| emitter.emit(resp);
    if config.archive_path.is_empty() {
        // ChannelReader is BufRead over the received chunks themselves, so
        // the parser scans and skips archive bytes in place; wrapping it in
        // a BufReader would memcpy the whole stream a second time.
        parse_into(RawReaderAdapter::new(ChannelReader::new(chunk_rx)), config, &mut emit);
    } else {
        match std::fs::File::open(&config.archive_path) {
            Ok(file) => {
                let reader = io::BufReader::with_capacity(input_buffer_size, file);
                parse_into(reader, config, &mut emit);
            }
            Err(e) => {
                emit(record_error(0, false, format!("failed to open archive_path {}: {e}", config.archive_path)));
            }
        }
    }
    emitter.flush();
}

/// Packs protocol events into gRPC messages and `try_send`s them so the
/// parser thread is not parked on HTTP/2 drain. Falls back to
/// `blocking_send` only when the response channel is actually full.
struct BatchEmitter<'a> {
    tx: &'a ResponseSender,
    batch: Vec<pb::ParseWarcResponse>,
    batch_size: usize,
    batch_bytes: usize,
}

impl BatchEmitter<'_> {
    fn new(tx: &ResponseSender, batch_size: usize) -> BatchEmitter<'_> {
        BatchEmitter {
            tx,
            batch: Vec::with_capacity(batch_size),
            batch_size,
            batch_bytes: 0,
        }
    }

    fn emit(&mut self, resp: pb::ParseWarcResponse) -> bool {
        if self.batch_size <= 1 {
            return send_response(self.tx, resp);
        }
        let add = event_wire_bytes(&resp);
        if !self.batch.is_empty() && self.batch_bytes + add > MAX_BATCH_BYTES && !self.flush() {
            return false;
        }
        self.batch_bytes += add;
        self.batch.push(resp);
        if self.batch.len() >= self.batch_size || self.batch_bytes >= MAX_BATCH_BYTES {
            self.flush()
        } else {
            true
        }
    }

    fn flush(&mut self) -> bool {
        if self.batch.is_empty() {
            return true;
        }
        self.batch_bytes = 0;
        let items = std::mem::take(&mut self.batch);
        let msg = if items.len() == 1 {
            items.into_iter().next().expect("checked non-empty")
        } else {
            response(pb::parse_warc_response::Kind::Batch(pb::RecordBatch { items }))
        };
        send_response(self.tx, msg)
    }
}

/// Cheap size estimate for batch flushing; protobuf tags add a little more.
fn event_wire_bytes(resp: &pb::ParseWarcResponse) -> usize {
    match resp.kind.as_ref() {
        Some(pb::parse_warc_response::Kind::PayloadChunk(chunk)) => chunk.data.len().saturating_add(64),
        Some(pb::parse_warc_response::Kind::RecordStart(start)) => start
            .metadata
            .as_ref()
            .and_then(|m| m.warc_headers.as_ref())
            .map_or(128, |h| h.raw_block.len().saturating_add(256)),
        Some(pb::parse_warc_response::Kind::Batch(batch)) => batch.items.iter().map(event_wire_bytes).sum(),
        _ => 64,
    }
}

fn send_response(tx: &ResponseSender, msg: pb::ParseWarcResponse) -> bool {
    match tx.try_send(Ok(msg)) {
        Ok(()) => true,
        Err(mpsc::error::TrySendError::Full(m)) => tx.blocking_send(m).is_ok(),
        Err(mpsc::error::TrySendError::Closed(_)) => false,
    }
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
        // HTTP parsing remains manual so a malformed embedded HTTP header is
        // a recoverable record error rather than a framing failure.
        parse_http: false,
        decode_http_payload: convert::auto_decode(config.decode_http_payload),
        verify_digests: config.verify_digests,
        quirks_mode: config.quirks_mode,
        max_header_len,
        // Reuse the iterator buffer when payload is not copied and the record
        // does not need to be frozen for digest verification.
        inplace: !convert::include_payload(config) && !config.verify_digests,
    };
    let iterator = ArchiveIterator::with_options(reader, options)
        .with_filter(|record| convert::record_passes_filters(record, config));

    for item in iterator {
        let record = match item {
            Ok(record) => record,
            Err(e) => {
                // Framing failure: the crate has lost the reader; do not continue.
                let _ = emit(record_error(0, false, e.to_string()));
                return;
            }
        };

        match process_record(&record, config, max_header_len, chunk_size, emit) {
            ProcessOutcome::Stop => return,
            ProcessOutcome::Emitted | ProcessOutcome::Continue => {}
        }
    }
}

/// Result of attempting to emit one framed record.
enum ProcessOutcome {
    /// `record_start` / chunks / `record_end` were sent.
    Emitted,
    /// Recoverable per-record failure (`record_error`); parse continues.
    Continue,
    /// Fatal: consumer gone or non-recoverable payload failure.
    Stop,
}

/// One record: optional HTTP parse, then `record_start` / `payload_chunk`* /
/// `record_end`. Digest filtering has already run in `ArchiveIterator`.
fn process_record(
    shared: &std::rc::Rc<std::cell::RefCell<WarcRecord>>,
    config: &pb::ParseWarcConfig,
    max_header_len: usize,
    chunk_size: usize,
    emit: EmitFn<'_>,
) -> ProcessOutcome {
    let mut record = shared.borrow_mut();

    if convert::parse_http(config)
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

    let metadata = convert::record_metadata(&record, convert::include_headers(config));
    if !emit(response(pb::parse_warc_response::Kind::RecordStart(pb::RecordStart {
        metadata: Some(metadata),
    }))) {
        return ProcessOutcome::Stop;
    }

    let payload_length = if convert::include_payload(config) {
        match stream_payload(&mut record, chunk_size, emit) {
            Ok(len) => len,
            Err(e) => {
                let stream_pos = record.stream_pos();
                let _ = emit(record_error(stream_pos, false, format!("failed to read record payload: {e}")));
                return ProcessOutcome::Stop;
            }
        }
    } else {
        record.content_length()
    };

    if emit(response(pb::parse_warc_response::Kind::RecordEnd(pb::RecordEnd { payload_length }))) {
        ProcessOutcome::Emitted
    } else {
        ProcessOutcome::Stop
    }
}

/// Emit remaining payload as `payload_chunk` messages; return total bytes.
///
/// Chunks are copied straight out of the record reader's `fill_buf` window
/// into exact-size `Bytes` (single copy, no scratch buffer), so a chunk may
/// be shorter than `chunk_size` when it ends at an input buffer boundary.
fn stream_payload(record: &mut WarcRecord, chunk_size: usize, emit: EmitFn<'_>) -> io::Result<u64> {
    let Some(reader) = record.reader_mut() else {
        return Ok(0);
    };
    let mut offset = 0u64;
    loop {
        let window = reader.fill_buf()?;
        if window.is_empty() {
            return Ok(offset);
        }
        let n = window.len().min(chunk_size);
        let data = Bytes::copy_from_slice(&window[..n]);
        reader.consume(n);
        if !emit(response(pb::parse_warc_response::Kind::PayloadChunk(pb::PayloadChunk { offset, data }))) {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "consumer gone"));
        }
        offset += n as u64;
    }
}

/// `BufRead` + `Seek` over streamed request chunks.
///
/// `fill_buf` hands the parser a window into the received `Bytes` chunk
/// itself, so header scans and payload skips run in place with no
/// intermediate copy. Blocks until the next chunk or EOF. Only
/// current-position seeks succeed; the parser only queries position during
/// linear reads. Digest verification freezes the record into an in-memory
/// `Cursor` before seeking, so identity seeks on this adapter are
/// sufficient for the linear parse path.
struct ChannelReader {
    rx: mpsc::Receiver<Bytes>,
    current: Bytes,
    pos: u64,
}

/// Compression autodetection reads the first four bytes from one `fill_buf`
/// window; coalesce the stream head until it can satisfy that.
const MAGIC_LEN: usize = 4;

impl ChannelReader {
    fn new(rx: mpsc::Receiver<Bytes>) -> Self {
        Self {
            rx,
            current: Bytes::new(),
            pos: 0,
        }
    }
}

impl Read for ChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let src = self.fill_buf()?;
        let n = src.len().min(buf.len());
        buf[..n].copy_from_slice(&src[..n]);
        self.consume(n);
        Ok(n)
    }
}

impl BufRead for ChannelReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        while self.current.is_empty() {
            match self.rx.blocking_recv() {
                Some(chunk) if chunk.is_empty() => {}
                Some(chunk) => self.current = chunk,
                None => return Ok(&[]),
            }
        }
        if self.pos == 0 && self.current.len() < MAGIC_LEN {
            let mut head = BytesMut::from(&self.current[..]);
            while head.len() < MAGIC_LEN {
                match self.rx.blocking_recv() {
                    Some(chunk) => head.extend_from_slice(&chunk),
                    None => break,
                }
            }
            self.current = head.freeze();
        }
        Ok(&self.current)
    }

    fn consume(&mut self, amt: usize) {
        let n = amt.min(self.current.len());
        self.current.advance(n);
        self.pos += n as u64;
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
