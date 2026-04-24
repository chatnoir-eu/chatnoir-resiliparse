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

use crate::stream_io::{CompressingStream, DecompressingStream, ReadSeek};
use std::io;
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use zlib_rs::{Deflate, DeflateFlush, Inflate, InflateFlush};

// ===========================================================
// GzipReader
// ===========================================================

/// Reader for Gzip-compressed streams.
pub struct GzipReader<T: ReadSeek> {
    inner: BufReader<T>,
    deflate: Inflate,
    stream_pos: u64,
    member_pos: u64,
    buf: Vec<u8>,
    buf_pos: usize,
    buf_len: usize,
    window_bits: u8,
    decomp_ratio: f32,
}

impl<T: ReadSeek> GzipReader<T> {
    /// Create a new [`GzipReader`].
    ///
    /// Allocates an internal buffer holding chunks of the uncompressed inner
    /// stream. A second, larger buffer is allocated for the decompressed data.
    /// Initially, the decompressed buffer will be twice the size of the
    /// uncompressed buffer, but its size can change based on demand.
    ///
    /// The default buffer size is 4096 bytes. For custom buffer sizes, use
    /// [`Self::with_capacity()`].
    pub fn new(inner: T) -> Self {
        Self::with_capacity(4096, inner)
    }

    /// Create a new [`GzipReader`] with a given buffer capacity.
    ///
    /// Allocates an internal buffer holding chunks of the uncompressed inner
    /// stream. A second, larger buffer is allocated for the decompressed data.
    /// Initially, the decompressed buffer will be twice the size of the
    /// uncompressed buffer, but its size can change based on demand.
    ///
    /// # Arguments
    ///
    /// * `inner` - input (inner) stream to read from
    pub fn with_capacity(capacity: usize, inner: T) -> Self {
        // Window bits above 15 (MAX_WBITS) enable gzip header decoding:
        // - +16 enables gzip header decoding only,
        // - +32 enables gzip/zlib header autodetection (we don't use this here).
        // This is standard behaviour in zlib, but so far undocumented in zlib-rs.
        let window_bits = 15 + 16;
        let decomp_ratio = 2.0;
        Self {
            inner: BufReader::with_capacity(capacity, inner),
            deflate: Inflate::new(true, window_bits),
            stream_pos: 0,
            member_pos: 0,
            buf: vec![0; capacity * decomp_ratio as usize],
            buf_pos: 0,
            buf_len: 0,
            window_bits,
            decomp_ratio,
        }
    }

    /// Unwraps this [`GzipReader`], returning the underlying reader.
    ///
    /// Note that any leftover data in the internal buffer is lost.
    pub fn into_inner(self) -> T {
        self.inner.into_inner()
    }

    /// Dynamically update the output buffer size using a moving average of the
    /// output to input size.
    ///
    /// TODO: Benchmark the parameters!
    ///
    /// # Arguments
    ///
    /// * `buf_len` - length of the uncompressed input buffer
    /// * `consumed` - number of bytes consumed from the input buffer
    /// * `produced` - number of bytes produced on the output buffer
    fn _update_buf_size(&mut self, read_len: usize, consumed: usize, produced: usize) {
        self.decomp_ratio = 0.9 * self.decomp_ratio + 0.1 * (produced as f32 / consumed as f32);
        let target_buf_size = ((read_len as f32 * self.decomp_ratio).ceil() as usize)
            .clamp(2 * self.inner.capacity(), 1 << 16)
            .max(self.buf_len)
            .next_power_of_two();

        if target_buf_size > self.buf.len() {
            // println!("Increasing output buffer to {}", target_buf_size);
            self.buf.resize(target_buf_size, 0);
        } else if target_buf_size * 2 < self.buf.len() {
            // println!("Shrinking output buffer to {}", target_buf_size);
            self.buf.truncate(target_buf_size);
        }
    }
}

impl<T: ReadSeek> io::Read for GzipReader<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let in_buf = self.fill_buf()?;
        let bytes_read = buf.len().min(in_buf.len());
        buf[..bytes_read].copy_from_slice(&in_buf[..bytes_read]);
        self.consume(bytes_read);
        Ok(bytes_read)
    }
}

impl<T: ReadSeek> Seek for GzipReader<T> {
    /// Seek to an offset, in bytes, in the decompressed output stream.
    ///
    /// Seeking in a compressed stream is not efficient with O(n) complexity,
    /// and backwards seeking and seeking from the end are not supported.
    ///
    /// # Arguments
    ///
    /// `pos` - seek position
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        super::_forward_seek(self, pos)
    }

    /// Returns the current seek position from the start of the decompressed output stream.
    fn stream_position(&mut self) -> io::Result<u64> {
        Ok(self.stream_pos)
    }
}

impl<T: ReadSeek> DecompressingStream for GzipReader<T> {
    fn inner_seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.deflate = Inflate::new(true, self.window_bits);
        self.buf_pos = 0;
        self.buf_len = 0;
        let new_pos = self.inner.seek(pos)?;
        self.member_pos = new_pos;
        Ok(new_pos)
    }

    fn inner_stream_position(&mut self) -> io::Result<u64> {
        self.inner.stream_position()
    }

    fn member_start_position(&mut self) -> io::Result<u64> {
        Ok(self.member_pos)
    }
}

impl<T: ReadSeek> BufRead for GzipReader<T> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.buf_pos < self.buf_len {
            return Ok(&self.buf[self.buf_pos..self.buf_len]);
        }

        self.buf_pos = 0;
        self.buf_len = 0;

        loop {
            let total_out = self.deflate.total_out();
            let total_in = self.deflate.total_in();

            // New member
            if total_in == 0 {
                self.member_pos = self.inner.stream_position()?;
            }

            let in_buf = self.inner.fill_buf()?;
            let in_buf_len = in_buf.len();

            let status = self
                .deflate
                .decompress(in_buf, &mut self.buf[self.buf_len..], InflateFlush::NoFlush)
                .map_err(|e| {
                    io::Error::new(io::ErrorKind::InvalidData, format!("Gzip decompression error: {}", e.as_str()))
                })?;

            let in_delta = self.deflate.total_in() - total_in;
            let out_delta = self.deflate.total_out() - total_out;
            self.inner.consume(in_delta as usize);
            self.buf_len += out_delta as usize;

            if matches!(status, zlib_rs::Status::StreamEnd) {
                self.deflate = Inflate::new(true, self.window_bits);
                break;
            }

            if in_delta > 0 {
                // Adjust output buffer size if needed
                self._update_buf_size(in_buf_len, in_delta as usize, out_delta as usize);
            }

            // Buffer full or no progress
            if self.buf_len == self.buf.len() || (in_delta == 0 && out_delta == 0) {
                break;
            }
        }

        Ok(&self.buf[..self.buf_len])
    }

    fn consume(&mut self, amount: usize) {
        let old_buf_os = self.buf_pos;
        self.buf_pos = self.buf.len().min(self.buf_pos + amount);
        self.stream_pos += (self.buf_pos - old_buf_os) as u64;
    }
}

// ===========================================================
// GzipWriter
// ===========================================================

pub struct GzipWriter<T: Write> {
    inner: Option<T>,
    deflate: Deflate,
    buf: Vec<u8>,
    buf_pos: usize,
    level: i32,
    window_bits: u8,
}

impl<T: Write> GzipWriter<T> {
    /// Create a new [`GzipWriter`].
    ///
    /// Maintains a small write buffer to temporarily store compressed data before flushing them
    /// to the underlying stream. The default buffer size is 8192 bytes. Use [`Self::with_capacity()`]
    /// for custom buffer sizes.
    ///
    /// The default compression level is 9 (best). Use [`Self::with_capacity_comp_level()`]
    /// for custom compression levels.
    ///
    /// # Arguments
    ///
    /// * `inner` - inner stream to write compressed output to
    pub fn new(inner: T) -> Self {
        Self::with_capacity(8192, inner)
    }

    /// Create a new [`GzipWriter`] a custom write buffer size.
    ///
    /// Maintains a small write buffer to temporarily store compressed data before flushing them
    /// to the underlying stream.
    ///
    /// The default compression level is 9 (best). Use [`Self::with_capacity_comp_level()`] for custom
    /// compression levels.
    ///
    /// # Arguments
    ///
    /// * `capacity` - write buffer size
    /// * `inner` - inner stream to write compressed output to
    pub fn with_capacity(capacity: usize, inner: T) -> Self {
        Self::with_capacity_comp_level(capacity, inner, 9)
    }

    /// Create a new [`GzipWriter`] with a custom write buffer size and compression level.
    ///
    /// # Arguments
    ///
    /// * `capacity` - write buffer size
    /// * `inner` - inner stream to write compressed output to
    /// * `level` - compression level (1-9)
    pub fn with_capacity_comp_level(capacity: usize, inner: T, level: i32) -> Self {
        let window_bits = 15 + 16;
        Self {
            inner: Some(inner),
            deflate: Deflate::new(level, true, window_bits),
            buf: vec![0; capacity],
            buf_pos: 0,
            level,
            window_bits,
        }
    }

    /// Unwraps this [`GzipWriter`], returning the underlying writer.
    ///
    /// Writes out buffer contents before returning the inner reader.
    pub fn into_inner(mut self) -> io::Result<T> {
        self.finish()?;
        self.flush()?;
        Ok(self.inner.take().unwrap())
    }

    /// Internal write implementation with configurable flush mode.
    fn _write(&mut self, buf: &[u8], flush: DeflateFlush) -> io::Result<usize> {
        let mut consumed = 0usize;
        let inner = self.inner.as_mut().unwrap();

        loop {
            let total_in = self.deflate.total_in();
            let total_out = self.deflate.total_out();

            let status = self
                .deflate
                .compress(&buf[consumed..], &mut self.buf[self.buf_pos..], flush)
                .map_err(|e| {
                    io::Error::new(io::ErrorKind::InvalidData, format!("Gzip compression error: {}", e.as_str()))
                })?;

            let in_delta = (self.deflate.total_in() - total_in) as usize;
            let out_delta = (self.deflate.total_out() - total_out) as usize;

            consumed += in_delta;
            self.buf_pos += out_delta;
            let stream_end = matches!(status, zlib_rs::Status::StreamEnd);

            // Write buffer stream if full or stream end
            if self.buf_pos == self.buf.len() || stream_end {
                inner.write_all(&self.buf[..self.buf_pos])?;
                self.buf_pos = 0;
            }

            // Break if all bytes consumed or stream end
            if stream_end {
                self.deflate = Deflate::new(self.level, true, self.window_bits);
                break;
            } else if consumed == buf.len() && !matches!(flush, DeflateFlush::Finish) {
                break;
            } else if in_delta == 0 && out_delta == 0 {
                return Err(io::Error::new(io::ErrorKind::WriteZero, "Deflate made no progress"));
            }
        }

        Ok(consumed)
    }

    /// Change the compression level for the next stream member.
    ///
    /// Has no effect until [`Self::finish()`] is called.
    ///
    /// # Arguments
    ///
    /// * `level` - new compression level (1-9)
    pub fn set_level(&mut self, level: i32) {
        self.level = level;
    }
}

impl<T: Write> CompressingStream for GzipWriter<T> {
    fn finish(&mut self) -> io::Result<usize> {
        let bytes_written = self._write(&[], DeflateFlush::Finish)?;
        Ok(bytes_written)
    }
}

impl<T: Write> Write for GzipWriter<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self._write(buf, DeflateFlush::NoFlush)
    }

    fn flush(&mut self) -> io::Result<()> {
        let inner = self.inner.as_mut().unwrap();
        if self.buf_pos > 0 {
            inner.write_all(&self.buf[..self.buf_pos])?;
            self.buf_pos = 0;
        }
        inner.flush()
    }
}

impl<T: Write> Drop for GzipWriter<T> {
    fn drop(&mut self) {
        if self.inner.is_some() {
            self.finish().ok();
            self.flush().ok();
        }
    }
}

// ===========================================================
// Tests
// ===========================================================

#[cfg(test)]
#[path = "gzip_test.rs"]
mod gzip_test;
