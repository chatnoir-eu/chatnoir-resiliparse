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

use std::io::{Read, Seek, SeekFrom, Write};

use fastwarc::stream_io::bufread::RawReaderAdapter;

use super::*;

/// Serves a fixed prefix of archive bytes, then fails every later read.
struct FailingReader {
    data: Vec<u8>,
    pos: usize,
}

impl BufRead for FailingReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.pos < self.data.len() {
            Ok(&self.data[self.pos..])
        } else {
            Err(io::Error::new(io::ErrorKind::ConnectionReset, "simulated mid-payload failure"))
        }
    }

    fn consume(&mut self, amt: usize) {
        self.pos = (self.pos + amt).min(self.data.len());
    }
}

impl Read for FailingReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let src = self.fill_buf()?;
        let n = src.len().min(buf.len());
        buf[..n].copy_from_slice(&src[..n]);
        self.consume(n);
        Ok(n)
    }
}

impl Seek for FailingReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let current = self.pos as u64;
        match pos {
            SeekFrom::Current(0) => Ok(current),
            SeekFrom::Start(p) if p == current => Ok(current),
            _ => Err(io::Error::new(io::ErrorKind::Unsupported, "no repositioning")),
        }
    }
}

/// A payload read failure after `record_start` was emitted must terminate
/// the open sequence: `record_error` followed by a `record_end` whose
/// `payload_length` counts the chunk bytes streamed before the failure.
#[test]
fn payload_failure_after_start_terminates_record_sequence() {
    // A record that declares more payload than the reader can serve: the
    // header and the first 10 payload bytes parse, the next read fails.
    let mut data = Vec::new();
    write!(
        data,
        "WARC/1.0\r\nWARC-Type: resource\r\nWARC-Record-ID: <urn:uuid:aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa>\r\nWARC-Date: 2020-01-01T00:00:00Z\r\nContent-Length: 1000\r\n\r\n"
    )
    .unwrap();
    data.extend_from_slice(b"0123456789");

    let config = pb::ParseWarcConfig {
        parse_http: Some(false),
        ..Default::default()
    };
    let mut events = Vec::new();
    let mut emit = |response: pb::ParseWarcResponse| {
        events.push(response);
        true
    };
    parse_into(RawReaderAdapter::new(FailingReader { data, pos: 0 }), &config, &mut emit);

    let kinds: Vec<_> = events.iter().map(|e| e.kind.as_ref().unwrap()).collect();
    let [
        pb::parse_warc_response::Kind::RecordStart(_),
        payload_chunks @ ..,
        pb::parse_warc_response::Kind::RecordError(error),
        pb::parse_warc_response::Kind::RecordEnd(end),
    ] = kinds.as_slice()
    else {
        panic!("expected record_start .. record_error record_end, got {kinds:?}");
    };
    let mut streamed = 0u64;
    for kind in payload_chunks {
        let pb::parse_warc_response::Kind::PayloadChunk(chunk) = kind else {
            panic!("expected only payload_chunk between record_start and record_error, got {kind:?}");
        };
        assert_eq!(chunk.offset, streamed, "non-contiguous payload chunk offset");
        streamed += chunk.data.len() as u64;
    }
    assert!(streamed > 0, "the failure should occur after at least one payload chunk");
    assert!(!error.recoverable);
    assert!(error.message.contains("failed to read record payload"), "unexpected message: {}", error.message);
    assert_eq!(end.payload_length, streamed, "record_end must count the bytes streamed before the failure");
}

#[test]
fn rejecting_payload_stops_emission() {
    let data = b"WARC/1.0\r\nWARC-Type: resource\r\nContent-Length: 4\r\n\r\nbody\r\n\r\n";
    let mut rejected = false;
    parse_into(io::Cursor::new(data), &pb::ParseWarcConfig::default(), &mut |event| {
        assert!(!rejected, "event emitted after the consumer stopped");
        rejected = matches!(event.kind, Some(pb::parse_warc_response::Kind::PayloadChunk(_)));
        !rejected
    });
    assert!(rejected);
}
