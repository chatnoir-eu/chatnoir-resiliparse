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

use super::impl_macros::*;
use crate::stream_io::{WarcReaderPy, WarcWriterPy, wrap_reader_stream, wrap_writer_stream};
use fastwarc::stream_io::brotli::{self, BrotliWriterOptions};
use fastwarc::stream_io::traits::{WarcRead, WarcWrite};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::io::{self, Read, Seek, Write};
use std::sync::Mutex;

/// Brotli reader.
///
/// Exceptions raised by a supplied stream or filesystem backend propagate unchanged.
///
/// :param inner: Raw input stream, file-like object, file name, or URL
/// :param buffer_size: Input buffer size
/// :param fsspec_args: Arguments for :mod:`fsspec`, or ``False`` to disable it
/// :raise OSError: If the file or stream cannot be opened or the codec cannot be initialized
/// :raise TypeError: If fsspec_args is neither a mapping nor False
#[pyclass(name = "BrotliReader", module = "fastwarc.stream_io", extends = WarcReaderPy, subclass)]
pub struct BrotliReaderPy {
    pub(crate) inner: Mutex<Option<Box<dyn WarcRead + Send>>>,
}

// noinspection DuplicatedCode
#[pymethods]
impl BrotliReaderPy {
    #[new]
    #[pyo3(signature = (inner, buffer_size=64 << 10, fsspec_args=None))]
    pub fn __new__(
        py: Python<'_>,
        inner: Py<PyAny>,
        buffer_size: usize,
        fsspec_args: Option<Py<PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = brotli::BrotliReaderOptions { capacity: buffer_size };
        let inner = wrap_reader_stream(
            py,
            inner,
            fsspec_args,
            |reader| -> io::Result<Box<dyn WarcRead + Send>> {
                Ok(Box::new(brotli::BrotliReader::with_options(reader, options)))
            },
            |path| Ok(Box::new(brotli::BrotliReader::from_path_with_options(path, options)?)),
        )?;
        Ok(PyClassInitializer::from(WarcReaderPy::__new__()).add_subclass(Self {
            inner: Mutex::new(Some(inner)),
        }))
    }

    /// Read bytes from the decoded stream.
    ///
    /// :param size: Maximum bytes to read, or -1 for all remaining bytes
    /// :return: Decoded bytes
    /// :raise ValueError: If the stream is closed
    /// :raise OSError: If the stream operation or codec processing fails
    /// :raise TypeError: If a supplied Python reader returns neither bytes nor bytearray
    #[pyo3(signature = (size=-1))]
    pub fn read<'py>(&self, py: Python<'py>, size: i128) -> PyResult<Bound<'py, PyBytes>> {
        impl_reader_read!(self, py, size)
    }

    /// Seek within the decoded stream.
    ///
    /// :param offset: Seek offset
    /// :param whence: Seek mode: 0 for start, 1 for current position, or 2 for end
    /// :return: New decoded stream offset
    /// :raise ValueError: If the stream is closed, whence is invalid, or offset is outside the supported range
    /// :raise OSError: If the stream operation or codec processing fails
    #[pyo3(signature = (offset, whence=0))]
    pub fn seek(&self, offset: i128, whence: u8) -> PyResult<u64> {
        impl_reader_seek!(self, offset, whence, seek)
    }

    /// Seek within the underlying stream.
    ///
    /// :param offset: Seek offset
    /// :param whence: Seek mode: 0 for start, 1 for current position, or 2 for end
    /// :return: New underlying stream offset
    /// :raise ValueError: If the stream is closed, whence is invalid, or offset is outside the supported range
    /// :raise OSError: If the stream operation or codec processing fails
    #[pyo3(signature = (offset, whence=0))]
    pub fn inner_seek(&self, offset: i128, whence: u8) -> PyResult<u64> {
        impl_reader_seek!(self, offset, whence, inner_seek)
    }

    /// Return the decoded stream offset.
    ///
    /// :return: Current decoded stream offset
    /// :raise ValueError: If the stream is closed
    /// :raise OSError: If the stream operation or codec processing fails
    pub fn tell(&self) -> PyResult<u64> {
        forward_fn_call!(self, stream_position)
    }

    /// Return the underlying stream offset.
    ///
    /// :return: Current underlying stream offset
    /// :raise ValueError: If the stream is closed
    /// :raise OSError: If the stream operation or codec processing fails
    pub fn inner_tell(&self) -> PyResult<u64> {
        forward_fn_call!(self, inner_stream_position)
    }

    /// Return the current compression-frame offset, if supported.
    ///
    /// :return: Frame start offset, or None if the codec does not expose one
    /// :raise ValueError: If the stream is closed
    /// :raise OSError: If the stream operation or codec processing fails
    #[allow(clippy::needless_question_mark)]
    pub fn frame_start_position(&mut self) -> io::Result<Option<u64>> {
        forward_fn_call!(self, frame_start_position)
    }

    pub fn close(&self) -> PyResult<()> {
        impl_reader_close!(self)
    }
}

/// Brotli writer.
///
/// Exceptions raised by a supplied stream or filesystem backend propagate unchanged.
///
/// :param inner: Raw output stream, file-like object, file name, or URL
/// :param buffer_size: Compression buffer size
/// :param fsspec_args: Arguments for :mod:`fsspec`, or ``False`` to disable it
/// :raise OSError: If the file or stream cannot be opened or the codec cannot be initialized
/// :raise TypeError: If fsspec_args is neither a mapping nor False
#[pyclass(name = "BrotliWriter", module = "fastwarc.stream_io", extends = WarcWriterPy, subclass)]
pub struct BrotliWriterPy {
    pub(crate) inner: Mutex<Option<Box<dyn WarcWrite + Send>>>,
}

// noinspection DuplicatedCode
#[pymethods]
impl BrotliWriterPy {
    #[new]
    #[pyo3(signature = (inner, buffer_size=8192, fsspec_args=None,))]
    pub fn __new__(
        py: Python<'_>,
        inner: Py<PyAny>,
        buffer_size: usize,
        fsspec_args: Option<Py<PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = brotli::BrotliWriterOptions {
            capacity: buffer_size,
            ..BrotliWriterOptions::default()
        };
        let inner = wrap_writer_stream(
            py,
            inner,
            fsspec_args,
            |reader| -> io::Result<Box<dyn WarcWrite + Send>> {
                Ok(Box::new(brotli::BrotliWriter::with_options(reader, options)))
            },
            |path| Ok(Box::new(brotli::BrotliWriter::from_path_with_options(path, options)?)),
        )?;
        Ok(PyClassInitializer::from(WarcWriterPy::__new__()).add_subclass(Self {
            inner: Mutex::new(Some(inner)),
        }))
    }

    /// Write bytes to the stream.
    ///
    /// :param data: Bytes to write
    /// :return: Number of bytes written
    /// :raise ValueError: If the stream is closed
    /// :raise OSError: If the stream operation or codec processing fails
    pub fn write(&self, data: &[u8]) -> PyResult<usize> {
        impl_writer_write!(self, data)
    }

    /// Flush buffered output.
    ///
    /// :raise ValueError: If the stream is closed
    /// :raise OSError: If the stream operation or codec processing fails
    pub fn flush(&self) -> PyResult<()> {
        forward_fn_call!(self, flush)
    }

    /// Finish encoding the stream.
    ///
    /// :raise ValueError: If the stream is closed
    /// :raise OSError: If the stream operation or codec processing fails
    pub fn finish(&self) -> PyResult<()> {
        forward_fn_call!(self, finish)
    }

    /// Close the writer after flushing buffered output.
    ///
    /// :raise OSError: If flushing the underlying stream fails
    pub fn close(&self) -> PyResult<()> {
        impl_writer_close!(self)
    }
}
