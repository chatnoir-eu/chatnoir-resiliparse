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

use crate::stream_io::traits::{ReadSeek, WarcRead, WarcWrite, Write as _Write};
use crate::stream_io::{impl_stream_from_path, impl_to_any_methods};
use brotli::{CompressorWriter, Decompressor};
use std::any::Any;
use std::io::{self, BufRead, Seek, SeekFrom, Write};

// ===========================================================
// BrotliReader
// ===========================================================

/// Reader for Brotli-compressed streams.
pub struct BrotliReader<T: ReadSeek> {
    inner: Option<io::BufReader<Decompressor<T>>>,
    stream_pos: u64,
}

/// Options for constructing a new [`BrotliReader`].
///
/// # Options
///
/// * `capacity` - Sets the internal buffer size (default: 4 KiB).
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct BrotliReaderOptions {
    pub capacity: usize,
}

impl Default for BrotliReaderOptions {
    fn default() -> Self {
        Self { capacity: 4096 }
    }
}

impl<T: ReadSeek> BrotliReader<T> {
    /// Create a new [`BrotliReader`].
    ///
    /// Allocates an internal buffer holding chunks of the inner stream.
    ///
    /// The default buffer size is 4096 bytes. For custom buffer sizes, use [`Self::with_capacity()`].
    pub fn new(inner: T) -> Self {
        Self::with_options(inner, BrotliReaderOptions::default())
    }

    /// Create a new [`BrotliReader`] with a given buffer capacity.
    ///
    /// Allocates an internal buffer holding chunks of the inner stream.
    ///
    /// # Arguments
    ///
    /// * `inner` - Input (inner) stream to read from
    /// * `capacity` - Internal buffer size
    pub fn with_capacity(inner: T, capacity: usize) -> Self {
        Self::with_options(inner, BrotliReaderOptions { capacity })
    }

    /// Create a new [`BrotliReader`] with the supplied options.
    ///
    /// # Arguments
    ///
    /// * `inner` - Input (inner) stream to read from
    /// * `options` - Reader options
    pub fn with_options(inner: T, options: BrotliReaderOptions) -> Self {
        Self {
            inner: Some(io::BufReader::with_capacity(options.capacity, Decompressor::new(inner, options.capacity))),
            stream_pos: 0,
        }
    }

    /// Unwrap this [`BrotliReader`], returning the underlying reader.
    ///.
    /// Discards input buffers, so continued reads on the unwrapped stream may fail.
    pub fn into_inner(self) -> T {
        self.inner.unwrap().into_inner().into_inner()
    }
}

impl_stream_from_path!(BrotliReader, BrotliReaderOptions, open);

// noinspection DuplicatedCode
impl<T: ReadSeek> io::Read for BrotliReader<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.fill_buf()?.read(buf)?;
        self.consume(n);
        Ok(n)
    }
}

impl<T: ReadSeek> Seek for BrotliReader<T> {
    /// Seek to an offset, in bytes, in the decompressed output stream.
    ///
    /// Seeking in a compressed stream is not efficient with O(n) complexity,
    /// and backwards seeking and seeking from the end are not supported.
    ///
    /// # Arguments
    ///
    /// `pos` - seek position
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.stream_pos = super::_forward_seek(self, pos)?;
        Ok(self.stream_pos)
    }

    /// Returns the current seek position from the start of the decompressed output stream.
    #[inline]
    fn stream_position(&mut self) -> io::Result<u64> {
        Ok(self.stream_pos)
    }
}

impl<T: ReadSeek> WarcRead for BrotliReader<T> {
    impl_to_any_methods!();

    #[inline]
    fn inner_seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let capacity = self.inner.as_ref().unwrap().capacity();
        let mut inner = self.inner.take().unwrap().into_inner().into_inner();
        let new_pos = inner.seek(pos)?;
        self.inner = Some(io::BufReader::with_capacity(capacity, Decompressor::new(inner, capacity)));
        Ok(new_pos)
    }

    #[inline]
    fn inner_stream_position(&mut self) -> io::Result<u64> {
        self.inner.as_mut().unwrap().get_mut().get_mut().stream_position()
    }

    #[inline]
    fn is_stream_decoder(&self) -> bool {
        true
    }
}

// noinspection DuplicatedCode
impl<T: ReadSeek> BufRead for BrotliReader<T> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.inner.as_mut().unwrap().fill_buf()
    }

    fn consume(&mut self, amount: usize) {
        if let Some(inner) = self.inner.as_mut() {
            inner.consume(amount);
            self.stream_pos += amount as u64;
        }
    }
}

// ===========================================================
// BrotliWriter
// ===========================================================

/// Writer for Brotli-compressed streams.
pub struct BrotliWriter<T: _Write> {
    inner: Option<CompressorWriter<T>>,
}

/// Options for constructing a new [`BrotliWriter`].
///
/// # Options
///
/// * `capacity` - Sets the internal buffer size.
/// * `quality` - Compression quality from 0 to 11 (default: 4).
/// * `lg_window_size` - Base-2 logarithm of the sliding-window size (default: 22).
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct BrotliWriterOptions {
    pub capacity: usize,
    pub quality: u32,
    pub lg_window_size: u32,
}

impl Default for BrotliWriterOptions {
    fn default() -> Self {
        Self {
            capacity: 8192,
            quality: 4,
            lg_window_size: 22,
        }
    }
}

impl<T: _Write> WarcWrite for BrotliWriter<T> {
    impl_to_any_methods!();
}

impl<T: _Write> BrotliWriter<T> {
    /// Create a new [`BrotliWriter`].
    ///
    /// Maintains a small write buffer to temporarily store compressed data before flushing them
    /// to the underlying stream. The default buffer size is 8192 bytes. Use [`Self::with_capacity()`]
    /// for custom buffer sizes.
    ///
    /// # Arguments
    ///
    /// * `inner` - Inner stream to write compressed output to
    pub fn new(inner: T) -> Self {
        Self::with_options(inner, BrotliWriterOptions::default())
    }

    /// Create a new [`BrotliWriter`] a custom write buffer size.
    ///
    /// Maintains a small write buffer to temporarily store compressed data before flushing them
    /// to the underlying stream.
    ///
    /// # Arguments
    ///
    /// * `inner` - Inner stream to write compressed output to
    /// * `capacity` - Write buffer size
    pub fn with_capacity(inner: T, capacity: usize) -> Self {
        Self::with_options(
            inner,
            BrotliWriterOptions {
                capacity,
                ..BrotliWriterOptions::default()
            },
        )
    }

    /// Create a new [`BrotliWriter`] with the supplied options.
    ///
    /// # Arguments
    ///
    /// * `inner` - Inner stream to write compressed output to
    /// * `options` - Writer options
    pub fn with_options(inner: T, options: BrotliWriterOptions) -> Self {
        Self {
            inner: Some(CompressorWriter::new(inner, options.capacity, options.quality, options.lg_window_size)),
        }
    }

    /// Unwrap this [`BrotliWriter`], returning the underlying writer.
    ///
    /// Writes out buffer contents before returning the inner reader.
    pub fn into_inner(mut self) -> io::Result<T> {
        let mut writer = self.inner.take().unwrap().into_inner();
        writer.flush()?;
        Ok(writer)
    }
}

impl_stream_from_path!(BrotliWriter, BrotliWriterOptions, create);

impl<T: _Write> io::Write for BrotliWriter<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.as_mut().unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.as_mut().unwrap().flush()?;
        Ok(())
    }
}

// noinspection DuplicatedCode
impl<T: _Write> Drop for BrotliWriter<T> {
    fn drop(&mut self) {
        if self.inner.is_some() {
            self.flush().ok();
        }
    }
}

// ===========================================================
// Tests
// ===========================================================

#[cfg(test)]
#[path = "brotli_test.rs"]
mod brotli_test;
