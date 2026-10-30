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
use fastwarc::stream_io::gzip::{self, MAX_WBITS};
use fastwarc::stream_io::traits::{WarcRead, WarcWrite};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::io::{self, Read, Seek, Write};
use std::sync::Mutex;

/// Gzip reader.
///
/// :param inner: raw input stream, file-like object, file name, or URL
/// :param buffer_size: input buffer size
/// :param zlib: use zlib-wrapped deflate instead of gzip framing
/// :param fsspec_args: arguments for :mod:`fsspec`, or ``False`` to disable it
#[pyclass(name = "GzipReader", module = "fastwarc.stream_io", extends = WarcReaderPy, subclass)]
pub struct GzipReaderPy {
    pub(crate) inner: Mutex<Option<Box<dyn WarcRead + Send>>>,
}

// noinspection DuplicatedCode
#[pymethods]
impl GzipReaderPy {
    #[new]
    #[pyo3(signature = (inner, buffer_size=64 << 10, zlib=false, fsspec_args=None))]
    pub fn __new__(
        py: Python<'_>,
        inner: Py<PyAny>,
        buffer_size: usize,
        zlib: bool,
        fsspec_args: Option<Py<PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = gzip::GzipReaderOptions {
            capacity: buffer_size,
            window_bits: if zlib { MAX_WBITS } else { MAX_WBITS + 16 },
            expect_header: true,
        };
        let inner = wrap_reader_stream(
            py,
            inner,
            fsspec_args,
            |reader| -> io::Result<Box<dyn WarcRead + Send>> {
                Ok(Box::new(gzip::GzipReader::with_options(reader, options)))
            },
            |path| Ok(Box::new(gzip::GzipReader::from_path_with_options(path, options)?)),
        )?;
        Ok(PyClassInitializer::from(WarcReaderPy::__new__()).add_subclass(Self {
            inner: Mutex::new(Some(inner)),
        }))
    }

    #[pyo3(signature = (size=-1))]
    pub fn read<'py>(&self, py: Python<'py>, size: i128) -> PyResult<Bound<'py, PyBytes>> {
        impl_reader_read!(self, py, size)
    }

    #[pyo3(signature = (offset, whence=0))]
    pub fn seek(&self, offset: i128, whence: u8) -> PyResult<u64> {
        impl_reader_seek!(self, offset, whence, seek)
    }

    #[pyo3(signature = (offset, whence=0))]
    pub fn inner_seek(&self, offset: i128, whence: u8) -> PyResult<u64> {
        impl_reader_seek!(self, offset, whence, inner_seek)
    }

    pub fn tell(&self) -> PyResult<u64> {
        forward_fn_call!(self, stream_position)
    }

    pub fn inner_tell(&self) -> PyResult<u64> {
        forward_fn_call!(self, inner_stream_position)
    }

    #[allow(clippy::needless_question_mark)]
    pub fn frame_start_position(&mut self) -> io::Result<Option<u64>> {
        forward_fn_call!(self, frame_start_position)
    }

    pub fn close(&self) -> PyResult<()> {
        impl_reader_close!(self)
    }
}

/// Gzip writer.
///
/// :param inner: raw output stream, file-like object, file name, or URL
/// :param compression_level: compression level
/// :param buffer_size: compression buffer size
/// :param zlib: use zlib-wrapped deflate instead of gzip framing
/// :param fsspec_args: arguments for :mod:`fsspec`, or ``False`` to disable it
#[pyclass(name = "GzipWriter", module = "fastwarc.stream_io", extends = WarcWriterPy, subclass)]
pub struct GzipWriterPy {
    pub(crate) inner: Mutex<Option<Box<dyn WarcWrite + Send>>>,
}

// noinspection DuplicatedCode
#[pymethods]
impl GzipWriterPy {
    #[new]
    #[pyo3(signature = (inner, compression_level=9, buffer_size=8192, zlib=false, fsspec_args=None,))]
    pub fn __new__(
        py: Python<'_>,
        inner: Py<PyAny>,
        compression_level: i32,
        buffer_size: usize,
        zlib: bool,
        fsspec_args: Option<Py<PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = gzip::GzipWriterOptions {
            capacity: buffer_size,
            window_bits: if zlib { MAX_WBITS } else { MAX_WBITS + 16 },
            expect_header: true,
            compression_level,
        };
        let inner = wrap_writer_stream(
            py,
            inner,
            fsspec_args,
            |writer| -> io::Result<Box<dyn WarcWrite + Send>> {
                Ok(Box::new(gzip::GzipWriter::with_options(writer, options)))
            },
            |path| Ok(Box::new(gzip::GzipWriter::from_path_with_options(path, options)?)),
        )?;

        Ok(PyClassInitializer::from(WarcWriterPy::__new__()).add_subclass(Self {
            inner: Mutex::new(Some(inner)),
        }))
    }

    pub fn write(&self, data: &[u8]) -> PyResult<usize> {
        impl_writer_write!(self, data)
    }

    pub fn flush(&self) -> PyResult<()> {
        forward_fn_call!(self, flush)
    }

    pub fn finish(&self) -> PyResult<()> {
        forward_fn_call!(self, finish)
    }

    pub fn close(&self) -> PyResult<()> {
        impl_writer_close!(self)
    }
}
