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

use crate::stream_io::ReadSeek;
use std::io;
use std::io::{BufRead, BufReader};
use zlib_rs::{Inflate, InflateFlush};

/// Reader for Gzip-compressed streams.
pub struct GzipReader<T> {
    inner: BufReader<T>,
    deflate: Inflate,
    buf: Vec<u8>,
    buf_pos: usize,
    buf_len: usize,
    window_bits: u8,
    decomp_ratio: f32,
}

impl<T: ReadSeek> GzipReader<T> {
    /// Create a new Gzip reader with parameters with a given buffer capacity.
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

    /// Create a new Gzip reader with parameters with a given buffer capacity.
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
            buf: vec![0; capacity * decomp_ratio as usize],
            buf_pos: 0,
            buf_len: 0,
            window_bits,
            decomp_ratio,
        }
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

impl<T: ReadSeek> io::Seek for GzipReader<T> {
    fn seek(&mut self, _: io::SeekFrom) -> io::Result<u64> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "Seek not supported on compressed streams"))
    }

    fn stream_position(&mut self) -> io::Result<u64> {
        self.inner.stream_position()
    }
}

impl<T: ReadSeek> BufRead for GzipReader<T> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.buf_pos < self.buf_len {
            return Ok(&self.buf[self.buf_pos..self.buf_len]);
        }

        self.buf_pos = 0;
        self.buf_len = 0;
        let mut total_out;
        let mut total_in;

        loop {
            total_out = self.deflate.total_out() as usize;
            total_in = self.deflate.total_in() as usize;

            let in_buf = self.inner.fill_buf()?;
            let in_buf_len = in_buf.len();
            if in_buf_len == 0 {
                // EOF
                break;
            }

            let status = self
                .deflate
                .decompress(in_buf, &mut self.buf[self.buf_len..], InflateFlush::NoFlush)
                .map_err(|e| {
                    io::Error::new(io::ErrorKind::InvalidData, format!("Gzip decompression error: {}", e.as_str()))
                })?;

            let consumed = self.deflate.total_in() as usize - total_in;
            let produced = self.deflate.total_out() as usize - total_out;
            self.inner.consume(consumed);
            self.buf_len += produced;

            let stream_end = matches!(status, zlib_rs::Status::StreamEnd);
            if stream_end {
                self.deflate = Inflate::new(true, self.window_bits);
            }

            if consumed > 0 && produced == 0 {
                // Need more data
                continue;
            } else if !stream_end && consumed > 0 {
                // Adjust output buffer size if needed (only if not stream end, which may be an outlier)
                self._update_buf_size(in_buf_len, consumed, produced);
            }

            break;
        }

        Ok(&self.buf[..self.buf_len])
    }

    fn consume(&mut self, amount: usize) {
        self.buf_pos = self.buf.len().min(self.buf_pos + amount);
    }
}

// ===========================================================
// Tests
// ===========================================================

#[cfg(test)]
#[path = "gzip_test.rs"]
mod gzip_test;
