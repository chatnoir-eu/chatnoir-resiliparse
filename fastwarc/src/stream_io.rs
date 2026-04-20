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

use std::any::Any;
use std::io;
use std::mem;

// ===========================================================
// BufReadSeek trait definition
// ===========================================================

pub trait BufReadSeek: io::BufRead + io::Seek + Any {}
impl<T: io::BufRead + io::Seek + Any + ?Sized> BufReadSeek for T {}

// ===========================================================
// Limited buffered reader
// ===========================================================

/// A limited seekable buffered reader.
/// Wraps an existing [`BufReadSeek`] reader, terminating when `limit` is reached.
pub struct LimitedBufReadSeek {
    pub(crate) reader: Box<dyn BufReadSeek>,
    pub(crate) limit: usize,
    pub(crate) pos: usize,
}

impl LimitedBufReadSeek {
    /// Create a new limited reader from a buffered reader instance.
    pub fn new(reader: Box<dyn BufReadSeek>, limit: Option<usize>) -> Self {
        Self {
            reader,
            limit: limit.unwrap_or(usize::MAX),
            pos: 0,
        }
    }

    /// Change the limit of the reader.
    /// Also resets the logical stream position to 0. Use [`Self::real_stream_position()`] to get
    /// the real position on the original stream.
    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
        self.pos = 0;
    }

    /// Get the real (not the logical) stream position.
    pub fn real_stream_position(&mut self) -> io::Result<u64> {
        self.reader.stream_position()
    }

    /// Replace the internal stream with a new one and hand ownership of the previous
    /// stream back to the caller. Resets `limit` and `pos`.
    pub fn replace_reader(&mut self, new_reader: Box<dyn BufReadSeek>) -> Box<dyn BufReadSeek> {
        self.limit = usize::MAX;
        self.pos = 0;
        mem::replace(&mut self.reader, new_reader)
    }
}

impl io::Read for LimitedBufReadSeek {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let l = buf.len();
        let buf_limited = &mut buf[..l.min(self.limit - self.pos)];
        let n = self.reader.read(buf_limited)?;
        self.pos += n;
        Ok(n)
    }
}

impl io::BufRead for LimitedBufReadSeek {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        let buf = self.reader.fill_buf()?;
        let buf_limited = &buf[..buf.len().min(self.limit - self.pos)];
        Ok(buf_limited)

        // Old implementation with Rc<RefCell<dyn BufReadSeek>>
        // xTODO: Find a way to avoid copying the buffer without unsafe lifetime erasure.
        // The problem here is RefCell, but without it, we get issues with mutability
        // in WarcRecord::reader(). Another way would be to get rid of Rc entirely, but then
        // we need to take ownership of the reader and hand it back afterwards,
        // which is difficult to do when WarcRecord::freeze() is called internally.
        // let mut reader = self.reader.borrow_mut();
        // let buf = reader.fill_buf()?;
        // let buf_limited = &buf[..buf.len().min(self.limit - self.pos)];
        // self.buf.clear();
        // self.buf.reserve(buf_limited.len());
        // self.buf.extend_from_slice(buf_limited);
        // Ok(self.buf.as_slice())
    }

    fn consume(&mut self, amount: usize) {
        let amount = amount.min(self.limit - self.pos);
        self.reader.consume(amount);
        self.pos += amount;
    }
}

impl io::Seek for LimitedBufReadSeek {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        let mut new_pos = match pos {
            io::SeekFrom::Start(p) => p as i128,
            io::SeekFrom::End(p) => self.limit as i128 + p as i128,
            io::SeekFrom::Current(p) => self.pos as i128 + p as i128,
        };

        if new_pos < 0 || new_pos > i64::MAX as i128 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Seek out of range"));
        } else if new_pos > self.limit as i128 {
            new_pos = self.limit as i128;
        }

        self.reader
            .seek(io::SeekFrom::Current(new_pos as i64 - self.pos as i64))?;
        self.pos = new_pos as usize;
        Ok(self.pos as u64)
    }
}

// ===========================================================
// Tests
// ===========================================================

#[cfg(test)]
#[path = "stream_io_test.rs"]
mod stream_io_test;
