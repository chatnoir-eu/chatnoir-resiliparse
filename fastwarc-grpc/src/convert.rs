// Copyright 2026 Kristian Rickert
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

//! Lossless conversions from `fastwarc` crate types to the `fastwarc.v1`
//! protobuf data model.
//!
//! Header blocks are carried as ordered byte pairs plus the verbatim raw
//! block, so no information the parser saw is lost on the way to the wire
//! (see `DESIGN.md` §5).

use crate::proto::fastwarc::v1 as pb;
use fastwarc::warc::header::{HeaderEncoding, HeaderMap};
use fastwarc::warc::iter::filter;
use fastwarc::warc::record::{AutoDecode, DigestError, WarcRecord, WarcRecordType};
use prost_types::Timestamp;
use time::OffsetDateTime;

/// Convert a [`WarcRecordType`] to its protobuf enum counterpart.
///
/// The crate's `NoType` maps to `WARC_RECORD_TYPE_UNSPECIFIED`; the `AnyType`
/// bitmask wildcard never occurs on a parsed record and also maps to
/// `WARC_RECORD_TYPE_UNSPECIFIED`.
#[must_use]
pub fn warc_record_type(record_type: WarcRecordType) -> pb::WarcRecordType {
    match record_type {
        WarcRecordType::WarcInfo => pb::WarcRecordType::Warcinfo,
        WarcRecordType::Response => pb::WarcRecordType::Response,
        WarcRecordType::Resource => pb::WarcRecordType::Resource,
        WarcRecordType::Request => pb::WarcRecordType::Request,
        WarcRecordType::Metadata => pb::WarcRecordType::Metadata,
        WarcRecordType::Revisit => pb::WarcRecordType::Revisit,
        WarcRecordType::Conversion => pb::WarcRecordType::Conversion,
        WarcRecordType::Continuation => pb::WarcRecordType::Continuation,
        WarcRecordType::Unknown => pb::WarcRecordType::Unknown,
        WarcRecordType::AnyType | WarcRecordType::NoType => pb::WarcRecordType::Unspecified,
    }
}

/// Map a protobuf [`pb::WarcRecordType`] to the crate's bitmask value.
///
/// Proto enum numbers are sequential (proto3 convention); the crate uses
/// bitmasks for filtering. Returns `None` for unspecified / unrecognized.
#[must_use]
pub fn warc_record_type_bit(value: i32) -> Option<u16> {
    match pb::WarcRecordType::try_from(value).unwrap_or_default() {
        pb::WarcRecordType::Unspecified => None,
        pb::WarcRecordType::Warcinfo => Some(WarcRecordType::WarcInfo as u16),
        pb::WarcRecordType::Response => Some(WarcRecordType::Response as u16),
        pb::WarcRecordType::Resource => Some(WarcRecordType::Resource as u16),
        pb::WarcRecordType::Request => Some(WarcRecordType::Request as u16),
        pb::WarcRecordType::Metadata => Some(WarcRecordType::Metadata as u16),
        pb::WarcRecordType::Revisit => Some(WarcRecordType::Revisit as u16),
        pb::WarcRecordType::Conversion => Some(WarcRecordType::Conversion as u16),
        pb::WarcRecordType::Continuation => Some(WarcRecordType::Continuation as u16),
        pb::WarcRecordType::Unknown => Some(WarcRecordType::Unknown as u16),
    }
}

/// OR-combine protobuf record-type enums into a crate filter bitmask.
///
/// An empty list means "any type" (`WarcRecordType::AnyType`).
#[must_use]
pub fn record_types_mask(types: &[i32]) -> u16 {
    let mut mask = 0u16;
    for t in types {
        if let Some(bit) = warc_record_type_bit(*t) {
            mask |= bit;
        }
    }
    if mask == 0 {
        WarcRecordType::AnyType as u16
    } else {
        mask
    }
}

/// Convert a [`HeaderEncoding`] to its protobuf enum counterpart.
#[must_use]
pub fn header_encoding(encoding: &HeaderEncoding) -> pb::HeaderEncoding {
    match encoding {
        HeaderEncoding::Unicode => pb::HeaderEncoding::Unicode,
        HeaderEncoding::Latin1 => pb::HeaderEncoding::Latin1,
    }
}

/// Convert a protobuf `AutoDecode` value to the crate's [`AutoDecode`].
///
/// Unrecognized values fall back to [`AutoDecode::None`].
#[must_use]
pub fn auto_decode(value: i32) -> AutoDecode {
    match pb::AutoDecode::try_from(value).unwrap_or_default() {
        pb::AutoDecode::Unspecified => AutoDecode::None,
        pb::AutoDecode::TransferEncoding => AutoDecode::TransferEncoding,
        pb::AutoDecode::ContentEncoding => AutoDecode::ContentEncoding,
        pb::AutoDecode::All => AutoDecode::All,
    }
}

/// Convert an [`OffsetDateTime`] to a protobuf [`Timestamp`].
#[must_use]
pub fn timestamp(date: OffsetDateTime) -> Timestamp {
    Timestamp {
        seconds: date.unix_timestamp(),
        // Subsecond nanoseconds are always in 0..1e9, so the conversion
        // can never actually fail.
        nanos: i32::try_from(date.nanosecond()).unwrap_or_default(),
    }
}

/// Convert a [`HeaderMap`] to a lossless protobuf `HeaderBlock`.
///
/// The parsed view is emitted as ordered raw byte pairs (duplicates and
/// original case preserved); `raw_block` carries the verbatim source bytes
/// via [`HeaderMap::write`], which writes the unmodified raw buffer.
#[must_use]
pub fn header_block(headers: &HeaderMap) -> pb::HeaderBlock {
    let mut raw_block = Vec::new();
    // Writing to a Vec is infallible.
    let _ = headers.write(&mut raw_block);
    pb::HeaderBlock {
        status_line: headers.status_line_bytes().map(std::borrow::Cow::into_owned),
        fields: headers
            .items_bytes()
            .map(|(name, value)| pb::HeaderField {
                name: name.into_owned(),
                value: value.into_owned(),
            })
            .collect(),
        encoding: header_encoding(&headers.encoding()).into(),
        raw_block,
    }
}

/// Whether payload bytes should be streamed. Unset defaults to true.
#[must_use]
pub fn include_payload(config: &pb::ParseWarcConfig) -> bool {
    config.include_payload.unwrap_or(true)
}

/// Whether lossless header blocks should be filled. Unset defaults to true.
#[must_use]
pub fn include_headers(config: &pb::ParseWarcConfig) -> bool {
    config.include_headers.unwrap_or(true)
}

/// Number of protocol events packed into one gRPC message. Zero means one
/// event per message.
#[must_use]
pub fn response_batch_size(config: &pb::ParseWarcConfig) -> usize {
    usize::try_from(config.response_batch_size).unwrap_or(1).max(1)
}

/// Build the `RecordMetadata` for a parsed record.
///
/// When `include_headers` is false, `warc_headers` and `http_headers` are
/// left unset so a scan that only needs type/length/position skips the
/// lossless header copy.
#[must_use]
pub fn record_metadata(record: &WarcRecord, record_index: u64, include_headers: bool) -> pb::RecordMetadata {
    let (warc_headers, http_headers) = if include_headers {
        (Some(header_block(record.headers())), record.http_headers().map(header_block))
    } else {
        (None, None)
    };
    pb::RecordMetadata {
        record_index,
        record_type: warc_record_type(record.record_type()).into(),
        warc_headers,
        content_length: record.content_length(),
        stream_pos: record.stream_pos(),
        is_http: record.is_http(),
        http_parsed: record.is_http_parsed(),
        http_headers,
        http_content_type: if include_headers {
            record.http_content_type()
        } else {
            None
        },
        http_charset: if include_headers {
            record.http_charset().map(std::borrow::Cow::into_owned)
        } else {
            None
        },
        record_id: if include_headers {
            record.record_id().map(std::borrow::Cow::into_owned)
        } else {
            None
        },
        record_date: if include_headers {
            record.record_date().map(timestamp)
        } else {
            None
        },
    }
}

/// Map the outcome of a digest verification to a `DigestStatus` plus an
/// optional detail string carrying the parser's error message.
#[must_use]
pub fn digest_status(result: Result<bool, DigestError>) -> (pb::DigestStatus, Option<String>) {
    match result {
        Ok(true) => (pb::DigestStatus::Valid, None),
        Ok(false) => (pb::DigestStatus::Mismatch, None),
        Err(e @ (DigestError::Missing(_) | DigestError::NoPayload(_))) => {
            (pb::DigestStatus::NotPresent, Some(e.to_string()))
        }
        Err(e @ DigestError::Unsupported(_)) => (pb::DigestStatus::UnsupportedAlgorithm, Some(e.to_string())),
        Err(e @ DigestError::FormatError(_)) => (pb::DigestStatus::FormatError, Some(e.to_string())),
        Err(e @ DigestError::StreamError(_)) => (pb::DigestStatus::Error, Some(e.to_string())),
    }
}

/// Whether a record passes the configured type / length / builtin filters.
#[must_use]
pub fn record_passes_filters(record: &mut WarcRecord, config: &pb::ParseWarcConfig) -> bool {
    let mask = record_types_mask(&config.record_types);
    if !filter::has_record_type(mask)(record) {
        return false;
    }
    if let Some(min) = config.min_content_length
        && !filter::has_content_length_gte(min)(record)
    {
        return false;
    }
    if let Some(max) = config.max_content_length
        && !filter::has_content_length_lte(max)(record)
    {
        return false;
    }
    for f in &config.filters {
        let keep = match pb::BuiltinFilter::try_from(*f).unwrap_or_default() {
            pb::BuiltinFilter::Unspecified => true,
            pb::BuiltinFilter::IsHttp => filter::is_http(record),
            pb::BuiltinFilter::IsConcurrent => filter::is_concurrent(record),
            pb::BuiltinFilter::HasBlockDigest => filter::has_block_digest(record),
            pb::BuiltinFilter::HasPayloadDigest => filter::has_payload_digest(record),
            pb::BuiltinFilter::IsWarc10 => filter::is_warc_10(record),
            pb::BuiltinFilter::IsWarc11 => filter::is_warc_11(record),
        };
        if !keep {
            return false;
        }
    }
    true
}

#[cfg(test)]
#[path = "convert_test.rs"]
mod convert_test;
