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

use super::stream_io::{BufReadSeek, LimitedBufReadSeek};
use digest::{Digest, DynDigest};
use encoding::all::WINDOWS_1252;
use encoding::{DecoderTrap, EncoderTrap, Encoding};
use sha2::digest;
use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::io;
use std::io::{BufRead, Read, Seek};
use uuid::Uuid;

// ===========================================================
// WARC record type enum
// ===========================================================

/// WARC record type enum
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WarcRecordType {
    WarcInfo = 2,
    Response = 4,
    Resource = 8,
    Request = 16,
    Metadata = 32,
    Revisit = 64,
    Conversion = 128,
    Continuation = 256,
    Unknown = 512,
    AnyType = 65535,
    #[default]
    NoType = 0,
}

impl WarcRecordType {
    pub fn as_str(&self) -> &'static str {
        match self {
            WarcRecordType::WarcInfo => "warcinfo",
            WarcRecordType::Response => "response",
            WarcRecordType::Resource => "resource",
            WarcRecordType::Request => "request",
            WarcRecordType::Metadata => "metadata",
            WarcRecordType::Revisit => "revisit",
            WarcRecordType::Conversion => "conversion",
            WarcRecordType::Continuation => "continuation",
            _ => "unknown",
        }
    }

    pub fn matches_bitmask(&self, bitmask: u16) -> bool {
        (*self as u16) & bitmask != 0
    }
}

impl TryFrom<u16> for WarcRecordType {
    type Error = &'static str;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            2 => Ok(WarcRecordType::WarcInfo),
            4 => Ok(WarcRecordType::Response),
            8 => Ok(WarcRecordType::Resource),
            16 => Ok(WarcRecordType::Request),
            32 => Ok(WarcRecordType::Metadata),
            64 => Ok(WarcRecordType::Revisit),
            128 => Ok(WarcRecordType::Conversion),
            256 => Ok(WarcRecordType::Continuation),
            512 => Ok(WarcRecordType::Unknown),
            65535 => Ok(WarcRecordType::AnyType),
            0 => Ok(WarcRecordType::NoType),
            _ => Err("Invalid enum value."),
        }
    }
}

impl TryFrom<&[u8]> for WarcRecordType {
    type Error = &'static str;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        match value.to_ascii_lowercase().as_slice() {
            b"warcinfo" => Ok(WarcRecordType::WarcInfo),
            b"response" => Ok(WarcRecordType::Response),
            b"resource" => Ok(WarcRecordType::Resource),
            b"request" => Ok(WarcRecordType::Request),
            b"metadata" => Ok(WarcRecordType::Metadata),
            b"revisit" => Ok(WarcRecordType::Revisit),
            b"conversion" => Ok(WarcRecordType::Conversion),
            b"continuation" => Ok(WarcRecordType::Continuation),
            b"unknown" => Ok(WarcRecordType::Unknown),
            _ => Err("Invalid enum value."),
        }
    }
}

impl TryFrom<&str> for WarcRecordType {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        TryFrom::try_from(value.as_bytes())
    }
}

impl From<WarcRecordType> for &'static str {
    fn from(value: WarcRecordType) -> Self {
        value.as_str()
    }
}

// ===========================================================
// WARC / HTTP header map
// ===========================================================

/// Case-insensitive string key for headers
#[derive(Debug, Eq, Clone)]
pub struct CaseInsensitiveKey(String);

impl CaseInsensitiveKey {
    fn new(s: impl Into<String>) -> Self {
        CaseInsensitiveKey(s.into())
    }

    #[allow(dead_code)]
    fn as_str(&self) -> &str {
        &self.0
    }

    fn to_lowercase(&self) -> String {
        self.0.to_lowercase()
    }
}

impl PartialEq for CaseInsensitiveKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_ignore_ascii_case(&other.0)
    }
}

impl From<CaseInsensitiveKey> for String {
    fn from(key: CaseInsensitiveKey) -> Self {
        key.0
    }
}

impl From<String> for CaseInsensitiveKey {
    fn from(key: String) -> Self {
        CaseInsensitiveKey::new(key)
    }
}

impl std::hash::Hash for CaseInsensitiveKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.to_lowercase().hash(state);
    }
}

impl From<&str> for CaseInsensitiveKey {
    fn from(s: &str) -> Self {
        CaseInsensitiveKey(s.to_string())
    }
}

#[derive(Default, Debug, Eq, PartialEq, Clone)]
pub enum HeaderEncoding {
    #[default]
    Unicode,
    Latin1,
}

/// Multimap structure representing a WARC or HTTP header block.
///
/// Headers can be set or retrieved by key and the whole header block can be
/// serialized to an [`io::BufRead`] stream.
///
/// WARC headers should be created with [`HeaderEncoding::Unicode`].
/// HTTP headers should use [`HeaderEncoding::Latin1`]. However, in either case,
/// you should still avoid non-ASCII characters).
#[derive(Default, Debug, Clone)]
pub struct HeaderMap {
    encoding: HeaderEncoding,
    status_line: Option<Vec<u8>>,
    headers: Vec<(Vec<u8>, Vec<u8>)>,
}

impl HeaderMap {
    /// Create a new header map with the specified encoding.
    ///
    /// # Arguments
    ///
    /// * `encoding` - Header source encoding
    pub fn new(encoding: HeaderEncoding) -> Self {
        HeaderMap {
            encoding,
            status_line: None,
            headers: Vec::new(),
        }
    }

    /// Get the header encoding.
    pub fn encoding(&self) -> HeaderEncoding {
        self.encoding.clone()
    }

    /// Encode a header value as either UTF-8 or Latin1.
    /// Decoding is lossy. Invalid characters are replaced.
    ///
    /// # Arguments
    ///
    /// * `s` - Byte sequence to decode
    fn _encode<'a>(&self, s: &'a str) -> Cow<'a, [u8]> {
        match &self.encoding {
            HeaderEncoding::Unicode => Cow::Borrowed(s.as_bytes()),
            HeaderEncoding::Latin1 => Cow::Owned(WINDOWS_1252.encode(s, EncoderTrap::Ignore).unwrap_or_default()),
        }
    }

    /// Decode a header value as either Unicode or Latin1.
    /// Decoding is lossy. Invalid characters are replaced.
    ///
    /// # Arguments
    ///
    /// * `b` - Byte sequence to decode
    fn _decode<'a>(&self, b: &'a [u8]) -> Cow<'a, str> {
        match &self.encoding {
            HeaderEncoding::Unicode => String::from_utf8_lossy(b),
            HeaderEncoding::Latin1 => Cow::Owned(WINDOWS_1252.decode(b, DecoderTrap::Ignore).unwrap_or_default()),
        }
    }

    /// Get the header status line.
    pub fn status_line(&self) -> Option<Cow<'_, str>> {
        self.status_line.as_deref().map(|s| self._decode(s))
    }

    /// Get the raw status line as bytes.
    pub fn status_line_bytes(&self) -> Option<&[u8]> {
        self.status_line.as_deref()
    }

    /// Set status line contents.
    ///
    /// # Arguments
    ///
    /// * `status_line` - New status line
    pub fn set_status_line(&mut self, status_line: &[u8]) {
        let mut status_line_sanitized = Vec::with_capacity(status_line.len());
        status_line_sanitized.extend(_sanitize_header_value(status_line, true));
        self.status_line = Some(status_line_sanitized);
    }

    /// HTTP status code (unset if header block is not an HTTP header block).
    pub fn status_code(&self) -> Option<u16> {
        let Some(s) = &self.status_line else {
            return None;
        };
        if !s.starts_with(b"HTTP/") {
            return None;
        }
        let mut parts = s.splitn(3, |&b| b == b' ');
        // Skip HTTP/
        parts.next()?;
        self._decode(parts.next()?).parse::<u16>().ok()
    }

    /// HTTP reason phrase.
    /// Returns None if the header block is not an HTTP header block or no reason phrase was given.
    pub fn reason_phrase(&self) -> Option<Cow<'_, str>> {
        let Some(s) = &self.status_line else {
            return None;
        };
        if !s.starts_with(b"HTTP/") {
            return None;
        }
        let mut parts = s.splitn(3, |&b| b == b' ');
        // Skip HTTP/ and status code
        parts.next()?;
        parts.next()?;
        Some(self._decode(parts.next()?))
    }

    /// Get value for a (case-insensitive) header key.
    /// If the header is present multiple times, only the first occurrence is returned.
    /// Use [`Self::get_multiple()`] if you want all values.
    ///
    /// Returns `None` if the header is not present.
    ///
    /// # Arguments
    ///
    /// * `key` - Header key
    pub fn get(&self, key: impl AsRef<str>) -> Option<Cow<'_, str>> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key.as_ref().as_bytes()))
            .map(|(_, v)| self._decode(v.as_slice()))
    }

    /// Get all values for a (case-insensitive) header key.
    /// Returns a vector of all values for the given key. Can return more than
    /// one element if the header is present multiple times.
    ///
    /// # Arguments
    ///
    /// * `key` - Header key
    pub fn get_multiple(&self, key: impl AsRef<str>) -> Vec<Cow<'_, str>> {
        self.headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case(key.as_ref().as_bytes()))
            .map(|(_, v)| self._decode(v.as_slice()))
            .collect()
    }

    /// Get byte value for a (case-insensitive) header key.
    /// If the header is present multiple times, only the first occurrence is returned.
    /// Use [`Self::get_bytes_multiple()`] if you want all values.
    ///
    /// Returns `None` if the header is not present.
    ///
    /// # Arguments
    ///
    /// * `key` - Header key
    pub fn get_bytes(&self, key: &[u8]) -> Option<Cow<'_, [u8]>> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| Cow::Borrowed(v.as_slice()))
    }

    /// Get all byte values for a (case-insensitive) header key.
    /// Returns a vector of all values for the given key. Can return more than
    /// one element if the header is present multiple times.
    ///
    /// # Arguments
    ///
    /// * `key` - Header key as bytes
    pub fn get_bytes_multiple(&self, key: &[u8]) -> Vec<Cow<'_, [u8]>> {
        self.headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| Cow::Borrowed(v.as_slice()))
            .collect()
    }

    /// Check if a (case-insensitive) header key exists.
    ///
    /// # Arguments
    ///
    /// * `key` - Header key as bytes
    pub fn contains_key(&self, key: &str) -> bool {
        let key_bytes = key.as_bytes();
        self.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(key_bytes))
    }

    /// Insert a new header and overwrite any existing header(s) if the key already exists.
    ///
    /// Insertion is not efficient and causes a full traversal of all headers.
    /// If a header already exists, its first occurrence will be updated and
    /// all following occurrences will be dropped. If duplicate headers are not a problem,
    /// use [`Self::append()`] instead for better efficiency.
    ///
    /// All data is represented internally as bytes to avoid encoding/decoding overhead
    /// and potential errors. Therefore, if you have data as bytes already, using
    /// [`Self::set_bytes()`] is slightly more efficient.
    ///
    /// # Arguments
    ///
    /// * `key` - Header key
    /// * `value` - Header value
    pub fn set(&mut self, key: impl AsRef<str>, value: impl AsRef<str>) {
        let key = self._encode(key.as_ref());
        let value = self._encode(value.as_ref());
        self.set_bytes(key.as_ref(), value.as_ref());
    }

    /// Insert a new header and overwrite any existing header(s) if the key already exists.
    ///
    /// Insertion is not efficient and causes a full traversal of all headers.
    /// If a header already exists, its first occurrence will be updated and
    /// all following occurrences will be dropped. If duplicate headers are not a problem,
    /// use [`Self::append_bytes()`] instead for better efficiency.
    ///
    /// # Arguments
    ///
    /// * `key` - Header key as bytes
    /// * `value` - Header value as bytes
    pub fn set_bytes(&mut self, key: &[u8], value: &[u8]) {
        let mut key_lower = Vec::with_capacity(key.len());
        key_lower.extend(_sanitize_header_value(&key.to_ascii_lowercase(), true));

        let mut found = false;
        self.headers.retain_mut(|h| {
            if h.0.to_ascii_lowercase() != key_lower {
                true
            } else if !found {
                *h = (_sanitize_header_value(key, true), _sanitize_header_value(value, false));
                found = true;
                true
            } else {
                false
            }
        });
        if !found {
            self.headers
                .push((_sanitize_header_value(key, true), _sanitize_header_value(value, false)));
        }
    }

    /// Append a header.
    ///
    /// Appending a new header is efficient and does not check for existing headers
    /// with the same name.
    ///
    /// All data is represented internally as bytes to avoid encoding/decoding overhead
    /// and potential errors. Therefore, if you have data as bytes already, using
    /// [`Self::append_bytes()`] is slightly more efficient.
    ///
    /// # Arguments
    ///
    /// * `key` - Header key
    /// * `value` - Header value
    pub fn append(&mut self, key: impl AsRef<str>, value: impl AsRef<str>) {
        let key = self._encode(key.as_ref());
        let value = self._encode(value.as_ref());
        self.append_bytes(key.as_ref(), value.as_ref());
    }

    /// Append a header.
    ///
    /// Appending a new header is efficient and does not check for existing headers
    /// with the same name.
    ///
    /// # Arguments
    ///
    /// * `key` - Header key as bytes
    /// * `value` - Header value as bytes
    pub fn append_bytes(&mut self, key: &[u8], value: &[u8]) {
        self.headers
            .push((_sanitize_header_value(key, true), _sanitize_header_value(value, false)));
    }

    /// Internal function for appending a header without sanitization
    /// (assumes data is already sanitized). Still trims leading and trailing white space.
    fn _append_bytes_no_sanitize(&mut self, key: &[u8], value: &[u8]) {
        self.headers
            .push((key.trim_ascii().to_vec(), value.trim_ascii().to_vec()));
    }

    /// Internal helper for adding a continuation line to the last-appended header.
    /// No value sanitization is performed, but white space is trimmed.
    fn _add_continuation_bytes(&mut self, value: &[u8]) {
        let trimmed = value.trim_ascii();
        if let Some(last) = self.headers.last_mut() {
            last.1.reserve(trimmed.len() + 1);
            last.1.push(b' ');
            last.1.extend_from_slice(trimmed);
        } else {
            self.headers.push((Vec::new(), trimmed.to_vec()));
        }
    }

    /// Remove a header if it exists.
    pub fn remove(&mut self, key: impl AsRef<str>) {
        let key = self._encode(key.as_ref());
        self.remove_bytes(key.as_ref());
    }

    /// Remove a header if it exists.
    pub fn remove_bytes(&mut self, key: &[u8]) {
        let key = _sanitize_header_value(key, true);
        self.headers.retain(|(k, _)| !k.eq_ignore_ascii_case(key.as_slice()));
    }

    /// Iterator of keys and values.
    pub fn items(&self) -> impl Iterator<Item = (Cow<'_, str>, Cow<'_, str>)> {
        self.headers.iter().map(|(k, v)| (self._decode(k), self._decode(v)))
    }

    /// Zero-copy iterator of keys and values as bytes.
    pub fn items_bytes(&self) -> impl Iterator<Item = &(Vec<u8>, Vec<u8>)> {
        self.headers.iter()
    }

    /// Iterator of header keys.
    pub fn keys(&self) -> impl Iterator<Item = Cow<'_, str>> {
        self.headers.iter().map(|(k, _)| self._decode(k))
    }

    /// Zero-copy iterator of header keys as bytes.
    pub fn keys_bytes(&self) -> impl Iterator<Item = &Vec<u8>> {
        self.headers.iter().map(|(k, _)| k)
    }

    /// Iterator of header values.
    pub fn values(&self) -> impl Iterator<Item = Cow<'_, str>> {
        self.headers.iter().map(|(_, v)| self._decode(v))
    }

    /// Zero-copy iterator of header values as bytes.
    pub fn values_bytes(&self) -> impl Iterator<Item = &Vec<u8>> {
        self.headers.iter().map(|(_, v)| v)
    }

    /// Return the headers as a [`HashMap`] of Unicode strings.
    ///
    /// If multiple headers have the same key, their values will be concatenated with `","`.
    pub fn to_map(&self) -> HashMap<CaseInsensitiveKey, String> {
        let mut map: HashMap<CaseInsensitiveKey, String> = HashMap::new();
        self.items().for_each(|(k, v)| {
            map.entry(CaseInsensitiveKey::new(k))
                .and_modify(|v_| {
                    v_.push(',');
                    v_.push_str(&v);
                })
                .or_insert(v.to_string());
        });
        map
    }

    /// Get the number of headers.
    pub fn len(&self) -> usize {
        self.headers.len()
    }

    /// Check if the header map is empty.
    pub fn is_empty(&self) -> bool {
        self.headers.is_empty()
    }

    /// Clear all headers and the status line.
    pub fn clear(&mut self) {
        self.headers.clear();
        self.status_line = None;
    }

    /// Write the header block onto a stream.
    pub fn write<W: io::Write>(&self, writer: &mut W) -> io::Result<usize> {
        let mut bytes_written = 0usize;
        if let Some(s) = &self.status_line
            && !s.is_empty()
        {
            writer.write_all(s)?;
            writer.write_all(b"\r\n")?;
            bytes_written += s.len() + 2;
        }
        for (key, value) in &self.headers {
            if !key.is_empty() {
                writer.write_all(key)?;
                writer.write_all(b": ")?;
                bytes_written += key.len() + 2;
            }
            writer.write_all(value)?;
            writer.write_all(b"\r\n")?;
            bytes_written += value.len() + 2;
        }
        // Header end
        writer.write_all(b"\r\n")?;
        bytes_written += 2;

        Ok(bytes_written)
    }
}

// ===========================================================
// WARC record
// ===========================================================

/// A WARC record.
///
/// WARC records are cloneable, but cloning will "freeze" the WARC record.
#[derive(Default)]
pub struct WarcRecord {
    record_type: WarcRecordType,
    headers: HeaderMap,
    content_length: usize,
    is_http: bool,
    http_parsed: bool,
    http_charset: Option<String>,
    http_headers: Option<HeaderMap>,
    reader: Option<LimitedBufReadSeek>,
    reader_original: Option<Box<dyn BufReadSeek>>,
    stream_pos: usize,
    frozen: bool,
}

#[derive(Debug, Clone)]
pub enum DigestError {
    Missing(String),
    Unsupported(String),
    FormatError(String),
    NoPayload(String),
    StreamError(String),
}

impl fmt::Debug for WarcRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dbg = f.debug_struct("WarcRecord");
        let mut fields = dbg
            .field("record_type", &self.record_type)
            .field("headers", &self.headers)
            .field("content_length", &self.content_length)
            .field("is_http", &self.is_http);
        if self.is_http {
            fields = fields
                .field("http_charset", &self.http_charset)
                .field("http_headers", &self.http_headers)
        }
        fields.finish_non_exhaustive()
    }
}

impl WarcRecord {
    /// Create a new empty WARC record.
    ///
    /// The new WARC record will have an empty [`HeaderMap`] and no payload.
    /// Before use, a [`WarcRecord`] must be initialized with either [`Self::attach_reader()`]
    /// or [`Self::set_bytes_payload()`]. Otherwise, operations relying on a
    /// existing payload will fail. Default headers can be initialized with
    /// [`Self::init_headers()`].
    pub fn new() -> Self {
        WarcRecord {
            record_type: WarcRecordType::NoType,
            headers: HeaderMap::new(HeaderEncoding::Unicode),
            is_http: false,
            http_parsed: false,
            http_charset: None,
            http_headers: None,
            content_length: 0,
            reader: None,
            reader_original: None,
            stream_pos: 0,
            frozen: false,
        }
    }

    /// Create a new WARC record instance from a buffered reader.
    /// The new instance is fully initialized with all headers present.
    ///
    /// Takes ownership of the reader instance until either [`Self::detach_reader()`]
    /// or [`Self::freeze()`] are called. This is the same as constructing a
    /// new empty record instance with [`Self::new()`] and then calling
    /// [`Self::attach_reader()`] and [`Self::parse_warc_headers()`].
    ///
    /// # Arguments
    ///
    /// * `reader` - Buffered reader instance
    pub fn from_reader(reader: Box<dyn BufReadSeek>) -> Result<Self, io::Error> {
        let mut record = WarcRecord::new();
        record.attach_reader(reader);
        record.parse_warc_headers()?;
        Ok(record)
    }

    /// Create a new frozen WARC record instance from a byte buffer.
    /// The new instance is fully initialized with all headers present.
    ///
    /// # Arguments
    ///
    /// * `payload` - Body as bytes
    pub fn from_bytes(payload: Vec<u8>) -> Result<Self, io::Error> {
        let reader = Box::new(io::BufReader::new(io::Cursor::new(payload)));
        let mut record = WarcRecord::from_reader(reader)?;
        record.frozen = true;
        Ok(record)
    }

    /// Attach a buffered reader to this [`WarcRecord`] instance.
    ///
    /// A [`WarcRecord`] must be initialized with either [`Self::attach_reader()`]
    /// or [`Self::set_bytes_payload()`]. Otherwise, operations relying on an
    /// existing payload will fail.
    ///
    /// Takes ownership of the reader instance until either [`Self::detach_reader()`]
    /// or [`Self::freeze()`] are called.
    ///
    /// # Arguments
    ///
    /// * `reader` - Shared pointer to a buffered reader instance
    pub fn attach_reader(&mut self, reader: Box<dyn BufReadSeek>) {
        self.reader = Some(LimitedBufReadSeek::new(reader, None));
        self.frozen = false;
    }

    /// Set the WARC payload as bytes.
    ///
    /// Replaces the currently attached reader instance with a byte buffer reader
    /// and marks the record as frozen.
    ///
    /// A [`WarcRecord`] must be initialized with either [`Self::attach_reader()`]
    /// or [`Self::set_bytes_payload()`]. Otherwise, operations relying on an
    /// existing payload will fail.
    ///
    /// # Arguments
    ///
    /// * `payload` - Body as bytes
    pub fn set_bytes_payload(&mut self, payload: Vec<u8>) {
        self.content_length = payload.len();
        let reader = Box::new(io::BufReader::new(io::Cursor::new(payload)));
        self.reader = Some(LimitedBufReadSeek::new(reader, Some(self.content_length)));
        self.headers
            .set_bytes(b"Content-Length", self.content_length.to_string().as_bytes());
        self.frozen = true;
    }

    /// Detach an attached buffered reader and hand ownership back to the caller.
    ///
    /// # Returns
    ///
    /// Reader instance or `None`
    pub fn detach_reader(&mut self) -> Option<Box<dyn BufReadSeek>> {
        self.frozen = false;
        if self.reader_original.is_some() {
            self.reader_original.take()
        } else if self.reader.is_some() {
            Some(self.reader.take().unwrap().reader)
        } else {
            None
        }
    }

    /// Get a mutable reference to the attached buffered reader.
    pub fn reader_mut(&mut self) -> Option<&mut LimitedBufReadSeek> {
        self.reader.as_mut()
    }

    /// Record type (same as `headers['WARC-Type']`).
    pub fn record_type(&self) -> WarcRecordType {
        self.record_type
    }

    /// Set the WARC record type.
    ///
    /// # Arguments
    ///
    /// * `record_type` - Record type
    pub fn set_record_type(&mut self, record_type: WarcRecordType) {
        self.record_type = record_type;
        self.headers.set_bytes(b"WARC-Type", record_type.as_str().as_bytes());
    }

    /// "Freeze" a record by baking in the remaining payload stream contents and return
    /// ownership of the attached reader instance.
    ///
    /// Freezing a record makes the [`WarcRecord`] instance copyable and reusable by decoupling
    /// it from the underlying raw WARC stream. Instead of reading directly from the raw stream, a
    /// frozen record maintains an internal buffer the size of the remaining payload stream contents
    /// at the time of calling [`Self::freeze()`].
    ///
    /// Freezing a record will advance the attached stream.
    ///
    /// It is safe to call this function multiple times, which is a no-op. However, subsequent
    /// calls will not return a previous reader instance anymore. A frozen instance created with
    /// [`Self::from_bytes()`] or with [`Self::set_bytes_payload()`] has no previous reader and
    /// will always return `None`.
    pub fn freeze(&mut self) -> Result<Option<Box<dyn BufReadSeek>>, io::Error> {
        if self.frozen {
            return Ok(self.reader_original.take());
        }
        self._freeze_internal()?;
        Ok(self.reader_original.take())
    }

    /// Internal [`Self::freeze()`] implementation.
    /// Stores the original reader in `self.reader_original` instead of returning ownership.
    fn _freeze_internal(&mut self) -> Result<(), io::Error> {
        if self.frozen {
            return Ok(());
        }
        let reader = self.reader.as_mut().ok_or_else(|| io::Error::other("No reader set"))?;
        let mut buf = Vec::with_capacity(self.content_length);
        self.content_length = reader.read_to_end(&mut buf)?;
        let new_reader = Box::new(io::BufReader::new(io::Cursor::new(buf)));
        self.reader_original = Some(reader.replace_reader(new_reader));
        self.frozen = true;
        Ok(())
    }

    /// Start parsing the WARC record header block. Requires a stream to be set.
    ///
    /// The parser will skip over any number of empty lines before the next valid
    /// `WARC/*` header line. Any other content that is not a valid WARC header
    /// start will return an error of type [`io::ErrorKind::InvalidData`].
    ///
    /// Parsing the WARC headers will automatically limit the attached reader to the
    /// remaining `Content-Length` bytes. The original reader instance remains unaffected.
    pub fn parse_warc_headers(&mut self) -> Result<(), io::Error> {
        self.parse_warc_headers_quirks(false)
    }

    /// Start parsing the WARC record header block. Requires a stream to be set.
    ///
    /// The parser will skip over any number of empty lines before the next valid
    /// `WARC/*` header line. If `quirks_mode == true`, any other invalid lines
    /// encountered before the next header start will be skipped as well.
    /// Otherwise, an error of type [`io::ErrorKind::InvalidData`] is returned.
    ///
    /// Parsing the WARC headers will automatically limit the attached reader to the
    /// remaining `Content-Length` bytes. The original reader instance remains unaffected.
    ///
    /// # Arguments
    ///
    /// * `quirks_mode` - Whether to skip non-empty lines before header start
    pub fn parse_warc_headers_quirks(&mut self, quirks_mode: bool) -> Result<(), io::Error> {
        let reader = self.reader.as_mut().ok_or_else(|| io::Error::other("No reader set"))?;
        let mut headers = HeaderMap::new(HeaderEncoding::Unicode);
        let mut line = Vec::with_capacity(32);
        loop {
            line.clear();

            // Try to find first WARC/* header
            self.stream_pos = reader.real_stream_position()? as usize;
            let n = reader.read_until(b'\n', &mut line)?;
            if n == 0 {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Stream ended before WARC header"));
            }

            // Trim ASCII whitespace (including CR/LF line endings)
            let trimmed = line.trim_ascii();
            if trimmed.is_empty() {
                // Skip empty lines (non-standard)
                continue;
            }

            // WARC/1.x header
            if matches!(trimmed, b"WARC/1.1" | b"WARC/1.0")
                // ClueWeb09/12 legacy
                || (trimmed.starts_with(b"WARC/0.") && trimmed.len() <= 9)
            {
                headers.status_line = Some(trimmed.to_owned());
                break;
            } else if !quirks_mode {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid WARC header"));
            } else {
                // Quirks mode, keep trying to find a valid WARC header
            }
        }

        self._parse_header_block(&mut headers, false)?;
        self.headers = headers;

        let mut parse_count = 0;
        for (k, v) in self.headers.items_bytes() {
            if k == b"WARC-Type" {
                self.record_type = WarcRecordType::try_from(v.as_slice()).unwrap_or(WarcRecordType::Unknown);
                parse_count += 1;
            } else if k == b"Content-Type" {
                self.is_http = v == b"application/http" || v.starts_with(b"application/http;");
                parse_count += 1;
            } else if k == b"Content-Length" {
                self.content_length = str::from_utf8(v).unwrap_or_default().parse().unwrap_or(0);
                parse_count += 1;
            }
            if parse_count == 3 {
                break;
            }
        }

        // // Replace reader with limited version
        // let limited_reader = LimitedBufReadSeek::new(self.reader.clone().unwrap(), self.content_length);
        // self.reader = Some(Rc::new(RefCell::new(limited_reader)));
        self.reader.as_mut().unwrap().set_limit(self.content_length);
        Ok(())
    }

    /// Record ID (same as [`Self::headers().get("WARC-Record-ID")`](HeaderMap::get)
    pub fn record_id(&self) -> Option<Cow<'_, str>> {
        self.headers.get("WARC-Record-ID")
    }

    /// WARC record headers.
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// WARC record headers (mutable).
    pub fn headers_mut(&mut self) -> &mut HeaderMap {
        &mut self.headers
    }

    /// Whether record is an HTTP record.
    pub fn is_http(&self) -> bool {
        self.is_http
    }

    /// Set whether this record is an HTTP record.
    /// Modifying this property will also affect the `Content-Type` of this record.
    pub fn set_is_http(&mut self, is_http: bool) {
        self.is_http = is_http;
        if self.is_http {
            self.headers.set_bytes(
                b"Content-Type",
                match self.record_type {
                    WarcRecordType::Request => b"application/http; msgtype=request",
                    WarcRecordType::Response => b"application/http; msgtype=response",
                    _ => b"application/http",
                },
            );
        }
    }

    /// Whether HTTP headers have been parsed.
    pub fn is_http_parsed(&self) -> bool {
        self.http_parsed
    }

    /// HTTP headers if record is an HTTP record and HTTP headers have been parsed yet.
    pub fn http_headers(&self) -> Option<&HeaderMap> {
        self.http_headers.as_ref()
    }

    /// Plain HTTP Content-Type without additional fields such as `charset=`.
    pub fn http_content_type(&self) -> Option<String> {
        if !self.http_parsed {
            return None;
        }
        self.http_headers
            .as_ref()?
            .get("Content-Type")?
            .split(";")
            .next()
            .map(|s| s.trim().to_string())
    }

    /// HTTP charset/encoding as returned by the server or `None` if no valid charset is set.
    ///
    /// A returned string is guaranteed to be a valid encoding name.
    pub fn http_charset(&self) -> Option<&str> {
        self.http_charset.as_deref()
    }

    /// Remaining WARC record length in bytes (not necessarily the same as the `Content-Length` header).
    pub fn content_length(&self) -> usize {
        self.content_length
    }

    /// WARC record start offset in the original (uncompressed) stream.
    pub fn stream_pos(&self) -> usize {
        self.stream_pos
    }

    /// Whether the record has been frozen.
    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    /// Initialize mandatory headers in a fresh WARC record instance.
    ///
    /// # Arguments
    ///
    /// * `content_length` - WARC record body length in bytes
    /// * `record_type` - WARC-Type
    /// * `record_urn` - WARC-Record-ID as URN without `'<'`, `'>'` (if unset, a random URN will be generated)
    pub fn init_headers(
        &mut self,
        content_length: usize,
        record_type: Option<WarcRecordType>,
        record_urn: Option<&[u8]>,
    ) {
        let urn = match record_urn {
            Some(urn) => urn.to_vec(),
            None => format!("urn:uuid:{}", Uuid::new_v4()).into_bytes(),
        };

        self.record_type = match record_type {
            Some(WarcRecordType::AnyType) | Some(WarcRecordType::NoType) => WarcRecordType::Unknown,
            Some(record_type) => record_type,
            _ => WarcRecordType::NoType,
        };

        self.headers.clear();
        self.headers.set_status_line(b"WARC/1.1");
        self.headers
            ._append_bytes_no_sanitize(b"WARC-Type", self.record_type.as_str().as_bytes());

        let date = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        self.headers._append_bytes_no_sanitize(b"WARC-Date", date.as_bytes());

        let record_id = format!("<{}>", String::from_utf8_lossy(&urn));
        self.headers
            ._append_bytes_no_sanitize(b"WARC-Record-ID", record_id.as_bytes());

        self.headers
            ._append_bytes_no_sanitize(b"Content-Length", content_length.to_string().as_bytes());
        self.content_length = content_length;
    }

    /// Internal helper for parsing a header block from a buffered reader.
    ///
    /// Can be used to parse both WARC and HTTP header blocks.
    ///
    /// # Arguments
    ///
    /// * `target` - Header map to fill
    /// * `has_status_line` - Whether the first line is a status line or already a header
    fn _parse_header_block(&mut self, target: &mut HeaderMap, has_status_line: bool) -> Result<usize, io::Error> {
        let mut bytes_consumed = 0;
        let mut line = Vec::new();
        let mut expect_first_line = has_status_line;
        let reader = self.reader.as_mut().ok_or_else(|| io::Error::other("No reader set"))?;

        loop {
            line.clear();
            let n = reader.read_until(b'\n', &mut line)?;
            if n == 0 {
                break;
            }
            bytes_consumed += n;

            // Trim (CR)LF line endings
            let trimmed = line
                .strip_suffix(b"\r\n")
                .or_else(|| line.strip_suffix(b"\n"))
                .unwrap_or(&line);

            // End of header
            if trimmed.is_empty() {
                break;
            }

            // Status line
            if expect_first_line {
                target.set_status_line(trimmed);
                expect_first_line = false;
                continue;
            }

            if matches!(trimmed.first(), Some(b' ' | b'\t')) {
                target._add_continuation_bytes(trimmed);
                continue;
            }

            // Parse header line
            if let Some(colon_pos) = trimmed.iter().position(|&c| c == b':') {
                let value = if colon_pos + 1 < trimmed.len() {
                    &trimmed[colon_pos + 1..]
                } else {
                    b""
                };
                target._append_bytes_no_sanitize(&trimmed[..colon_pos], value);
            } else {
                // Invalid header, try to preserve it
                target._add_continuation_bytes(trimmed);
            }
        }

        Ok(bytes_consumed)
    }

    /// Parse HTTP headers and advance content reader.
    ///
    /// It is safe to call this method multiple times, even if the record is not an HTTP record.
    pub fn parse_http(&mut self) -> Result<(), io::Error> {
        if self.http_parsed || !self.is_http {
            return Ok(());
        }

        let mut http_headers = HeaderMap::new(HeaderEncoding::Latin1);
        let bytes_consumed = self._parse_header_block(&mut http_headers, true)?;

        // Parse charset if present
        if let Some(content_type) = http_headers.get("content-type").map(|c| c.to_ascii_lowercase()) {
            let charset_key = "charset=";
            if let Some(charset_pos) = content_type.find(charset_key) {
                let charset_start = charset_pos + charset_key.len();
                self.http_charset = content_type[charset_start..]
                    .split(';')
                    .next()
                    .map(|c| c.trim_ascii().to_owned());
            }
        }

        // Update content to skip HTTP headers
        self.content_length -= bytes_consumed;
        self.http_headers = Some(http_headers);
        self.http_parsed = true;
        Ok(())
    }
    /// Write WARC record onto a stream.
    ///
    /// The default block size is 16384 bytes and no record checksums are calculated.
    /// Use [`Self::write_with_block_size`] or [`Self::write_with_checksum_block_size`] for more control.
    ///
    /// # Arguments
    ///
    /// * `writer` - Output stream
    ///
    /// # Returns
    ///
    /// Number of bytes written
    pub fn write<W: io::Write>(&mut self, writer: &mut W) -> io::Result<usize> {
        self.write_with_checksum_block_size(writer, false, 16384)
    }

    /// Write WARC record onto a stream with a given block size.
    ///
    /// By default, no record checksums are calculated. Use [`Self::write_with_checksum_block_size`] or
    /// `write_with_checksum_block_size` for more control.
    ///
    /// # Arguments
    ///
    /// * `writer` - Output stream
    /// * `block_size` - Block size for writing the record body
    ///
    /// # Returns
    ///
    /// Number of bytes written
    pub fn write_with_block_size<W: io::Write>(&mut self, writer: &mut W, block_size: usize) -> io::Result<usize> {
        self.write_with_checksum_block_size(writer, false, block_size)
    }

    /// Write WARC record onto a stream and calculate SHA-1 record checksums.
    ///
    /// The default block size is 16384 bytes, and SHA-1 checksums are calculated for the
    /// block and payload data (if available). Use [`Self::write_with_checksum_block_size`]
    /// for more control.
    ///
    /// # Arguments
    ///
    /// * `writer` - Output stream
    ///
    /// # Returns
    ///
    /// Number of bytes written
    pub fn write_with_checksum<W: io::Write>(&mut self, writer: &mut W) -> io::Result<usize> {
        self.write_with_checksum_block_size(writer, true, 16384)
    }

    /// Write WARC record onto a stream.
    ///
    /// # Arguments
    ///
    /// * `writer` - Output stream
    /// * `checksum_data` - Whether to write data checksums
    /// * `chunk_size` - Chunk size for writing the record body
    ///
    /// # Returns
    ///
    /// Number of bytes written
    pub fn write_with_checksum_block_size<W: io::Write>(
        &mut self,
        writer: &mut W,
        checksum_data: bool,
        chunk_size: usize,
    ) -> io::Result<usize> {
        let mut bytes_written = 0usize;

        if checksum_data {
            self._freeze_internal()?;
            let reader = self.reader.as_mut().ok_or_else(|| io::Error::other("No reader set"))?;

            use data_encoding::BASE32;
            use sha1::Sha1;
            let mut block_digest = Sha1::new();

            if self.http_parsed
                && let Some(h) = &self.http_headers
            {
                let mut buf = Vec::with_capacity(512);
                h.write(&mut buf)?;

                let mut payload_digest = Sha1::new();
                Digest::update(&mut block_digest, &buf);
                Digest::update(&mut payload_digest, &buf);
                self.headers
                    .set_bytes(b"WARC-Payload-Digest", BASE32.encode(&payload_digest.finalize()).as_bytes());
            }

            loop {
                let mut buf = [0u8; 4096];
                let n = reader.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                Digest::update(&mut block_digest, buf);
            }
            self.headers
                .set_bytes(b"WARC-Block-Digest", BASE32.encode(&block_digest.finalize()).as_bytes());
            reader.rewind()?;
        }

        let reader = self.reader.as_mut().ok_or_else(|| io::Error::other("No reader set"))?;

        // Ensure Content-Length is correct
        self.headers
            .set_bytes(b"Content-Length", self.content_length.to_string().as_bytes());

        // TODO: Start and end members on compressing stream

        // Write WARC headers
        bytes_written += self.headers.write(writer)?;
        bytes_written += 2;

        // Write HTTP headers if parsed
        if self.http_parsed
            && let Some(ref http_headers) = self.http_headers
        {
            bytes_written += http_headers.write(writer)?;
        }

        // Write content
        let mut buf = Vec::with_capacity(chunk_size);
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            writer.write_all(&buf[..n])?;
            bytes_written += n;
        }

        // Write record separator
        writer.write_all(b"\r\n\r\n")?;
        bytes_written += 4;

        Ok(bytes_written)
    }

    /// Verify whether the record's `WARC-Block-Digest` is valid.
    ///
    /// Returns a boolean whether the digest matches or an error if no `WARC-Block-Digest`
    /// exists, if the record has an unsupported digest type, or on failure
    /// to read the digest header value or stream.
    ///
    /// # Arguments
    ///
    /// * `consume` - Do not create an in-memory copy of the record stream
    ///   (will consume the rest of the record)
    pub fn verify_block_digest(&mut self, consume: bool) -> Result<bool, DigestError> {
        let digest = self
            .headers
            .get("WARC-Block-Digest")
            .ok_or_else(|| DigestError::Missing("Missing WARC-Block-Digest header".into()))?
            .to_string();
        self._verify_digest(&digest, consume)
    }

    /// Verify whether the record's `WARC-Payload-Digest` is valid.
    ///
    /// Returns a boolean whether the digest matches or an error if no `WARC-Payload-Digest`
    /// exists, if the record has an unsupported digest type, or on failure
    /// to read the digest header value or stream.
    ///
    /// # Arguments
    ///
    /// * `consume` - Do not create an in-memory copy of the record stream
    ///   (will consume the rest of the record)
    pub fn verify_payload_digest(&mut self, consume: bool) -> Result<bool, DigestError> {
        if !self.http_parsed || !self.is_http {
            return Err(DigestError::NoPayload("HTTP payload not parsed or missing".into()));
        }

        let digest = self
            .headers
            .get("WARC-Payload-Digest")
            .ok_or_else(|| DigestError::Missing("Missing WARC-Payload-Digest header".into()))?
            .to_string();
        self._verify_digest(&digest, consume)
    }

    /// Internal helper for verifying digests.
    fn _verify_digest(&mut self, digest_str: &str, consume: bool) -> Result<bool, DigestError> {
        let parts: Vec<&str> = digest_str.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(DigestError::FormatError("Invalid digest header formatting (':' not found)".into()));
        }
        let algorithm = parts[0].to_ascii_lowercase();
        let expected_digest = parts[1].trim_ascii().as_bytes();

        use data_encoding::{BASE32, HEXLOWER_PERMISSIVE};
        let expected_digest = match BASE32.decode(expected_digest) {
            Ok(bytes) => bytes,
            // Hex digests are non-standard, but are created by some libraries such as warcprox
            Err(_) => match HEXLOWER_PERMISSIVE.decode(expected_digest) {
                Ok(bytes) => bytes,
                Err(_) => {
                    return Err(DigestError::FormatError("Invalid digest encoding".into()));
                }
            },
        };

        if !consume && !self.frozen {
            self._freeze_internal()
                .map_err(|e| DigestError::StreamError(format!("Failed to freeze record: {}", e)))?;
        }

        let reader = self
            .reader
            .as_mut()
            .ok_or_else(|| DigestError::StreamError("No reader set".into()))?;

        let mut digest = _get_digest(&algorithm)?;
        let mut buf = [0u8; 4096];
        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| DigestError::StreamError(format!("Failed to read stream: {}", e)))?;
            if n == 0 {
                break;
            }
            digest.update(&buf[..n]);
        }
        if !consume {
            reader
                .seek(io::SeekFrom::Start(self.stream_pos as u64))
                .map_err(|e| DigestError::StreamError(format!("Failed to seek stream: {}", e)))?;
        }

        Ok(digest.finalize().to_vec() == expected_digest)
    }
}

// ===========================================================
// Helper functions
// ===========================================================

/// Internal helper for constructing a digest instance.
fn _get_digest(algorithm: &str) -> Result<Box<dyn DynDigest>, DigestError> {
    match algorithm.to_ascii_lowercase().as_str() {
        "md5" => {
            use md5::Md5;
            Ok(Box::new(Md5::new()))
        }
        "sha1" => {
            use sha1::Sha1;
            Ok(Box::new(Sha1::new()))
        }
        "sha256" => {
            use sha2::Sha256;
            Ok(Box::new(Sha256::new()))
        }
        "sha512" => {
            use sha2::Sha512;
            Ok(Box::new(Sha512::new()))
        }
        _ => Err(DigestError::Unsupported(format!("Unsupported hash algorithm: {}", algorithm))),
    }
}

/// Internal helper for trimming leading and trailing white space and
/// removing internal CR and LF characters. Also strips `':'` if `is_key == true`.
fn _sanitize_header_value(value: &[u8], is_key: bool) -> Vec<u8> {
    let mut value_sanitized = Vec::with_capacity(value.len());
    value_sanitized.extend(value.trim_ascii().iter().flat_map(|b| match b {
        b'\r' | b'\n' => Some(b' '),
        b':' if is_key => None,
        other => Some(*other),
    }));
    value_sanitized
}

// ===========================================================
// Tests
// ===========================================================

#[cfg(test)]
#[path = "record_test.rs"]
mod record_test;
