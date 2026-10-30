// Copyright 2026 Janek Bevendorff
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

use super::*;
use std::io::Cursor;

#[test]
fn binary_chunks_and_extensions_preserve_payload_bytes() {
    let mut reader = Cursor::new(b"3;extension=x\r\n\0\xff\x80\r\n1\r\nx\r\n0\r\n\r\ntrailer");
    let chunks = iterate_http_chunks(&mut reader)
        .collect::<io::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(chunks, [vec![0, 255, 128], b"x".to_vec()]);
    assert_eq!(&reader.get_ref()[reader.position() as usize..], b"trailer");
}

#[test]
fn short_chunk_and_invalid_header_stop_without_fabricating_bytes() {
    assert_eq!(read_http_chunk(&mut Cursor::new(b"5\r\nabc")).unwrap(), b"abc");
    let mut reader = Cursor::new(b"invalid\r\nbody");
    assert!(read_http_chunk(&mut reader).unwrap().is_empty());
    assert_eq!(&reader.get_ref()[reader.position() as usize..], b"dy");
}

#[test]
fn size_prefixes_and_overflow() {
    // These permissive forms are part of the documented legacy strtol contract.
    for (header, expected) in [
        (b" \t\x0b+0XAf;name=value\r\n".as_slice(), 175),
        (b"-a\r\n".as_slice(), 10usize.wrapping_neg()),
        (b"+\r\n".as_slice(), 0),
        (b"0x\r\n".as_slice(), 0),
        (b"a".as_slice(), 10),
        (b"".as_slice(), 0),
    ] {
        assert_eq!(chunk_size(header), expected, "{header:?}");
    }
    // The C long range differs between Windows and Unix targets.
    let positive = format!("{}\r\n", "f".repeat(128));
    let negative = format!("-{}\r\n", "f".repeat(128));
    assert_eq!(chunk_size(positive.as_bytes()), std::ffi::c_long::MAX as usize);
    assert_eq!(chunk_size(negative.as_bytes()), (std::ffi::c_long::MIN as usize));
}

/// A short reader that injects one I/O error at a byte boundary.
struct FaultyReader {
    data: Cursor<Vec<u8>>,
    fail_at: u64,
    error: io::ErrorKind,
    failed: bool,
    calls: std::rc::Rc<std::cell::Cell<usize>>,
}

impl Read for FaultyReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.calls.set(self.calls.get() + 1);
        if !self.failed && self.data.position() == self.fail_at {
            self.failed = true;
            return Err(self.error.into());
        }
        let limit = if self.failed {
            buf.len()
        } else {
            buf.len().min((self.fail_at - self.data.position()) as usize)
        };
        self.data.read(&mut buf[..limit])
    }
}

#[test]
fn interrupted_reads_retry() {
    // Interrupt the header, payload start, and payload interior without losing bytes.
    for fail_at in [0, 3, 5] {
        let reader = FaultyReader {
            data: Cursor::new(b"4\r\nWiki\r\n0\r\n\r\n".to_vec()),
            fail_at,
            error: io::ErrorKind::Interrupted,
            failed: false,
            calls: Default::default(),
        };
        let chunks = iterate_http_chunks(io::BufReader::with_capacity(2, reader))
            .collect::<io::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(chunks, [b"Wiki".to_vec()]);
    }
}

#[test]
fn errors_are_yielded_once() {
    // A permanent error terminates iteration; later next() calls must not retry I/O.
    for fail_at in [0, 3, 5] {
        let calls = std::rc::Rc::new(std::cell::Cell::new(0));
        let reader = FaultyReader {
            data: Cursor::new(b"4\r\nWiki\r\n".to_vec()),
            fail_at,
            error: io::ErrorKind::ConnectionReset,
            failed: false,
            calls: calls.clone(),
        };
        let mut chunks = iterate_http_chunks(io::BufReader::with_capacity(2, reader));
        assert_eq!(chunks.next().unwrap().unwrap_err().kind(), io::ErrorKind::ConnectionReset);
        let after_error = calls.get();
        assert!(chunks.next().is_none());
        assert!(chunks.next().is_none());
        assert_eq!(calls.get(), after_error);
    }
}

#[test]
fn empty_chunk_ends_iteration() {
    // A zero-size chunk ends this payload even if more readable bytes follow it.
    let mut reader = Cursor::new(b"0\r\n\r\n4\r\nWiki\r\n");
    {
        let mut chunks = iterate_http_chunks(&mut reader);
        assert!(chunks.next().is_none());
        assert!(chunks.next().is_none());
    }
    assert_eq!(&reader.get_ref()[reader.position() as usize..], b"4\r\nWiki\r\n");
}
