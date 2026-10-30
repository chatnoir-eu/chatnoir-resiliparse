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
/// :param inner: raw input stream, file-like object, file name, or URL
/// :param buffer_size: input buffer size
/// :param fsspec_args: arguments for :mod:`fsspec`, or ``False`` to disable it
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

    pub fn close(&self) -> PyResult<()> {
        impl_reader_close!(self)
    }
}

/// HTTP chunked-transfer writer.
///
/// :param inner: raw output stream, file-like object, file name, or URL
/// :param min_chunk_size: minimum chunk size
/// :param fsspec_args: arguments for :mod:`fsspec`, or ``False`` to disable it
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
