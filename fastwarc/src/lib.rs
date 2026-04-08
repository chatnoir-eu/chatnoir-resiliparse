// Copyright 2025 Janek Bevendorff
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

use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::fmt;
use std::rc::Rc;
use encoding::{Encoding, DecoderTrap};
use encoding::all::WINDOWS_1252;
use md5::Digest;
use uuid::Uuid;


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

impl TryFrom<&str> for WarcRecordType {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "warcinfo" => Ok(WarcRecordType::WarcInfo),
            "response" => Ok(WarcRecordType::Response),
            "resource" =>Ok( WarcRecordType::Resource),
            "request" => Ok(WarcRecordType::Request),
            "metadata" => Ok(WarcRecordType::Metadata),
            "revisit" => Ok(WarcRecordType::Revisit),
            "conversion" => Ok(WarcRecordType::Conversion),
            "continuation" => Ok(WarcRecordType::Continuation),
            "unknown" => Ok(WarcRecordType::Unknown),
            _ => Err("Invalid enum value."),
        }
    }
}


impl From<WarcRecordType> for &'static str {
    fn from(value: WarcRecordType) -> Self {
        value.as_str()
    }
}

#[derive(Debug)]
pub enum DigestError {
    Missing(String),
    Unsupported(String),
    Error(String),
    NoPayload(String)
}

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
    Latin1
}

/// Dict-like type representing a WARC or HTTP header block.
#[derive(Default, Debug, Clone)]
pub struct HeaderMap {
    encoding: HeaderEncoding,
    status_line: Vec<u8>,
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
            encoding: encoding.into(),
            status_line: Vec::new(),
            headers: Vec::new(),
        }
    }

    /// Get the header encoding.
    pub fn encoding(&self) -> HeaderEncoding {
        self.encoding.clone()
    }

    /// Get the header status line.
    pub fn status_line(&self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.status_line.clone())
    }

    /// Get the raw status line as bytes.
    pub fn status_line_bytes(&self) -> &[u8] {
        &self.status_line
    }

    /// Set status line contents.
    ///
    /// # Arguments
    ///
    /// * `status_line` - New status line
    pub fn set_status_line(&mut self, status_line: impl AsRef<[u8]>) {
        self.status_line = status_line.as_ref().trim_ascii().to_vec();
    }

    /// HTTP status code (unset if header block is not an HTTP header block).
    pub fn status_code(&self) -> Option<u16> {
        if !self.status_line.starts_with(b"HTTP/") {
            return None;
        }
        let mut parts = self.status_line.splitn(3, |&b| b == b' ');
        // Skip HTTP/
        parts.next()?;
        String::from_utf8_lossy(parts.next()?).parse::<u16>().ok()
    }

    fn decode(&self, byte_str: &[u8]) -> String {
        match &self.encoding {
            HeaderEncoding::Unicode => String::from_utf8_lossy(byte_str).to_string(),
            HeaderEncoding::Latin1 => WINDOWS_1252.decode(byte_str, DecoderTrap::Ignore)
                .unwrap_or_else(|_| String::new())
        }
    }

    /// HTTP reason phrase.
    /// Returns None if the header block is not an HTTP header block or no reason phrase was given.
    pub fn reason_phrase(&self) -> Option<String> {
        if !self.status_line.starts_with(b"HTTP/") {
            return None;
        }
        let mut parts = self.status_line.splitn(3, |&b| b == b' ');
        // Skip HTTP/ and status code
        parts.next()?;
        parts.next()?;
        Some(self.decode(parts.next()?))
    }

    /// Get value for (case-insensitive) header key a string.
    /// Duplicate headers are returned as a single value joined with `","`.
    ///
    /// # Arguments
    ///
    /// * `key` - Header key
    pub fn get(&self, key: &str) -> Option<String> {
        Some(self.decode(&self.get_bytes(key.as_bytes())?))
    }

    /// Get value for (case-insensitive) header key as bytes.
    /// Duplicate headers are returned as a single value joined with `","`.
    ///
    /// # Arguments
    ///
    /// * `key` - Header key
    pub fn get_bytes(&self, key: &[u8]) -> Option<Vec<u8>> {
        let values: Vec<&[u8]> = self.headers.iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_slice())
            .collect();
        if !values.is_empty() {
            Some(values.as_slice().join(b",".as_slice()))
        } else {
            None
        }
    }

    /// Check if a (case-insensitive) header key exists.
    ///
    /// # Arguments
    ///
    /// * `key` - Header key
    pub fn contains_key(&self, key: &str) -> bool {
        let key_bytes = key.as_bytes();
        self.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(key_bytes))
    }

    /// Insert new header and overwrite existing header(s) if the key already exists.
    ///
    /// Insertion is not efficient and causes a full traversal of all headers.
    /// If a header already exists, its first occurrence will be updated and
    /// all following occurrences will be dropped.
    ///
    /// # Arguments
    ///
    /// * `key` - Header key
    /// * `value` - Header value
    pub fn set(&mut self, key: impl AsRef<str>, value: impl AsRef<str>) {
        self.set_bytes(key.as_ref().as_bytes(), value.as_ref().as_bytes());
    }

    /// Insert new header and overwrite existing header(s) if the key already exists.
    ///
    /// Insertion is not efficient and causes a full traversal of all headers.
    /// If a header already exists, its first occurrence will be updated and
    /// all following occurrences will be dropped.
    ///
    /// # Arguments
    ///
    /// * `key` - Header key as bytes
    /// * `value` - Header value as bytes
    fn set_bytes(&mut self, key: &[u8], value: &[u8]) {
        let key_lower = key.to_ascii_lowercase();
        let mut found = false;
        self.headers.retain_mut(|h| {
            if h.0.to_ascii_lowercase() != key_lower {
                true
            } else if !found {
                *h = (key.trim_ascii().to_vec(), value.trim_ascii().to_vec());
                found = true;
                true
            } else {
                false
            }
        });
        if !found {
            self.headers.push((key.trim_ascii().to_vec(), value.trim_ascii().to_vec()));
        }
    }

    /// Append header.
    ///
    /// Appending a new header is efficient and does not check for
    /// existing headers with the same name.
    ///
    /// # Arguments
    ///
    /// * `key` - Header key
    /// * `value` - Header value
    pub fn append(&mut self, key: impl AsRef<str>, value: impl AsRef<str>) {
        self.append_bytes(key.as_ref().as_bytes(), value.as_ref().as_bytes());
    }

    /// Append header.
    ///
    /// Appending a new header is efficient and does not check for
    /// existing headers with the same name.
    ///
    /// # Arguments
    ///
    /// * `key` - Header key as bytes
    /// * `value` - Header value as bytes
    pub fn append_bytes(&mut self, key: &[u8], value: &[u8]) {
        self.headers.push((key.trim_ascii().to_vec(), value.trim_ascii().to_vec()));
    }

    /// Iterator of keys and values.
    pub fn items(&self) -> impl Iterator<Item = (String, String)> + use<'_> {
        self.headers
            .iter()
            .map(|(k, v)| (self.decode(k), self.decode(v)))
    }

    /// Iterator of header keys.
    pub fn keys(&self) -> impl Iterator<Item = String> + use<'_> {
        self.headers
            .iter()
            .map(|(k, _)| self.decode(k))
    }

    /// Iterator of header values.
    pub fn values(&self) -> impl Iterator<Item = String> + use<'_> {
        self.headers
            .iter()
            .map(|(_, v)| self.decode(v))
    }

    /// Headers as a series of String tuples.
    ///
    /// Duplicate headers will be preserved.
    /// This is a convenience wrapper around `HeaderMap::items().collect()`.
    pub fn to_tuples(&self) -> Vec<(String, String)> {
        self.items().collect()
    }

    /// Headers as a String HashMap.
    ///
    /// If multiple headers have the same key, the values will be concatenated with `","`.
    pub fn to_map(&self) -> HashMap<CaseInsensitiveKey, String> {
        let mut map: HashMap<CaseInsensitiveKey, String> = HashMap::new();
        self.items()
            .for_each(|(k, v)| {
                map.entry(CaseInsensitiveKey::new(k)).and_modify(|v_| {
                    v_.push(',');
                    v_.push_str(&v);
                }).or_insert(v);
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

    /// Clear all headers.
    pub fn clear(&mut self) {
        self.headers.clear();
        self.status_line.clear();
    }

    /// Write header block into stream.
    pub fn write<W: io::Write>(&self, writer: &mut W) -> io::Result<usize> {
        let mut bytes_written = 0usize;
        if !self.status_line.is_empty() {
            writer.write_all(&self.status_line)?;
            bytes_written += self.status_line.len();
            writer.write_all(b"\r\n")?;
            bytes_written += 2;
        }
        for (key, value) in &self.headers {
            if !key.is_empty() {
                writer.write_all(key)?;
                bytes_written += key.len();
                writer.write_all(b": ")?;
                bytes_written += 2;
            }
            // TODO: Sanitise newlines
            writer.write_all(value)?;
            bytes_written += value.len();
            writer.write_all(b"\r\n")?;
            bytes_written += 2;
        }
        Ok(bytes_written)
    }

    fn add_continuation(&mut self, value: &[u8]) {
        if let Some(last) = self.headers.last_mut() {
            last.1.push(b' ');
            last.1.extend_from_slice(value.trim_ascii());
        } else {
            self.headers.push((Vec::new(), value.trim_ascii().to_vec()));
        }
    }
}


/// A WARC record.
///
/// WARC records are cloneable, but cloning will "freeze" the WARC record.
#[derive(Default, Clone)]
pub struct WarcRecord {
    record_type: WarcRecordType,
    headers: HeaderMap,
    content_length: usize,
    is_http: bool,
    http_parsed: bool,
    http_charset: Option<String>,
    http_headers: Option<HeaderMap>,
    reader: Option<Rc<RefCell<dyn io::BufRead>>>,
    stream_pos: usize,
    content: Vec<u8>,
    stale: bool,
    frozen: bool,
}

impl<'a> fmt::Debug for WarcRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dbg = f.debug_struct("WarcRecord");
        let mut fields = dbg.field("record_type", &self.record_type)
            .field("headers", &self.headers)
            .field("content_length", &self.content_length)
            .field("is_http", &self.is_http);
        if self.is_http {
            fields = fields.field("http_charset", &self.http_charset)
                .field("http_headers", &self.http_headers)
        }
        fields.finish_non_exhaustive()
    }
}

impl WarcRecord {
    /// Create a new empty WARC record.
    pub fn new() -> Self {
        WarcRecord {
            record_type: WarcRecordType::Unknown,
            headers: HeaderMap::new(HeaderEncoding::Unicode),
            is_http: false,
            http_parsed: false,
            http_charset: None,
            http_headers: None,
            content_length: 0,
            reader: None,
            stream_pos: 0,
            content: Vec::new(),
            stale: false,
            frozen: false,
        }
    }

    /// Record type (same as `headers['WARC-Type']`).
    pub fn record_type(&self) -> WarcRecordType {
        self.record_type
    }

    /// Set record type.
    ///
    /// # Arguments
    ///
    /// * `record_type` - Record type
    pub fn set_record_type(&mut self, record_type: WarcRecordType) {
        self.record_type = record_type;
        self.headers.set_bytes(b"WARC-Type", record_type.as_str().as_bytes());
    }

    pub fn set_reader(&mut self, reader: Rc<RefCell<dyn io::BufRead>>) {
        self.reader = Some(reader);
    }

    /// Set WARC body.
    ///
    /// # Arguments
    ///
    /// * `content` - Body as bytes
    pub fn set_bytes_content(&mut self, content: Vec<u8>) {
        self.content_length = content.len();
        self.reader = Some(Rc::new(RefCell::new(io::BufReader::new(io::Cursor::new(content)))));
        self.headers.set_bytes(b"Content-Length", self.content_length.to_string().as_bytes());
    }

    /// Record ID (same as `headers['WARC-Record-ID']`).
    pub fn record_id(&self) -> Option<String> {
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
            self.headers.set_bytes(b"Content-Type", match self.record_type {
                WarcRecordType::Request => b"application/http; msgtype=request",
                WarcRecordType::Response => b"application/http; msgtype=response",
                _ => b"application/http",
            });
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
        self.http_headers.as_ref()?
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

    /// Get the record content as a byte slice.
    // pub fn content(&self) -> &[u8] {
    //     &self.content
    // }

    /// WARC record start offset in the original (uncompressed) stream.
    pub fn stream_pos(&self) -> usize {
        self.stream_pos
    }

    /// Whether the record has been frozen.
    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    /// Whether the record is stale.
    pub fn is_stale(&self) -> bool {
        self.stale
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
            None => format!("urn:uuid:{}", Uuid::new_v4()).into_bytes()
        };

        self.record_type = match record_type {
            Some(WarcRecordType::AnyType) | Some(WarcRecordType::NoType) => WarcRecordType::Unknown,
            Some(record_type) => record_type,
            _ => WarcRecordType::NoType,
        };

        self.headers.clear();
        self.headers.set_status_line(b"WARC/1.1");
        self.headers.append_bytes(b"WARC-Type", self.record_type.as_str().as_bytes());

        let date = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        self.headers.append_bytes(b"WARC-Date", date.as_bytes());

        let record_id = format!("<{}>", String::from_utf8_lossy(&urn));
        self.headers.append_bytes(b"WARC-Record-ID", record_id.as_bytes());

        self.headers.append_bytes(b"Content-Length", content_length.to_string().as_bytes());
        self.content_length = content_length;
    }

    /// Parse a header block from a buffered reader.
    ///
    /// Helper function for parsing WARC or HTTP header blocks.
    ///
    /// # Arguments
    ///
    /// * `reader` - Input reader
    /// * `target` - Header map to fill
    /// * `has_status_line` - Whether first line is a status line or already a header
    /// * `strict_mode` - Enforce `CRLF` line endings, setting this to `false` will allow plain `LF` also
    ///
    /// # Returns
    ///
    /// Number of bytes read from `reader`
    pub fn parse_header_block<R: io::BufRead + ?Sized>(&self,
                                                       reader: &mut R,
                                                       target: &mut HeaderMap,
                                                       has_status_line: bool) -> Result<usize, io::Error> {
        let mut bytes_consumed = 0;
        let mut line = Vec::new();
        let mut expect_first_line = has_status_line;

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

            if matches!(trimmed.first(), Some(b' ' | b'\t')) {
                target.add_continuation(trimmed);
                continue;
            }

            // Status line
            if expect_first_line {
                target.set_status_line(trimmed);
                expect_first_line = false;
                continue;
            }

            // Parse header line
            if let Some(colon_pos) = trimmed.iter().position(|&c| c == b':') {
                let value = if colon_pos + 1 < trimmed.len() {
                    &trimmed[colon_pos + 1..]
                } else {
                    b""
                };
                target.append_bytes(&trimmed[..colon_pos], value);
            } else {
                // Invalid header, try to preserve it
                target.add_continuation(trimmed);
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

        if let Some(reader) = &self.reader {
            let mut http_headers = HeaderMap::new(HeaderEncoding::Latin1);
            let bytes_consumed = self.parse_header_block(&mut *reader.borrow_mut(), &mut http_headers, true)?;

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
            self.content_length = self.content_length - bytes_consumed;
            self.http_headers = Some(http_headers);
            self.http_parsed = true;
        }
        Ok(())
    }

    /// "Freeze" a record by baking in the remaining payload stream contents.
    ///
    /// Freezing a record makes the `WarcRecord` instance copyable and reusable by decoupling
    /// it from the underlying raw WARC stream. Instead of reading directly from the raw stream, a
    /// frozen record maintains an internal buffer the size of the remaining payload stream contents
    /// at the time of calling `freeze()`.
    ///
    /// Freezing a record will advance the underlying raw stream.
    pub fn freeze(&mut self) {
        if let Some(reader) = self.reader.take() {
            let mut reader = reader.borrow_mut();
            use io::Read;
            let mut reader_limited = (&mut *reader).take(self.content_length as u64);
            let mut buf = Vec::with_capacity(self.content_length);
            let _ = reader_limited.read_to_end(&mut buf);
            self.reader = Some(Rc::new(RefCell::new(io::Cursor::new(buf))));
        }
        self.frozen = true;
    }

    /// Write WARC record onto a stream.
    ///
    /// # Arguments
    ///
    /// * `writer` - Output stream
    ///
    /// # Returns
    ///
    /// Number of bytes written
    pub fn write<W: io::Write>(&self, writer: &mut W) -> io::Result<usize> {
        let mut bytes_written = 0;

        // Write WARC headers
        bytes_written += self.headers.write(writer)?;
        writer.write_all(b"\r\n")?;
        bytes_written += 2;

        // Write HTTP headers if parsed
        if self.http_parsed && let Some(ref http_headers) = self.http_headers {
            bytes_written += http_headers.write(writer)?;
            writer.write_all(b"\r\n")?;
            bytes_written += 2;
        }

        // Write content
        writer.write_all(&self.content)?;
        bytes_written += self.content.len();

        // Write record separator
        writer.write_all(b"\r\n\r\n")?;
        bytes_written += 4;

        Ok(bytes_written)
    }

    // TODO: Unchecked conversion AI slop from here on:

    /// Verify whether record digest is valid.
    ///
    /// # Arguments
    ///
    /// * `consume` - Do not create an in-memory copy of the record stream
    ///               (will fully consume the rest of the record)
    ///
    /// # Returns
    ///
    /// `true` if digest exists and is valid
    pub fn verify_block_digest(&mut self, consume: bool) -> Result<bool, DigestError> {
        self.headers
            .get("WARC-Block-Digest")
            .ok_or_else(|| DigestError::Missing("Missing WARC-Block-Digest header".into()))
            .and_then(|d| self._verify_digest(&d, consume))
    }

    /// Verify whether record block digest is valid.
    ///
    /// # Arguments
    ///
    /// * `consume` - Do not create an in-memory copy of the record stream
    ///               (will fully consume the rest of the record)
    ///
    /// # Returns
    ///
    /// `true` if digest exists and is valid
    pub fn verify_payload_digest(&mut self, consume: bool) -> Result<bool, DigestError> {
        if !self.http_parsed || !self.is_http {
            return Err(DigestError::NoPayload("HTTP payload not parsed or missing".into()));
        }

        self.headers
            .get("WARC-Payload-Digest")
            .ok_or_else(|| DigestError::Missing("Missing WARC-Payload-Digest header".into()))
            .and_then(|d| self._verify_digest(&d, consume))
    }

    /// Verify whether record block digest is valid.
    ///
    /// # Arguments
    ///
    /// * `digest_str` - Digest string from header (e.g., "sha1:BASE32HASH")
    /// * `consume` - Do not create an in-memory copy of the record stream
    ///               (will fully consume the rest of the record)
    ///
    /// # Returns
    ///
    /// `true` if digest exists and is valid, None if digest header is missing or invalid
    fn _verify_digest(&mut self, digest_str: &str, consume: bool) -> Result<bool, DigestError> {
        let parts: Vec<&str> = digest_str.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(DigestError::Error("Invalid digest header formatting (':' not found)".into()));
        }
        let algorithm = parts[0].to_ascii_lowercase();
        let expected_digest = parts[1].trim_ascii().as_bytes();

        use data_encoding::{BASE32, HEXLOWER_PERMISSIVE};
        let expected_digest = match BASE32.decode(expected_digest) {
            Ok(bytes) => bytes,
            // Hex digests are non-standard, but are created by some libraries such as warcprox
            Err(_) => match HEXLOWER_PERMISSIVE.decode(expected_digest) {
                Ok(bytes) => bytes,
                Err(_) => return Err(DigestError::Error("Invalid digest encoding".into())),
            },
        };

        if !consume && !self.frozen {
            self.freeze()
        }

        let hash_fn = |payload| {
            match algorithm.as_str() {
                "md5" => {
                    use md5::Md5;
                    let mut hasher = Md5::new();
                    hasher.update(payload);
                    Ok(hasher.finalize().to_vec())
                },
                "sha1" => {
                    use sha1::Sha1;
                    let mut hasher = Sha1::new();
                    hasher.update(payload);
                    Ok(hasher.finalize().to_vec())
                },
                "sha256" => {
                    use sha2::Sha256;
                    let mut hasher = Sha256::new();
                    hasher.update(payload);
                    Ok(hasher.finalize().to_vec())
                },
                "sha512" => {
                    use sha2::Sha512;
                    let mut hasher = Sha512::new();
                    hasher.update(payload);
                    Ok(hasher.finalize().to_vec())
                },
                _ => Err(DigestError::Unsupported(format!("Unsupported hash algorithm: {}", algorithm)))
            }
        };

        // TODO: Read content
        // cdef string block
        // while True:
        //     block = self._reader.read(4096)
        // if block.empty():
        // break
        //     h.update(block)
        //
        // if not consume:
        //     self._reader.stream.seek(0)

        hash_fn(&self.content).map(|d| d == expected_digest)
    }
}

//
// impl Default for WarcRecord {
//     fn default() -> Self {
//         Self::new()
//     }
// }
//
//
// /// Archive iterator configuration.
// #[derive(Debug, Clone)]
// pub struct ArchiveIteratorConfig {
//     /// Whether to parse HTTP records automatically
//     pub parse_http: bool,
//     /// Skip records which have no or an invalid block digest
//     pub verify_digests: bool,
//     /// Skip records with Content-Length less than this
//     pub min_content_length: Option<usize>,
//     /// Skip records with Content-Length larger than this
//     pub max_content_length: Option<usize>,
//     /// Bitmask of record types to return (others will be skipped)
//     pub record_type_filter: u16,
//     /// Enforce strict spec compliance
//     pub strict_mode: bool,
// }
//
// impl Default for ArchiveIteratorConfig {
//     fn default() -> Self {
//         ArchiveIteratorConfig {
//             parse_http: true,
//             verify_digests: false,
//             min_content_length: None,
//             max_content_length: None,
//             record_type_filter: WarcRecordType::AnyType as u16,
//             strict_mode: true,
//         }
//     }
// }
//
// /// WARC record stream iterator.
// pub struct ArchiveIterator<R: BufRead> {
//     reader: R,
//     config: ArchiveIteratorConfig,
//     current_pos: usize,
// }
//
// impl<R: BufRead> ArchiveIterator<R> {
//     /// Create a new archive iterator.
//     ///
//     /// # Arguments
//     ///
//     /// * `reader` - Input stream
//     /// * `config` - Iterator configuration
//     pub fn new(reader: R, config: ArchiveIteratorConfig) -> Self {
//         ArchiveIterator {
//             reader,
//             config,
//             current_pos: 0,
//         }
//     }
//
//     /// Read the next WARC record from the stream.
//     ///
//     /// # Returns
//     ///
//     /// `Ok(Some(record))` if a record was read, `Ok(None)` if EOF, or an error
//     pub fn read_next(&mut self) -> Result<Option<WarcRecord>, Box<dyn std::error::Error>> {
//         loop {
//             let mut record = WarcRecord::new();
//             record.stream_pos = self.current_pos;
//
//             // Read version line
//             let mut version_line = String::new();
//             loop {
//                 version_line.clear();
//                 let n = self.reader.read_line(&mut version_line)?;
//                 if n == 0 {
//                     return Ok(None); // EOF
//                 }
//                 self.current_pos += n;
//
//                 let trimmed = version_line.trim();
//                 if trimmed.is_empty() {
//                     continue;
//                 }
//
//                 if trimmed.starts_with("WARC/1.") || trimmed.starts_with("WARC/0.") {
//                     record.headers.set_status_line(trimmed.as_bytes());
//                     break;
//                 }
//
//                 if self.config.strict_mode {
//                     return Ok(None);
//                 }
//             }
//
//             // Parse WARC headers
//             let bytes_consumed = parse_header_block(&mut self.reader, &mut record.headers, false, self.config.strict_mode)?;
//             self.current_pos += bytes_consumed;
//
//             // Extract record metadata
//             if let Some(warc_type) = record.headers.get("WARC-Type") {
//                 record.record_type = WarcRecordType::from_str(&warc_type);
//             }
//
//             if let Some(content_length_str) = record.headers.get("Content-Length") {
//                 record.content_length = content_length_str.parse().unwrap_or(0);
//             }
//
//             if let Some(content_type) = record.headers.get("Content-Type") {
//                 record.is_http = content_type.contains("application/http");
//             }
//
//             // Check filters
//             let should_skip = !record.record_type.matches_bitmask(self.config.record_type_filter)
//                 || self.config.min_content_length.map_or(false, |min| record.content_length < min)
//                 || self.config.max_content_length.map_or(false, |max| record.content_length > max);
//
//             // Read content
//             let mut content = vec![0u8; record.content_length];
//             self.reader.read_exact(&mut content)?;
//             self.current_pos += record.content_length;
//             record.content = content;
//
//             // Skip trailing CRLF
//             let mut separator = vec![0u8; 4];
//             let _ = self.reader.read(&mut separator);
//             self.current_pos += 4;
//
//             if should_skip {
//                 continue;
//             }
//
//             // Parse HTTP if requested
//             if self.config.parse_http && record.is_http {
//                 record.parse_http(self.config.strict_mode)?;
//             }
//
//             return Ok(Some(record));
//         }
//     }
// }
//
// impl<R: BufRead> Iterator for ArchiveIterator<R> {
//     type Item = Result<WarcRecord, Box<dyn std::error::Error>>;
//
//     fn next(&mut self) -> Option<Self::Item> {
//         self.read_next().transpose()
//     }
// }
//
// /// Filter predicate for checking if record is a WARC/1.0 record.
// ///
// /// # Arguments
// ///
// /// * `record` - WARC record
// pub fn is_warc_10(record: &WarcRecord) -> bool {
//     record.headers().status_line_bytes() == b"WARC/1.0"
// }
//
// /// Filter predicate for checking if record is a WARC/1.1 record.
// ///
// /// # Arguments
// ///
// /// * `record` - WARC record
// pub fn is_warc_11(record: &WarcRecord) -> bool {
//     record.headers().status_line_bytes() == b"WARC/1.1"
// }
//
// /// Filter predicate for checking if record has a block digest.
// ///
// /// # Arguments
// ///
// /// * `record` - WARC record
// pub fn has_block_digest(record: &WarcRecord) -> bool {
//     record.headers().contains_key("WARC-Block-Digest")
// }
//
// /// Filter predicate for checking if record has a payload digest.
// ///
// /// # Arguments
// ///
// /// * `record` - WARC record
// pub fn has_payload_digest(record: &WarcRecord) -> bool {
//     record.headers().contains_key("WARC-Payload-Digest")
// }
//
// /// Filter predicate for checking if record is an HTTP record.
// ///
// /// # Arguments
// ///
// /// * `record` - WARC record
// pub fn is_http(record: &WarcRecord) -> bool {
//     record.is_http()
// }
//
// /// Filter predicate for checking if record is concurrent to another record.
// ///
// /// # Arguments
// ///
// /// * `record` - WARC record
// pub fn is_concurrent(record: &WarcRecord) -> bool {
//     record.headers().contains_key("WARC-Concurrent-To")
// }
