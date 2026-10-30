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
use fastwarc::stream_io::chunked;
use fastwarc::stream_io::traits::{WarcRead, WarcWrite};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::io::{self, Read, Seek, Write};
use std::sync::Mutex;

/// HTTP chunked-transfer reader.
///
/// Exceptions raised by a supplied stream or filesystem backend propagate unchanged.
///
/// :param inner: Raw input stream, file-like object, file name, or URL
/// :param buffer_size: Input buffer size
/// :param fsspec_args: Arguments for :mod:`fsspec`, or ``False`` to disable it
/// :raise OSError: If the file or stream cannot be opened or the codec cannot be initialized
/// :raise TypeError: If fsspec_args is neither a mapping nor False
#[pyclass(name = "ChunkedReader", module = "fastwarc.stream_io", extends = WarcReaderPy, subclass)]
pub struct ChunkedReaderPy {
    pub(crate) inner: Mutex<Option<Box<dyn WarcRead + Send>>>,
}

// noinspection DuplicatedCode
#[pymethods]
impl ChunkedReaderPy {
    #[new]
    #[pyo3(signature = (inner, buffer_size=4096, fsspec_args=None))]
    pub fn __new__(
        py: Python<'_>,
        inner: Py<PyAny>,
        buffer_size: usize,
        fsspec_args: Option<Py<PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = chunked::ChunkedReaderOptions { capacity: buffer_size };
        let inner = wrap_reader_stream(
            py,
            inner,
            fsspec_args,
            |reader| -> io::Result<Box<dyn WarcRead + Send>> {
                Ok(Box::new(chunked::ChunkedReader::with_options(reader, options)))
            },
            |path| Ok(Box::new(chunked::ChunkedReader::from_path_with_options(path, options)?)),
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
    /// :raise ValueError: If the stream is closed
    /// :raise ValueError: If whence is invalid or offset is outside the supported range
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
    /// :raise ValueError: If the stream is closed
    /// :raise ValueError: If whence is invalid or offset is outside the supported range
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

    pub fn close(&self) -> PyResult<()> {
        impl_reader_close!(self)
    }
}

/// HTTP chunked-transfer writer.
///
/// Exceptions raised by a supplied stream or filesystem backend propagate unchanged.
///
/// :param inner: Raw output stream, file-like object, file name, or URL
/// :param min_chunk_size: Minimum chunk size
/// :param fsspec_args: Arguments for :mod:`fsspec`, or ``False`` to disable it
/// :raise OSError: If the file or stream cannot be opened or the codec cannot be initialized
/// :raise TypeError: If fsspec_args is neither a mapping nor False
#[pyclass(name = "ChunkedWriter", module = "fastwarc.stream_io", extends = WarcWriterPy, subclass)]
pub struct ChunkedWriterPy {
    pub(crate) inner: Mutex<Option<Box<dyn WarcWrite + Send>>>,
}

// noinspection DuplicatedCode
#[pymethods]
impl ChunkedWriterPy {
    #[new]
    #[pyo3(signature = (inner, min_chunk_size=1024, fsspec_args=None,))]
    pub fn __new__(
        py: Python<'_>,
        inner: Py<PyAny>,
        min_chunk_size: usize,
        fsspec_args: Option<Py<PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = chunked::ChunkedWriterOptions { min_chunk_size };
        let inner = wrap_writer_stream(
            py,
            inner,
            fsspec_args,
            |reader| -> io::Result<Box<dyn WarcWrite + Send>> {
                Ok(Box::new(chunked::ChunkedWriter::with_options(reader, options)))
            },
            |path| Ok(Box::new(chunked::ChunkedWriter::from_path_with_options(path, options)?)),
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
