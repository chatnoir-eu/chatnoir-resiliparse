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

use crate::stream_io::bufread::TrackingBufReader;
use crate::stream_io::traits::{ReadSeek, WarcRead, WarcWrite, Write as _Write};
use crate::stream_io::{impl_stream_from_path, impl_to_any_methods};
use std::any::Any;
use std::io::{self, BufRead, Read, Seek, SeekFrom, Write};

// ===========================================================
// Decoder for HTTP Transfer-Encoding: chunked
// ===========================================================

/// Reader for chunked HTTP streams.
pub struct ChunkedReader<T: ReadSeek> {
    inner: Option<TrackingBufReader<T>>,
    out_buf: Vec<u8>,
    out_buf_pos: usize,
    stream_pos: u64,
    chunk_size: usize,
    chunk_read: usize,
}

/// Options for constructing a new [`ChunkedReader`].
///
/// # Options
///
/// * `capacity` - Sets the internal buffer size (default: 4 KiB).
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ChunkedReaderOptions {
    pub capacity: usize,
}

impl Default for ChunkedReaderOptions {
    fn default() -> Self {
        Self { capacity: 4096 }
    }
}

impl<T: ReadSeek> ChunkedReader<T> {
    /// Create a new [`ChunkedReader`].
    ///
    /// Allocates an internal buffer holding chunks of the chunked inner stream.
    ///
    /// The default buffer size is 4 KiB. For custom buffer sizes, use [`Self::with_capacity()`].
    pub fn new(inner: T) -> Self {
        Self::with_options(inner, ChunkedReaderOptions::default())
    }

    /// Create a new [`ChunkedReader`] with a given buffer capacity.
    ///
    /// Allocates an internal buffer holding parts of the chunked inner stream.
    ///
    /// # Arguments
    ///
    /// * `inner` - Input (inner) stream to read from
    /// * `capacity` - Internal buffer size
    pub fn with_capacity(inner: T, capacity: usize) -> Self {
        Self::with_options(inner, ChunkedReaderOptions { capacity })
    }

    /// Create a new [`ChunkedReader`] with the supplied options.
    ///
    /// # Arguments
    ///
    /// * `inner` - Input (inner) stream to read from
    /// * `options` - Reader options
    pub fn with_options(inner: T, options: ChunkedReaderOptions) -> Self {
        Self {
            inner: Some(TrackingBufReader::with_capacity(options.capacity, inner)),
            out_buf: Vec::with_capacity(options.capacity),
            out_buf_pos: 0,
            stream_pos: 0,
            chunk_size: 0,
            chunk_read: 0,
        }
    }

    /// Unwrap this [`ChunkedReader`], returning the underlying reader.
    ///
    /// Discards input buffers, so continued reads on the unwrapped stream may fail.
    pub fn into_inner(self) -> T {
        self.inner.unwrap().into_inner()
    }
}

impl_stream_from_path!(ChunkedReader, ChunkedReaderOptions, open);

// noinspection DuplicatedCode
impl<T: ReadSeek> io::Read for ChunkedReader<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.fill_buf()?.read(buf)?;
        self.consume(n);
        Ok(n)
    }
}

// noinspection DuplicatedCode
impl<T: ReadSeek> Seek for ChunkedReader<T> {
    /// Seek to an offset, in bytes, in the output stream.
    ///
    /// Seeking in a chunked stream is not efficient with O(n) complexity,
    /// and backwards seeking and seeking from the end are not supported.
    ///
    /// # Arguments
    ///
    /// `pos` - seek position
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        if pos == SeekFrom::Current(0) {
            return Ok(self.stream_pos);
        }
        super::_forward_seek(self, pos)
    }

    /// Returns the current seek position from the start of the output stream.
    #[inline]
    fn stream_position(&mut self) -> io::Result<u64> {
        Ok(self.stream_pos)
    }
}

impl<T: ReadSeek> WarcRead for ChunkedReader<T> {
    impl_to_any_methods!();

    #[inline]
    fn inner_seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let new_pos = self.inner.as_mut().unwrap().seek(pos)?;
        self.stream_pos = 0;
        self.chunk_size = 0;
        self.out_buf_pos = 0;
        Ok(new_pos)
    }

    #[inline]
    fn inner_stream_position(&mut self) -> io::Result<u64> {
        self.inner.as_mut().unwrap().stream_position()
    }

    #[inline]
    fn is_stream_decoder(&self) -> bool {
        true
    }
}

impl<T: ReadSeek> ChunkedReader<T> {
    fn consume_chunk_end(&mut self) -> io::Result<()> {
        let mut buf = [0u8; 2];
        self.inner
            .as_mut()
            .unwrap()
            .read_exact(&mut buf)
            .map_err(|_| io::ErrorKind::UnexpectedEof)?;
        self.chunk_read = 0;
        self.chunk_size = 0;
        Ok(())
    }
}
impl<T: ReadSeek> BufRead for ChunkedReader<T> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if !self.out_buf[self.out_buf_pos..].is_empty() {
            return Ok(&self.out_buf[self.out_buf_pos..]);
        }
        loop {
            let inner = self.inner.as_mut().unwrap();

            // New chunk
            if self.chunk_read == self.chunk_size {
                let mut header = Vec::with_capacity(8);
                inner.read_until(b'\n', &mut header)?;
                if header.is_empty() {
                    return Ok(&self.out_buf[..0]);
                }
                let header = String::from_utf8_lossy(&header);
                let header = header.trim_ascii();
                self.chunk_size = usize::from_str_radix(header, 16).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, format!("Invalid chunk header: '{}'", header))
                })?;

                // EOF
                if self.chunk_size == 0 {
                    self.consume_chunk_end()?;
                    return Ok(&self.out_buf[..0]);
                }
            }

            // Within chunk
            if self.chunk_read < self.chunk_size {
                let remaining = self.chunk_size - self.chunk_read;
                let in_buf = inner.fill_buf()?;
                let n = in_buf.len().min(remaining);
                if n == 0 {
                    return Err(io::ErrorKind::UnexpectedEof.into());
                }
                self.out_buf.resize(n, 0);
                self.out_buf_pos = 0;
                self.out_buf[..n].copy_from_slice(&in_buf[..n]);
                self.chunk_read += n;
                inner.consume(n);

                // End of chunk, consume trailing CRLF
                if self.chunk_read == self.chunk_size {
                    self.consume_chunk_end()?;
                }

                return Ok(&self.out_buf);
            }
        }
    }

    fn consume(&mut self, amount: usize) {
        self.stream_pos += amount as u64;
        self.out_buf_pos += amount;
    }
}

// ===========================================================
// ChunkedWriter
// ===========================================================

/// Writer for chunked HTTP streams.
pub struct ChunkedWriter<T: _Write> {
    inner: Option<T>,
    chunk_buffer: Vec<u8>,
    min_chunk_size: usize,
    stream_started: bool,
}

/// Options for constructing a new [`ChunkedWriter`].
///
/// # Options
///
/// * `min_chunk_size` - Minimum number of buffered payload bytes before emitting a chunk (default: 1024).
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ChunkedWriterOptions {
    pub min_chunk_size: usize,
}

impl Default for ChunkedWriterOptions {
    fn default() -> Self {
        Self { min_chunk_size: 1024 }
    }
}

// noinspection DuplicatedCode
impl<T: _Write> ChunkedWriter<T> {
    /// Create a new [`ChunkedWriter`].
    ///
    /// Maintains a small write buffer to temporarily accumulate data in a chunk before flushing it
    /// to the underlying stream. The default minimum chunk size is 1024 bytes. Use [`Self::with_chunk_size()`]
    /// for custom buffer sizes.
    ///
    /// # Arguments
    ///
    /// * `inner` - Inner stream to write chunked output to
    pub fn new(inner: T) -> Self {
        Self::with_options(inner, ChunkedWriterOptions::default())
    }

    /// Create a new [`ChunkedWriter`] a custom write buffer size.
    ///
    /// Maintains a small write buffer to temporarily accumulate data in a chunk before flushing it
    /// to the underlying stream. The default minimum chunk size is 1024 bytes.
    ///
    /// # Arguments
    ///
    /// * `inner` - Inner stream to write chunked output to
    /// * `min_chunk_size` - Chunk write buffer size
    pub fn with_chunk_size(inner: T, min_chunk_size: usize) -> Self {
        Self::with_options(inner, ChunkedWriterOptions { min_chunk_size })
    }

    /// Create a new [`ChunkedWriter`] with the supplied options.
    ///
    /// # Arguments
    ///
    /// * `inner` - Inner stream to write chunked output to
    /// * `options` - Writer options
    pub fn with_options(inner: T, options: ChunkedWriterOptions) -> Self {
        Self {
            inner: Some(inner),
            chunk_buffer: Vec::with_capacity(options.min_chunk_size),
            min_chunk_size: options.min_chunk_size,
            stream_started: false,
        }
    }

    /// Unwrap this [`ChunkedWriter`], returning the underlying writer.
    ///
    /// Writes out buffer contents before returning the inner reader.
    pub fn into_inner(mut self) -> io::Result<T> {
        self.finish()?;
        self.flush()?;
        Ok(self.inner.take().unwrap())
    }
}

impl<T: _Write> ChunkedWriter<T> {
    fn write_chunk_buffer(&mut self) -> io::Result<usize> {
        let inner = self.inner.as_mut().unwrap();
        let header = format!("{:x}\r\n", self.chunk_buffer.len()).into_bytes();
        inner.write_all(&header)?;
        self.chunk_buffer.extend_from_slice(b"\r\n");
        inner.write_all(&self.chunk_buffer)?;
        let n = header.len() + self.chunk_buffer.len();
        self.chunk_buffer.clear();
        Ok(n)
    }
}

impl_stream_from_path!(ChunkedWriter, ChunkedWriterOptions, create);

impl<T: _Write> WarcWrite for ChunkedWriter<T> {
    impl_to_any_methods!();

    fn finish(&mut self) -> io::Result<()> {
        if self.chunk_buffer.is_empty() && !self.stream_started {
            return Ok(());
        }
        self.write_chunk_buffer()?;
        self.inner.as_mut().unwrap().write_all(b"0\r\n\r\n")?;
        self.stream_started = false;
        Ok(())
    }
}

impl<T: _Write> io::Write for ChunkedWriter<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream_started = true;
        self.chunk_buffer.extend_from_slice(buf);
        if self.chunk_buffer.len() >= self.min_chunk_size {
            self.write_chunk_buffer()?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.as_mut().unwrap().flush()
    }
}

// noinspection DuplicatedCode
impl<T: _Write> Drop for ChunkedWriter<T> {
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
#[path = "chunked_test.rs"]
mod chunked_test;
