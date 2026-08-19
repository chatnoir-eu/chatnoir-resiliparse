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

use std::io::{self, BufRead, Read, Seek, SeekFrom};

use prost::bytes::{Buf, Bytes, BytesMut};
use tokio::sync::mpsc;

const MAGIC_LEN: usize = 4;

/// Blocking reader over the chunks received by the streaming RPC.
pub(super) struct ChannelReader {
    rx: mpsc::Receiver<Bytes>,
    current: Bytes,
    pos: u64,
}

impl ChannelReader {
    pub(super) fn new(rx: mpsc::Receiver<Bytes>) -> Self {
        Self {
            rx,
            current: Bytes::new(),
            pos: 0,
        }
    }
}

impl Read for ChannelReader {
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

impl BufRead for ChannelReader {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_read_does_not_wait_for_input() {
        let (_tx, rx) = mpsc::channel(1);
        let mut reader = ChannelReader::new(rx);
        assert_eq!(reader.read(&mut []).unwrap(), 0);
    }
}
