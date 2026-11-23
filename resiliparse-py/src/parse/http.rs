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

//! Python reader adapters for chunked HTTP payloads.

use pyo3::prelude::*;
use pyo3::types::PyBytes;
use resiliparse::parse::http;
use std::io::{self, BufRead, Read};

/// Python stream adapter that does not read beyond the current chunk.
struct PythonReader<'a, 'py> {
    reader: &'a Bound<'py, PyAny>,
    buffered: Option<u8>,
    error: Option<PyErr>,
}

impl Read for PythonReader<'_, '_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        if let Some(byte) = self.buffered.take() {
            output[0] = byte;
            return Ok(1);
        }
        let result = self.reader.call_method1("read", (output.len(),)).and_then(|value| {
            let bytes = value.cast::<PyBytes>()?;
            Ok(bytes.as_bytes().to_vec())
        });
        match result {
            Ok(bytes) if bytes.len() <= output.len() => {
                output[..bytes.len()].copy_from_slice(&bytes);
                Ok(bytes.len())
            }
            Ok(_) => Err(io::Error::new(io::ErrorKind::InvalidData, "Reader returned too many bytes")),
            Err(error) => {
                self.error = Some(error);
                Err(io::Error::other("Python reader failed"))
            }
        }
    }
}

impl BufRead for PythonReader<'_, '_> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.buffered.is_none() {
            let mut byte = [0];
            if self.read(&mut byte)? != 0 {
                self.buffered = Some(byte[0]);
            }
        }
        Ok(self.buffered.as_slice())
    }

    fn consume(&mut self, amount: usize) {
        if amount > 0 {
            self.buffered = None;
        }
    }
}

/// Read one HTTP chunk using the supplied decoder.
fn read_chunk<'py>(reader: &Bound<'py, PyAny>, decoder: &mut http::HttpChunkDecoder) -> PyResult<Bound<'py, PyBytes>> {
    let mut adapter = PythonReader {
        reader,
        buffered: None,
        error: None,
    };
    match decoder.read_chunk(&mut adapter) {
        Ok(data) => Ok(PyBytes::new(reader.py(), &data)),
        Err(error) => Err(adapter
            .error
            .unwrap_or_else(|| pyo3::exceptions::PyOSError::new_err(error.to_string()))),
    }
}

/// Read one chunk from an HTTP payload.
///
/// Chunk extensions and trailers are consumed. Each chunk is limited to 16 MiB.
/// Use :func:`iterate_http_chunks` to enforce a cumulative framing limit.
///
/// .. versionchanged:: 2.1.0
///    Validate chunk framing, consume trailers, and limit chunk sizes and framing overhead.
///
/// :param reader: Binary reader positioned at the next chunk
/// :return: Chunk bytes, or empty bytes after the final chunk and trailers
/// :raise OSError: If the stream or chunk framing is invalid
#[pyfunction]
pub fn read_http_chunk<'py>(reader: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyBytes>> {
    read_chunk(reader, &mut http::HttpChunkDecoder::default())
}

#[pyclass(module = "resiliparse.parse.http", name = "_HttpChunks")]
pub struct HttpChunksPy {
    reader: Py<PyAny>,
    done: bool,
    decoder: http::HttpChunkDecoder,
}

#[pymethods]
impl HttpChunksPy {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__<'py>(&mut self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyBytes>>> {
        if self.done {
            return Ok(None);
        }
        // Like a generator, remain exhausted after EOF or an exception.
        self.done = true;
        let chunk = read_chunk(self.reader.bind(py), &mut self.decoder)?;
        if chunk.as_bytes().is_empty() {
            return Ok(None);
        }
        self.done = false;
        Ok(Some(chunk))
    }
}

/// Lazily iterate over :func:`read_http_chunk` until an empty chunk is returned.
///
/// Construction does not read from the stream. Each step yields one nonempty
/// chunk, preserving its raw bytes. The first empty chunk terminates iteration
/// and is not yielded. Reader exceptions propagate once, after which the
/// iterator remains exhausted. Chunk framing and trailer handling follow
/// :func:`read_http_chunk`.
///
/// :param reader: Input reader
/// :return: Iterator of chunks
/// :raise AttributeError: If the reader lacks read()
/// :raise TypeError: If the reader methods do not return bytes
/// :raise OSError: If reading from the stream fails
#[pyfunction]
pub fn iterate_http_chunks(reader: Py<PyAny>) -> HttpChunksPy {
    HttpChunksPy {
        reader,
        done: false,
        decoder: http::HttpChunkDecoder::default(),
    }
}

#[pymodule(name = "http", module = "resiliparse.parse")]
pub mod _http {
    #[pymodule_export]
    pub use super::{iterate_http_chunks, read_http_chunk};
}
