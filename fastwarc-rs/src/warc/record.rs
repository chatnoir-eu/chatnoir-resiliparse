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

use crate::stream_io::bufread::{LimitedBufReader, RawReaderAdapter};
use crate::stream_io::traits::{IntoWarcReader, WarcRead};
use crate::stream_io::{brotli, chunked, gzip, zstd};
use crate::warc::header::{_trim_ascii_offsets, CowHeaderValue, HeaderEncoding, HeaderMap, WarcHeader};
use digest::{Digest, DynDigest};
use sha2::digest;
use std::borrow::Cow;
use std::cell::RefCell;
use std::convert::TryFrom;
use std::fmt::{self, Debug, Display, Formatter};
use std::io::{self, BufRead, Read, Seek};
use std::ops::{BitAnd, BitOr, BitXor, Not};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use time::OffsetDateTime;
use time::format_description::well_known::Iso8601;
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
    Unknown = 32768,
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
            65535 => Ok(WarcRecordType::AnyType),
            0 => Ok(WarcRecordType::NoType),
            _ => Err("Invalid enum value."),
        }
    }
}

impl From<WarcRecordType> for u16 {
    fn from(value: WarcRecordType) -> Self {
        value as u16
    }
}

impl BitOr for WarcRecordType {
    type Output = u16;

    fn bitor(self, rhs: Self) -> u16 {
        self as u16 | rhs as u16
    }
}

impl BitAnd for WarcRecordType {
    type Output = u16;

    fn bitand(self, rhs: Self) -> Self::Output {
        self as u16 & rhs as u16
    }
}

impl BitXor for WarcRecordType {
    type Output = u16;

    fn bitxor(self, rhs: Self) -> Self::Output {
        self as u16 ^ rhs as u16
    }
}

impl Not for WarcRecordType {
    type Output = u16;

    fn not(self) -> u16 {
        !(self as u16)
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
// WARC record
// ===========================================================

/// Record reader types (wrapped in [`LimitedBufReader`]s.
///
/// # Types
///
/// * `Original` - original reader as passed to [`WarcRecord::attach_reader()`].
/// * `Frozen` - frozen reader and, optionally, the preserved original reader.
/// * `Wrapped` - original reader wrapped in (multiple) content- or transfer-decoding readers.
pub(super) enum ReaderType {
    Original(LimitedBufReader),
    Frozen((LimitedBufReader, Option<LimitedBufReader>)),
    Wrapped(LimitedBufReader),
}

/// A WARC record.
///
/// WARC records are cloneable, but cloning will "freeze" the WARC record.
#[derive(Default)]
pub struct WarcRecord {
    pub(super) record_type: WarcRecordType,
    pub(super) headers: HeaderMap,
    pub(super) quirks_mode: bool,
    pub(super) content_length: u64,
    pub(super) is_http: bool,
    pub(super) http_parsed: bool,
    pub(super) http_charset: Option<String>,
    pub(super) http_headers: Option<HeaderMap>,
    pub(super) reader: Option<ReaderType>,
    pub(super) stream_pos: u64,
}

#[derive(Debug, Clone)]
pub enum DigestError {
    Missing(String),
    Unsupported(String),
    FormatError(String),
    NoPayload(String),
    StreamError(String),
}

impl Display for DigestError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            DigestError::Missing(s) => write!(f, "Missing digest header: {}", s),
            DigestError::Unsupported(s) => write!(f, "Unsupported digest algorithm: {}", s),
            DigestError::FormatError(s) => write!(f, "Digest format error: {}", s),
            DigestError::NoPayload(s) => write!(f, "Missing payload: {}", s),
            DigestError::StreamError(s) => write!(f, "Stream error: {}", s),
        }
    }
}

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
        _ => Err(DigestError::Unsupported(algorithm.to_string())),
    }
}

/// Internal: Parse Content-Length header bytes into a number.
#[inline]
fn _parse_content_length_bytes(value: &[u8]) -> u64 {
    let mut parsed = 0u64;
    for &b in value {
        if !b.is_ascii_digit() {
            return 0;
        }
        parsed = parsed.saturating_mul(10).saturating_add((b - b'0') as u64);
    }
    parsed
}

/// Auto-decode options for [`WarcRecord::parse_http()`].
///
/// # Options:
///
/// * `None` - Do not auto-decode content stream.
/// * `TransferEncoding` - Auto-decode `Transfer-Encoding`.
/// * `ContentEncoding` - Auto-decode `Content-Encoding`.
/// * `All` - Auto-decode both.
#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub enum AutoDecode {
    #[default]
    None,
    TransferEncoding,
    ContentEncoding,
    All,
}

impl Display for WarcRecord {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let headers = self.headers.to_string();
        let http_headers = self.http_headers.as_ref().map(|h| h.to_string()).unwrap_or_default();
        write!(f, "{}{}...", headers, http_headers)
    }
}

impl Debug for WarcRecord {
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

impl PartialEq for WarcRecord {
    fn eq(&self, other: &Self) -> bool {
        if !self.is_frozen() || !other.is_frozen() {
            return false;
        }

        self.record_type == other.record_type
            && self.headers == other.headers
            && self.content_length == other.content_length
            && self.is_http == other.is_http
            && self.http_parsed == other.http_parsed
            && self.http_charset == other.http_charset
            && self.http_headers == other.http_headers
            && self.frozen_payload_bytes() == other.frozen_payload_bytes()
    }
}

/// Inner reader accessor that doesn't borrow self.
macro_rules! get_reader_mut {
    ($self:ident) => {{
        let reader: Option<&mut LimitedBufReader> = match &mut $self.reader {
            Some(ReaderType::Original(r)) => Some(r),
            Some(ReaderType::Frozen(r)) => Some(&mut r.0),
            Some(ReaderType::Wrapped(r)) => Some(r),
            _ => None,
        };
        reader
    }};
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
            quirks_mode: false,
            is_http: false,
            http_parsed: false,
            http_charset: None,
            http_headers: None,
            content_length: 0,
            reader: None,
            stream_pos: 0,
        }
    }

    /// Create a new WARC record instance from a buffered reader.
    /// The new instance is fully initialized with all headers present.
    ///
    /// Takes ownership of the reader instance until [`Self::detach_reader()`] is called.
    /// This is the same as constructing a new empty record instance with [`Self::new()`]
    /// and then calling [`Self::attach_reader()`] and [`Self::parse_warc_headers()`].
    ///
    /// Returns an error if a parsed header exceeds 32 KiB in size. Use
    /// [`Self::from_reader_with_opts()`] if you need to set a larger limit.
    ///
    /// # Arguments
    ///
    /// * `reader` - buffered reader instance
    ///
    /// # Returns
    ///
    /// WARC record parsed from the stream.
    pub fn from_reader(reader: impl IntoWarcReader) -> io::Result<Self> {
        Self::from_reader_with_opts(reader, false, 32 << 10)
    }

    /// Create a new WARC record instance from a buffered reader.
    ///
    /// This constructor is equivalent to [`Self::from_reader()`] but allows setting additional options.
    ///
    /// Quirks mode enables more lenient parsing, which may be required for some ClueWebs or other
    /// non-standard WARCs.
    ///
    /// # Arguments
    ///
    /// * `reader` - buffered reader instance
    /// * `quirks_mode` - whether to enable lenient parsing ("quirks mode")
    /// * `max_header_len` - maximum WARC header length (will return an error if exceeded, default: 32 KiB)
    ///
    /// # Returns
    ///
    /// WARC record parsed from the stream.
    pub fn from_reader_with_opts(
        reader: impl IntoWarcReader,
        quirks_mode: bool,
        max_header_len: usize,
    ) -> io::Result<Self> {
        match Self::from_reader_internal(reader, quirks_mode, max_header_len)? {
            Some(record) => Ok(record),
            None => Err(io::Error::new(io::ErrorKind::UnexpectedEof, "No WARC record found")),
        }
    }

    /// Internal helper: Create a new WARC record instance from a buffered reader.
    /// The new instance is fully initialized with all headers present.
    ///
    /// Wraps `Self` into an `Option` to indicate `EOF` (for use in iterators).
    ///
    /// # Arguments
    ///
    /// * `reader` - Buffered reader instance
    /// * `quirks_mode` - whether to enable lenient parsing
    /// * `max_header_len` - maximum WARC header length (will return an error if exceeded, default: 32 KiB)
    ///
    /// # Returns
    ///
    /// `OK(Some(record))` if record found. `OK(None)` if regular EOF reached. `Err` otherwise.
    fn from_reader_internal(
        reader: impl IntoWarcReader,
        quirks_mode: bool,
        max_header_len: usize,
    ) -> io::Result<Option<Self>> {
        let mut record = WarcRecord::new();
        record.attach_reader(reader);
        record.quirks_mode = quirks_mode;
        if record.parse_warc_headers_with_opts(quirks_mode, max_header_len)? == 0 {
            return Ok(None);
        }
        Ok(Some(record))
    }

    /// Create a new frozen WARC record instance from a byte buffer.
    /// The new instance is fully initialized with all headers present.
    ///
    /// # Arguments
    ///
    /// * `payload` - Body as bytes
    pub fn from_bytes(payload: Vec<u8>) -> Result<Self, io::Error> {
        let mut record = WarcRecord::from_reader_with_opts(io::Cursor::new(payload), false, usize::MAX)?;
        record.freeze()?;
        let Some(ReaderType::Frozen((frozen, _orig))) = record.reader.take() else {
            unreachable!("Invalid internal reader state: Reader not frozen");
        };
        record.reader = Some(ReaderType::Frozen((frozen, None)));
        Ok(record)
    }

    /// Attach a buffered reader to this [`WarcRecord`] instance.
    ///
    /// A [`WarcRecord`] must be initialized with either [`Self::attach_reader()`] or
    /// [`Self::set_bytes_payload()`]. Otherwise, operations relying on an existing
    /// payload will fail.
    ///
    /// Takes ownership of the reader instance until [`Self::detach_reader()`] is called.
    ///
    /// # Arguments
    ///
    /// * `reader` - stream reader instance
    pub fn attach_reader(&mut self, reader: impl IntoWarcReader) {
        let mut reader = reader.into_warc_reader();
        // Initialize stream position from inner stream
        if self.stream_pos == 0 {
            self.stream_pos = reader.inner_stream_position().unwrap();
        }
        self.reader = Some(ReaderType::Original(LimitedBufReader::new(reader, None)));
    }

    /// Set the WARC payload as bytes.
    ///
    /// Replaces the currently attached reader instance with a byte buffer reader and marks
    /// the record as frozen (see [`Self::freeze()`]).
    ///
    /// A [`WarcRecord`] must be initialized with either [`Self::attach_reader()`]
    /// or [`Self::set_bytes_payload()`]. Otherwise, operations relying on an existing
    /// payload will fail.
    ///
    /// If a (non-frozen) reader is already attached when this is called, the original reader
    /// instance is retained and can still be detached later with [`Self::detach_reader()`].
    ///
    /// # Arguments
    ///
    /// * `payload` - Body as bytes
    pub fn set_bytes_payload(&mut self, payload: Vec<u8>) {
        self.content_length = payload.len() as u64;
        let orig = match self.reader.take() {
            Some(ReaderType::Original(r)) => Some(r.into_inner()),
            _ => None,
        };
        self.reader = Some(ReaderType::Frozen((
            LimitedBufReader::new(io::Cursor::new(payload), None),
            orig.map(|o| LimitedBufReader::new(o, None)),
        )));
        self.headers
            .set_bytes(b"Content-Length", self.content_length.to_string().as_bytes());
    }

    /// Get the raw payload bytes content of a frozen record.
    /// Returns `None` if the record is not frozen.
    pub fn frozen_payload_bytes(&self) -> Option<&[u8]> {
        let ReaderType::Frozen((reader, _)) = self.reader.as_ref()? else {
            return None;
        };
        let raw = reader
            .inner_as_any()
            .downcast_ref::<RawReaderAdapter<io::Cursor<Vec<u8>>>>()?;
        Some(raw.get_ref().get_ref().as_slice())
    }

    /// Detach an attached buffered reader and hand ownership back to the caller.
    ///
    /// Returns `None` if the reader is already detached or if the [`WarcRecord`] instance
    /// was initialized with [`Self::from_bytes()`].
    ///
    /// # Returns
    ///
    /// Reader instance or `None`
    pub fn detach_reader(&mut self) -> Option<Box<dyn WarcRead>> {
        let reader = self.reader.take()?;
        match reader {
            ReaderType::Wrapped(reader) => {
                macro_rules! downcast {
                    ($reader:ident, $($Type:tt)::+) => {{
                        $reader
                            .downcast::<$($Type)::+<Box<dyn WarcRead>>>()
                            .unwrap()
                            .into_inner()
                            .into_any()
                    }};
                }
                // Unwrap content decoders. The first one in the chain is always a `LimitedBufReader`.
                let mut reader_any = reader
                    .into_warc_reader()
                    .into_any()
                    .downcast::<LimitedBufReader>()
                    .unwrap()
                    .into_inner()
                    .into_any();

                loop {
                    if reader_any.is::<gzip::GzipReader<Box<dyn WarcRead>>>() {
                        reader_any = downcast!(reader_any, gzip::GzipReader);
                    } else if reader_any.is::<brotli::BrotliReader<Box<dyn WarcRead>>>() {
                        reader_any = downcast!(reader_any, brotli::BrotliReader);
                    } else if reader_any.is::<zstd::ZstdReader<Box<dyn WarcRead>>>() {
                        reader_any = downcast!(reader_any, zstd::ZstdReader);
                    } else if reader_any.is::<chunked::ChunkedReader<Box<dyn WarcRead>>>() {
                        reader_any = downcast!(reader_any, chunked::ChunkedReader);
                    } else {
                        break;
                    }
                }
                // Last one is always the original reader wrapped in a `LimitedBufReader`.
                Some(reader_any.downcast::<LimitedBufReader>().unwrap().into_inner())
            }
            ReaderType::Original(reader) => Some(reader.into_inner()),
            ReaderType::Frozen((frozen, orig)) => {
                self.reader = Some(ReaderType::Frozen((frozen, None)));
                orig.map(|r| {
                    r.into_warc_reader()
                        .into_any()
                        .downcast::<LimitedBufReader>()
                        .unwrap()
                        .into_inner()
                })
            }
        }
    }

    /// Get a mutable reference to the attached buffered reader.
    pub fn reader_mut(&mut self) -> Option<&mut LimitedBufReader> {
        get_reader_mut!(self)
    }

    /// WARC record type.
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
        self.headers
            .set_bytes(WarcHeader::WarcType, record_type.as_str().as_bytes());
    }

    /// "Freeze" a record by baking in the remaining payload stream contents.
    ///
    /// Freezing a record advances the attached reader and replaces the stream with a byte buffer
    /// the size of the remaining payload contents at the time of calling [`Self::freeze()`]. The
    /// original reader instance is retained and can still be obtained with [`Self::detach_reader()`].
    /// A frozen [`WarcRecord`] instance is copyable and reusable, since it is decoupled from
    /// the attached WARC input stream.
    ///
    /// It is safe to call this function multiple times, which is a no-op.
    pub fn freeze(&mut self) -> Result<(), io::Error> {
        if let Some(ReaderType::Frozen(_r)) = &self.reader {
            return Ok(());
        }
        let mut reader = match self.reader.take() {
            Some(ReaderType::Original(r)) => Ok(r),
            _ => Err(io::Error::other("No reader set")),
        }?;
        let mut buf = Vec::with_capacity(self.content_length as usize);
        self.content_length = reader.read_to_end(&mut buf)? as u64;
        let frozen = LimitedBufReader::new(io::Cursor::new(buf), None);
        self.reader = Some(ReaderType::Frozen((frozen, Some(reader))));
        Ok(())
    }

    /// Consume the remaining bytes of the WARC record payload without allocating
    /// an additional buffer. Consumes at most `Content-Length` bytes and does nothing if
    /// the WARC headers have not been parsed.
    ///
    /// # Returns
    ///
    /// Number of bytes consumed.
    pub fn consume(&mut self) -> Result<usize, io::Error> {
        self.consume_n(self.content_length as usize)
    }

    /// Consume up to `n` bytes of the WARC record payload without allocating
    /// an additional buffer. Consumes at most `Content-Length` bytes and does nothing if
    /// the WARC headers have not been parsed.
    ///
    /// # Arguments
    ///
    /// * `n` - Maximum number of bytes to consume
    ///
    /// # Returns
    ///
    /// Number of bytes consumed.
    pub fn consume_n(&mut self, n: usize) -> Result<usize, io::Error> {
        if n == 0 {
            return Ok(0);
        }
        if let Some(r) = get_reader_mut!(self) {
            let mut consumed = 0;
            while consumed < n {
                let buf = r.fill_buf()?;
                if buf.is_empty() {
                    break;
                }
                let n_ = buf.len().min(n - consumed);
                consumed += n_;
                r.consume(n_);
            }
            Ok(consumed)
        } else {
            Err(io::Error::other("No reader set"))
        }
    }

    /// Start parsing the WARC record header block. Requires a stream to be set.
    ///
    /// The parser will skip over any number of empty lines before the next valid
    /// `WARC/*` header line. Any other content that is not a valid WARC header
    /// start will return an error of type [`io::ErrorKind::InvalidData`].
    ///
    /// Parsing the WARC headers automatically limits the attached reader to the
    /// remaining `Content-Length` bytes. Detaching the reader will restore its
    /// original EOF limit.
    ///
    /// Returns an error if a WARC header exceeds 32 KiB in size. Use
    /// [`Self::parse_warc_headers_with_opts()`] if you need to set a larger limit.
    ///
    /// # Returns
    ///
    /// Number of bytes read (zero if EOF reached).
    pub fn parse_warc_headers(&mut self) -> Result<usize, io::Error> {
        self.parse_warc_headers_with_opts(false, 32 << 10)
    }

    /// Start parsing the WARC record header block. Requires a stream to be set.
    ///
    /// The parameters can be used to enable lenient parsing ("quirks mode") and to
    /// set a custom header length limit.
    ///
    /// The parser will skip over any number of empty lines before the next valid
    /// `WARC/*` header line. If `quirks_mode == true`, any other invalid lines
    /// encountered before the next header start will be skipped as well.
    /// Otherwise, an error of type [`io::ErrorKind::InvalidData`] is returned.
    ///
    /// Parsing the WARC headers automatically limits the attached reader to the
    /// remaining `Content-Length` bytes. Detaching the reader will restore its
    /// original EOF limit.
    ///
    /// # Arguments
    ///
    /// * `quirks_mode` - Whether to skip non-empty lines before header start
    /// * `max_header_len` - maximum allowed header length (will return an error if exceeded, default: 32 KiB)
    ///
    /// # Returns
    ///
    /// Number of bytes read (zero if EOF reached).
    pub fn parse_warc_headers_with_opts(
        &mut self,
        quirks_mode: bool,
        max_header_len: usize,
    ) -> Result<usize, io::Error> {
        let reader = get_reader_mut!(self).ok_or_else(|| io::Error::other("No reader set"))?;
        let mut bytes_read = 0usize;
        let mut line = Vec::with_capacity(32);

        loop {
            line.clear();

            // Try to find first WARC/* header
            self.stream_pos = reader.inner_stream_position()?;
            let n = reader.read_line(&mut line, 256)?;
            if n == 0 {
                // EOF
                return Ok(0);
            }
            bytes_read += n;

            // Trim ASCII whitespace (including CR/LF line endings)
            let trimmed = line.trim_ascii_end();
            if trimmed.is_empty() {
                // Skip empty lines
                continue;
            }

            // WARC/1.x header
            if matches!(trimmed, b"WARC/1.1" | b"WARC/1.0")
                // ClueWeb09/12 legacy
                || (trimmed.starts_with(b"WARC/0.") && trimmed.len() <= 9)
            {
                // Pre-seed WARC header map
                let offsets = _trim_ascii_offsets(&line);
                self.headers.clear();
                self.headers.raw_header_block.extend_from_slice(&line);
                self.headers.status_line = Some(CowHeaderValue::Offsets(offsets));
                // If supported, use the (potentially more accurate) member start position
                // instead of the starting inner stream position.
                if let Some(p) = reader.frame_start_position()?
                    && p > 0
                {
                    self.stream_pos = p;
                }
                break;
            } else if !quirks_mode {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid WARC header"));
            } else {
                // Quirks mode, keep trying to find a valid WARC header
            }
        }

        bytes_read += self
            .headers
            .parse_with_opts(reader, false, max_header_len, quirks_mode)?;

        let mut parse_count = 0;
        for (k, v) in self.headers.items_bytes() {
            if k.as_ref() == WarcHeader::WarcType.as_str().as_bytes() {
                self.record_type = WarcRecordType::try_from(v.as_ref()).unwrap_or(WarcRecordType::Unknown);
                parse_count += 1;
            } else if k.as_ref() == WarcHeader::ContentType.as_str().as_bytes() {
                self.is_http = v.as_ref() == b"application/http" || v.starts_with(b"application/http;");
                parse_count += 1;
            } else if k.as_ref() == WarcHeader::ContentLength.as_str().as_bytes() {
                self.content_length = str::from_utf8(v.as_ref()).unwrap_or_default().parse().unwrap_or(0);
                parse_count += 1;
            }
            if parse_count == 3 {
                break;
            }
        }

        if quirks_mode
            && self.record_type == WarcRecordType::WarcInfo
            && !self.headers.contains_key_bytes(WarcHeader::ContentLength)
        {
            // ClueWeb22 has no Content-Length header in the initial warcinfo record.
            // Read until the next empty line or until max_header_len if quirks mode is on.
            reader.set_limit(max_header_len as u64);
            let mut payload = Vec::new();
            loop {
                line.clear();
                let n = reader.read_line(&mut line, max_header_len)?;
                if n == 0 {
                    if payload.len() == max_header_len {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "warcinfo record payload exceeds maximum header length",
                        ));
                    }
                    break;
                }
                if line.trim_ascii_end().is_empty() {
                    break;
                }
                payload.extend_from_slice(&line);
            }
            self.set_bytes_payload(payload);
        } else {
            reader.set_limit(self.content_length);
        }
        Ok(bytes_read)
    }

    /// WARC record ID.
    pub fn record_id(&self) -> Option<Cow<'_, str>> {
        self.headers.get(WarcHeader::WarcRecordId)
    }

    /// Set WARC record ID.
    pub fn set_record_id(&mut self, record_id: impl AsRef<str>) {
        self.headers.set(WarcHeader::WarcRecordId, record_id)
    }

    /// WARC record date.
    pub fn record_date(&self) -> Option<OffsetDateTime> {
        if let Some(date) = self.headers.get(WarcHeader::WarcDate) {
            return OffsetDateTime::parse(&date, &Iso8601::DEFAULT).ok();
        }
        None
    }

    /// Trim subsecond part to six digits
    fn _clean_iso_datetime(&mut self, date: String) -> String {
        if let Some((datetime, rest)) = date.split_once('.') {
            let (subsec, offset) = rest.split_at(9); // 9 digits nanoseconds
            let trimmed = &subsec[..6];
            if trimmed == "0".repeat(6) {
                format!("{datetime}{offset}")
            } else {
                format!("{datetime}.{trimmed}{offset}")
            }
        } else {
            date
        }
    }

    /// Set WARC record date.
    pub fn set_record_date(&mut self, date: OffsetDateTime) {
        let formatted = self._clean_iso_datetime(date.format(&Iso8601::DEFAULT).unwrap());
        self.headers.set_bytes(WarcHeader::WarcDate, formatted.as_bytes());
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
            let content_type: &[u8] = match self.record_type {
                WarcRecordType::Request => b"application/http; msgtype=request",
                WarcRecordType::Response => b"application/http; msgtype=response",
                _ => b"application/http",
            };
            self.headers.set_bytes(WarcHeader::ContentType, content_type);
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

    /// HTTP headers if record is an HTTP record and HTTP headers have been parsed yet (mutable).
    pub fn http_headers_mut(&mut self) -> Option<&mut HeaderMap> {
        self.http_headers.as_mut()
    }

    /// Plain HTTP Content-Type without additional fields such as `charset=`.
    pub fn http_content_type(&self) -> Option<String> {
        if !self.http_parsed {
            return None;
        }
        self.http_headers
            .as_ref()?
            .get(WarcHeader::ContentType)?
            .split(";")
            .next()
            .map(|s| s.trim().to_string())
    }

    /// HTTP charset/encoding as returned by the server or `None` if no valid charset is set.
    ///
    /// A returned string is guaranteed to be a valid encoding name.
    pub fn http_charset(&self) -> Option<Cow<'_, str>> {
        self.http_charset.as_deref().map(Cow::Borrowed)
    }

    /// Remaining WARC record length in bytes (not necessarily the same as the `Content-Length` header).
    pub fn content_length(&self) -> u64 {
        self.content_length
    }

    /// WARC record start offset in the original (uncompressed) stream.
    pub fn stream_pos(&self) -> u64 {
        self.stream_pos
    }

    /// Whether the record has been frozen.
    pub fn is_frozen(&self) -> bool {
        matches!(self.reader, Some(ReaderType::Frozen(_)))
    }

    /// Initialize mandatory headers in a fresh WARC record instance.
    ///
    /// You can set the value of the record type and a record ID. The record type defaults
    /// to [`WarcRecordType::NoType`]. If no ID was specified, a random UUID is generated.
    ///
    /// # Arguments
    ///
    /// * `record_type` - WARC-Type
    /// * `record_urn` - WARC-Record-ID as URN without `'<urn:'`, `'>'` (if unset, a random URN will be generated)
    pub fn init_headers(&mut self, record_type: WarcRecordType, record_urn: Option<&[u8]>) {
        let urn = match record_urn {
            Some(urn) => urn.to_vec(),
            None => format!("uuid:{}", Uuid::new_v4()).into_bytes(),
        };

        self.record_type = match record_type {
            WarcRecordType::AnyType => WarcRecordType::Unknown,
            record_type => record_type,
        };

        self.headers.clear();
        self.headers.set_status_line_bytes(b"WARC/1.1");
        self.headers
            .append_bytes_no_sanitize(WarcHeader::WarcType, self.record_type.as_str().as_bytes());
        let date = self._clean_iso_datetime(OffsetDateTime::now_utc().format(&Iso8601::DEFAULT).unwrap());
        self.headers
            .append_bytes_no_sanitize(WarcHeader::WarcDate, date.as_bytes());

        let record_id = format!("<urn:{}>", String::from_utf8_lossy(&urn));
        self.headers
            .append_bytes_no_sanitize(WarcHeader::WarcRecordId, record_id.as_bytes());
        self.headers.append_bytes_no_sanitize(WarcHeader::ContentLength, b"0");
    }

    /// Parse HTTP headers and advance content reader.
    ///
    /// It is safe to call this method multiple times, even if the record is not an HTTP record.
    ///
    /// If the HTTP payload is still transfer- or content-encoded, use [`Self::parse_http_with_opts()`]
    /// to automatically wrap the payload reader in the required [`WarcRead`] implementation.
    /// Usually, web archivers already decode the contents, so in most cases, this shouldn't be necessary.
    ///
    /// Returns an error if a header exceeds 32 KiB in size. Use [`Self::parse_http_with_opts()`] if you
    /// need to set a larger limit.
    pub fn parse_http(&mut self) -> Result<(), io::Error> {
        self.parse_http_with_opts(AutoDecode::None, 32 << 10, false)
    }

    /// Parse HTTP headers and advance content reader. Transfer- or content-encoded payloads can be
    /// automatically decoded based on the value of `auto_decode`.
    ///
    /// It is safe to call this method multiple times, even if the record is not an HTTP record.
    ///
    /// If `auto_decode` is not `None` and the HTTP payload is still transfer- or content-encoded,
    /// the payload reader is wrapped automatically in the required [`WarcRead`] implementation.
    /// Auto-decoding relies on the `Transfer-Encoding` and `Content-Encoding` headers to be present.
    /// Usually, web archivers already decode the contents and rename the headers to prevent double-decoding,
    ///
    /// Returns an error if a header exceeds 32 KiB in size. Use [`Self::parse_http_with_opts()`] if you
    /// need to set a larger limit.
    pub fn parse_http_with_decode(&mut self, auto_decode: AutoDecode) -> Result<(), io::Error> {
        self.parse_http_with_opts(auto_decode, 32 << 10, false)
    }

    /// Parse HTTP headers, advance the content reader, and wrap it in a decoder. The parameters can
    /// be used to enable auto-decoding of the payload and to set a custom header length limit.
    ///
    /// It is safe to call this method multiple times, even if the record is not an HTTP record.
    ///
    /// If `auto_decode` is not `None` and the HTTP payload is still transfer- or content-encoded,
    /// the payload reader is wrapped automatically in the required [`WarcRead`] implementation.
    /// Auto-decoding relies on the `Transfer-Encoding` and `Content-Encoding` headers to be present.
    /// Usually, web archivers already decode the contents and rename the headers to prevent double-decoding,
    ///
    /// If a parsed header exceeds `max_header_len`, an error is returned.
    ///
    /// Quirks mode allows parsing of headers terminated with only LF instead of CRLF.
    ///
    /// # Arguments
    ///
    /// * `auto_decode` - Whether to auto-decode `Transfer-Encoding`, `Content-Encoding`, both, or none.
    /// * `max_header_len` - Maximum accepted header length (returns an error if exceeded).
    /// * `quirks_mode` - Whether to allow parsing of headers terminated with only LF instead of CRLF.
    pub fn parse_http_with_opts(
        &mut self,
        auto_decode: AutoDecode,
        max_header_len: usize,
        quirks_mode: bool,
    ) -> Result<(), io::Error> {
        if self.http_parsed || !self.is_http {
            return Ok(());
        }

        let mut http_headers = HeaderMap::new(HeaderEncoding::Latin1);
        let reader = get_reader_mut!(self).ok_or_else(|| io::Error::other("No reader set"))?;
        let bytes_consumed = http_headers.parse_with_opts(reader, true, max_header_len, quirks_mode)?;

        // Parse charset if present
        if let Some(content_type) = http_headers
            .get(WarcHeader::ContentType)
            .map(|c| c.to_ascii_lowercase())
        {
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
        self.content_length -= bytes_consumed as u64;
        reader.set_limit(self.content_length);
        self.http_parsed = true;

        // Wrap readers for transfer- or content-encoded payloads
        if matches!(auto_decode, AutoDecode::All | AutoDecode::TransferEncoding)
            && let Some(enc) = http_headers.get_bytes(b"Transfer-Encoding").map(|e| e.into_owned())
        {
            self.wrap_reader_in_payload_decoders(&enc, true)?
        }
        if matches!(auto_decode, AutoDecode::All | AutoDecode::ContentEncoding)
            && let Some(enc) = http_headers.get_bytes(b"Content-Encoding").map(|e| e.into_owned())
        {
            self.wrap_reader_in_payload_decoders(&enc, true)?
        }

        self.http_headers = Some(http_headers);

        Ok(())
    }

    /// Helper for wrapping the reader in decompressing readers for handling
    /// Transfer-Encoding and Content-Encoding.
    ///
    /// # Arguments
    ///
    /// * `encoding_str` - comma-separated list of encodings as ASCII bytes
    /// * `transfer` - whether this is transfer- or content-encoding
    fn wrap_reader_in_payload_decoders(&mut self, encoding_str: &[u8], transfer: bool) -> io::Result<()> {
        let mut frozen_orig: Option<LimitedBufReader> = None;
        let mut is_frozen = false;
        let wrapped = match self.reader.take() {
            Some(ReaderType::Original(r)) => Ok(r),
            Some(ReaderType::Frozen((reader, orig))) => {
                frozen_orig = orig;
                is_frozen = true;
                Ok(reader)
            }
            Some(ReaderType::Wrapped(r)) => Ok(r),
            None => Err(io::Error::other("Record has no reader set")),
        }?;
        let mut wrapped = wrapped.into_warc_reader();

        let encoding_it = encoding_str
            .split(|c| *c == b',')
            .map(|enc| enc.trim_ascii().to_ascii_lowercase())
            .rev();
        for enc in encoding_it {
            match enc.as_slice() {
                b"gzip" => wrapped = gzip::GzipReader::new(wrapped).into_warc_reader(),
                b"deflate" => {
                    wrapped = gzip::GzipReader::with_options(
                        wrapped,
                        gzip::GzipReaderOptions {
                            window_bits: gzip::MAX_WBITS,
                            ..gzip::GzipReaderOptions::default()
                        },
                    )
                    .into_warc_reader()
                }
                b"br" => wrapped = brotli::BrotliReader::new(wrapped).into_warc_reader(),
                b"zstd" => wrapped = zstd::ZstdReader::new(wrapped).into_warc_reader(),
                b"chunked" if transfer => wrapped = chunked::ChunkedReader::new(wrapped).into_warc_reader(),
                b"identity" | b"" => (),
                _ => {
                    return Err(io::Error::other(format!(
                        "Unsupported {}-Encoding: {}",
                        if transfer { "Transfer" } else { "Content" },
                        String::from_utf8_lossy(&enc)
                    )));
                }
            }
        }

        if is_frozen {
            // Frozen records don't need to be unwrapped later: eagerly decode stream to enable backward seeking.
            let mut buf = Vec::with_capacity(self.content_length as usize * 4);
            wrapped.read_to_end(&mut buf)?;
            buf.shrink_to_fit();
            self.content_length = buf.len() as u64;
            let wrapped = LimitedBufReader::new(io::Cursor::new(buf), None);
            self.reader = Some(ReaderType::Frozen((wrapped, frozen_orig)));
        } else {
            let wrapped = LimitedBufReader::new(wrapped, None);
            self.reader = Some(ReaderType::Wrapped(wrapped));
        }
        Ok(())
    }

    /// Write WARC record onto a stream.
    ///
    /// The default block size is 16384 bytes and no record checksums are calculated.
    /// Use [`Self::write_with_block_size`] or [`Self::write_with_block_size_checksum`] for more control.
    ///
    /// Does not explicitly start or end compression members. If you are writing into a
    /// [`crate::stream_io::traits::WarcWrite`] that supports multi-member streams, it's up to the
    /// user to call [`crate::stream_io::traits::WarcWrite::finish()`] afterwards.
    ///
    /// # Arguments
    ///
    /// * `writer` - Output stream
    ///
    /// # Returns
    ///
    /// Number of bytes written
    pub fn write<W: io::Write>(&mut self, writer: &mut W) -> io::Result<usize> {
        self.write_with_block_size_checksum(writer, 16384, false)
    }

    /// Write WARC record onto a stream with a given block size.
    ///
    /// By default, no record checksums are calculated. Use [`Self::write_with_block_size_checksum`] or
    /// `write_with_checksum_block_size` for more control.
    ///
    /// Does not explicitly start or end compression members. If you are writing into a
    /// [`crate::stream_io::traits::WarcWrite`] that supports multi-member streams, it's up to the
    /// user to call [`crate::stream_io::traits::WarcWrite::finish()`] afterwards.
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
        self.write_with_block_size_checksum(writer, block_size, false)
    }

    /// Write WARC record onto a stream and calculate SHA-1 record checksums.
    ///
    /// The default block size is 16384 bytes, and SHA-1 checksums are calculated for the
    /// block and payload data (if available). Use [`Self::write_with_block_size_checksum`]
    /// for more control.
    ///
    /// Does not explicitly start or end compression members. If you are writing into a
    /// [`crate::stream_io::traits::WarcWrite`] that supports multi-member streams, it's up to the
    /// user to call [`crate::stream_io::traits::WarcWrite::finish()`] afterwards.
    /// # Arguments
    ///
    /// * `writer` - Output stream
    ///
    /// # Returns
    ///
    /// Number of bytes written
    pub fn write_with_checksum<W: io::Write>(&mut self, writer: &mut W) -> io::Result<usize> {
        self.write_with_block_size_checksum(writer, 16384, true)
    }

    /// Write WARC record onto a stream with a given block size and calculate SHA-1 record checksums.
    ///
    /// Does not explicitly start or end compression members. If you are writing into a
    /// [`crate::stream_io::traits::WarcWrite`] that supports multi-member streams, it's up to the
    /// user to call [`crate::stream_io::traits::WarcWrite::finish()`] afterwards.
    ///
    /// # Arguments
    ///
    /// * `writer` - Output stream
    /// * `chunk_size` - Chunk size for writing the record body
    /// * `checksum_data` - Whether to write data checksums
    ///
    /// # Returns
    ///
    /// Number of bytes written
    pub fn write_with_block_size_checksum<W: io::Write>(
        &mut self,
        writer: &mut W,
        chunk_size: usize,
        checksum_data: bool,
    ) -> io::Result<usize> {
        let mut bytes_written = 0usize;
        let http_header_len = if self.http_parsed {
            self.http_headers.as_ref().map_or(0usize, |h| {
                let mut buf = Vec::with_capacity(512);
                h.write(&mut buf).unwrap_or_default()
            })
        } else {
            0
        };
        let block_content_length = self.content_length as usize + http_header_len;

        if checksum_data {
            self.freeze()?;
            let reader = get_reader_mut!(self).ok_or_else(|| io::Error::other("No reader set"))?;

            use data_encoding::BASE32;
            use sha1::Sha1;
            let mut block_digest = Sha1::new();
            let mut payload_digest = None;
            if self.http_parsed
                && let Some(h) = &self.http_headers
            {
                payload_digest = Some(Sha1::new());
                let mut buf = Vec::with_capacity(512);
                h.write(&mut buf)?;
                Digest::update(&mut block_digest, &buf);
            }

            let mut payload_buf = [0u8; 4096];
            loop {
                let n = reader.read(&mut payload_buf)?;
                if n == 0 {
                    break;
                }
                Digest::update(&mut block_digest, &payload_buf[..n]);
                if let Some(d) = &mut payload_digest {
                    Digest::update(d, &payload_buf[..n]);
                }
            }

            if let Some(d) = payload_digest {
                let payload_digest = format!("sha1:{}", BASE32.encode(&d.finalize()));
                self.headers
                    .set_bytes(WarcHeader::WarcPayloadDigest, payload_digest.as_bytes());
            }
            let block_digest = format!("sha1:{}", BASE32.encode(&block_digest.finalize()));
            self.headers
                .set_bytes(WarcHeader::WarcBlockDigest, block_digest.as_bytes());
            reader.rewind()?;
        }

        let reader = get_reader_mut!(self).ok_or_else(|| io::Error::other("No reader set"))?;

        // Ensure Content-Length is correct
        let block_content_length = block_content_length.to_string();
        if self.headers.get_bytes(WarcHeader::ContentLength).as_deref() != Some(block_content_length.as_bytes()) {
            self.headers
                .set_bytes(WarcHeader::ContentLength, block_content_length.as_bytes());
        }

        // Write WARC headers
        bytes_written += self.headers.write(writer)?;

        // Write HTTP headers if parsed
        if self.http_parsed
            && let Some(ref http_headers) = self.http_headers
        {
            bytes_written += http_headers.write(writer)?;
        }

        // Write content
        let mut buf = vec![0u8; chunk_size];
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
            .get(WarcHeader::WarcBlockDigest)
            .ok_or_else(|| DigestError::Missing(WarcHeader::WarcBlockDigest.as_str().to_string()))?
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
            .get(WarcHeader::WarcPayloadDigest)
            .ok_or_else(|| DigestError::Missing(WarcHeader::WarcPayloadDigest.as_str().to_string()))?
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

        if !consume && !self.is_frozen() {
            self.freeze()
                .map_err(|e| DigestError::StreamError(format!("Failed to freeze record: {}", e)))?;
        }

        let reader = get_reader_mut!(self).ok_or_else(|| DigestError::StreamError("No reader set".into()))?;
        let restore_pos = if consume {
            None
        } else {
            Some(
                reader
                    .stream_position()
                    .map_err(|e| DigestError::StreamError(format!("Failed to get stream position: {}", e)))?,
            )
        };

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
        if let Some(restore_pos) = restore_pos {
            reader
                .seek(io::SeekFrom::Start(restore_pos))
                .map_err(|e| DigestError::StreamError(format!("Failed to seek stream: {}", e)))?;
        }

        Ok(digest.finalize().to_vec() == expected_digest)
    }

    /// Internal next() implementation
    #[inline]
    pub(super) fn next_impl(&mut self, max_header_len: usize) -> Option<io::Result<Self>> {
        if !matches!(self.reader, Some(ReaderType::Frozen(_)))
            && self.content_length > 0
            && let Err(e) = self.consume()
        {
            return Some(Err(e));
        }
        let reader = self.detach_reader()?;
        Self::from_reader_internal(reader, self.quirks_mode, max_header_len).transpose()
    }

    /// Read the next record from the attached stream. Mutates this [`WarcRecord`] instance
    /// in-place instead of returning a new instance.
    ///
    /// # Returns
    ///
    /// `Ok(())` if next record read successfully, `None` at EOF, `Err(_)` if an I/O error
    /// occurred. If the result is not `Ok(_)`, the instance should be treated as tainted and should
    /// no longer be used. You may still call [`Self::detach_reader()`], however.
    #[inline]
    pub fn next_inplace(&mut self) -> Option<io::Result<()>> {
        self.next_inplace_impl(32 << 10)
    }

    /// Internal next_inplace() implementation
    #[inline]
    pub(super) fn next_inplace_impl(&mut self, max_header_len: usize) -> Option<io::Result<()>> {
        if !matches!(self.reader, Some(ReaderType::Frozen(_)))
            && self.content_length > 0
            && let Err(e) = self.consume()
        {
            return Some(Err(e));
        }
        match &mut self.reader {
            Some(ReaderType::Original(r)) => r.reset_limit(),
            Some(ReaderType::Wrapped(_) | ReaderType::Frozen(_)) => {
                // Resume on the archive reader after decoded or buffered payloads.
                let reader = self.detach_reader()?;
                self.attach_reader(reader);
            }
            _ => return None,
        };

        self.record_type = WarcRecordType::NoType;
        self.is_http = false;
        self.http_parsed = false;
        self.http_charset = None;
        self.http_headers = None;

        match self.parse_warc_headers_with_opts(self.quirks_mode, max_header_len) {
            Err(e) => Some(Err(e)),
            Ok(0) => None,
            Ok(_) => Some(Ok(())),
        }
    }
}

impl Iterator for WarcRecord {
    type Item = io::Result<Self>;

    /// Read the next record from the attached stream. Detaches the reader from the
    /// current record instance.
    ///
    /// # Returns
    ///
    /// Next [`WarcRecord`] instance from the stream or `None`.
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.next_impl(32 << 10)
    }
}

/// Reference-counted handle for holding [`WarcRecord`] instances.
///
/// This is a common accessor trait for the two reference-counted types
/// `Rc<RefCell<WarcRecord>>` and `Arc<Mutex<WarcRecord>>` used by [`super::iter::ArchiveIterator`]
/// and [`super::iter::ArchiveIteratorThreadSafe`], respectively.
pub trait SharedWarcRecord: Clone {
    /// Create a new reference-counted [`WarcRecord`] handle.
    fn new(record: WarcRecord) -> Self;

    /// Replace the currently held [`WarcRecord`] with another one.
    fn replace(&self, record: WarcRecord);

    /// Execute the closure with a mutable reference to the held record.
    /// This is to abstract from the individual accessors of the underlying reference
    /// counting mechanism ([`RefCell::borrow_mut()`] vs. [`Mutex::lock()::unwrap()`](Mutex::lock()).
    fn with_mut<R>(&self, f: impl FnOnce(&mut WarcRecord) -> R) -> R;
}

impl SharedWarcRecord for Rc<RefCell<WarcRecord>> {
    fn new(record: WarcRecord) -> Self {
        Rc::new(RefCell::new(record))
    }

    fn replace(&self, record: WarcRecord) {
        *self.borrow_mut() = record;
    }

    fn with_mut<R>(&self, f: impl FnOnce(&mut WarcRecord) -> R) -> R {
        f(&mut self.borrow_mut())
    }
}

impl SharedWarcRecord for Arc<Mutex<WarcRecord>> {
    fn new(record: WarcRecord) -> Self {
        Arc::new(Mutex::new(record))
    }

    fn replace(&self, record: WarcRecord) {
        *self.lock().unwrap() = record;
    }

    fn with_mut<R>(&self, f: impl FnOnce(&mut WarcRecord) -> R) -> R {
        f(&mut self.lock().unwrap())
    }
}

// ===========================================================
// Tests
// ===========================================================

#[cfg(test)]
#[path = "record_test.rs"]
mod record_test;
