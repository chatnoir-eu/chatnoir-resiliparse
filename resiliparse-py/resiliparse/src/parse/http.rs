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
use pyo3::types::{PyBytes, PySlice};
use resiliparse::parse::http;

/// Helper function for reading chunked HTTP payloads.
///
/// Each call reads the next chunk. A zero-sized or invalid chunk header returns
/// an empty byte string. Reader exceptions propagate to the caller.
///
/// The reader must implement ``readline()`` and ``read(size)``, returning bytes.
/// The hexadecimal size prefix is read from the header line, allowing chunk
/// extensions. A single ``read(size + 2)`` call consumes the payload and its
/// following delimiter; short reads return the available payload bytes. The
/// delimiter is discarded without validating CRLF, and HTTP trailer headers
/// are not parsed or skipped as a block.
///
/// :param reader: input reader
/// :type reader: typing.BinaryIO
/// :return: contents of the next chunk or empty string if EOF
/// :rtype: bytes
#[pyfunction]
pub fn read_http_chunk<'py>(reader: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyBytes>> {
    let header = reader.call_method0("readline")?;
    let header = header.cast::<PyBytes>()?;
    let size = http::chunk_size(header.as_bytes());
    let data = reader.call_method1("read", (size.wrapping_add(2),))?;
    // Preserve the Python reader's single-read and slicing behavior on short input.
    data.get_item(PySlice::new(reader.py(), 0, size as isize, 1))?
        .cast_into::<PyBytes>()
        .map_err(Into::into)
}

#[pyclass(module = "resiliparse.parse.http", name = "_HttpChunks")]
pub struct HttpChunksPy {
    reader: Py<PyAny>,
    done: bool,
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
        let chunk = read_http_chunk(self.reader.bind(py))?;
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
/// :param reader: input reader
/// :type reader: typing.BinaryIO
/// :return: iterator of chunks
/// :rtype: typing.Iterator[bytes]
#[pyfunction]
pub fn iterate_http_chunks(reader: Py<PyAny>) -> HttpChunksPy {
    HttpChunksPy { reader, done: false }
}

#[pymodule(name = "http", module = "resiliparse.parse")]
pub mod _http {
    #[pymodule_export]
    pub use super::{iterate_http_chunks, read_http_chunk};
}
