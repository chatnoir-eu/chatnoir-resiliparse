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
use fastwarc::stream_io::traits::{WarcRead, WarcWrite};
use fastwarc::stream_io::zstd;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyList, PyString};
use std::io::{self, Read, Seek, Write};
use std::sync::Mutex;

/// Zstandard reader.
///
/// :param inner: raw input stream, file-like object, file name, or URL
/// :param buffer_size: input buffer size
/// :param fsspec_args: arguments for :mod:`fsspec`, or ``False`` to disable it
/// :param dictionary: optional decompression dictionary
#[pyclass(name = "ZstdReader", module = "fastwarc.stream_io", extends = WarcReaderPy, subclass)]
pub struct ZstdReaderPy {
    pub(crate) inner: Mutex<Option<Box<dyn WarcRead + Send>>>,
}

// noinspection DuplicatedCode
#[pymethods]
impl ZstdReaderPy {
    #[new]
    #[pyo3(signature = (inner, buffer_size=64 << 10, fsspec_args=None, dictionary=None))]
    pub fn __new__(
        py: Python<'_>,
        inner: Py<PyAny>,
        buffer_size: usize,
        fsspec_args: Option<Py<PyAny>>,
        dictionary: Option<Vec<u8>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = zstd::ZstdReaderOptions { capacity: buffer_size };
        let inner = wrap_reader_stream(
            py,
            inner,
            fsspec_args,
            |reader| -> io::Result<Box<dyn WarcRead + Send>> {
                match dictionary {
                    Some(d) => Ok(Box::new(zstd::ZstdReader::with_dictionary(reader, d, Some(options)))),
                    _ => Ok(Box::new(zstd::ZstdReader::with_options(reader, options))),
                }
            },
            |path| Ok(Box::new(zstd::ZstdReader::from_path_with_options(path, options)?)),
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

/// Zstandard writer.
///
/// :param inner: raw output stream, file-like object, file name, or URL
/// :param buffer_size: compression buffer size
/// :param fsspec_args: arguments for :mod:`fsspec`, or ``False`` to disable it
/// :param dictionary: optional compression dictionary
/// :param compress_dictionary_frame: include dictionary frames in compressed output
#[pyclass(name = "ZstdWriter", module = "fastwarc.stream_io", extends = WarcWriterPy, subclass)]
pub struct ZstdWriterPy {
    pub(crate) inner: Mutex<Option<Box<dyn WarcWrite + Send>>>,
}

// noinspection DuplicatedCode
#[pymethods]
impl ZstdWriterPy {
    #[new]
    #[pyo3(signature = (inner, buffer_size=8192, compression_level=3, fsspec_args=None, dictionary=None, compress_dictionary_frame=false))]
    pub fn __new__(
        py: Python<'_>,
        inner: Py<PyAny>,
        buffer_size: usize,
        compression_level: u8,
        fsspec_args: Option<Py<PyAny>>,
        dictionary: Option<Vec<u8>>,
        compress_dictionary_frame: bool,
    ) -> PyResult<PyClassInitializer<Self>> {
        let options = zstd::ZstdWriterOptions {
            capacity: buffer_size,
            level: compression_level as i32,
            compress_dictionary_frame,
            ..zstd::ZstdWriterOptions::default()
        };
        let inner = wrap_writer_stream(
            py,
            inner,
            fsspec_args,
            |reader| -> io::Result<Box<dyn WarcWrite + Send>> {
                match dictionary {
                    Some(d) => Ok(Box::new(zstd::ZstdWriter::with_dictionary(reader, d, Some(options)))),
                    _ => Ok(Box::new(zstd::ZstdWriter::with_options(reader, options))),
                }
            },
            |path| Ok(Box::new(zstd::ZstdWriter::from_path_with_options(path, options)?)),
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

/// Train a Zstandard dictionary from a stream of samples.
///
/// :param sample_data: continuous stream of sample bytes
/// :param sample_sizes: sample boundaries
/// :param max_size: maximum dictionary size
/// :returns: dictionary as bytes
#[pyfunction]
pub fn zstd_train_dictionary_from_continuous(
    sample_data: &[u8],
    sample_sizes: Vec<usize>,
    max_size: usize,
) -> PyResult<Vec<u8>> {
    Ok(zstd::train_dictionary_from_continuous(sample_data, sample_sizes.as_slice(), max_size)?)
}

/// Train a Zstandard dictionary from a set of files.
///
/// :param filenames: input file names
/// :param max_size: maximum dictionary size
/// :returns: dictionary as bytes
#[pyfunction]
pub fn zstd_train_dictionary_from_files(filenames: Bound<'_, PyList>, max_size: usize) -> PyResult<Vec<u8>> {
    let it = filenames
        .iter()
        .map(|f| Ok(std::path::PathBuf::from(f.cast::<PyString>()?.to_str()?)))
        .collect::<PyResult<Vec<_>>>()?;
    Ok(zstd::train_dictionary_from_files(it, max_size)?)
}

/// Train a Zstandard dictionary from a set of samples.
///
/// :param sample_data: list of byte samples
/// :param max_size: maximum dictionary size
/// :returns: dictionary as bytes
#[pyfunction]
pub fn zstd_train_dictionary_from_samples(samples: Bound<'_, PyList>, max_size: usize) -> PyResult<Vec<u8>> {
    let it = samples
        .iter()
        .map(|f| Ok(f.cast_into::<PyBytes>()?.as_bytes().to_vec()))
        .collect::<PyResult<Vec<_>>>()?;
    Ok(zstd::train_dictionary_from_samples(&it, max_size)?)
}
