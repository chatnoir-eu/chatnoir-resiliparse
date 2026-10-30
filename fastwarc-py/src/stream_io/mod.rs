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

use self::gzip::{GzipReaderPy, GzipWriterPy};
use self::lz4::{Lz4ReaderPy, Lz4WriterPy};
use self::zstd::{ZstdReaderPy, ZstdWriterPy};
use fastwarc::stream_io::traits::{WarcRead, WarcWrite};
use pyo3::exceptions::{PyModuleNotFoundError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyByteArray, PyBytes, PyDict, PyString};
use std::any::Any;
use std::io::{self, BufRead, Read, Seek, SeekFrom, Write};

// ===========================================================
// Submodules
// ===========================================================

pub mod brotli;
pub mod chunked;
pub mod gzip;
pub mod lz4;
pub mod zstd;

// ===========================================================
// Exported stream parent classes
// ===========================================================

/// Abstract base class for reader objects in :mod:`fastwarc.stream_io`.
#[pyclass(name = "WarcReader", module = "fastwarc.stream_io", subclass)]
pub struct WarcReaderPy {}

#[pymethods]
impl WarcReaderPy {
    #[new]
    pub fn __new__() -> Self {
        Self {}
    }

    /// Read bytes from the stream.
    ///
    /// :param size: maximum number of bytes to read, or ``-1`` for all remaining bytes
    #[pyo3(signature = (size=-1))]
    pub fn read<'py>(&self, py: Python<'py>, size: i128) -> PyResult<Py<PyAny>> {
        let _ = size;
        Ok(py.NotImplemented())
    }

    /// Seek within the decoded stream.
    ///
    /// :param offset: seek offset
    /// :param whence: seek mode (``0`` = start, ``1`` = current, ``2`` = end)
    #[pyo3(signature = (offset, whence=0))]
    pub fn seek(&self, py: Python<'_>, offset: i128, whence: u8) -> PyResult<Py<PyAny>> {
        let _ = (offset, whence);
        Ok(py.NotImplemented())
    }

    /// Return the current decoded stream offset.
    ///
    pub fn tell(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(py.NotImplemented())
    }

    /// Close the stream.
    pub fn close(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(py.None())
    }

    pub fn __enter__(slf: PyRef<'_, Self>) -> PyResult<PyRef<'_, Self>> {
        Ok(slf)
    }

    // noinspection DuplicatedCode
    pub fn __exit__(
        slf: Bound<'_, Self>,
        _exc_type: Option<Py<PyAny>>,
        _exc_val: Option<Py<PyAny>>,
        _exc_tb: Option<Py<PyAny>>,
    ) -> PyResult<()> {
        slf.call_method0("close")?;
        Ok(())
    }

    /// Seek within the wrapped inner stream.
    ///
    /// :param offset: seek offset
    /// :param whence: seek mode (``0`` = start, ``1`` = current, ``2`` = end)
    #[pyo3(signature = (offset, whence=0))]
    pub fn inner_seek(&self, py: Python<'_>, offset: i128, whence: u8) -> PyResult<Py<PyAny>> {
        let _ = (offset, whence);
        Ok(py.NotImplemented())
    }

    /// Return the current inner stream offset.
    ///
    pub fn inner_tell(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(py.NotImplemented())
    }

    /// Return the start offset of the current compression frame or member, if supported.
    ///
    pub fn frame_start_position(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(py.None())
    }
}

/// Abstract base class for writer objects in :mod:`fastwarc.stream_io`.
#[pyclass(name = "WarcWriter", module = "fastwarc.stream_io", subclass)]
pub struct WarcWriterPy {}

#[pymethods]
impl WarcWriterPy {
    #[new]
    pub fn __new__() -> Self {
        Self {}
    }

    /// Write bytes to the stream.
    ///
    /// :param data: bytes to write
    /// :return: number of bytes written
    pub fn write(&self, py: Python<'_>, data: Py<PyBytes>) -> PyResult<Py<PyAny>> {
        let _ = data;
        Ok(py.NotImplemented())
    }

    /// Flush buffered output.
    pub fn flush(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(py.None())
    }

    /// Close the stream.
    pub fn close(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(py.None())
    }

    pub fn __enter__(slf: PyRef<'_, Self>) -> PyResult<PyRef<'_, Self>> {
        Ok(slf)
    }

    // noinspection DuplicatedCode
    pub fn __exit__(
        slf: Bound<'_, Self>,
        _exc_type: Option<Py<PyAny>>,
        _exc_val: Option<Py<PyAny>>,
        _exc_tb: Option<Py<PyAny>>,
    ) -> PyResult<()> {
        slf.call_method0("close")?;
        Ok(())
    }

    /// Finish the current compression member or frame, if supported.
    pub fn finish(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(py.None())
    }
}

// ===========================================================
// Adapters for Python file-like objects
// ===========================================================

/// Python object stream adapter trait.
pub(crate) trait PyStreamAdapter: Sized {
    fn new(inner: Py<PyAny>) -> PyResult<Self>;
}

enum PyReaderType {
    GzipReader(Py<GzipReaderPy>),
    ZstdReader(Py<ZstdReaderPy>),
    Lz4Reader(Py<Lz4ReaderPy>),
    Other(Py<PyAny>),
}

impl Clone for PyReaderType {
    fn clone(&self) -> Self {
        Python::attach(|py| match self {
            Self::GzipReader(inner) => Self::GzipReader(inner.clone_ref(py)),
            Self::ZstdReader(inner) => Self::ZstdReader(inner.clone_ref(py)),
            Self::Lz4Reader(inner) => Self::Lz4Reader(inner.clone_ref(py)),
            Self::Other(inner) => Self::Other(inner.clone_ref(py)),
        })
    }
}

/// Reader adapter for Python file-like objects.
/// Wrapped types from `stream_io` are optimized with static dispatch. Other
/// `BytesIO` types will use dynamic Python `read()` / `seek()` calls.
pub(crate) struct PyReaderAdapter {
    inner: PyReaderType,
    pos: Option<u64>,
    buf: Vec<u8>,
    buf_pos: usize,
    buf_len: usize,
}

macro_rules! native_inner_call_path {
    ($self:ident, $DynTrait:ident, $inner:ident, $py:ident, $closure:expr) => {{
        let inner = &$inner.bind($py).borrow().inner;
        let mut guard = inner.lock().unwrap();
        let inner: &mut dyn $DynTrait = guard.as_mut().unwrap().as_mut();
        $closure($self, inner)
    }};
}

impl PyReaderAdapter {
    pub fn new(inner: Py<PyAny>) -> PyResult<Self> {
        Self::with_capacity(inner, 64 << 10)
    }

    pub fn with_capacity(inner: Py<PyAny>, capacity: usize) -> PyResult<Self> {
        let inner = Python::attach(|py| {
            let bound = inner.bind(py);
            if bound.is_exact_instance_of::<GzipReaderPy>() {
                PyReaderType::GzipReader(bound.cast::<GzipReaderPy>().unwrap().clone().unbind())
            } else if bound.is_exact_instance_of::<ZstdReaderPy>() {
                PyReaderType::ZstdReader(bound.cast::<ZstdReaderPy>().unwrap().clone().unbind())
            } else if bound.is_exact_instance_of::<Lz4ReaderPy>() {
                PyReaderType::Lz4Reader(bound.cast::<Lz4ReaderPy>().unwrap().clone().unbind())
            } else {
                PyReaderType::Other(inner)
            }
        });
        Ok(Self {
            inner,
            pos: None,
            buf: vec![0; capacity],
            buf_pos: 0,
            buf_len: 0,
        })
    }

    fn native_call_or_forward<N, F, R>(&mut self, native_call_closure: N, forward_call_closure: F) -> io::Result<R>
    where
        N: FnOnce(&mut Self, &mut dyn WarcRead) -> io::Result<R>,
        F: FnOnce(&mut Self, &Bound<'_, PyAny>) -> PyResult<R>,
    {
        Python::attach(|py| -> io::Result<R> {
            match self.inner.clone() {
                PyReaderType::GzipReader(inner) => {
                    native_inner_call_path!(self, WarcRead, inner, py, native_call_closure)
                }
                PyReaderType::ZstdReader(inner) => {
                    native_inner_call_path!(self, WarcRead, inner, py, native_call_closure)
                }
                PyReaderType::Lz4Reader(inner) => {
                    native_inner_call_path!(self, WarcRead, inner, py, native_call_closure)
                }
                PyReaderType::Other(inner) => Ok(forward_call_closure(self, inner.bind(py))?),
            }
        })
    }
}

impl PyStreamAdapter for PyReaderAdapter {
    fn new(inner: Py<PyAny>) -> PyResult<Self> {
        PyReaderAdapter::new(inner)
    }
}

impl WarcRead for PyReaderAdapter {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn inner_seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.native_call_or_forward(
            |slf, inner| {
                let new_pos = inner.inner_seek(pos)?;
                slf.pos = Some(inner.stream_position()?);
                slf.buf_pos = 0;
                slf.buf_len = 0;
                Ok(new_pos)
            },
            |slf, _| Ok(slf.seek(pos)?),
        )
    }

    fn inner_stream_position(&mut self) -> io::Result<u64> {
        self.native_call_or_forward(
            |_, inner| inner.inner_stream_position(),
            |slf, _| {
                // Calling inner.tell() would report wrong offsets due to our buffering
                Ok(slf.stream_position()?)
            },
        )
    }
}

impl Read for PyReaderAdapter {
    // noinspection DuplicatedCode
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.fill_buf()?.read(buf)?;
        self.consume(n);
        Ok(n)
    }
}

impl Seek for PyReaderAdapter {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        if self.pos.is_none() {
            // Initialize stream position.
            self.stream_position()?;
        }

        let self_pos = self.pos.unwrap();
        if pos == SeekFrom::Current(0) {
            return Ok(self_pos);
        }
        let new_pos = match pos {
            SeekFrom::Start(p) => SeekFrom::Start(p),
            SeekFrom::Current(p) => SeekFrom::Start(
                u64::try_from(p as i128 + self_pos as i128)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Seek out of range"))?,
            ),
            SeekFrom::End(p) => SeekFrom::End(p),
        };
        if let SeekFrom::Start(p) = new_pos {
            let buf_start = self_pos.saturating_sub(self.buf_pos as u64);
            let buf_end = buf_start + self.buf_len as u64;
            if p >= buf_start && p < buf_end {
                self.buf_pos = (p - buf_start) as usize;
                self.pos = Some(p);
                return Ok(p);
            }
        }

        let buffered = (self.buf_len - self.buf_pos) as i64;
        let pos = match pos {
            SeekFrom::Start(offset) => SeekFrom::Start(offset),
            SeekFrom::Current(offset) => SeekFrom::Current(offset - buffered),
            SeekFrom::End(offset) => SeekFrom::End(offset),
        };
        let new_pos = self.native_call_or_forward(
            |_, inner| inner.seek(pos),
            |_, inner| {
                let result = match pos {
                    SeekFrom::Start(offset) => inner.call_method1("seek", (offset, 0)),
                    SeekFrom::Current(offset) => inner.call_method1("seek", (offset, 1)),
                    SeekFrom::End(offset) => inner.call_method1("seek", (offset, 2)),
                }?;
                result.extract::<u64>()
            },
        )?;
        self.pos = Some(new_pos);
        self.buf_pos = 0;
        self.buf_len = 0;
        Ok(new_pos)
    }

    fn stream_position(&mut self) -> io::Result<u64> {
        if self.pos.is_none() {
            self.pos = Some(self.native_call_or_forward(
                |_, inner| inner.stream_position(),
                |_, inner| inner.call_method0("tell").and_then(|result| result.extract::<u64>()),
            )?);
        }
        Ok(self.pos.unwrap())
    }
}

impl BufRead for PyReaderAdapter {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.buf_pos < self.buf_len {
            return Ok(&self.buf[self.buf_pos..self.buf_len]);
        }
        if self.pos.is_none() {
            // Initialize stream position.
            self.stream_position()?;
        }

        let n = self.native_call_or_forward(
            |slf, inner| inner.read(&mut slf.buf),
            |slf, inner| {
                let result = inner.call_method1("read", (slf.buf.len(),))?;
                if let Ok(bytes) = result.cast::<PyBytes>() {
                    let data = bytes.as_bytes();
                    slf.buf[..data.len()].copy_from_slice(data);
                    Ok(data.len())
                } else if let Ok(bytearray) = result.cast::<PyByteArray>() {
                    let data = bytearray.to_vec();
                    slf.buf[..data.len()].copy_from_slice(&data);
                    Ok(data.len())
                } else {
                    Err(PyTypeError::new_err("read() must return bytes"))
                }
            },
        )?;

        self.buf_len = n;
        self.buf_pos = 0;
        Ok(&self.buf[..self.buf_len])
    }

    fn consume(&mut self, amount: usize) {
        self.pos = Some(self.pos.expect("consume() called without fill_buf()") + amount as u64);
        self.buf_pos += amount;
    }
}

enum PyWriterType {
    GzipWriter(Py<GzipWriterPy>),
    ZstdWriter(Py<ZstdWriterPy>),
    Lz4Writer(Py<Lz4WriterPy>),
    Other(Py<PyAny>),
}

impl Clone for PyWriterType {
    fn clone(&self) -> Self {
        Python::attach(|py| match self {
            Self::GzipWriter(inner) => Self::GzipWriter(inner.clone_ref(py)),
            Self::ZstdWriter(inner) => Self::ZstdWriter(inner.clone_ref(py)),
            Self::Lz4Writer(inner) => Self::Lz4Writer(inner.clone_ref(py)),
            Self::Other(inner) => Self::Other(inner.clone_ref(py)),
        })
    }
}

/// Writer adapter for Python file-like objects.
/// Wrapped types from `stream_io` are optimized with static dispatch. Other
/// `BytesIO` types will use dynamic Python `write()` / `flush()` calls.
pub(crate) struct PyWriterAdapter {
    inner: PyWriterType,
}

impl PyStreamAdapter for PyWriterAdapter {
    fn new(inner: Py<PyAny>) -> PyResult<Self> {
        PyWriterAdapter::new(inner)
    }
}

impl PyWriterAdapter {
    pub fn new(inner: Py<PyAny>) -> PyResult<Self> {
        let inner = Python::attach(|py| {
            let bound = inner.bind(py);
            if bound.is_exact_instance_of::<GzipWriterPy>() {
                PyWriterType::GzipWriter(bound.cast::<GzipWriterPy>().unwrap().clone().unbind())
            } else if bound.is_exact_instance_of::<ZstdWriterPy>() {
                PyWriterType::ZstdWriter(bound.cast::<ZstdWriterPy>().unwrap().clone().unbind())
            } else if bound.is_exact_instance_of::<Lz4WriterPy>() {
                PyWriterType::Lz4Writer(bound.cast::<Lz4WriterPy>().unwrap().clone().unbind())
            } else {
                PyWriterType::Other(inner)
            }
        });
        Ok(Self { inner })
    }

    fn native_call_or_forward<N, F, R>(&mut self, native_call_closure: N, forward_call_closure: F) -> io::Result<R>
    where
        N: FnOnce(&mut Self, &mut dyn WarcWrite) -> io::Result<R>,
        F: FnOnce(&mut Self, &Bound<'_, PyAny>) -> PyResult<R>,
    {
        Python::attach(|py| -> io::Result<R> {
            match self.inner.clone() {
                PyWriterType::GzipWriter(inner) => {
                    native_inner_call_path!(self, WarcWrite, inner, py, native_call_closure)
                }
                PyWriterType::ZstdWriter(inner) => {
                    native_inner_call_path!(self, WarcWrite, inner, py, native_call_closure)
                }
                PyWriterType::Lz4Writer(inner) => {
                    native_inner_call_path!(self, WarcWrite, inner, py, native_call_closure)
                }
                PyWriterType::Other(inner) => Ok(forward_call_closure(self, inner.bind(py))?),
            }
        })
    }
}

impl WarcWrite for PyWriterAdapter {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

impl Write for PyWriterAdapter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.native_call_or_forward(
            |_, inner| inner.write(buf),
            |_, inner| {
                inner
                    .call_method1("write", (buf,))
                    .and_then(|result| result.extract::<usize>())
            },
        )
    }

    fn flush(&mut self) -> io::Result<()> {
        self.native_call_or_forward(|_, inner| inner.flush(), |_, inner| inner.call_method0("flush").map(|_| ()))
    }
}

// ===========================================================
// Helper functions
// ===========================================================

/// Convert a path-like object into a String.
pub(crate) fn path_like_to_string(obj: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(s) = obj.cast::<PyString>() {
        return Ok(s.to_str()?.to_owned());
    }
    let os_fspath = obj.py().import("os")?.getattr("fspath")?;
    os_fspath.call1((obj,))?.extract()
}

/// Convert the `whence` argument of `BinaryIO.seek()` to a `SeekFrom`.
pub(crate) fn python_whence_to_seekfrom(offset: i128, whence: u8) -> PyResult<SeekFrom> {
    Ok(match whence {
        0 => SeekFrom::Start(u64::try_from(offset).map_err(|_| PyValueError::new_err("Seek offset out of range."))?),
        1 => SeekFrom::Current(i64::try_from(offset).map_err(|_| PyValueError::new_err("Seek offset out of range."))?),
        2 => SeekFrom::End(i64::try_from(offset).map_err(|_| PyValueError::new_err("Seek offset out of range."))?),
        _ => return Err(PyValueError::new_err("Invalid value for `whence` argument. Must be 0, 1, or 2.")),
    })
}

/// Wrap a Python stream object or file path / URL into a reader adapter.
/// Returns an `fsspec` stream object if `raw_stream` is a string starting with a
/// protocol prefix and `fsspec_args` is not `False`.
///
/// # Arguments
///
/// * `raw_stream` - input python stream / file-like object
/// * `fsspec_open_mode` - open mode for `fsspec.open()`
/// * `fsspec_args` - dict of arguments to pass to `fsspec` or `None` for defaults (set to `False` to disable `fsspec`)
/// * `wrap_stream_fn` - closure for constructing a native reader from a [`PyStreamAdapter`]
/// * `wrap_path_fn` - closure for constructing a native reader from a `String` file path
pub(crate) fn wrap_stream<T, A, FStream, FPath>(
    py: Python<'_>,
    raw_stream: Py<PyAny>,
    fsspec_open_mode: &str,
    fsspec_args: Option<Py<PyAny>>,
    wrap_stream_fn: FStream,
    wrap_path_fn: FPath,
) -> PyResult<T>
where
    A: PyStreamAdapter,
    FStream: FnOnce(A) -> io::Result<T>,
    FPath: FnOnce(String) -> io::Result<T>,
{
    // Check whether `raw_stream` is a string or path-like object or wrap in adapter.
    let Ok(path) = path_like_to_string(raw_stream.bind(py)) else {
        return Ok(wrap_stream_fn(A::new(raw_stream)?)?);
    };

    let use_fsspec = path.split_once("://").is_some()
        && fsspec_args
            .as_ref()
            .is_none_or(|a| !a.bind(py).is(PyBool::new(py, false)));
    if use_fsspec {
        match py.import("fsspec") {
            Ok(fsspec) => {
                let handle = if let Some(args) = fsspec_args {
                    fsspec
                        .getattr("open")?
                        .call((path.as_str(), fsspec_open_mode), Some(args.bind(py).cast::<PyDict>()?))?
                } else {
                    fsspec.getattr("open")?.call1((path.as_str(), fsspec_open_mode))?
                }
                .call_method0("open")?;
                return Ok(wrap_stream_fn(A::new(handle.unbind())?)?);
            }
            Err(err) => {
                if err.matches(py, py.get_type::<PyModuleNotFoundError>())? {
                    // fall-through
                } else {
                    return Err(err);
                }
            }
        }
    }

    // String argument, and we're not using fsspec: Construct native reader.
    Ok(wrap_path_fn(path)?)
}

/// Shorthand for [`wrap_stream::<_, PyReaderAdapter, _, _>(...)`](wrap_stream).
pub(crate) fn wrap_reader_stream<T, FStream, FPath>(
    py: Python<'_>,
    raw_stream: Py<PyAny>,
    fsspec_args: Option<Py<PyAny>>,
    wrap_stream_fn: FStream,
    wrap_path_fn: FPath,
) -> PyResult<T>
where
    FStream: FnOnce(PyReaderAdapter) -> io::Result<T>,
    FPath: FnOnce(String) -> io::Result<T>,
{
    wrap_stream::<T, PyReaderAdapter, _, _>(py, raw_stream, "rb", fsspec_args, wrap_stream_fn, wrap_path_fn)
}

/// Shorthand for [`wrap_stream::<_, PyWriterAdapter, _, _>(...)`](wrap_stream).
pub(crate) fn wrap_writer_stream<T, FStream, FPath>(
    py: Python<'_>,
    raw_stream: Py<PyAny>,
    fsspec_args: Option<Py<PyAny>>,
    wrap_stream_fn: FStream,
    wrap_path_fn: FPath,
) -> PyResult<T>
where
    FStream: FnOnce(PyWriterAdapter) -> io::Result<T>,
    FPath: FnOnce(String) -> io::Result<T>,
{
    wrap_stream::<T, PyWriterAdapter, _, _>(py, raw_stream, "wb", fsspec_args, wrap_stream_fn, wrap_path_fn)
}

// ===========================================================
// Helper macros for redundant Reader/Writer implementations
// ===========================================================

pub(crate) mod impl_macros {
    macro_rules! impl_reader_read {
        ($self: ident, $py: ident, $size: ident) => {{
            let mut reader = $self.inner.lock().unwrap();
            let reader = reader
                .as_mut()
                .ok_or_else(|| PyValueError::new_err("Trying I/O on closed file."))?;
            let mut buf;
            let n;
            if $size < 0 {
                buf = Vec::with_capacity(2048);
                n = reader.read_to_end(&mut buf)?;
            } else {
                buf = Vec::with_capacity($size as usize);
                n = reader.take($size as u64).read_to_end(&mut buf)?;
            }
            Ok(PyBytes::new($py, &buf[..n]))
        }};
    }
    pub(crate) use impl_reader_read;

    macro_rules! impl_reader_seek {
        ($self: ident, $offset: ident, $whence: ident, $seek_fn_name: ident) => {{
            let mut reader = $self.inner.lock().unwrap();
            let reader = reader
                .as_mut()
                .ok_or_else(|| PyValueError::new_err("Trying I/O on closed file."))?;
            use crate::stream_io::python_whence_to_seekfrom;
            Ok(reader.$seek_fn_name(python_whence_to_seekfrom($offset, $whence)?)?)
        }};
    }
    pub(crate) use impl_reader_seek;

    macro_rules! forward_fn_call {
        ($self: ident, $fn_name: ident) => {{
            let mut reader = $self.inner.lock().unwrap();
            let reader = reader
                .as_mut()
                .ok_or_else(|| PyValueError::new_err("Trying I/O on closed file."))?;
            Ok(reader.$fn_name()?)
        }};
    }
    pub(crate) use forward_fn_call;

    macro_rules! impl_reader_close {
        ($self: ident) => {{
            let mut reader = $self.inner.lock().unwrap();
            reader.take();
            Ok(())
        }};
    }
    pub(crate) use impl_reader_close;

    macro_rules! impl_writer_write {
        ($self: ident, $data: ident) => {{
            let mut writer = $self.inner.lock().unwrap();
            let writer = writer
                .as_mut()
                .ok_or_else(|| PyValueError::new_err("Trying I/O on closed file."))?;
            Ok(writer.write($data)?)
        }};
    }
    pub(crate) use impl_writer_write;

    macro_rules! impl_writer_close {
        ($self: ident) => {{
            let mut writer = $self.inner.lock().unwrap();
            if let Some(w) = writer.as_mut() {
                w.flush()?;
            }
            writer.take();
            Ok(())
        }};
    }
    pub(crate) use impl_writer_close;
}
