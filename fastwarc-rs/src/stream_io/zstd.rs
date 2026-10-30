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

use crate::stream_io::bufread::{DEFAULT_BUFFER_SIZE, TrackingBufReader};
use crate::stream_io::traits::{ReadSeek, WarcRead, WarcWrite, Write as _Write};
use crate::stream_io::{impl_stream_from_path, impl_to_any_methods};
use std::any::Any;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use zstd::stream::{Decoder, Encoder};

// ===========================================================
// ZstdReader
// ===========================================================

/// Reader for Zstd-compressed streams.
pub struct ZstdReader<T: ReadSeek> {
    inner: Option<BufReader<Decoder<'static, TrackingBufReader<T>>>>,
    dict: Option<Vec<u8>>,
    stream_pos: u64,
    frame_start_pos: u64,
}

pub use ::zstd::dict::from_continuous as train_dictionary_from_continuous;
pub use ::zstd::dict::from_files as train_dictionary_from_files;
pub use ::zstd::dict::from_sample_iterator as train_dictionary_from_sample_iterator;
pub use ::zstd::dict::from_samples as train_dictionary_from_samples;

/// Options for constructing a new [`ZstdReader`].
///
/// # Options
///
/// * `capacity` - Sets the internal buffer size (default: 64 KiB).
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ZstdReaderOptions {
    pub capacity: usize,
}

impl Default for ZstdReaderOptions {
    fn default() -> Self {
        Self {
            capacity: DEFAULT_BUFFER_SIZE,
        }
    }
}

impl<T: ReadSeek> ZstdReader<T> {
    /// Create a new [`ZstdReader`].
    ///
    /// Allocates an internal buffer holding chunks of the uncompressed inner stream.
    ///
    /// The default buffer size is 64 KiB. For custom buffer sizes, use [`Self::with_capacity()`].
    pub fn new(inner: T) -> Self {
        Self::with_options(inner, ZstdReaderOptions::default())
    }

    /// Create a new [`ZstdReader`] with a given buffer capacity.
    ///
    /// Allocates an internal buffer holding chunks of the uncompressed inner stream.
    ///
    /// # Arguments
    ///
    /// * `inner` - Input (inner) stream to read from
    /// * `capacity` - Internal buffer size
    pub fn with_capacity(inner: T, capacity: usize) -> Self {
        Self::with_options(inner, ZstdReaderOptions { capacity })
    }

    /// Create a new [`ZstdReader`] with the supplied options.
    ///
    /// # Arguments
    ///
    /// * `inner` - Input (inner) stream to read from
    /// * `options` - Reader options
    pub fn with_options(inner: T, options: ZstdReaderOptions) -> Self {
        Self::with_dict_options_internal(inner, None, Some(options))
    }

    /// Create a new [`ZstdReader`] with the supplied decompression dictionary and options.
    ///
    /// The dictionary used during decompression must be the same the stream was originally
    /// compressed with. If the stream starts with a WARC dictionary frame, the dictionary
    /// is loaded from it automatically.
    ///
    /// # Arguments
    ///
    /// * `inner` - Input (inner) stream to read from
    /// * `dict` - Zstd dictionary
    /// * `options` - Reader options
    pub fn with_dictionary(inner: T, dict: Vec<u8>, options: Option<ZstdReaderOptions>) -> Self {
        Self::with_dict_options_internal(inner, Some(dict), options)
    }

    fn with_dict_options_internal(inner: T, dict: Option<Vec<u8>>, options: Option<ZstdReaderOptions>) -> Self {
        let options = options.unwrap_or_default();
        let mut reader = TrackingBufReader::with_capacity(options.capacity, inner);
        let frame_start_pos = reader.stream_position().unwrap_or(0);
        let decoder = if let Some(dict) = &dict {
            Decoder::with_dictionary(reader, dict)
        } else {
            Decoder::with_buffer(reader)
        }
        .expect("Failed to create Zstd decoder.")
        .single_frame();

        Self {
            inner: Some(BufReader::new(decoder)),
            dict,
            stream_pos: 0,
            frame_start_pos,
        }
    }

    /// Get the dictionary used for decompression.
    /// Returns `None` if no dictionary has been set, and the stream did not start with a dictionary frame.
    pub fn dictionary(&self) -> Option<&Vec<u8>> {
        self.dict.as_ref()
    }

    /// Unwrap this [`ZstdReader`], returning the underlying reader.
    ///
    /// Discards input buffers, so continued reads on the unwrapped stream may fail.
    pub fn into_inner(self) -> T {
        self.inner.unwrap().into_inner().finish().into_inner()
    }

    /// Internal method: Read a dictionary frame.
    /// Assumes the frame magic number has already been consumed.
    ///
    /// Dictionary frame format: [https://iipc.github.io/warc-specifications/specifications/warc-zstd/#dictionary-frame-format]
    fn read_dict_frame(&mut self) -> io::Result<Option<Vec<u8>>> {
        // Reach through Decoder to inner stream
        let inner = self.inner.as_mut().unwrap().get_mut().get_mut();
        let buf = inner.fill_buf()?;
        // Check dictionary frame magic number
        if buf.len() < 4 || buf[..4] != 0x184D2A5Du32.to_le_bytes() {
            return Ok(None);
        }

        // Read dictionary length
        inner.consume(4);
        let mut dict_len = [0u8; 4];
        inner
            .read_exact(&mut dict_len)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Failed to read dictionary size."))?;
        let dict_len = u32::from_le_bytes(dict_len) as usize;
        if dict_len > 1024 * 1024 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Dictionary size too large."));
        } else if dict_len < 4 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Dictionary truncated."));
        }

        // Read dictionary contents
        let mut dict = vec![0u8; dict_len];
        inner
            .read_exact(&mut dict)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Failed to read dictionary (wrong size)."))?;
        match dict[..4] {
            // Uncompressed
            [0x37, 0xA4, 0x30, 0xEC] => Ok(Some(dict)),
            // Compressed
            [0x28, 0xB5, 0x2F, 0xFD] => {
                let mut decoder = Decoder::new(io::Cursor::new(dict))?;
                let mut dict_dec = Vec::with_capacity(dict_len * 2);
                decoder.read_to_end(&mut dict_dec)?;
                Ok(Some(dict_dec))
            }
            _ => Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid dictionary.")),
        }
    }

    /// Internal: Reset the Decoder at a frame boundary and advance `self.member_pos`.
    ///
    /// # Arguments
    ///
    /// * `seek_pos` - Optional seek position on inner buffered reader before Decoder construction
    ///
    /// # Returns
    ///
    /// Member start position
    fn reset_decoder(&mut self, seek_pos: Option<SeekFrom>) -> io::Result<u64> {
        let capacity = self.inner.as_ref().unwrap().capacity();
        let mut inner = self.inner.take().unwrap().into_inner().finish();
        let mpos = inner.seek(seek_pos.unwrap_or(SeekFrom::Current(0)))?;
        let decoder = match self.dict.as_deref() {
            Some(dict) => Decoder::with_dictionary(inner, dict)?,
            None => Decoder::with_buffer(inner)?,
        }
        .single_frame();
        self.inner = Some(BufReader::with_capacity(capacity, decoder));
        self.frame_start_pos = mpos;
        Ok(mpos)
    }

    /// Internal: Load dictionary frame at stream start if one is available and
    /// no explicit dictionary was supplied by the user.
    fn maybe_load_dict_frame(&mut self) -> io::Result<()> {
        if self.stream_pos == 0
            && self.frame_start_pos == 0
            && self.dict.is_none()
            && let Some(dict) = self.read_dict_frame()?
        {
            self.dict = Some(dict);
            self.reset_decoder(None)?;
        }
        Ok(())
    }

    /// Internal: Ensure that more data can be read and reset Decoder at frame boundaries.
    fn ensure_frame_data(&mut self) -> io::Result<()> {
        self.maybe_load_dict_frame()?;

        loop {
            // Check if more input in buffer.
            let buf = self.inner.as_mut().unwrap().fill_buf()?;
            if !buf.is_empty() {
                return Ok(());
            }

            // Frame has ended, check for more input in underlying stream.
            let has_more_input = !self.inner.as_mut().unwrap().get_mut().get_mut().fill_buf()?.is_empty();
            if has_more_input {
                self.reset_decoder(None)?;
                continue;
            }
            return Ok(());
        }
    }
}

impl_stream_from_path!(ZstdReader, ZstdReaderOptions, open);

// noinspection DuplicatedCode
impl<T: ReadSeek> io::Read for ZstdReader<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.ensure_frame_data()?;
        let n = self.fill_buf()?.read(buf)?;
        self.consume(n);
        Ok(n)
    }
}

// noinspection DuplicatedCode
impl<T: ReadSeek> Seek for ZstdReader<T> {
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
        if pos == SeekFrom::Current(0) {
            return Ok(self.stream_pos);
        }
        super::_forward_seek(self, pos)
    }

    /// Returns the current seek position from the start of the decompressed output stream.
    #[inline]
    fn stream_position(&mut self) -> io::Result<u64> {
        Ok(self.stream_pos)
    }
}

impl<T: ReadSeek> WarcRead for ZstdReader<T> {
    impl_to_any_methods!();

    #[inline]
    fn inner_seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let new_pos = self.reset_decoder(Some(pos))?;
        self.stream_pos = 0;
        self.frame_start_pos = new_pos;
        Ok(new_pos)
    }

    #[inline]
    fn inner_stream_position(&mut self) -> io::Result<u64> {
        self.inner.as_mut().unwrap().get_mut().get_mut().stream_position()
    }

    #[inline]
    fn frame_start_position(&mut self) -> io::Result<Option<u64>> {
        Ok(Some(self.frame_start_pos))
    }

    #[inline]
    fn is_stream_decoder(&self) -> bool {
        true
    }
}

// noinspection DuplicatedCode
impl<T: ReadSeek> BufRead for ZstdReader<T> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.ensure_frame_data()?;
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
// ZstdWriter
// ===========================================================

/// Writer for Zstd-compressed streams.
pub struct ZstdWriter<T: _Write> {
    inner: Option<Encoder<'static, BufWriter<T>>>,
    options: ZstdWriterOptions,
    frame_started: bool,
    stream_started: bool,
    dict: Option<Vec<u8>>,
}

/// Options for constructing a new [`ZstdWriter`].
///
/// # Options
///
/// * `capacity` - Sets the internal buffer size.
/// * `level` - Compression level.
/// * `multithread_workers` - Number of threads to use for compression (0 to disable).
/// * `include_checksum` - Include `Content_Checksum` in each frame (required for WARCs).
/// * `include_content_size` - Include `Frame_Content_Size` in each frame (required for WARCs).
/// * `include_dictid` - Include dictionary ID when using one.
/// * `compress_dictionary_frame` - If given a custom dictionary (with [`ZstdWriter::with_dictionary()`]),
///   compress the dictionary frame contents
/// * `write_dictionary_frame` - Write a dictionary frame at the start of the stream if a custom
///   dictionary was given.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ZstdWriterOptions {
    pub capacity: usize,
    pub level: i32,
    pub multithread_workers: u32,
    pub include_checksum: bool,
    pub include_content_size: bool,
    pub include_dictid: bool,
    pub write_dictionary_frame: bool,
    pub compress_dictionary_frame: bool,
}

impl Default for ZstdWriterOptions {
    fn default() -> Self {
        Self {
            capacity: 8192,
            level: 3,
            multithread_workers: 4,
            include_checksum: true,
            include_content_size: true,
            include_dictid: true,
            write_dictionary_frame: true,
            compress_dictionary_frame: false,
        }
    }
}

impl<T: _Write> ZstdWriter<T> {
    /// Create a new [`ZstdWriter`].
    ///
    /// Maintains a small write buffer to temporarily store compressed data before flushing them
    /// to the underlying stream. The default buffer size is 8192 bytes. Use [`Self::with_capacity()`]
    /// for custom buffer sizes.
    ///
    /// # Arguments
    ///
    /// * `inner` - Inner stream to write compressed output to
    pub fn new(inner: T) -> Self {
        Self::with_options(inner, ZstdWriterOptions::default())
    }

    /// Create a new [`ZstdWriter`] a custom write buffer size.
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
            ZstdWriterOptions {
                capacity,
                ..ZstdWriterOptions::default()
            },
        )
    }

    /// Create a new [`ZstdWriter`] with the supplied options.
    ///
    /// # Arguments
    ///
    /// * `inner` - Inner stream to write compressed output to
    /// * `options` - Writer options
    pub fn with_options(inner: T, options: ZstdWriterOptions) -> Self {
        Self {
            inner: Some(
                Self::new_encoder_with_options(BufWriter::new(inner), None, options)
                    .expect("Failed to set Zstd encoder options."),
            ),
            options,
            frame_started: false,
            stream_started: false,
            dict: None,
        }
    }

    /// Create a new [`ZstdWriter`] with the supplied compression dictionary and options.
    ///
    /// Specifying a custom dictionary will write a dictionary frame before the first record frame,
    /// unless `options.write_dictionary_frame` is set to `false`. The dictionary frame contents can
    /// be compressed by setting `options.compress_dictionary_frame` to `true`.
    ///
    /// Use one of the `train_dictionary_*` functions in this module for training a custom dictionary.
    ///
    /// # Arguments
    ///
    /// * `inner` - Inner stream to write compressed output to
    /// * `dict` - Zstd dictionary
    /// * `options` - Writer options
    pub fn with_dictionary(inner: T, dict: Vec<u8>, options: Option<ZstdWriterOptions>) -> Self {
        let options = options.unwrap_or_default();
        Self {
            inner: Some(
                Self::new_encoder_with_options(BufWriter::new(inner), Some(&dict), options)
                    .expect("Failed to set Zstd encoder options."),
            ),
            options,
            frame_started: false,
            stream_started: false,
            dict: Some(dict),
        }
    }

    /// Unwrap this [`ZstdWriter`], returning the underlying writer.
    ///
    /// Writes out buffer contents before returning the inner reader.
    pub fn into_inner(mut self) -> io::Result<T> {
        let mut writer = self.inner.take().unwrap().finish()?.into_inner()?;
        writer.flush()?;
        Ok(writer)
    }

    /// Internal method: Create a new Zstd encoded with the supplied options and dictionary.
    fn new_encoder_with_options(
        inner: BufWriter<T>,
        dict: Option<&[u8]>,
        options: ZstdWriterOptions,
    ) -> io::Result<Encoder<'static, BufWriter<T>>> {
        let mut encoder = match dict {
            Some(d) => Encoder::with_dictionary(inner, options.level, &d)?,
            None => Encoder::new(inner, options.level)?,
        };
        encoder.multithread(options.multithread_workers)?;
        encoder.include_checksum(options.include_checksum)?;
        encoder.include_contentsize(options.include_content_size)?;
        encoder.include_dictid(options.include_dictid)?;
        Ok(encoder)
    }

    /// Internal method: Write out a dictionary frame.
    ///
    /// Dictionary frame format: [https://iipc.github.io/warc-specifications/specifications/warc-zstd/#dictionary-frame-format]
    fn write_dict_frame(&mut self) -> io::Result<()> {
        if !self.options.write_dictionary_frame {
            return Ok(());
        }
        let inner = self.inner.as_mut().unwrap().get_mut();
        let mut dict = self.dict.as_ref().unwrap().clone();

        if self.options.compress_dictionary_frame {
            let dict_enc = Vec::with_capacity(dict.len() / 2);
            let mut encoder = Encoder::new(dict_enc, self.options.level)?;
            encoder.include_contentsize(true)?;
            encoder.include_checksum(true)?;
            encoder.write_all(&dict)?;
            dict = encoder.finish()?;
        }

        // Write magic number, frame length, and (compressed) User_Data
        inner.write_all(&0x184D2A5Du32.to_le_bytes())?;
        inner.write_all(&(dict.len() as u32).to_le_bytes())?;
        inner.write_all(&dict)
    }
}

impl_stream_from_path!(ZstdWriter, ZstdWriterOptions, create);

impl<T: _Write> WarcWrite for ZstdWriter<T> {
    impl_to_any_methods!();

    fn finish(&mut self) -> io::Result<()> {
        if !self.frame_started {
            return Ok(());
        }
        let inner = self.inner.take().unwrap().finish()?;
        self.inner = Some(Self::new_encoder_with_options(inner, self.dict.as_deref(), self.options)?);
        self.frame_started = false;
        Ok(())
    }
}

impl<T: _Write> io::Write for ZstdWriter<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !self.stream_started && self.dict.is_some() {
            self.write_dict_frame()?;
            self.stream_started = true;
        }
        self.frame_started = true;
        self.inner.as_mut().unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.as_mut().unwrap().flush()?;
        Ok(())
    }
}

// noinspection DuplicatedCode
impl<T: _Write> Drop for ZstdWriter<T> {
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
#[path = "zstd_test.rs"]
mod zstd_test;
