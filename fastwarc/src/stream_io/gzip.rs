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

/// Reader for Gzip-compressed streams (supports Zlib and DEFLATE formats as well).
pub struct GzipReader<T> {
    inner: BufReader<T>,
    deflate: Inflate,
    buf: Vec<u8>,
    buf_pos: usize,
    buf_len: usize,
    zlib_header: bool,
    window_bits: u8,
}

///
#[derive(Default)]
pub enum GzipHeaderType {
    #[default]
    Gzip,
    Zlib,
    Raw,
}

impl<T: ReadSeek> GzipReader<T> {
    /// Create a new [`DecompressingStream`] on a given input stream.
    ///
    /// Default buffer size: 8192 bytes.
    ///
    /// # Arguments
    ///
    /// * `input_stream` - input (inner) stream to read from
    pub fn new(input_stream: T) -> Self {
        Self::new_with_params(input_stream, 8192, GzipHeaderType::Gzip, 15)
    }

    /// Create a new Gzip reader with parameters.
    ///
    /// # Arguments
    ///
    /// * `input_stream` - input (inner) stream to read from
    /// * `buffer_size` - size of the decompression buffer (another buffer half the size for the
    ///   uncompressed input data will be created as well)
    /// * `header_type` - whether to write a gzip, zlib, or no header (raw)
    /// * `window_bits` - base-two logarithm of the window size (max: 15)
    pub fn new_with_params(
        input_stream: T,
        buffer_size: usize,
        header_type: GzipHeaderType,
        mut window_bits: u8,
    ) -> Self {
        if matches!(header_type, GzipHeaderType::Gzip) {
            // Window sizes above 16 enable gzip header and checksum (standard behaviour in
            // zlib, but undocumented in zlib-rs).
            window_bits += 16;
        }
        let zlib_header = !matches!(header_type, GzipHeaderType::Raw);
        Self {
            inner: BufReader::with_capacity(buffer_size / 2, input_stream),
            deflate: Inflate::new(zlib_header, window_bits),
            buf: vec![0; buffer_size],
            buf_pos: 0,
            buf_len: 0,
            zlib_header,
            window_bits,
        }
    }

    fn _decompress_buf(&mut self) -> io::Result<()> {
        let mut total_out;
        let mut total_in;
        self.buf_pos = 0;
        self.buf_len = 0;

        loop {
            total_out = self.deflate.total_out();
            total_in = self.deflate.total_in();

            let in_buf = self.inner.fill_buf()?;
            if in_buf.is_empty() {
                // EOF
                break;
            }

            match self
                .deflate
                .decompress(in_buf, &mut self.buf[self.buf_len..], InflateFlush::NoFlush)
            {
                Err(e) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Gzip decompression error: {}", e.as_str()),
                    ));
                }
                Ok(status) => {
                    let consumed = self.deflate.total_in() - total_in;
                    let produced = self.deflate.total_out() - total_out;
                    self.inner.consume(consumed as usize);
                    self.buf_len += produced as usize;
                    match status {
                        zlib_rs::Status::StreamEnd => {
                            self.deflate = Inflate::new(self.zlib_header, self.window_bits);
                        }
                        _ if consumed > 0 && produced == 0 => {
                            // Need more data
                            continue;
                        }
                        _ => {}
                    };
                    break;
                }
            }
        }

        Ok(())
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
        if self.buf_pos >= self.buf_len {
            self._decompress_buf()?;
        }
        Ok(&self.buf[self.buf_pos..self.buf_len])
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
