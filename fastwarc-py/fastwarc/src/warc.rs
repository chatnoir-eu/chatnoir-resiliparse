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

use crate::stream_io::{
    PyReaderAdapter, PyWriterAdapter, WarcReaderPy, path_like_to_string, python_whence_to_seekfrom, wrap_reader_stream,
};
use fastwarc::stream_io::bufread::{LimitedBufReader, TrackingBufReader};
use fastwarc::stream_io::traits::{IntoWarcReader, WarcRead};
use fastwarc::warc::header::{HeaderEncoding, HeaderMap, WarcHeader};
use fastwarc::warc::iter::{ArchiveIteratorOptions, ArchiveIteratorThreadSafe, ArchiveIteratorTrait, filter};
use fastwarc::warc::record::{AutoDecode, WarcRecord, WarcRecordType};
use fastwarc::warc::record::{DigestError::StreamError, SharedWarcRecord};
use pyo3::exceptions::{PyKeyError, PyOSError, PyValueError};
use pyo3::intern;
use pyo3::prelude::*;
use pyo3::types::{
    PyBool, PyBytes, PyDateAccess, PyDateTime, PyDelta, PyDeltaAccess, PyDict, PyIterator, PyString, PyTimeAccess,
    PyTuple, PyType,
};
use std::io::{self, Read, Seek};
use std::sync::{Arc, Mutex, MutexGuard};
use time::{Date, Month, OffsetDateTime, Time, UtcOffset};

// ===========================================================
// WarcRecordType
// ===========================================================

/// Enum indicating a WARC record's type as given by its :class:`WARC-Type` header.
///
/// Multiple types can be combined with boolean operators for filtering records in
/// :class:`ArchiveIterator`.
#[allow(non_camel_case_types)]
#[pyclass(name = "WarcRecordType", module = "fastwarc.warc", eq, eq_int, from_py_object)]
#[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
pub enum WarcRecordTypePy {
    warcinfo = 2,
    response = 4,
    resource = 8,
    request = 16,
    metadata = 32,
    revisit = 64,
    conversion = 128,
    continuation = 256,
    unknown = 32768,
    any_type = 65535,
    #[default]
    no_type = 0,
}

impl From<WarcRecordTypePy> for WarcRecordType {
    fn from(value: WarcRecordTypePy) -> Self {
        match value {
            WarcRecordTypePy::warcinfo => WarcRecordType::WarcInfo,
            WarcRecordTypePy::response => WarcRecordType::Response,
            WarcRecordTypePy::resource => WarcRecordType::Resource,
            WarcRecordTypePy::request => WarcRecordType::Request,
            WarcRecordTypePy::metadata => WarcRecordType::Metadata,
            WarcRecordTypePy::revisit => WarcRecordType::Revisit,
            WarcRecordTypePy::conversion => WarcRecordType::Conversion,
            WarcRecordTypePy::continuation => WarcRecordType::Continuation,
            WarcRecordTypePy::unknown => WarcRecordType::Unknown,
            WarcRecordTypePy::any_type => WarcRecordType::AnyType,
            WarcRecordTypePy::no_type => WarcRecordType::NoType,
        }
    }
}

impl From<WarcRecordType> for WarcRecordTypePy {
    fn from(value: WarcRecordType) -> Self {
        match value {
            WarcRecordType::WarcInfo => WarcRecordTypePy::warcinfo,
            WarcRecordType::Response => WarcRecordTypePy::response,
            WarcRecordType::Resource => WarcRecordTypePy::resource,
            WarcRecordType::Request => WarcRecordTypePy::request,
            WarcRecordType::Metadata => WarcRecordTypePy::metadata,
            WarcRecordType::Revisit => WarcRecordTypePy::revisit,
            WarcRecordType::Conversion => WarcRecordTypePy::conversion,
            WarcRecordType::Continuation => WarcRecordTypePy::continuation,
            WarcRecordType::Unknown => WarcRecordTypePy::unknown,
            WarcRecordType::AnyType => WarcRecordTypePy::any_type,
            WarcRecordType::NoType => WarcRecordTypePy::no_type,
        }
    }
}

#[pymethods]
impl WarcRecordTypePy {
    pub fn __index__(&self) -> u16 {
        *self as u16
    }

    pub fn __or__(&self, other: u16) -> u16 {
        (*self as u16) | other
    }

    pub fn __ror__(&self, other: u16) -> u16 {
        other | (*self as u16)
    }

    pub fn __and__(&self, other: u16) -> u16 {
        (*self as u16) & other
    }

    pub fn __rand__(&self, other: u16) -> u16 {
        other & (*self as u16)
    }

    pub fn __xor__(&self, other: u16) -> u16 {
        (*self as u16) ^ other
    }

    pub fn __rxor__(&self, other: u16) -> u16 {
        other ^ (*self as u16)
    }

    pub fn __invert__(&self) -> u16 {
        !(*self as u16)
    }

    pub fn __str__(&self) -> &'static str {
        WarcRecordType::from(*self).as_str()
    }
}

// ===========================================================
// WarcHeader
// ===========================================================

/// Pre-defined set of standard WARC 1.1 headers.
/// This enum can be used in place of ``bytes`` or ``str`` values in  :class:`HeaderMap` methods
/// to void misspellings.
#[allow(non_camel_case_types)]
#[pyclass(name = "WarcHeader", module = "fastwarc.warc", eq, from_py_object)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum WarcHeaderPy {
    // TODO: Replace with py_native_enum when https://github.com/PyO3/pyo3/pull/6020 is merged.
    WARC_TYPE,
    WARC_RECORD_ID,
    WARC_DATE,
    CONTENT_LENGTH,
    CONTENT_TYPE,
    WARC_CONCURRENT_TO,
    WARC_BLOCK_DIGEST,
    WARC_PAYLOAD_DIGEST,
    WARC_IP_ADDRESS,
    WARC_REFERS_TO,
    WARC_REFERS_TO_TARGET_URI,
    WARC_REFERS_TO_DATE,
    WARC_TARGET_URI,
    WARC_TRUNCATED,
    WARC_WARCINFO_ID,
    WARC_FILENAME,
    WARC_PROFILE,
    WARC_IDENTIFIED_PAYLOAD_TYPE,
    WARC_SEGMENT_ORIGIN_ID,
    WARC_SEGMENT_NUMBER,
    WARC_SEGMENT_TOTAL_LENGTH,
}

impl From<WarcHeaderPy> for WarcHeader {
    fn from(value: WarcHeaderPy) -> Self {
        match value {
            WarcHeaderPy::WARC_TYPE => WarcHeader::WarcType,
            WarcHeaderPy::WARC_RECORD_ID => WarcHeader::WarcRecordId,
            WarcHeaderPy::WARC_DATE => WarcHeader::WarcDate,
            WarcHeaderPy::CONTENT_LENGTH => WarcHeader::ContentLength,
            WarcHeaderPy::CONTENT_TYPE => WarcHeader::ContentType,
            WarcHeaderPy::WARC_CONCURRENT_TO => WarcHeader::WarcConcurrentTo,
            WarcHeaderPy::WARC_BLOCK_DIGEST => WarcHeader::WarcBlockDigest,
            WarcHeaderPy::WARC_PAYLOAD_DIGEST => WarcHeader::WarcPayloadDigest,
            WarcHeaderPy::WARC_IP_ADDRESS => WarcHeader::WarcIpAddress,
            WarcHeaderPy::WARC_REFERS_TO => WarcHeader::WarcRefersTo,
            WarcHeaderPy::WARC_REFERS_TO_TARGET_URI => WarcHeader::WarcRefersToTargetUri,
            WarcHeaderPy::WARC_REFERS_TO_DATE => WarcHeader::WarcRefersToDate,
            WarcHeaderPy::WARC_TARGET_URI => WarcHeader::WarcTargetUri,
            WarcHeaderPy::WARC_TRUNCATED => WarcHeader::WarcTruncated,
            WarcHeaderPy::WARC_WARCINFO_ID => WarcHeader::WarcWarcinfoId,
            WarcHeaderPy::WARC_FILENAME => WarcHeader::WarcFilename,
            WarcHeaderPy::WARC_PROFILE => WarcHeader::WarcProfile,
            WarcHeaderPy::WARC_IDENTIFIED_PAYLOAD_TYPE => WarcHeader::WarcIdentifiedPayloadType,
            WarcHeaderPy::WARC_SEGMENT_ORIGIN_ID => WarcHeader::WarcSegmentOriginId,
            WarcHeaderPy::WARC_SEGMENT_NUMBER => WarcHeader::WarcSegmentNumber,
            WarcHeaderPy::WARC_SEGMENT_TOTAL_LENGTH => WarcHeader::WarcSegmentTotalLength,
        }
    }
}

#[pymethods]
impl WarcHeaderPy {
    pub fn __str__(&self) -> &'static str {
        WarcHeader::from(*self).as_str()
    }
}

#[derive(FromPyObject)]
pub enum HeaderKeyStrPy {
    Header(WarcHeaderPy),
    String(String),
}

impl HeaderKeyStrPy {
    fn into_string(self) -> String {
        match self {
            HeaderKeyStrPy::Header(key) => WarcHeader::from(key).as_str().to_string(),
            HeaderKeyStrPy::String(key) => key,
        }
    }
}

#[derive(FromPyObject)]
pub enum HeaderKeyBytesPy {
    Header(WarcHeaderPy),
    Bytes(Vec<u8>),
}

impl HeaderKeyBytesPy {
    fn into_bytes(self) -> Vec<u8> {
        match self {
            HeaderKeyBytesPy::Header(key) => WarcHeader::from(key).as_str().as_bytes().to_vec(),
            HeaderKeyBytesPy::Bytes(key) => key,
        }
    }
}

// ===========================================================
// HeaderMap
// ===========================================================

/// Dict-like type representing a WARC or HTTP header block.
///
/// :param encoding: header source encoding
#[pyclass(name = "HeaderMap", module = "fastwarc.warc")]
pub struct HeaderMapPy {
    inner: HeaderMapBacking,
}

enum HeaderMapBacking {
    Owned(HeaderMap),
    Warc(Arc<Mutex<WarcRecord>>),
    Http(Arc<Mutex<WarcRecord>>),
}

impl HeaderMapPy {
    fn from_warc(record: Arc<Mutex<WarcRecord>>) -> Self {
        Self {
            inner: HeaderMapBacking::Warc(record),
        }
    }

    fn from_http(record: Arc<Mutex<WarcRecord>>) -> Self {
        Self {
            inner: HeaderMapBacking::Http(record),
        }
    }

    fn with_headers<F, R>(&self, closure: F) -> R
    where
        F: FnOnce(&HeaderMap) -> R,
    {
        match &self.inner {
            HeaderMapBacking::Owned(header_map) => closure(header_map),
            HeaderMapBacking::Warc(record) => closure(record.lock().unwrap().headers()),
            HeaderMapBacking::Http(record) => closure(
                record
                    .lock()
                    .unwrap()
                    .http_headers()
                    .expect("No HTTP headers in record"),
            ),
        }
    }

    fn with_headers_mut<F, R>(&mut self, closure: F) -> R
    where
        F: FnOnce(&mut HeaderMap) -> R,
    {
        match &mut self.inner {
            HeaderMapBacking::Owned(header_map) => closure(header_map),
            HeaderMapBacking::Warc(record) => closure(record.lock().unwrap().headers_mut()),
            HeaderMapBacking::Http(record) => closure(
                record
                    .lock()
                    .unwrap()
                    .http_headers_mut()
                    .expect("No HTTP headers in record"),
            ),
        }
    }
}

impl From<HeaderMap> for HeaderMapPy {
    fn from(inner: HeaderMap) -> Self {
        Self {
            inner: HeaderMapBacking::Owned(inner),
        }
    }
}

#[pymethods]
impl HeaderMapPy {
    #[new]
    #[pyo3(signature = (encoding="utf-8"))]
    pub fn __new__(encoding: &str) -> PyResult<Self> {
        let enc = match encoding.to_ascii_lowercase().as_str() {
            "utf-8" => HeaderEncoding::Unicode,
            "latin1" => HeaderEncoding::Latin1,
            _ => return Err(PyValueError::new_err(format!("Unsupported encoding: {}", encoding))),
        };
        Ok(Self {
            inner: HeaderMapBacking::Owned(HeaderMap::new(enc)),
        })
    }

    pub fn __getnewargs__(&self) -> (&'static str,) {
        (self.encoding(),)
    }

    pub fn __getstate__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyBytes>, bool)> {
        self.with_headers(|headers| {
            let mut buf = Vec::with_capacity(headers.len() * 40);
            headers.write(&mut buf)?;
            Ok((PyBytes::new(py, buf.as_ref()), headers.status_line().is_some()))
        })
    }

    pub fn __setstate__<'py>(&mut self, state: (Bound<'py, PyBytes>, bool)) -> PyResult<()> {
        let (header_bytes, has_status_line) = state;
        let mut headers = HeaderMap::new(match self.encoding() {
            "utf-8" => HeaderEncoding::Unicode,
            "latin1" => HeaderEncoding::Latin1,
            e => return Err(PyValueError::new_err(format!("Unsupported encoding: {}", e))),
        });
        headers.parse(&mut io::Cursor::new(header_bytes.as_bytes().to_vec()), has_status_line)?;
        self.inner = HeaderMapBacking::Owned(headers);
        Ok(())
    }

    pub fn __repr__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        PyTuple::new(
            py,
            [
                self.status_line(py)
                    .unwrap_or_else(|| PyString::new(py, "<no status line>"))
                    .as_any(),
                self.to_tuples(py)?.as_any(),
            ],
        )?
        .call_method0("__repr__")
    }

    pub fn __str__(&self) -> String {
        self.with_headers(|headers| headers.to_string())
    }

    /// Header source encoding.
    ///
    pub fn encoding(&self) -> &'static str {
        match self.with_headers(|h| h.encoding()) {
            HeaderEncoding::Unicode => "utf-8",
            HeaderEncoding::Latin1 => "latin1",
        }
    }

    /// Parse a header block from a stream.
    ///
    /// :param reader: input stream or reader object
    /// :param has_status_line: whether the first line is a status line
    /// :return: number of bytes read
    #[pyo3(signature = (reader, has_status_line=true))]
    pub fn parse(&mut self, reader: Py<PyAny>, has_status_line: bool) -> PyResult<usize> {
        let mut reader = PyReaderAdapter::new(reader)?;
        Ok(self.with_headers_mut(|h| h.parse(&mut reader, has_status_line))?)
    }

    /// Write the header block into a stream.
    ///
    /// :param writer: output stream or writer object
    /// :return: number of bytes written
    pub fn write(&self, writer: Py<PyAny>) -> PyResult<usize> {
        let mut writer = PyWriterAdapter::new(writer)?;
        Ok(self.with_headers(|h| h.write(&mut writer))?)
    }

    /// HTTP reason phrase if this is an HTTP header block.
    ///
    /// :type: str or None
    #[getter]
    pub fn reason_phrase<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyString>> {
        self.with_headers(|h| h.reason_phrase().map(|s| PyString::new(py, s.as_ref())))
    }

    /// HTTP status code if this is an HTTP header block.
    ///
    /// :type: int or None
    #[getter]
    pub fn status_code(&self) -> Option<u16> {
        self.with_headers(HeaderMap::status_code)
    }

    /// Header status line.
    ///
    /// :type: str or None
    #[getter]
    pub fn status_line<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyString>> {
        self.with_headers(|h| h.status_line().map(|s| PyString::new(py, s.as_ref())))
    }

    /// Set status line contents.
    ///
    /// :param status_line: new status line
    #[setter]
    pub fn set_status_line(&mut self, status_line: &str) {
        self.with_headers_mut(|h| h.set_status_line(status_line));
    }

    #[getter]
    pub fn status_line_bytes<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyBytes>> {
        self.with_headers(|h| h.status_line_bytes().as_ref().map(|s| PyBytes::new(py, s)))
    }

    #[setter]
    pub fn set_status_line_bytes(&mut self, status_line: &[u8]) {
        self.with_headers_mut(|h| h.set_status_line_bytes(status_line));
    }

    /// Append header.
    ///
    /// Use this if the header name is not unique.
    ///
    /// :param key: header key
    /// :param value: header value
    pub fn append(&mut self, key: HeaderKeyStrPy, value: &str) {
        let key = key.into_string();
        self.with_headers_mut(|h| h.append(&key, value));
    }

    /// Headers as Python dict.
    ///
    /// If multiple headers have the same key, only the last occurrence is returned.
    ///
    pub fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        self.with_headers(|h| {
            h.to_map()
                .into_iter()
                .try_for_each(|(key, value)| dict.set_item(key.as_ref(), value.as_str()))
        })?;
        Ok(dict)
    }

    pub fn asdict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        self.to_dict(py)
    }

    /// Headers as a sequence of ``(key, value)`` tuples.
    ///
    /// Use this instead of :meth:`to_dict` if header keys are not unique.
    ///
    pub fn to_tuples<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.items(py)
    }

    pub fn astuples<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.to_tuples(py)
    }

    /// Whether the header map is empty.
    ///
    pub fn is_empty(&self) -> bool {
        self.with_headers(HeaderMap::is_empty)
    }

    /// Clear all headers and the status line.
    pub fn clear(&mut self) {
        self.with_headers_mut(HeaderMap::clear);
    }

    /// Get a header value or ``default``.
    ///
    /// If multiple headers have the same key, only the last occurrence is returned.
    ///
    /// :param key: header key
    /// :param default: default value if ``key`` is not present
    #[pyo3(signature = (key, default=None))]
    pub fn get<'py>(
        &self,
        py: Python<'py>,
        key: HeaderKeyStrPy,
        default: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let key = key.into_string();
        Ok(self.with_headers(|h| h.get(&key).map(|s| s.into_owned())).map_or_else(
            || default.unwrap_or_else(|| py.None().bind(py).clone()),
            |s| PyString::new(py, &s).into_any(),
        ))
    }

    /// Get all occurrences of a header.
    ///
    /// :param key: header key
    pub fn get_multiple<'py>(&self, py: Python<'py>, key: HeaderKeyStrPy) -> PyResult<Bound<'py, PyTuple>> {
        let key = key.into_string();
        self.with_headers(|h| {
            let items = h.get_multiple(&key).into_iter().map(|s| PyString::new(py, &s));
            PyTuple::new(py, items)
        })
    }

    #[pyo3(signature = (key, default=None))]
    pub fn get_bytes<'py>(
        &self,
        py: Python<'py>,
        key: HeaderKeyBytesPy,
        default: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let key = key.into_bytes();
        Ok(self
            .with_headers(|h| h.get_bytes(&key).map(|s| s.into_owned()))
            .map_or_else(|| default.unwrap_or_else(|| py.None().bind(py).clone()), |s| PyBytes::new(py, &s).into_any()))
    }

    pub fn get_bytes_multiple<'py>(&self, py: Python<'py>, key: HeaderKeyBytesPy) -> PyResult<Bound<'py, PyTuple>> {
        let key = key.into_bytes();
        self.with_headers(|h| {
            let items = h
                .get_bytes_multiple(&key)
                .into_iter()
                .map(|s| PyBytes::new(py, s.as_ref()));
            PyTuple::new(py, items)
        })
    }

    pub fn contains_key(&self, key: HeaderKeyStrPy) -> bool {
        let key = key.into_string();
        self.with_headers(|h| h.contains_key(&key))
    }

    pub fn contains_key_bytes(&self, key: HeaderKeyBytesPy) -> bool {
        let key = key.into_bytes();
        self.with_headers(|h| h.contains_key_bytes(&key))
    }

    /// Set a header value, overwriting any previous value for the same key.
    ///
    /// :param key: header key
    /// :param value: header value
    pub fn set(&mut self, key: HeaderKeyStrPy, value: &str) {
        let key = key.into_string();
        self.with_headers_mut(|h| h.set(&key, value));
    }

    pub fn set_bytes(&mut self, key: HeaderKeyBytesPy, value: &[u8]) {
        let key = key.into_bytes();
        self.with_headers_mut(|h| h.set_bytes(&key, value));
    }

    pub fn append_bytes(&mut self, key: HeaderKeyBytesPy, value: &[u8]) {
        let key = key.into_bytes();
        self.with_headers_mut(|h| h.append_bytes(&key, value));
    }

    /// Remove all headers matching ``key``.
    ///
    /// :param key: header key
    pub fn remove(&mut self, key: HeaderKeyStrPy) {
        let key = key.into_string();
        self.with_headers_mut(|h| h.remove(&key));
    }

    pub fn remove_bytes(&mut self, key: HeaderKeyBytesPy) {
        let key = key.into_bytes();
        self.with_headers_mut(|h| h.remove_bytes(&key));
    }

    /// Item view of keys and values.
    ///
    pub fn items<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let items = self.with_headers(|h| {
            h.items()
                .map(|(k, v)| PyTuple::new(py, [k.as_ref(), v.as_ref()]))
                .collect::<PyResult<Vec<_>>>()
        })?;
        PyTuple::new(py, items)
    }

    pub fn items_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let items = self.with_headers(|h| {
            h.items_bytes()
                .map(|(k, v)| PyTuple::new(py, [k, v]))
                .collect::<PyResult<Vec<_>>>()
        })?;
        PyTuple::new(py, items)
    }

    /// Iterable of header keys.
    ///
    pub fn keys<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let keys = self.with_headers(|h| h.keys().map(|k| PyString::new(py, &k)).collect::<Vec<_>>());
        PyTuple::new(py, keys)
    }

    pub fn keys_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let keys = self.with_headers(|h| h.keys_bytes().map(|k| PyBytes::new(py, &k)).collect::<Vec<_>>());
        PyTuple::new(py, keys)
    }

    /// Iterable of header values.
    ///
    pub fn values<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let values = self.with_headers(|h| h.values().map(|k| PyString::new(py, &k)).collect::<Vec<_>>());
        PyTuple::new(py, values)
    }

    pub fn values_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let values = self.with_headers(|h| h.values_bytes().map(|k| PyBytes::new(py, &k)).collect::<Vec<_>>());
        PyTuple::new(py, values)
    }

    pub fn __getitem__<'py>(&self, py: Python<'py>, item: HeaderKeyStrPy) -> PyResult<Bound<'py, PyString>> {
        let item = item.into_string();
        self.with_headers(|h| h.get(&item).map(|s| s.into_owned()))
            .map(|s| PyString::new(py, &s))
            .ok_or_else(|| PyKeyError::new_err(item))
    }

    pub fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyIterator>> {
        PyIterator::from_object(self.items(py)?.as_any())
    }

    pub fn __len__(&self) -> usize {
        self.with_headers(HeaderMap::len)
    }

    pub fn __setitem__(&mut self, key: HeaderKeyStrPy, value: &str) {
        let key = key.into_string();
        self.with_headers_mut(|h| h.set(&key, value));
    }

    pub fn __contains__(&self, item: HeaderKeyStrPy) -> bool {
        let item = item.into_string();
        self.with_headers(|h| h.contains_key(&item))
    }

    fn __eq__(&self, other: Bound<'_, PyAny>) -> bool {
        if let Ok(other) = other.cast::<HeaderMapPy>() {
            self.with_headers(|left| other.borrow().with_headers(|right| left == right))
        } else {
            false
        }
    }

    fn __ne__(&self, other: Bound<'_, PyAny>) -> bool {
        !self.__eq__(other)
    }
}

// ===========================================================
// WarcRecord
// ===========================================================

/// Reader for the remaining WARC record payload.
///
/// This object is tied to the lifetime of its parent :class:`WarcRecord`. If the
/// record belongs to an active :class:`ArchiveIterator`, the reader becomes stale
/// once iteration advances unless the record has been frozen with :meth:`WarcRecord.freeze`.
#[pyclass(name = "WarcRecordPayloadReader", module = "fastwarc.warc", extends = WarcReaderPy)]
pub struct WarcRecordPayloadReaderPy {
    record: Arc<Mutex<WarcRecord>>,
}

impl WarcRecordPayloadReaderPy {
    fn with_payload_reader<F, T>(&self, closure: F) -> PyResult<T>
    where
        F: FnOnce(&mut LimitedBufReader) -> PyResult<T>,
    {
        let mut record = self.record.lock().unwrap();
        let reader = record
            .reader_mut()
            .ok_or_else(|| PyOSError::new_err("WarcRecord has no active reader"))?;
        closure(reader)
    }
}

#[pymethods]
impl WarcRecordPayloadReaderPy {
    /// Read payload bytes.
    ///
    /// :param size: maximum number of bytes to read, or ``-1`` for all remaining bytes
    #[pyo3(signature = (size=-1))]
    pub fn read<'py>(&self, py: Python<'py>, size: i128) -> PyResult<Bound<'py, PyBytes>> {
        self.with_payload_reader(|reader| {
            let mut buf;
            let n;
            if size < 0 {
                buf = Vec::with_capacity(2048);
                n = reader.read_to_end(&mut buf)?;
            } else {
                buf = Vec::with_capacity(size as usize);
                n = reader.take(size as u64).read_to_end(&mut buf)?;
            }
            Ok(PyBytes::new(py, &buf[..n]))
        })
    }

    /// Read a single payload line.
    ///
    /// :param max_line_len: maximum line length
    #[pyo3(signature = (max_line_len=8192))]
    pub fn readline<'py>(&self, py: Python<'py>, max_line_len: usize) -> PyResult<Bound<'py, PyBytes>> {
        self.with_payload_reader(|reader| {
            let mut buf = Vec::with_capacity(max_line_len.min(128));
            reader.read_line(&mut buf, max_line_len)?;
            Ok(PyBytes::new(py, &buf))
        })
    }

    /// Consume payload bytes without returning them.
    ///
    /// :param size: maximum number of bytes to consume, or ``-1`` for all remaining bytes
    #[pyo3(signature = (size=-1))]
    pub fn consume(&self, size: i128) -> PyResult<usize> {
        let mut record = self.record.lock().unwrap();
        if size < 0 {
            Ok(record.consume()?)
        } else {
            Ok(record.consume_n(size as usize)?)
        }
    }

    /// Return the current payload offset.
    ///
    pub fn tell(&self) -> PyResult<u64> {
        self.with_payload_reader(|reader| Ok(reader.stream_position()?))
    }

    /// Seek within the payload stream.
    ///
    /// Backward seeking is only supported on frozen or in-memory payloads.
    ///
    /// :param offset: seek offset
    /// :param whence: seek mode (``0`` = start, ``1`` = current, ``2`` = end)
    #[pyo3(signature = (offset, whence=0))]
    pub fn seek(&self, offset: i128, whence: u8) -> PyResult<u64> {
        self.with_payload_reader(|reader| Ok(reader.seek(python_whence_to_seekfrom(offset, whence)?)?))
    }

    pub fn close(&self) {}
}

/// A WARC record.
///
/// WARC records are pickleable. Pickling preserves the current record state,
/// including parsed HTTP headers if they have already been parsed.
#[pyclass(name = "WarcRecord", subclass, module = "fastwarc.warc")]
pub struct WarcRecordPy {
    inner: Arc<Mutex<WarcRecord>>,
}

impl WarcRecordPy {
    fn lock(&self) -> MutexGuard<'_, WarcRecord> {
        self.inner.lock().unwrap()
    }

    fn from_record(record: WarcRecord) -> Self {
        Self {
            inner: Arc::new(Mutex::new(record)),
        }
    }
}

#[pymethods]
impl WarcRecordPy {
    #[new]
    pub fn __new__() -> Self {
        Self::from_record(WarcRecord::new())
    }

    #[classmethod]
    pub fn from_bytes(_cls: &Bound<'_, PyType>, payload: Vec<u8>) -> PyResult<Self> {
        Ok(Self::from_record(WarcRecord::from_bytes(payload)?))
    }

    // noinspection DuplicatedCode
    #[classmethod]
    #[pyo3(signature = (reader, quirks_mode=false))]
    pub fn from_reader(
        _cls: &Bound<'_, PyType>,
        py: Python<'_>,
        reader: Py<PyAny>,
        quirks_mode: bool,
    ) -> PyResult<Self> {
        let reader = wrap_reader_stream(
            py,
            reader,
            None,
            |reader| -> io::Result<Box<dyn WarcRead>> { Ok(Box::new(reader)) },
            |path| Ok(io::BufReader::new(std::fs::File::open(path)?).into_warc_reader()),
        )?;
        Ok(Self::from_record(WarcRecord::from_reader_with_opts(reader, quirks_mode, 32 << 10)?))
    }

    pub fn __getnewargs__<'py>(&self, py: Python<'py>) -> Bound<'py, PyTuple> {
        PyTuple::empty(py)
    }

    pub fn __getstate__(&mut self) -> PyResult<(Vec<u8>, bool)> {
        let mut record = self.lock();
        let http_parsed = record.is_http_parsed();
        let old_reader_pos = record.reader_mut().unwrap().stream_position()?;
        if record.is_frozen() {
            record.reader_mut().unwrap().rewind()?;
        }
        let mut serialized = Vec::with_capacity(record.content_length() as usize + 400);
        record.write_with_block_size(&mut serialized, 16384)?;
        if record.is_frozen() {
            record.reader_mut().unwrap().seek(io::SeekFrom::Start(old_reader_pos))?;
        }
        Ok((serialized, http_parsed))
    }

    pub fn __setstate__(&mut self, state: (Vec<u8>, bool)) -> PyResult<()> {
        let (serialized, http_parsed) = state;
        let mut record = WarcRecord::from_bytes(serialized)?;
        if http_parsed {
            record.parse_http()?;
        }
        self.inner = Arc::new(Mutex::new(record));
        Ok(())
    }

    pub fn __repr__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyString>> {
        PyString::from_fmt(
            py,
            format_args!(
                "<WarcRecord type='{:?}', id={:?}>",
                self.record_type(),
                self.record_id(py).unwrap_or_else(|| PyString::new(py, "<no_id>"))
            ),
        )
    }

    fn __eq__(&self, other: Bound<'_, PyAny>) -> bool {
        let Ok(other) = other.cast::<WarcRecordPy>() else {
            return false;
        };
        if Arc::ptr_eq(&self.inner, &other.borrow().inner) {
            return self.lock().is_frozen();
        }
        let left = self.lock();
        let other_ref = other.borrow();
        let right = other_ref.lock();
        *left == *right
    }

    fn __ne__(&self, other: Bound<'_, PyAny>) -> bool {
        !self.__eq__(other)
    }

    pub fn __str__(&self) -> String {
        self.inner.with_mut(|r| r.to_string())
    }

    /// Record ID.
    ///
    /// This is the same as ``headers[WarcHeader.WARC_RECORD_ID]`` if present.
    ///
    /// :type: str or None
    #[getter]
    pub fn record_id<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyString>> {
        self.lock().record_id().map(|s| PyString::new(py, s.as_ref()))
    }

    /// Set record ID.
    ///
    /// :param record_id: record identifier
    #[setter]
    pub fn set_record_id<'py>(&self, record_id: &str) {
        self.lock().set_record_id(record_id);
    }

    /// Record type.
    ///
    /// :type: WarcRecordType
    #[getter]
    pub fn record_type(&self) -> WarcRecordTypePy {
        self.lock().record_type().into()
    }

    /// Set record type.
    ///
    /// :param record_type: record type
    #[setter]
    pub fn set_record_type(&mut self, record_type: WarcRecordTypePy) {
        self.lock().set_record_type(record_type.into());
    }

    /// Remaining WARC record length in bytes.
    ///
    /// This is not necessarily the same as the WARC ``Content-Length`` header.
    ///
    /// :type: int
    #[getter]
    pub fn content_length(&self) -> u64 {
        self.lock().content_length()
    }

    /// WARC Date.
    ///
    /// :type: datetime.datetime or None
    #[getter]
    pub fn record_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyDateTime>>> {
        let inner = self.lock();
        let Some(parsed) = inner.record_date() else {
            return Ok(None);
        };

        let tz = py
            .import(intern!(py, "datetime"))?
            .getattr(intern!(py, "timezone"))?
            .call1((py
                .import(intern!(py, "datetime"))?
                .getattr(intern!(py, "timedelta"))?
                .call1((0, parsed.offset().whole_seconds()))?,))?;

        Ok(Some(PyDateTime::new(
            py,
            parsed.year(),
            u8::from(parsed.month()),
            parsed.day(),
            parsed.hour(),
            parsed.minute(),
            parsed.second(),
            parsed.microsecond(),
            Some(tz.cast()?),
        )?))
    }

    /// Set WARC Date.
    ///
    /// ``record_date`` must be timezone-aware.
    ///
    /// :param record_date: datetime object
    #[setter]
    pub fn set_record_date<'py>(&mut self, py: Python<'py>, record_date: Bound<'py, PyDateTime>) -> PyResult<()> {
        if record_date.getattr("tzinfo")?.is_none() {
            return Err(PyValueError::new_err("Trying to set naive datetime without timezone info."));
        }

        let date = Date::from_calendar_date(
            record_date.get_year(),
            Month::try_from(record_date.get_month()).map_err(|_| PyValueError::new_err("Invalid month"))?,
            record_date.get_day(),
        )
        .map_err(|e| PyValueError::new_err(format!("Failed to create date: {}", e)))?;

        let time = Time::from_hms_micro(
            record_date.get_hour(),
            record_date.get_minute(),
            record_date.get_second(),
            record_date.get_microsecond(),
        )
        .map_err(|e| PyValueError::new_err(format!("Failed to create time: {}", e)))?;

        let tzinfo = record_date.getattr(intern!(py, "tzinfo"))?;
        let delta = tzinfo
            .call_method1(intern!(py, "utcoffset"), (record_date,))?
            .cast_into::<PyDelta>()?;
        let offset = UtcOffset::from_whole_seconds(delta.get_days() * 86400 + delta.get_seconds())
            .map_err(|e| PyValueError::new_err(format!("Failed to create UTC offset: {}", e)))?;

        self.lock()
            .set_record_date(OffsetDateTime::new_in_offset(date, time, offset));
        Ok(())
    }

    /// WARC record headers.
    ///
    /// Mutating the returned :class:`HeaderMap` updates the record directly.
    ///
    /// :type: HeaderMap
    #[getter]
    pub fn headers(&self) -> HeaderMapPy {
        HeaderMapPy::from_warc(self.inner.clone())
    }

    /// Whether this record is an HTTP record.
    ///
    /// Modifying this property also updates the WARC ``Content-Type`` header.
    ///
    /// :type: bool
    #[getter]
    pub fn is_http(&self) -> bool {
        self.lock().is_http()
    }

    /// Set whether this record is an HTTP record.
    ///
    /// :param is_http: whether the record is an HTTP record
    #[setter]
    pub fn set_is_http(&mut self, is_http: bool) {
        self.lock().set_is_http(is_http);
    }

    /// Whether HTTP headers have been parsed.
    ///
    /// :type: bool
    #[getter]
    pub fn is_http_parsed(&self) -> bool {
        self.lock().is_http_parsed()
    }

    /// Parsed HTTP headers, if available.
    ///
    /// :type: HeaderMap or None
    #[getter]
    pub fn http_headers(&self) -> Option<HeaderMapPy> {
        self.lock()
            .http_headers()
            .map(|_| HeaderMapPy::from_http(self.inner.clone()))
    }

    /// Plain HTTP ``Content-Type`` without fields such as ``charset=``.
    ///
    /// :type: str or None
    #[getter]
    pub fn http_content_type<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyString>> {
        self.lock().http_content_type().as_deref().map(|s| PyString::new(py, s))
    }

    /// HTTP charset/encoding returned by the server.
    ///
    /// The returned value is guaranteed to be a valid Python encoding name.
    ///
    /// :type: str or None
    #[getter]
    pub fn http_charset<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyString>> {
        self.lock().http_charset().as_ref().map(|s| PyString::new(py, s))
    }

    /// Parsed HTTP ``Date`` header.
    ///
    /// :type: datetime.datetime or None
    #[getter]
    pub fn http_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        http_datetime_to_py(py, self.lock().http_headers().and_then(|h| h.get("Date")).as_deref())
    }

    /// Parsed HTTP ``Last-Modified`` header.
    ///
    /// :type: datetime.datetime or None
    #[getter]
    pub fn http_last_modified<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        http_datetime_to_py(
            py,
            self.lock()
                .http_headers()
                .and_then(|h| h.get("Last-Modified"))
                .as_deref(),
        )
    }

    /// Reader for the remaining WARC record payload.
    ///
    /// :type: WarcRecordPayloadReader or None
    #[getter]
    pub fn reader<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        if self.inner.lock().unwrap().reader_mut().is_none() {
            return Ok(py.None().into_bound(py));
        }
        let reader = Py::new(
            py,
            PyClassInitializer::from(WarcReaderPy::__new__()).add_subclass(WarcRecordPayloadReaderPy {
                record: self.inner.clone(),
            }),
        )?;
        Ok(reader.into_bound(py).into_any())
    }

    /// WARC record start offset in the original input stream.
    ///
    /// :type: int
    #[getter]
    pub fn stream_pos(&self) -> u64 {
        self.lock().stream_pos()
    }

    /// Whether this record has been frozen.
    ///
    /// :type: bool
    #[getter]
    pub fn is_frozen(&self) -> bool {
        self.lock().is_frozen()
    }

    /// Initialize mandatory headers in a fresh :class:`WarcRecord` instance.
    ///
    /// The ``content_length`` keyword argument is accepted for compatibility but
    /// is deprecated and ignored. The value of the ``Content-Length`` header and the
    /// ``content_length`` property are determined automatically by the length of the
    /// record payload.
    ///
    /// :param record_type: WARC-Type
    /// :param record_urn: WARC-Record-ID as URN without ``'<urn:'`` and ``'>'``
    /// :param content_length: deprecated compatibility argument, ignored
    #[pyo3(signature = (record_type=WarcRecordTypePy::no_type, record_urn=None, *, content_length=None)
    )]
    pub fn init_headers(
        &mut self,
        record_type: WarcRecordTypePy,
        record_urn: Option<&[u8]>,
        content_length: Option<u64>, // deprecated
    ) {
        _ = content_length;
        self.lock().init_headers(record_type.into(), record_urn);
    }

    /// Freeze the record payload.
    ///
    /// Freezing copies the remaining payload bytes into memory so the record can
    /// outlive the iterator stream and support backward seeking.
    ///
    pub fn freeze(&mut self) -> PyResult<bool> {
        self.lock().freeze()?;
        Ok(true)
    }

    /// Set the WARC payload as bytes.
    ///
    /// :param content: payload as bytes
    pub fn set_bytes_content(&mut self, content: &[u8]) {
        self.set_bytes_payload(content);
    }

    /// Set the WARC payload as bytes.
    ///
    /// :param content: payload as bytes
    pub fn set_bytes_payload(&mut self, content: &[u8]) {
        self.lock().set_bytes_payload(content.to_vec());
    }

    /// Consume payload bytes without returning them.
    ///
    /// :param n: maximum number of bytes to consume, or ``None`` for all remaining bytes
    #[pyo3(signature = (n=None))]
    pub fn consume(&mut self, n: Option<usize>) -> PyResult<usize> {
        match n {
            Some(n) => Ok(self.lock().consume_n(n)?),
            None => Ok(self.lock().consume()?),
        }
    }

    /// Parse the WARC header block from the attached stream.
    ///
    /// :param quirks_mode: enable lenient parsing
    /// :param max_header_len: maximum allowed header length (throws an error if exceeded)
    /// :return: number of bytes read
    #[pyo3(signature = (quirks_mode=false, max_header_len=32 << 10))]
    pub fn parse_warc_headers(&mut self, quirks_mode: bool, max_header_len: usize) -> PyResult<usize> {
        Ok(self.lock().parse_warc_headers_with_opts(quirks_mode, max_header_len)?)
    }

    /// Parse HTTP headers and advance the payload reader.
    ///
    /// It is safe to call this method multiple times, even if the record is not
    /// an HTTP record.
    ///
    /// If a parsed header exceeds `max_header_len`, an error is raised.
    ///
    /// Quirks mode allows parsing of headers terminated with only LF instead of CRLF.
    ///
    /// :param auto_decode: automatically decode HTTP payload encodings
    ///                     (accepted values: ``'none'``, ``'content'``, ``'transfer'``, ``'all'``)
    /// :param max_header_len: maximum allowed header length (throws an error if exceeded)
    /// :param quirks_mode: enable parsing of LF-only headers.
    /// :param strict_mode: this argument is deprecated and ignored.
    #[pyo3(signature = (auto_decode="none", max_header_len=32 << 10, quirks_mode=false, *, strict_mode=true))]
    pub fn parse_http(
        &mut self,
        auto_decode: &str,
        max_header_len: usize,
        quirks_mode: bool,
        strict_mode: bool,
    ) -> PyResult<()> {
        let _ = strict_mode;
        self.lock()
            .parse_http_with_opts(auto_decode_str_to_enum(auto_decode)?, max_header_len, quirks_mode)?;
        Ok(())
    }

    /// Verify whether ``WARC-Block-Digest`` matches the current record block.
    ///
    /// Returns ``False`` for missing or invalid digest metadata and raises
    /// :class:`OSError` only for stream I/O failures.
    ///
    /// :param consume: consume the remaining record payload instead of preserving it
    #[pyo3(signature = (consume=false))]
    pub fn verify_block_digest(&mut self, consume: bool) -> PyResult<bool> {
        match self.lock().verify_block_digest(consume) {
            Ok(valid) => Ok(valid),
            Err(StreamError(e)) => Err(PyOSError::new_err(e.to_string())),
            Err(_) => Ok(false),
        }
    }

    /// Verify whether ``WARC-Payload-Digest`` matches the current HTTP payload.
    ///
    /// HTTP headers must have been parsed first with :meth:`parse_http`.
    /// Returns ``False`` for missing or invalid digest metadata and raises
    /// :class:`OSError` only for stream I/O failures.
    ///
    /// :param consume: consume the remaining payload instead of preserving it
    #[pyo3(signature = (consume=false))]
    pub fn verify_payload_digest(&mut self, consume: bool) -> PyResult<bool> {
        match self.lock().verify_payload_digest(consume) {
            Ok(valid) => Ok(valid),
            Err(StreamError(e)) => Err(PyOSError::new_err(e.to_string())),
            Err(_) => Ok(false),
        }
    }

    /// Write this record to a stream.
    ///
    /// :param stream: output stream
    /// :param checksum_data: calculate and add block and payload digests
    /// :param payload_digest: optional SHA-1 payload digest bytes
    /// :param chunk_size: write block size
    /// :return: number of bytes written
    #[pyo3(signature = (stream, checksum_data=false, payload_digest=None, chunk_size=16384))]
    pub fn write<'py>(
        &mut self,
        py: Python<'py>,
        stream: Bound<'py, PyAny>,
        checksum_data: bool,
        payload_digest: Option<&[u8]>,
        chunk_size: usize,
    ) -> PyResult<usize> {
        if let Some(payload_digest) = payload_digest {
            let encoded = py
                .import(intern!(py, "base64"))?
                .getattr(intern!(py, "b32encode"))?
                .call1((PyBytes::new(py, payload_digest),))?
                .extract::<Bound<'_, PyBytes>>()?;
            let mut digest_header = b"sha1:".to_vec();
            digest_header.extend_from_slice(encoded.as_bytes());
            self.lock()
                .headers_mut()
                .set_bytes(b"WARC-Payload-Digest", &digest_header);
        }

        // Record correct before stream position for legacy shims.
        // TODO: Remove when shims are removed.
        let raw_stream = stream.getattr("_raw_stream").ok();
        let pos_before = match &raw_stream {
            Some(raw_stream) => raw_stream.call_method0("tell").and_then(|r| r.extract::<usize>()).ok(),
            None => None,
        };

        let mut writer = PyWriterAdapter::new(stream.clone().unbind())?;
        let bytes_written = if checksum_data {
            self.lock()
                .write_with_block_size_checksum(&mut writer, chunk_size, true)?
        } else {
            self.lock().write_with_block_size(&mut writer, chunk_size)?
        };

        // Call legacy shim end_member and calculate bytes written.
        // TODO: Remove when shims are removed.
        if stream.hasattr("end_member")? {
            stream.call_method0("end_member")?;
        }
        Ok(match (pos_before, raw_stream) {
            (Some(pos_before), Some(raw_stream)) => raw_stream
                .call_method0("tell")
                .and_then(|r| r.extract::<usize>())
                .map(|pos_after| pos_after - pos_before)
                .unwrap_or(bytes_written),
            _ => bytes_written,
        })
    }
}

fn http_datetime_to_py<'py>(py: Python<'py>, value: Option<&str>) -> PyResult<Option<Bound<'py, PyAny>>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let parsed = py
        .import(intern!(py, "email.utils"))?
        .getattr(intern!(py, "parsedate_to_datetime"))?
        .call1((value,));
    match parsed {
        Ok(obj) => Ok(Some(obj)),
        Err(_) => Ok(None),
    }
}

// ===========================================================
// ArchiveIterator
// ===========================================================

/// WARC record stream iterator.
///
/// The iterator can be initialized from a file-like Python object, a path-like
/// object, or a URL string. If installed, :mod:`fsspec` is used for opening
/// paths and URLs unless ``fsspec_args=False``.
///
/// :param stream: input stream, file-like object, file name, or URL
/// :param record_types: bitmask of :class:`WarcRecordType` values to return
/// :param parse_http: parse HTTP records automatically
/// :param min_content_length: skip records smaller than this length, or ``None`` to disable
/// :param max_content_length: skip records larger than this length, or ``None`` to disable
/// :param func_filter: Python callable taking a :class:`WarcRecord` and returning ``bool``
/// :param verify_digests: skip records with missing or invalid block digests
/// :param quirks_mode: enable lenient parsing for malformed records
/// :param auto_decode: automatically decode HTTP payload encodings
/// :param max_header_len: maximum allowed header length (throws an error if exceeded)
/// :param stream_detect: auto-detect gzip, zstd, or lz4 compressed streams
/// :param default_buffer_size: default buffer size to use for reading from files
///                             (has no effect if ``stream`` is not a path-like object)
/// :param inplace: Reuse and mutate the same :class:`WarcRecord` instance instead of creating
///                 a new instance in each iteration.
/// :param fsspec_args: arguments for :mod:`fsspec`, or ``False`` to disable it
/// :param strict_mode: this argument is deprecated and ignored. Use ``quirks_mode`` instead.
#[pyclass(name = "ArchiveIterator", module = "fastwarc.warc")]
pub struct ArchiveIteratorPy {
    inner: Box<dyn ArchiveIteratorTrait<Arc<Mutex<WarcRecord>>> + Send + Sync>,
    py_func_filter: Option<Py<PyAny>>,
    restart_factory: Py<PyAny>,
    inplace: bool,
    current_record: Option<Py<WarcRecordPy>>,
}

fn auto_decode_str_to_enum(value: &str) -> PyResult<AutoDecode> {
    Ok(match value {
        "none" => AutoDecode::None,
        "transfer" => AutoDecode::TransferEncoding,
        "content" => AutoDecode::ContentEncoding,
        "all" => AutoDecode::All,
        _ => return Err(PyValueError::new_err(format!("Invalid value for auto_decode: '{}'", value))),
    })
}

/// Distinguish between native and non-native func_filters for more efficient dispatch.
/// Returns a tuple of options with (native kind, py object), of which one is always `None`.
fn extract_func_filter(
    py: Python<'_>,
    func_filter: Option<Py<PyAny>>,
) -> PyResult<(Option<NativeFilterKind>, Option<Py<PyAny>>)> {
    Ok(match func_filter {
        Some(f) => match f.cast_bound::<NativeFilterPy>(py) {
            Ok(b) => (Some(b.get().kind), None),
            Err(_) => {
                let bound = f.bind(py);
                let native = if bound.getattr(intern!(py, "__module__"))?.eq("fastwarc.warc")? {
                    NativeFilterKind::from_name(&bound.getattr(intern!(py, "__name__"))?.extract::<String>()?)
                } else {
                    None
                };
                if native.is_some() {
                    (native, None)
                } else {
                    (None, Some(f))
                }
            }
        },
        None => (None, None),
    })
}

#[pymethods]
impl ArchiveIteratorPy {
    // noinspection DuplicatedCode
    #[allow(clippy::too_many_arguments)]
    #[new]
    #[pyo3(signature = (
        stream,
        record_types=WarcRecordTypePy::any_type as u16,
        parse_http=true,
        min_content_length=None,
        max_content_length=None,
        func_filter=None,
        verify_digests=false,
        quirks_mode=false,
        auto_decode="none",
        max_header_len=32 << 10,
        stream_detect=true,
        buffer_size=64 << 10,
        inplace=false,
        fsspec_args=None,
        *,
        strict_mode=true
    ))]
    pub fn __new__(
        py: Python<'_>,
        stream: Py<PyAny>,
        record_types: u16,
        parse_http: bool,
        min_content_length: Option<u64>,
        max_content_length: Option<u64>,
        func_filter: Option<Py<PyAny>>,
        verify_digests: bool,
        quirks_mode: bool,
        auto_decode: &str,
        max_header_len: usize,
        stream_detect: bool,
        buffer_size: usize,
        inplace: bool,
        fsspec_args: Option<Py<PyAny>>,
        strict_mode: bool,
    ) -> PyResult<Self> {
        let _ = strict_mode;

        // Check if fsspec is `False`
        let use_fsspec = fsspec_args
            .as_ref()
            .and_then(|f| f.cast_bound::<PyBool>(py).ok())
            .map(|b| b.is_true())
            .unwrap_or(true);
        let restart_fsspec_args = fsspec_args.as_ref().map(|args| args.clone_ref(py));

        let mut iterator;
        if !use_fsspec && let Ok(path) = path_like_to_string(stream.bind(py)) {
            let opts = ArchiveIteratorOptions {
                stream_detect,
                ..ArchiveIteratorOptions::default()
            };
            iterator = ArchiveIteratorThreadSafe::from_path_with_options(path, opts)?;
        } else {
            let reader = wrap_reader_stream(
                py,
                stream,
                fsspec_args,
                |reader| -> io::Result<Box<dyn WarcRead>> { Ok(Box::new(reader)) },
                |path| Ok(TrackingBufReader::with_capacity(buffer_size, std::fs::File::open(path)?).into_warc_reader()),
            )?;
            iterator = ArchiveIteratorThreadSafe::new(reader).with_stream_detect(stream_detect);
        }

        iterator = iterator
            .with_quirks_mode(quirks_mode)
            .with_parse_http(parse_http)
            .with_max_header_len(max_header_len)
            .with_verify_digests(verify_digests)
            .with_decode_http_payload(auto_decode_str_to_enum(auto_decode)?)
            .with_inplace(inplace);
        let restart_func_filter = func_filter.as_ref().map(|filter| filter.clone_ref(py));
        let func_filter = extract_func_filter(py, func_filter)?;
        let native_filter_kind = func_filter.0;
        let native_func_filter = move |r: &mut WarcRecord| -> bool {
            match &native_filter_kind {
                Some(native) => native.apply(r),
                None => true,
            }
        };

        let filter = move |r: &mut WarcRecord| {
            if let Some(min) = min_content_length
                && !filter::has_content_length_gte(min)(r)
            {
                return false;
            }
            if let Some(min) = max_content_length
                && !filter::has_content_length_lte(min)(r)
            {
                return false;
            }
            if record_types != WarcRecordTypePy::any_type as u16 && !filter::has_record_type(record_types)(r) {
                return false;
            }
            native_func_filter(r)
        };

        // Keep the constructor options so a retry can replace the source stream
        // without losing filters or parsing configuration.
        let kwargs = PyDict::new(py);
        kwargs.set_item("record_types", record_types)?;
        kwargs.set_item("parse_http", parse_http)?;
        kwargs.set_item("min_content_length", min_content_length)?;
        kwargs.set_item("max_content_length", max_content_length)?;
        kwargs.set_item("func_filter", restart_func_filter)?;
        kwargs.set_item("verify_digests", verify_digests)?;
        kwargs.set_item("quirks_mode", quirks_mode)?;
        kwargs.set_item("auto_decode", auto_decode)?;
        kwargs.set_item("max_header_len", max_header_len)?;
        kwargs.set_item("stream_detect", stream_detect)?;
        kwargs.set_item("buffer_size", buffer_size)?;
        kwargs.set_item("inplace", inplace)?;
        kwargs.set_item("fsspec_args", restart_fsspec_args)?;
        kwargs.set_item("strict_mode", strict_mode)?;
        let class = PyModule::import(py, "fastwarc.warc")?.getattr("ArchiveIterator")?;
        let restart_factory = PyModule::import(py, "functools")?
            .getattr("partial")?
            .call((class,), Some(&kwargs))?
            .unbind();

        Ok(Self {
            inner: Box::new(iterator.with_filter(filter)),
            py_func_filter: func_filter.1,
            restart_factory,
            inplace,
            current_record: None,
        })
    }

    /// Return :class:`WarcRecord` iterator.
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Create a new iterator over a replacement stream with the same parsing options.
    ///
    /// This is useful when resuming archive iteration after a stream failure. The returned iterator
    /// starts at the beginning of `stream`.
    pub fn restart(&self, stream: Py<PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.restart_factory.bind(py).call1((stream,)).map(Bound::unbind)
    }

    /// Return the next :class:`WarcRecord` from the stream.
    pub fn __next__<'py>(&mut self, py: Python<'py>) -> PyResult<Option<Py<WarcRecordPy>>> {
        loop {
            let Some(next) = self.inner.next() else {
                return Ok(None);
            };
            let record = next?;

            let record_obj = if self.inplace {
                if let Some(record_obj) = &self.current_record {
                    record_obj.clone_ref(py)
                } else {
                    let record_obj = Py::new(py, WarcRecordPy { inner: record.clone() })?;
                    self.current_record = Some(record_obj.clone_ref(py));
                    record_obj
                }
            } else {
                Py::new(py, WarcRecordPy { inner: record.clone() })?
            };

            // Evaluate non-native func_filter.
            if let Some(func_filter) = &self.py_func_filter
                && !func_filter.bind(py).call1((record_obj.bind(py),))?.is_truthy()?
            {
                continue;
            }

            return Ok(Some(record_obj));
        }
    }
}

// ===========================================================
// ArchiveIterator filter predicates
// ===========================================================

/// Tag identifying pre-defined native filter predicates.
///
/// Used by [`ArchiveIteratorPy`] to bypass the Python dispatch when `func_filter`
/// is one of the pre-defined native filters.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum NativeFilterKind {
    IsWarc10,
    IsWarc11,
    HasBlockDigest,
    HasValidBlockDigest,
    HasPayloadDigest,
    HasValidPayloadDigest,
    IsHttp,
    IsConcurrent,
    HasRecordType(u16),
    HasContentLengthLte(u64),
    HasContentLengthGte(u64),
}

impl NativeFilterKind {
    fn apply(&self, record: &mut WarcRecord) -> bool {
        match self {
            Self::IsWarc10 => filter::is_warc_10(record),
            Self::IsWarc11 => filter::is_warc_11(record),
            Self::HasBlockDigest => filter::has_block_digest(record),
            Self::HasValidBlockDigest => filter::has_valid_block_digest(record),
            Self::HasPayloadDigest => filter::has_payload_digest(record),
            Self::HasValidPayloadDigest => filter::has_valid_payload_digest(record),
            Self::IsHttp => filter::is_http(record),
            Self::IsConcurrent => filter::is_concurrent(record),
            Self::HasRecordType(mask) => filter::has_record_type(*mask)(record),
            Self::HasContentLengthLte(max) => filter::has_content_length_lte(*max)(record),
            Self::HasContentLengthGte(min) => filter::has_content_length_gte(*min)(record),
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "is_warc_10" => Some(Self::IsWarc10),
            "is_warc_11" => Some(Self::IsWarc11),
            "has_block_digest" => Some(Self::HasBlockDigest),
            "has_valid_block_digest" => Some(Self::HasValidBlockDigest),
            "has_payload_digest" => Some(Self::HasPayloadDigest),
            "has_valid_payload_digest" => Some(Self::HasValidPayloadDigest),
            "is_http" => Some(Self::IsHttp),
            "is_concurrent" => Some(Self::IsConcurrent),
            _ => None,
        }
    }
}

/// Callable wrapper around pre-defined native filter predicates.
///
/// Instances behave like regular Python callables (``filter(record)``), but
/// :class:`ArchiveIterator` dispatches them more efficiently than actual
/// Python functions.
#[pyclass(name = "NativeFilter", module = "fastwarc.warc", frozen)]
pub struct NativeFilterPy {
    pub kind: NativeFilterKind,
}

#[pymethods]
impl NativeFilterPy {
    pub fn __call__(&self, record: &WarcRecordPy) -> bool {
        let mut record = record.lock();
        self.kind.apply(&mut record)
    }
}

fn apply_filter(record: &WarcRecordPy, predicate: impl FnOnce(&mut WarcRecord) -> bool) -> bool {
    let mut record = record.lock();
    predicate(&mut record)
}

/// Filter predicate for checking if a record is a WARC/1.0 record.
///
/// :param record: WARC record
#[pyfunction(name = "is_warc_10")]
pub fn is_warc_10_py(record: &WarcRecordPy) -> bool {
    apply_filter(record, filter::is_warc_10)
}

/// Filter predicate for checking if a record is a WARC/1.1 record.
///
/// :param record: WARC record
#[pyfunction(name = "is_warc_11")]
pub fn is_warc_11_py(record: &WarcRecordPy) -> bool {
    apply_filter(record, filter::is_warc_11)
}

/// Filter predicate for checking if a record has a block digest.
///
/// :param record: WARC record
#[pyfunction(name = "has_block_digest")]
pub fn has_block_digest_py(record: &WarcRecordPy) -> bool {
    apply_filter(record, filter::has_block_digest)
}

/// Filter predicate for checking if a record has a valid block digest.
///
/// :param record: WARC record
#[pyfunction(name = "has_valid_block_digest")]
pub fn has_valid_block_digest_py(record: &WarcRecordPy) -> bool {
    apply_filter(record, filter::has_valid_block_digest)
}

/// Filter predicate for checking if a record has a payload digest.
///
/// :param record: WARC record
#[pyfunction(name = "has_payload_digest")]
pub fn has_payload_digest_py(record: &WarcRecordPy) -> bool {
    apply_filter(record, filter::has_payload_digest)
}

/// Filter predicate for checking if a record has a valid payload digest.
///
/// :param record: WARC record
#[pyfunction(name = "has_valid_payload_digest")]
pub fn has_valid_payload_digest_py(record: &WarcRecordPy) -> bool {
    apply_filter(record, filter::has_valid_payload_digest)
}

/// Filter predicate for checking if a record is an HTTP record.
///
/// :param record: WARC record
#[pyfunction(name = "is_http")]
pub fn is_http_py(record: &WarcRecordPy) -> bool {
    apply_filter(record, filter::is_http)
}

/// Filter predicate for checking if a record is concurrent to another record.
///
/// :param record: WARC record
#[pyfunction(name = "is_concurrent")]
pub fn is_concurrent_py(record: &WarcRecordPy) -> bool {
    apply_filter(record, filter::is_concurrent)
}

/// Parameterized filter predicate for checking if a record’s record type matches the given bitmask.
///
/// This predicate is equivalent to using ``record_types`` in :class:`ArchiveIterator`.
///
/// :param record_type_bitmask: :class:`WarcRecordType` or bitmask of types
/// :returns: WARC record filter
#[pyfunction(name = "has_record_type")]
pub fn has_record_type_py(record_type_bitmask: u16) -> NativeFilterPy {
    NativeFilterPy {
        kind: NativeFilterKind::HasRecordType(record_type_bitmask),
    }
}

/// Parameterized filter predicate for checking if a record’s ``Content-Length`` is less than or equal to ``max``.
///
/// This predicate is equivalent to using ``max_content_length`` in :class:`ArchiveIterator`.
///
/// :param max: maximum ``Content-Length``
/// :returns: WARC record filter
#[pyfunction(name = "has_content_length_lte")]
pub fn has_content_length_lte_py(max: u64) -> NativeFilterPy {
    NativeFilterPy {
        kind: NativeFilterKind::HasContentLengthLte(max),
    }
}

/// Parameterized filter predicate for checking if a record’s Content-Length is greater than or equal to ``min``.
///
/// This predicate is equivalent to using ``min_content_length`` in :class:`ArchiveIterator`.
///
/// :param min: minimum ``Content-Length``
/// :returns: WARC record filter
#[pyfunction(name = "has_content_length_gte")]
pub fn has_content_length_gte_py(min: u64) -> NativeFilterPy {
    NativeFilterPy {
        kind: NativeFilterKind::HasContentLengthGte(min),
    }
}
