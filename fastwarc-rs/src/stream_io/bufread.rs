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

use crate::stream_io::impl_to_any_methods;
use crate::stream_io::traits::{
    BufReadSeek, IntoWarcReader, IntoWarcWriter, ReadSeek, WarcRead, WarcWrite, Write as _Write,
};
use memchr::memchr;
use std::any::Any;
use std::io;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};

pub(crate) const DEFAULT_BUFFER_SIZE: usize = 64 << 10;

// ===========================================================
// Tracking buffered reader.
// ===========================================================

/// A thin wrapper around [`BufReader`] that implements [`Read`] + [`Seek`], but
/// caches its stream position to make calls to [`Seek::stream_position()`] and
/// [`Seek::seek(SeekFrom::Current(0))`](Seek::seek()) cheap.
///
/// Caching the current reader position is important because WARC parsing relies on
/// seeking to determine the current record start position, but forwarding that to a syscall
/// for every record is prohibitively expensive.
///
/// The wrapped inner stream must also implement [`Seek`] but should not implement [`BufRead`]
/// to avoid unnecessary double buffering. Use [`RawReaderAdapter`] instead in that case.
///
/// Especially for linear WARC parsing, it is recommended to use a large buffer size (64-256 KiB)
/// for at least the lowest layer in the reader stack. That's why the default buffer size of
/// [`TrackingBufReader`] is much larger than the default of [`BufReader`].
///
/// Even though [`TrackingBufReader`] implements [`WarcRead`], it should not be used to wrap other
/// [`WarcRead`] readers, as it does not actually forward [`WarcRead::inner_stream_position`] and
/// [`WarcRead::inner_seek()`] to the wrapped stream.
pub struct TrackingBufReader<T> {
    inner: BufReader<T>,
    pos: u64,
}

impl<T: Read + Seek> TrackingBufReader<T> {
    /// Create a new [`TrackingBufReader`] with a default buffer size of 64 KiB bytes.
    ///
    /// This constructor will issue a single seek call to the inner stream to determine
    /// its initial position. If the call fails, the position defaults to `0`.
    ///
    /// # Arguments
    ///
    /// * `inner` - Wrapped stream
    pub fn new(inner: T) -> Self {
        Self::with_capacity(DEFAULT_BUFFER_SIZE, inner)
    }

    /// Create a new [`TrackingBufReader`] with a chosen buffer capacity.
    ///
    /// This constructor will issue a single seek call to the inner stream to determine
    /// its initial position. If the call fails, the position defaults to `0`.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Buffer capacity
    /// * `inner` - Wrapped stream
    pub fn with_capacity(capacity: usize, mut inner: T) -> Self {
        let pos = inner.stream_position().unwrap_or(0);
        Self {
            inner: BufReader::with_capacity(capacity, inner),
            pos,
        }
    }

    /// Get a reference to the inner reader.
    #[inline]
    pub fn get_ref(&self) -> &T {
        self.inner.get_ref()
    }

    /// Get a mutable reference to the inner reader.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }

    /// Unwrap this [`TrackingBufReader`], returning the underlying reader.
    ///
    /// Discards input buffers, so continued reads on the unwrapped stream may fail.
    #[inline]
    pub fn into_inner(self) -> T {
        self.inner.into_inner()
    }

    /// Return a reference to the internally buffered data.
    #[inline]
    pub fn buffer(&self) -> &[u8] {
        self.inner.buffer()
    }

    /// Return the number of bytes the internal buffer can hold at once.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }
}

macro_rules! impl_tracking_bufread_seek {
    ($Type:ty, $($TraitBounds:tt)+) => {
        impl<T: $($TraitBounds)+> Read for $Type {
            #[inline]
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                let n = self.fill_buf()?.read(buf)?;
                self.consume(n);
                Ok(n)
            }
        }

        impl<T: $($TraitBounds)+> Seek for $Type {
            #[inline]
            fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
                if let SeekFrom::Current(pos) = pos {
                    if pos == 0 {
                        return Ok(self.pos);
                    } else if pos > 0 && (pos as usize) <= self.fill_buf()?.len() {
                        self.consume(pos as usize);
                        return Ok(self.pos);
                    }
                } else if pos == SeekFrom::Start(self.pos) {
                    return Ok(self.pos);
                }
                self.pos = self.inner.seek(pos)?;
                Ok(self.pos)
            }
        }

        impl<T: $($TraitBounds)+> BufRead for $Type {
            #[inline]
            fn fill_buf(&mut self) -> io::Result<&[u8]> {
                self.inner.fill_buf()
            }

            #[inline]
            fn consume(&mut self, amount: usize) {
                self.inner.consume(amount);
                self.pos += amount as u64;
            }
        }
    };
}

impl_tracking_bufread_seek!(TrackingBufReader<T>, Read + Seek);

impl<T: ReadSeek> WarcRead for TrackingBufReader<T> {
    impl_to_any_methods!();

    #[inline]
    fn inner_seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.seek(pos)
    }

    #[inline]
    fn inner_stream_position(&mut self) -> io::Result<u64> {
        self.stream_position()
    }
}

// ===========================================================
// RawReaderAdapter
// ===========================================================

/// Wrapper type for arbitrary [`BufReadSeek`] readers that implements [`WarcRead`].
/// Like [`TrackingBufReader`], this reader implementation caches its current stream
/// position to make calls to [`Seek::stream_position()`] and
/// [`Seek::seek(SeekFrom::Current(0))`](Seek::seek()) cheap.
///
/// Use this type over [`TrackingBufReader`] if the wrapped reader alrady implements
/// [`BufReadSeek`] to avoid unnecessary double buffering.
///
/// Especially for linear WARC parsing, it is recommended to use a large buffer size
/// (64-256 KiB) for at least the lowest layer in the reader stack. Consider constructing
/// the inner stream accordingly before wrapping it.
pub struct RawReaderAdapter<T> {
    inner: T,
    pos: u64,
}

impl<T: BufReadSeek> RawReaderAdapter<T> {
    /// Create a new [`RawReaderAdapter`] from an existing buffered reader.
    ///
    /// This constructor will issue a single seek call to the inner stream to determine
    /// its initial position. If the call fails, the position defaults to `0`.
    ///
    /// # Arguments
    ///
    /// * `inner` - Wrapped stream
    pub fn new(mut inner: T) -> Self {
        let pos = inner.stream_position().unwrap_or(0);
        Self { inner, pos }
    }

    /// Get a reference to the inner reader.
    #[inline]
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Get a mutable reference to the inner reader.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Unwrap this [`RawReaderAdapter`], returning the underlying reader.
    ///
    /// Discards input buffers, so continued reads on the unwrapped stream may fail.
    #[inline]
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T: Send> WarcRead for RawReaderAdapter<T>
where
    T: BufReadSeek,
{
    impl_to_any_methods!();

    #[inline]
    fn inner_seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.seek(pos)
    }

    #[inline]
    fn inner_stream_position(&mut self) -> io::Result<u64> {
        self.stream_position()
    }
}

impl_tracking_bufread_seek!(RawReaderAdapter<T>, BufReadSeek);

// ===========================================================
// IntoWarcReader implementations
// ===========================================================

impl<T: WarcRead> IntoWarcReader for T {
    #[inline]
    fn into_warc_reader(self) -> Box<dyn WarcRead> {
        Box::new(self)
    }
}

impl IntoWarcReader for Box<dyn WarcRead> {
    #[inline]
    fn into_warc_reader(self) -> Box<dyn WarcRead> {
        self
    }
}

impl IntoWarcReader for Box<dyn WarcRead + Send> {
    #[inline]
    fn into_warc_reader(self) -> Box<dyn WarcRead> {
        self
    }
}

impl IntoWarcReader for Box<dyn WarcRead + Send + Sync> {
    #[inline]
    fn into_warc_reader(self) -> Box<dyn WarcRead> {
        self
    }
}

impl<T: ReadSeek> IntoWarcReader for BufReader<T> {
    #[inline]
    fn into_warc_reader(self) -> Box<dyn WarcRead> {
        Box::new(RawReaderAdapter::new(self))
    }
}

impl<T: AsRef<[u8]> + Send + 'static> IntoWarcReader for io::Cursor<T> {
    #[inline]
    fn into_warc_reader(self) -> Box<dyn WarcRead> {
        Box::new(RawReaderAdapter::new(self))
    }
}

impl IntoWarcReader for std::fs::File {
    #[inline]
    fn into_warc_reader(self) -> Box<dyn WarcRead> {
        Box::new(TrackingBufReader::new(self))
    }
}

// ===========================================================
// RawWriterAdapter
// ===========================================================

/// Wrapper type for arbitrary [`Write`] readers that implements [`WarcWrite`].
pub struct RawWriterAdapter<T> {
    inner: T,
}

impl<T: _Write> RawWriterAdapter<T> {
    /// Create a new [`RawWriterAdapter`] from an existing writer.
    ///
    /// # Arguments
    ///
    /// * `inner` - Wrapped stream
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    /// Get a reference to the inner writer.
    #[inline]
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Get a mutable reference to the inner writer.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Unwrap this [`RawWriterAdapter`], returning the underlying reader.
    ///
    /// Discards input buffers, so continued reads on the unwrapped stream may fail.
    #[inline]
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T: _Write> Write for RawWriterAdapter<T> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<T: _Write> WarcWrite for RawWriterAdapter<T> {
    impl_to_any_methods!();
}

// ===========================================================
// IntoWarcWriter implementations
// ===========================================================

impl<T: WarcWrite> IntoWarcWriter for T {
    #[inline]
    fn into_warc_writer(self) -> Box<dyn WarcWrite> {
        Box::new(self)
    }
}

impl IntoWarcWriter for Box<dyn WarcWrite> {
    #[inline]
    fn into_warc_writer(self) -> Box<dyn WarcWrite> {
        self
    }
}

impl IntoWarcWriter for Box<dyn WarcWrite + Send> {
    #[inline]
    fn into_warc_writer(self) -> Box<dyn WarcWrite> {
        self
    }
}

impl IntoWarcWriter for Box<dyn WarcWrite + Send + Sync> {
    #[inline]
    fn into_warc_writer(self) -> Box<dyn WarcWrite> {
        self
    }
}

impl<T: _Write> IntoWarcWriter for io::BufWriter<T> {
    #[inline]
    fn into_warc_writer(self) -> Box<dyn WarcWrite> {
        Box::new(RawWriterAdapter::new(self))
    }
}

impl<T> IntoWarcWriter for io::Cursor<T>
where
    T: AsMut<[u8]> + Send + 'static,
    io::Cursor<T>: Write,
{
    #[inline]
    fn into_warc_writer(self) -> Box<dyn WarcWrite> {
        Box::new(RawWriterAdapter::new(self))
    }
}

impl IntoWarcWriter for std::fs::File {
    #[inline]
    fn into_warc_writer(self) -> Box<dyn WarcWrite> {
        Box::new(RawWriterAdapter::new(self))
    }
}

// ===========================================================
// Limited buffered reader
// ===========================================================

/// A limited seekable buffered reader.
///
/// Wraps an existing [`BufReadSeek`] reader, setting the current reader position to be the new
/// stream start, and terminating with a premature, virtual EOF when the set limit is reached.
/// Calls to [`Self::stream_position()`] and [`Self::seek()`] work with and return the new
/// logical stream positions.
///
/// Does not allocate a new buffer. All calls are passed directly to the underlying reader.
pub struct LimitedBufReader {
    inner: Box<dyn WarcRead>,
    limit: u64,
    pos: u64,
}

impl LimitedBufReader {
    /// Create a new limited reader from a buffered reader instance.
    ///
    /// # Arguments
    ///
    /// * `reader` - Inner reader to put the limit on
    /// * `limit` - Limit in bytes at which EOF is returned
    pub fn new(reader: impl IntoWarcReader, limit: Option<u64>) -> Self {
        let mut reader = reader.into_warc_reader();
        let pos = reader.stream_position().unwrap_or(0);
        Self {
            inner: reader,
            limit: limit.unwrap_or(u64::MAX),
            pos,
        }
    }

    /// Change the limit of the reader.
    /// Also resets the logical stream position to 0. Use [`Self::real_stream_position()`] to get
    /// the real position on the original stream.
    ///
    /// # Arguments
    ///
    /// * `limit` - New reader limit
    #[inline]
    pub fn set_limit(&mut self, limit: u64) {
        self.limit = limit;
        self.pos = 0;
    }

    /// Reset the reader to being unlimited.
    #[inline]
    pub fn reset_limit(&mut self) {
        self.limit = u64::MAX;
        self.pos = self.inner.stream_position().unwrap_or(0);
    }

    /// Get the current limit.
    #[inline]
    pub fn limit(&self) -> u64 {
        self.limit
    }

    /// Get the real (not the logical) stream position in the inner stream.
    /// This method is non-recursive. Use [`Self::inner_stream_position()`] if you need
    /// to reach through a decoding inner reader stack.
    #[inline]
    pub fn real_stream_position(&mut self) -> io::Result<u64> {
        self.inner.stream_position()
    }

    /// Unwrap this [`LimitedBufReader`], returning the underlying reader.
    ///
    /// Discards input buffers, so continued reads on the unwrapped stream may fail.
    #[inline]
    pub fn into_inner(self) -> Box<dyn WarcRead> {
        self.inner
    }

    /// Borrow the wrapped reader as [`Any`] for type inspection.
    #[inline]
    pub fn inner_as_any(&self) -> &dyn Any {
        self.inner.as_any()
    }

    /// Read until a linefeed (LF) is found or `max_line_len` is reached.
    /// The results are appended to the provided buffer.
    ///
    /// Apart from the line length limitation, the behavior is the same as [`io::BufRead::read_line()`].
    /// Unlike `Read::take(max_line_len).read_until(b'\n', buf)`, the read is guaranteed to be complete.
    /// The function will not return until a linefeed is found or EOF is reached.
    ///
    /// # Arguments
    ///
    /// * `buf` - Output buffer to append to
    /// * `max_line_len` - Maximum line length to read if no LF found
    ///
    /// # Returns
    ///
    /// Number of bytes read
    #[inline]
    pub fn read_line(&mut self, buf: &mut Vec<u8>, max_line_len: usize) -> io::Result<usize> {
        let start_len = buf.len();
        let max_line_len = max_line_len.min(self.limit() as usize);
        buf.reserve(max_line_len);

        while buf.len() - start_len < max_line_len {
            let chunk = self.fill_buf()?;
            if chunk.is_empty() {
                break;
            }
            let remaining = max_line_len - (buf.len() - start_len);
            let limit = chunk.len().min(remaining);

            if let Some(pos) = memchr(b'\n', &chunk[..limit]) {
                buf.extend_from_slice(&chunk[..=pos]);
                self.consume(pos + 1);
                return Ok(buf.len() - start_len);
            }
            buf.extend_from_slice(&chunk[..limit]);
            self.consume(limit);
        }
        Ok(buf.len() - start_len)
    }
}

impl WarcRead for LimitedBufReader {
    impl_to_any_methods!();

    #[inline]
    fn inner_seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.inner_seek(pos)
    }

    #[inline]
    fn inner_stream_position(&mut self) -> io::Result<u64> {
        self.inner.inner_stream_position()
    }

    #[inline]
    fn frame_start_position(&mut self) -> io::Result<Option<u64>> {
        self.inner.frame_start_position()
    }

    #[inline]
    fn is_stream_decoder(&self) -> bool {
        self.inner.is_stream_decoder()
    }
}

impl BufRead for LimitedBufReader {
    #[inline]
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        let buf = self.inner.fill_buf()?;
        let buf_limited = &buf[..(buf.len() as u64).min(self.limit - self.pos) as usize];
        Ok(buf_limited)
    }

    #[inline]
    fn consume(&mut self, amount: usize) {
        let amount = (amount as u64).min(self.limit - self.pos) as usize;
        self.inner.consume(amount);
        self.pos += amount as u64;
    }
}

impl Read for LimitedBufReader {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let l = buf.len();
        let buf_limited = &mut buf[..(l as u64).min(self.limit - self.pos) as usize];
        let n = self.inner.read(buf_limited)?;
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for LimitedBufReader {
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        if pos == SeekFrom::Current(0) {
            return Ok(self.pos);
        }

        let mut new_pos = match pos {
            SeekFrom::Start(p) => p as i128,
            SeekFrom::End(p) => self.limit as i128 + p as i128,
            SeekFrom::Current(p) => self.pos as i128 + p as i128,
        };

        if new_pos < 0 || new_pos > i64::MAX as i128 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Seek out of range"));
        } else if new_pos > self.limit as i128 {
            new_pos = self.limit as i128;
        }

        self.inner.seek(SeekFrom::Current(new_pos as i64 - self.pos as i64))?;
        self.pos = new_pos as u64;
        Ok(self.pos)
    }

    #[inline]
    fn stream_position(&mut self) -> io::Result<u64> {
        Ok(self.pos)
    }
}
