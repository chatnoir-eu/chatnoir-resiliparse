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
/// Exceptions raised by a supplied stream or filesystem backend propagate unchanged.
///
/// :param inner: Raw input stream, file-like object, file name, or URL
/// :param buffer_size: Input buffer size
/// :param fsspec_args: Arguments for :mod:`fsspec`, or ``False`` to disable it
/// :param dictionary: Optional decompression dictionary
/// :raise OSError: If the file or stream cannot be opened or the codec cannot be initialized
/// :raise TypeError: If fsspec_args is neither a mapping nor False
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

/// Zstandard writer.
///
/// Exceptions raised by a supplied stream or filesystem backend propagate unchanged.
///
/// :param inner: Raw output stream, file-like object, file name, or URL
/// :param buffer_size: Compression buffer size
/// :param compression_level: Zstandard compression level, default 3
/// :param fsspec_args: Arguments for :mod:`fsspec`, or ``False`` to disable it
/// :param dictionary: Optional compression dictionary
/// :param compress_dictionary_frame: Include dictionary frames in compressed output
/// :raise OSError: If the file or stream cannot be opened or the codec cannot be initialized
/// :raise TypeError: If fsspec_args is neither a mapping nor False
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

/// Train a Zstandard dictionary from a stream of samples.
///
/// :param sample_data: Continuous stream of sample bytes
/// :param sample_sizes: Sample boundaries
/// :param max_size: Maximum dictionary size
/// :return: Dictionary as bytes
/// :raise OSError: If dictionary training fails
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
/// :param filenames: Input file names
/// :param max_size: Maximum dictionary size
/// :return: Dictionary as bytes
/// :raise OSError: If dictionary training or reading a sample file fails
/// :raise TypeError: If a filename is not a string
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
/// :param samples: List of byte samples
/// :param max_size: Maximum dictionary size
/// :return: Dictionary as bytes
/// :raise OSError: If dictionary training fails
/// :raise TypeError: If a sample is not bytes
#[pyfunction]
pub fn zstd_train_dictionary_from_samples(samples: Bound<'_, PyList>, max_size: usize) -> PyResult<Vec<u8>> {
    let it = samples
        .iter()
        .map(|f| Ok(f.cast_into::<PyBytes>()?.as_bytes().to_vec()))
        .collect::<PyResult<Vec<_>>>()?;
    Ok(zstd::train_dictionary_from_samples(&it, max_size)?)
}
