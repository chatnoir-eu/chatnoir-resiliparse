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

use std::cell::RefCell;
use std::io::{self, BufRead};
use std::rc::Rc;

use fastwarc::stream_io::traits::IntoWarcReader;
use fastwarc::warc::iter::{ArchiveIterator, ArchiveIteratorOptions};
use fastwarc::warc::record::WarcRecord;
use prost::bytes::Bytes;

use crate::convert;
use crate::defaults::{DEFAULT_MAX_HEADER_LEN, DEFAULT_PAYLOAD_CHUNK_SIZE};
use crate::proto::fastwarc::v1 as pb;

type EmitFn<'a> = &'a mut dyn FnMut(pb::ParseWarcResponse) -> bool;

/// Parses an archive and emits one ordered response sequence per record.
pub(super) fn parse_into(reader: impl IntoWarcReader, config: &pb::ParseWarcConfig, emit: EmitFn<'_>) {
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
            http_error.replace(Some((record.stream_pos(), format!("failed to parse HTTP headers: {error}"))));
            // Yield once so the error is emitted before iteration resumes.
            return true;
        }
        convert::record_passes_filters(record, config)
    });

    for item in iterator {
        let record = match item {
            Ok(record) => record,
            Err(error) => {
                let _ = emit(record_error(0, false, error.to_string()));
                return;
            }
        };
        if let Some((stream_pos, message)) = http_error.borrow_mut().take() {
            if !emit(record_error(stream_pos, true, message)) {
                return;
            }
            continue;
        }
        if !emit_record(&record, config, chunk_size, emit) {
            return;
        }
    }
}

fn emit_record(
    shared: &Rc<RefCell<WarcRecord>>,
    config: &pb::ParseWarcConfig,
    chunk_size: usize,
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
        match stream_payload(&mut record, chunk_size, emit) {
            Ok(len) => len,
            Err(error) => {
                let stream_pos = record.stream_pos();
                let _ = emit(record_error(stream_pos, false, format!("failed to read record payload: {error}")));
                return false;
            }
        }
    } else {
        record.content_length()
    };

    emit(response(pb::parse_warc_response::Kind::RecordEnd(pb::RecordEnd { payload_length })))
}

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

fn response(kind: pb::parse_warc_response::Kind) -> pb::ParseWarcResponse {
    pb::ParseWarcResponse { kind: Some(kind) }
}

pub(super) fn record_error(stream_pos: u64, recoverable: bool, message: String) -> pb::ParseWarcResponse {
    response(pb::parse_warc_response::Kind::RecordError(pb::RecordError {
        stream_pos,
        recoverable,
        message,
    }))
}
