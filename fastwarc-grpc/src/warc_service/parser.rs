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

//! Drives the [FastWARC](https://docs.rs/fastwarc) iterator and emits the
//! `ParseWarc` event sequence.

use std::cell::RefCell;
use std::io::{self, BufRead};
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard, PoisonError};

use fastwarc::stream_io::traits::IntoWarcReader;
use fastwarc::warc::iter::{ArchiveIterator, ArchiveIteratorOptions};
use fastwarc::warc::record::WarcRecord;
use prost::bytes::Bytes;

use crate::convert;
use crate::defaults::{DEFAULT_MAX_HEADER_LEN, DEFAULT_PAYLOAD_CHUNK_SIZE};
use crate::proto::fastwarc::v1 as pb;

/// Receives parser events. Returning `false` stops parsing.
type EmitFn<'a> = &'a mut dyn FnMut(pb::ParseWarcResponse) -> bool;

/// Buffered payload bytes for the chunk currently being filled.
#[derive(Default)]
pub(super) struct PendingPayload(Mutex<PayloadBuffer>);

#[derive(Default)]
struct PayloadBuffer {
    offset: u64,
    data: Vec<u8>,
}

impl PendingPayload {
    fn lock(&self) -> MutexGuard<'_, PayloadBuffer> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn reset(&self) {
        *self.lock() = PayloadBuffer::default();
    }

    /// Takes the buffered bytes as a payload chunk event, if any.
    pub(super) fn take_chunk(&self) -> Option<pb::ParseWarcResponse> {
        let mut buf = self.lock();
        if buf.data.is_empty() {
            return None;
        }
        let data = Bytes::from(std::mem::take(&mut buf.data));
        let offset = buf.offset;
        buf.offset += data.len() as u64;
        Some(response(pb::parse_warc_response::Kind::PayloadChunk(pb::PayloadChunk { offset, data })))
    }
}

/// Parses records and emits start, payload, and end events.
/// HTTP header failures are recoverable while the record retains its reader.
/// Framing, decoder setup, and payload read failures stop parsing.
pub(super) fn parse_into(
    reader: impl IntoWarcReader,
    config: &pb::ParseWarcConfig,
    pending: &PendingPayload,
    emit: EmitFn<'_>,
) {
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
    let options = ArchiveIteratorOptions {
        stream_detect: config.stream_detect.unwrap_or(true),
        // Parse in the filter so HTTP and framing errors remain distinct.
        parse_http: false,
        decode_http_payload: convert::auto_decode(config.decode_http_payload),
        verify_digests: config.verify_digests,
        quirks_mode: config.quirks_mode,
        max_header_len,
        inplace: !convert::include_payload(config) && !config.verify_digests,
    };
    let http_error = RefCell::new(None);
    let iterator = ArchiveIterator::with_options(reader, options).with_filter(|record| {
        // Skip unwanted types before HTTP parsing so they cannot surface HTTP errors.
        if !convert::record_passes_header_filters(record, config) {
            return false;
        }
        // Match ArchiveIterator's HTTP parsing order while preserving header
        // failures as recoverable record errors.
        if convert::parse_http(config)
            && record.is_http()
            && let Err(error) = record.parse_http_with_opts(
                convert::auto_decode(config.decode_http_payload),
                max_header_len,
                config.quirks_mode,
            )
        {
            // Decoder setup can consume the reader, preventing further iteration.
            let recoverable = record.reader_mut().is_some();
            let message = if recoverable {
                format!("failed to parse HTTP headers: {error}")
            } else {
                format!("failed to set up HTTP payload decoder: {error}")
            };
            http_error.replace(Some((record.stream_pos(), recoverable, message)));
            // Yield once so the error is emitted before iteration resumes.
            return true;
        }
        convert::record_passes_length_filters(record, config)
    });

    for item in iterator {
        let record = match item {
            Ok(record) => record,
            Err(error) => {
                let _ = emit(record_error(0, false, error.to_string()));
                return;
            }
        };
        if let Some((stream_pos, recoverable, message)) = http_error.borrow_mut().take() {
            if !emit(record_error(stream_pos, recoverable, message)) || !recoverable {
                return;
            }
            continue;
        }
        if !emit_record(&record, config, chunk_size, pending, emit) {
            return;
        }
    }
}

/// Emits one record. A payload read failure emits an error followed by
/// `record_end` with the bytes sent so far, then stops parsing.
fn emit_record(
    shared: &Rc<RefCell<WarcRecord>>,
    config: &pb::ParseWarcConfig,
    chunk_size: usize,
    pending: &PendingPayload,
    emit: EmitFn<'_>,
) -> bool {
    let mut record = shared.borrow_mut();
    let metadata = convert::record_metadata(&record, convert::include_headers(config));
    if !emit(response(pb::parse_warc_response::Kind::RecordStart(pb::RecordStart {
        metadata: Some(metadata),
    }))) {
        return false;
    }

    let payload_length = if convert::include_payload(config) {
        match stream_payload(&mut record, chunk_size, pending, emit) {
            Ok(Some(len)) => len,
            Ok(None) => return false,
            Err((bytes_streamed, error)) => {
                let stream_pos = record.stream_pos();
                if emit(record_error(stream_pos, false, format!("failed to read record payload: {error}"))) {
                    let _ = emit(response(pb::parse_warc_response::Kind::RecordEnd(pb::RecordEnd {
                        payload_length: bytes_streamed,
                    })));
                }
                return false;
            }
        }
    } else {
        record.content_length()
    };

    emit(response(pb::parse_warc_response::Kind::RecordEnd(pb::RecordEnd { payload_length })))
}

/// Streams chunks and returns their total length. On failure, returns the
/// bytes sent so far and the error. `None` means the consumer stopped.
fn stream_payload(
    record: &mut WarcRecord,
    chunk_size: usize,
    pending: &PendingPayload,
    emit: EmitFn<'_>,
) -> Result<Option<u64>, (u64, io::Error)> {
    let Some(reader) = record.reader_mut() else {
        return Ok(Some(0));
    };
    pending.reset();
    loop {
        let end = fill_chunk(reader, chunk_size, pending);
        if let Some(chunk) = pending.take_chunk()
            && !emit(chunk)
        {
            return Ok(None);
        }
        match end {
            None => {}
            Some(Ok(())) => return Ok(Some(pending.lock().offset)),
            Some(Err(error)) => return Err((pending.lock().offset, error)),
        }
    }
}

/// Reads into `pending` until it holds `chunk_size` bytes, EOF, or an error.
fn fill_chunk(reader: &mut impl BufRead, chunk_size: usize, pending: &PendingPayload) -> Option<io::Result<()>> {
    loop {
        let window = match reader.fill_buf() {
            Ok([]) => return Some(Ok(())),
            Ok(window) => window,
            Err(error) => return Some(Err(error)),
        };
        let mut buf = pending.lock();
        let data = &mut buf.data;
        let n = window.len().min(chunk_size - data.len());
        let needed = data.len() + n;
        if needed > data.capacity() {
            let capacity = needed.max(data.capacity().saturating_mul(2)).min(chunk_size);
            data.reserve_exact(capacity - data.len());
        }
        data.extend_from_slice(&window[..n]);
        reader.consume(n);
        if data.len() >= chunk_size {
            return None;
        }
    }
}

/// Wraps an event kind in a `ParseWarcResponse` envelope.
fn response(kind: pb::parse_warc_response::Kind) -> pb::ParseWarcResponse {
    pb::ParseWarcResponse { kind: Some(kind) }
}

/// Builds an error event. Use a zero offset when the failed record position is unknown.
pub(super) fn record_error(stream_pos: u64, recoverable: bool, message: String) -> pb::ParseWarcResponse {
    response(pb::parse_warc_response::Kind::RecordError(pb::RecordError {
        stream_pos,
        recoverable,
        message,
    }))
}

#[cfg(test)]
#[path = "parser_test.rs"]
mod parser_test;
