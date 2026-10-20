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

//! Blocking `Read`/`BufRead` adapter over the client's chunk channel.

use std::io::{self, BufRead, Read, Seek, SeekFrom};

use prost::bytes::{Buf, Bytes, BytesMut};
use tokio::sync::mpsc;

/// Prefix length required by compression detection.
const MAGIC_LEN: usize = 4;

/// Blocking reader over input chunks. A closed channel signals EOF.
/// Tracks consumed bytes for position queries; repositioning is unsupported.
pub(super) struct ChannelReader {
    /// Receiving half of the chunk channel; `None` from it means the client
    /// stream ended.
    rx: mpsc::Receiver<Bytes>,
    /// Unconsumed remainder of the most recently received chunk.
    current: Bytes,
    /// Absolute position in the logical archive stream, i.e. the number of
    /// bytes consumed so far.
    pos: u64,
}

impl ChannelReader {
    /// A reader that yields the bytes arriving on `rx` in order.
    pub(super) fn new(rx: mpsc::Receiver<Bytes>) -> Self {
        Self {
            rx,
            current: Bytes::new(),
            pos: 0,
        }
    }
}

impl Read for ChannelReader {
    /// Reads from the buffered chunk, waiting for input when necessary.
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let n = self.fill_buf()?.read(buf)?;
        self.consume(n);
        Ok(n)
    }
}

impl BufRead for ChannelReader {
    /// Skips empty chunks and buffers the initial compression prefix.
    /// Returns an empty slice only after the input channel closes.
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        while self.current.is_empty() {
            match self.rx.blocking_recv() {
                Some(chunk) if chunk.is_empty() => {}
                Some(chunk) => self.current = chunk,
                None => return Ok(&[]),
            }
        }

        // Compression detection needs the first four bytes in one window.
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

    /// Advances by at most the number of buffered bytes.
    fn consume(&mut self, amt: usize) {
        let n = amt.min(self.current.len());
        self.current.advance(n);
        self.pos += n as u64;
    }
}

impl Seek for ChannelReader {
    /// Accepts only seeks to the current position; other seeks return `Unsupported`.
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Current(0) => Ok(self.pos),
            SeekFrom::Start(p) if p == self.pos => Ok(self.pos),
            _ => Err(io::Error::new(io::ErrorKind::Unsupported, "streamed WARC input does not support repositioning")),
        }
    }
}

#[cfg(test)]
#[path = "channel_reader_test.rs"]
mod channel_reader_test;
