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
// Submodules
// ===========================================================

pub mod gzip;

// ===========================================================
// Global trait definitions
// ===========================================================

pub trait ReadSeek: io::Read + io::Seek + Any {}
impl<T: io::Read + io::Seek + Any + ?Sized> ReadSeek for T {}

pub trait BufReadSeek: io::BufRead + io::Seek + Any {}
impl<T: io::BufRead + io::Seek + Any + ?Sized> BufReadSeek for T {}

// ===========================================================
// Compressing / decompressing stream
// ===========================================================

/// Trait for [`io::Read`] stream implementations reading from
/// compressed input streams.
pub trait DecompressingStream: ReadSeek + Sized {
    /// Seek to an offset, in bytes, in the compressed inner stream.
    /// The semantics are the same as [`io::Seek::seek()`].
    ///
    /// Seeking on the inner stream may reset the state of the decompressor.
    /// It is up to the user to seek valid positions from which decompression
    /// can be resumed.
    fn inner_seek(&mut self, pos: io::SeekFrom) -> io::Result<u64>;

    /// Return the current seek position from the start of the compressed inner stream.
    /// The semantics are the same as [`io::Seek::stream_position()`].
    fn inner_stream_position(&mut self) -> io::Result<u64>;

    /// Return the start position, in bytes, of the current member / frame
    /// in the inner stream. If the compression format does not support
    /// multi-member streams, this is always the beginning of the stream.
    ///
    /// # Returns
    ///
    /// Position, in bytes, of the current member
    fn member_start_position(&mut self) -> io::Result<u64> {
        Ok(0)
    }
}

/// Trait for [`io::Write`] stream implementations that write compressed data
/// onto an output stream.
pub trait CompressingStream: io::Write + Sized {
    /// Finish a compression member / frame and reset the compressor state.
    ///
    /// If the compressor supports multi-member streams, the writer can be
    /// used again after this to start a new member / frame. Otherwise, writing
    /// further bytes may yield an error.
    ///
    /// Does not necessarily flush buffer contents to the inner stream.
    /// Users should call [`io::Write::flush()`] afterward to ensure that
    /// all pending data is safely written.
    ///
    /// The behavior is implementation-specific and may do nothing.
    ///
    /// # Returns
    ///
    /// Number of bytes written to the stream.
    fn finish(&mut self) -> io::Result<usize> {
        Ok(0)
    }
}

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
// Helper functions
// ===========================================================

/// Internal helper that implements forward seek in compressed streams.
///
/// For this to work, `reader.stream_position()` must report an accurate
/// position after calling `reader.consume()`.
fn _forward_seek(reader: &mut impl BufReadSeek, pos: io::SeekFrom) -> io::Result<u64> {
    let diff = match pos {
        io::SeekFrom::Start(p) => -(reader.stream_position()? as i128) + p as i128,
        io::SeekFrom::Current(p) => p as i128,
        io::SeekFrom::End(_) => {
            return Err(io::Error::new(io::ErrorKind::Unsupported, "Seeking from end not supported"));
        }
    };
    if diff < 0 {
        return Err(io::Error::new(io::ErrorKind::Unsupported, "Backward seeking not supported"));
    }

    let mut remaining =
        usize::try_from(diff).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Seek out of range"))?;

    while remaining > 0 {
        let n = reader.fill_buf()?.len().min(remaining);
        if n == 0 {
            break;
        }
        reader.consume(n);
        remaining -= n;
    }
    reader.stream_position()
}

// ===========================================================
// Tests
// ===========================================================

#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;
