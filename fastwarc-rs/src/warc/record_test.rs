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

use super::*;
use crate::stream_io::bufread::LimitedBufReader;
use crate::warc::header::{HeaderEncoding, HeaderMap, WarcHeader};
use crate::warc::iter::ArchiveIterator;
use crate::warc::mod_test::*;
use data_encoding::{BASE32, HEXLOWER};
use md5::Md5;
use pretty_assertions::{assert_eq, assert_ne};
use sha1::{Digest, Sha1};
use sha2::{Sha256, Sha512};
use std::io::{self, BufRead, Read, Seek, Write};

#[test]
fn limited_buf_read_seek_limit_seek() -> io::Result<()> {
    let mut limited = LimitedBufReader::new(io::Cursor::new(b"abcdef".to_vec()), Some(4));

    assert_eq!(limited.fill_buf()?, b"abcd");
    limited.consume(2);
    assert_eq!(limited.stream_position()?, 2);
    assert_eq!(limited.real_stream_position()?, 2);

    let mut buf = [0u8; 4];
    assert_eq!(limited.read(&mut buf)?, 2);
    assert_eq!(&buf[..2], b"cd");
    assert_eq!(limited.read(&mut buf)?, 0);

    limited.set_limit(3);
    assert_eq!(limited.stream_position()?, 0);

    Ok(())
}

#[test]
fn record_type_try_from_helpers() {
    assert_eq!(WarcRecordType::try_from(4u16), Ok(WarcRecordType::Response));
    assert_eq!(WarcRecordType::try_from("REQUEST"), Ok(WarcRecordType::Request));
    assert_eq!(WarcRecordType::try_from(b"metadata".as_slice()), Ok(WarcRecordType::Metadata));
    assert!(WarcRecordType::try_from(3u16).is_err());
}

#[test]
fn record_type_from_helpers() {
    assert_eq!(<&'static str>::from(WarcRecordType::Continuation), "continuation");
    assert_eq!(<&'static str>::from(WarcRecordType::NoType), "unknown");
}

#[test]
fn record_type_conversions() {
    let named_types = [
        (WarcRecordType::WarcInfo, 2u16, "warcinfo"),
        (WarcRecordType::Response, 4u16, "response"),
        (WarcRecordType::Resource, 8u16, "resource"),
        (WarcRecordType::Request, 16u16, "request"),
        (WarcRecordType::Metadata, 32u16, "metadata"),
        (WarcRecordType::Revisit, 64u16, "revisit"),
        (WarcRecordType::Conversion, 128u16, "conversion"),
        (WarcRecordType::Continuation, 256u16, "continuation"),
    ];

    for (record_type, value, name) in named_types {
        assert_eq!(WarcRecordType::try_from(value), Ok(record_type));
        assert_eq!(WarcRecordType::try_from(name), Ok(record_type));
        assert_eq!(WarcRecordType::try_from(name.as_bytes()), Ok(record_type));
        assert_eq!(<&'static str>::from(record_type), name);
    }

    assert_eq!(WarcRecordType::try_from(WarcRecordType::AnyType as u16), Ok(WarcRecordType::AnyType));
    assert_eq!(WarcRecordType::try_from(WarcRecordType::NoType as u16), Ok(WarcRecordType::NoType));
    assert_eq!(<&'static str>::from(WarcRecordType::AnyType), "unknown");
    assert_eq!(<&'static str>::from(WarcRecordType::NoType), "unknown");
    assert!(WarcRecordType::try_from(3u16).is_err());
    assert!(WarcRecordType::try_from("not-a-record-type").is_err());
    assert!(WarcRecordType::try_from(b"not-a-record-type".as_slice()).is_err());
}

#[test]
fn record_type_bitmask_helpers() {
    let http_exchange = WarcRecordType::Request as u16 | WarcRecordType::Response as u16;
    assert!(WarcRecordType::Request.matches_bitmask(http_exchange));
    assert!(WarcRecordType::Response.matches_bitmask(http_exchange));
    assert!(!WarcRecordType::Resource.matches_bitmask(http_exchange));

    let metadata_or_revisit = WarcRecordType::Metadata as u16 | WarcRecordType::Revisit as u16;
    assert!(WarcRecordType::Metadata.matches_bitmask(metadata_or_revisit));
    assert!(WarcRecordType::Revisit.matches_bitmask(metadata_or_revisit));
    assert!(!WarcRecordType::Conversion.matches_bitmask(metadata_or_revisit));

    assert!(WarcRecordType::Response.matches_bitmask(WarcRecordType::AnyType as u16));
}

#[test]
fn parse_warc_headers() -> io::Result<()> {
    let record_data1 = warc_record_data("request", "<urn:uuid:record1>", None, b"ABC");
    let record_data2 = warc_record_data("response", "<urn:uuid:record2>", None, b"DEFGHI");
    let warc_data = [record_data1.as_slice(), record_data2.as_slice()].concat();

    let reader = io::Cursor::new(warc_data);
    let mut record1 = WarcRecord::new();

    assert_eq!(record1.content_length(), 0);
    assert_eq!(record1.record_id(), None);
    assert_eq!(record1.record_type(), WarcRecordType::NoType);

    // Parse first record (construct manually)
    record1.attach_reader(reader);
    record1.parse_warc_headers()?;
    assert_eq!(record1.stream_pos(), 0);
    assert_eq!(record1.content_length(), 3);
    assert_eq!(record1.record_type(), WarcRecordType::Request);

    let headers = record1.headers();
    assert_eq!(headers.status_line().as_deref(), Some("WARC/1.1"));
    assert_eq!(headers.status_line_bytes().as_deref(), Some(b"WARC/1.1".as_slice()));
    assert!(!record1.is_http());
    assert_eq!(headers.get(WarcHeader::WarcType).as_deref(), Some("request"));
    assert_eq!(headers.get_bytes(WarcHeader::WarcType).as_deref(), Some(b"request".as_slice()));
    assert_eq!(record1.record_id().as_deref(), Some("<urn:uuid:record1>"));
    assert_eq!(headers.get(WarcHeader::WarcRecordId).as_deref(), Some("<urn:uuid:record1>"));
    assert_eq!(headers.get_bytes(WarcHeader::WarcRecordId).as_deref(), Some(b"<urn:uuid:record1>".as_slice()));
    assert_eq!(headers.get(WarcHeader::ContentLength).as_deref(), Some("3"));
    assert_eq!(headers.get_bytes(WarcHeader::ContentLength).as_deref(), Some(b"3".as_slice()));

    // Change headers
    let headers = record1.headers_mut();
    headers.set_status_line("WARC/1.0");
    assert_eq!(headers.status_line().as_deref(), Some("WARC/1.0"));
    headers.set("X-Foo", "Bar");
    assert_eq!(headers.get("X-Foo").as_deref(), Some("Bar"));

    // Read payload
    let mut buf = Vec::new();
    record1.reader_mut().unwrap().read_to_end(&mut buf)?;
    assert_eq!(String::from_utf8_lossy(&buf), "ABC");

    // Parse second record (construct directly from stream)
    let reader = record1.detach_reader().unwrap();
    let mut record2 = WarcRecord::from_reader(reader)?;

    assert_eq!(record2.stream_pos(), record_data1.len() as u64);
    assert_eq!(record2.content_length(), 6);
    assert_eq!(record2.record_type(), WarcRecordType::Response);

    buf.clear();
    record2.reader_mut().unwrap().read_to_end(&mut buf)?;
    assert_eq!(String::from_utf8_lossy(&buf), "DEFGHI");

    Ok(())
}

#[test]
fn parse_http_headers() -> io::Result<()> {
    let payload = "Hello World";
    let warc_data = http_response_warc_data("<urn:uuid:record-id>", payload);

    let reader = io::Cursor::new(warc_data);
    let mut record = WarcRecord::new();
    record.attach_reader(reader);
    record.parse_warc_headers()?;

    let warc_headers = record.headers();
    assert_eq!(warc_headers.status_line().as_deref(), Some("WARC/1.1"));
    assert!(record.is_http());
    assert!(!record.is_http_parsed());
    assert!(record.http_headers().is_none());

    record.parse_http()?;
    assert!(record.is_http_parsed());
    let http_headers = record.http_headers().unwrap();
    assert_eq!(http_headers.status_line().as_deref(), Some("HTTP/1.1 200 OK"));
    assert_eq!(http_headers.status_code(), Some(200));
    assert_eq!(http_headers.reason_phrase().as_deref(), Some("OK"));
    assert_eq!(http_headers.get(WarcHeader::ContentType).as_deref(), Some("text/plain; charset=utf-8"));
    assert_eq!(record.http_charset().as_deref(), Some("utf-8"));
    assert_eq!(record.http_content_type().as_deref(), Some("text/plain"));

    let mut buf = Vec::new();
    record.reader_mut().unwrap().read_to_end(&mut buf)?;
    assert_eq!(String::from_utf8_lossy(&buf), payload);

    Ok(())
}

#[test]
fn record_iter_next() -> io::Result<()> {
    let record_data1 = warc_record_data("request", "<urn:uuid:record1>", None, b"ABC");
    let record_data2 = warc_record_data("response", "<urn:uuid:record2>", None, b"DEFGHI");
    let warc_data = [record_data1.as_slice(), record_data2.as_slice()].concat();

    let mut record1 = WarcRecord::from_reader(io::Cursor::new(warc_data.clone()))?;
    assert_eq!(record1.stream_pos(), 0);
    assert_eq!(record1.record_id().as_deref(), Some("<urn:uuid:record1>"));

    let mut payload = Vec::new();
    record1.reader_mut().unwrap().read_to_end(&mut payload)?;
    assert_eq!(payload, b"ABC");

    let mut record2 = record1.next().unwrap()?;
    assert_eq!(record2.record_id().as_deref(), Some("<urn:uuid:record2>"));
    assert_eq!(record2.stream_pos(), record_data1.len() as u64);
    payload.clear();
    record2.reader_mut().unwrap().read_to_end(&mut payload)?;
    assert_eq!(payload, b"DEFGHI");
    assert!(record2.next().is_none());

    Ok(())
}

#[test]
fn record_iter_next_inplace() -> io::Result<()> {
    let record_data1 = http_response_warc_data("<urn:uuid:record1>", "Hello World");
    let record_data2 = warc_record_data("resource", "<urn:uuid:record2>", None, b"XYZ");
    let warc_data = [record_data1.as_slice(), record_data2.as_slice()].concat();

    let mut record = WarcRecord::new();
    record.attach_reader(io::Cursor::new(warc_data));
    record.parse_warc_headers()?;
    record.parse_http()?;

    assert!(record.is_http());
    assert!(record.is_http_parsed());
    assert!(record.http_headers().is_some());
    assert_eq!(record.record_id().as_deref(), Some("<urn:uuid:record1>"));

    assert!(record.next_inplace().transpose()?.is_some());
    assert_eq!(record.record_type(), WarcRecordType::Resource);
    assert_eq!(record.record_id().as_deref(), Some("<urn:uuid:record2>"));
    assert_eq!(record.stream_pos(), record_data1.len() as u64);
    assert!(!record.is_http());
    assert!(!record.is_http_parsed());
    assert!(record.http_headers().is_none());
    assert_eq!(record.content_length(), 3);

    let mut buf = Vec::new();
    record.reader_mut().unwrap().read_to_end(&mut buf)?;
    assert_eq!(buf, b"XYZ");

    // EOF
    assert!(record.next_inplace().is_none());

    Ok(())
}

#[test]
fn record_http_parsing() -> io::Result<()> {
    let file = get_fixture_path("warcfile.warc");
    let mut parsed_count = 0usize;

    for rec in ArchiveIterator::from_path(&file)?.with_parse_http(true) {
        let rec = rec?;
        let mut rec = rec.borrow_mut();
        if rec.record_type() != WarcRecordType::Response {
            continue;
        }

        parsed_count += 1;

        assert!(rec.is_http());
        assert!(rec.is_http_parsed());

        let warc_headers = rec.headers();
        assert_eq!(warc_headers.status_code(), None);
        assert_eq!(warc_headers.reason_phrase(), None);

        let http_headers = rec.http_headers().unwrap();
        assert!(http_headers.status_code().is_some());
        assert!(http_headers.reason_phrase().is_some());
        let status_code = http_headers.status_code().unwrap();
        let reason_phrase = http_headers.reason_phrase().unwrap();
        let status_line = http_headers.status_line().unwrap();
        assert!(status_line.contains(&status_code.to_string()));
        assert!(status_line.contains(reason_phrase.as_ref()));
        assert!(http_headers.to_map().len() <= http_headers.items().count());

        let content_type = rec.http_content_type().unwrap();
        assert!(content_type.starts_with("text/"));
        assert!(http_headers.contains_key(WarcHeader::ContentType));
        let content_type_header = http_headers.get(WarcHeader::ContentType).unwrap();
        if let Some((_, charset)) = content_type_header.split_once("charset=") {
            assert_eq!(rec.http_charset().map(|c| c.into_owned()), Some(charset.trim().to_ascii_lowercase()));
        }

        let mut prefix = [0u8; 5];
        rec.reader_mut().unwrap().read_exact(&mut prefix)?;
        assert_ne!(&prefix, b"HTTP/");
    }

    assert!(parsed_count > 0);

    let mut raw_count = 0usize;
    for rec in ArchiveIterator::from_path(&file)?.with_parse_http(false) {
        let rec = rec?;
        let mut rec = rec.borrow_mut();
        if rec.record_type() != WarcRecordType::Response {
            continue;
        }

        raw_count += 1;

        assert!(rec.is_http());
        assert!(!rec.is_http_parsed());
        assert!(rec.http_headers().is_none());
        assert_eq!(rec.http_content_type(), None);
        assert_eq!(rec.http_charset(), None);
        assert_eq!(rec.headers().status_code(), None);
        assert_eq!(rec.headers().reason_phrase(), None);

        let mut prefix = [0u8; 5];
        rec.reader_mut().unwrap().read_exact(&mut prefix)?;
        assert_eq!(&prefix, b"HTTP/");
    }

    assert_eq!(parsed_count, raw_count);

    Ok(())
}

#[test]
fn warcinfo_quirks_mode_without_length_respects_buffer_limit() {
    let data = format!("WARC/1.1\r\nWARC-Type: warcinfo\r\n\r\nsoftware: {}\r\n\r\n", "x".repeat(100));
    let error = WarcRecord::from_reader_with_opts(io::Cursor::new(data), true, 64).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn parse_warc_headers_quirks_and_payload_replacement() -> io::Result<()> {
    let warc_data = b"garbage before header\r\n\
                      \r\n\
                      WARC/1.1\r\n\
                      WARC-Type: resource\r\n\
                      Content-Length: 3\r\n\
                      \r\n\
                      ABC\r\n\r\nrest"
        .to_vec();

    let mut strict = WarcRecord::new();
    strict.attach_reader(io::Cursor::new(warc_data.clone()));
    let err = strict.parse_warc_headers().unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);

    let mut record = WarcRecord::new();
    record.attach_reader(io::Cursor::new(warc_data));
    let bytes_read = record.parse_warc_headers_with_opts(true, 8192)?;
    assert!(bytes_read > 0);
    assert_eq!(record.headers().status_line().as_deref(), Some("WARC/1.1"));
    assert_eq!(record.record_type(), WarcRecordType::Resource);
    assert_eq!(record.content_length(), 3);

    record.set_bytes_payload(b"XYZ".to_vec());
    assert!(record.is_frozen());
    assert_eq!(record.content_length(), 3);
    assert_eq!(record.headers().get(WarcHeader::ContentLength).as_deref(), Some("3"));

    let mut payload = Vec::new();
    record.reader_mut().unwrap().read_to_end(&mut payload)?;
    assert_eq!(payload, b"XYZ");

    let detached = record.detach_reader().unwrap();
    let mut detached = detached;
    assert!(detached.stream_position()? > 0);

    Ok(())
}

#[test]
fn create_new_warc_record() -> io::Result<()> {
    let new_record_bytes_content = b"HTTP/1.1 200 OK\r\n\
Content-Type: text/html; charset=utf-8\r\n\
Content-Length: 69\r\n\
X-Multiline-Header: Hello\r\n\
\x20\x20World\r\n\r\n\
<!doctype html>\n\
<meta charset=\"utf-8\">\n\
<title>Test</title>\n\n\
Barbaz\n";

    let mut src_record = WarcRecord::new();
    src_record.init_headers(WarcRecordType::NoType, None);
    assert_eq!(src_record.headers().status_line().as_deref(), Some("WARC/1.1"));
    assert!(src_record.record_id().is_some_and(|id| id.starts_with("<urn:")));
    assert_eq!(src_record.record_type(), WarcRecordType::NoType);
    assert_eq!(src_record.content_length(), 0);
    assert!(src_record.headers().contains_key(WarcHeader::WarcType));
    assert!(src_record.headers().contains_key(WarcHeader::WarcDate));
    assert!(src_record.headers().contains_key(WarcHeader::WarcRecordId));
    assert_eq!(src_record.headers().get(WarcHeader::WarcRecordId), src_record.record_id());
    assert_eq!(src_record.headers().get(WarcHeader::ContentLength).as_deref(), Some("0"));
    src_record.headers_mut().set("X-Multiline-Header", "Hello\r\nWorld");
    assert_eq!(src_record.headers().get("X-Multiline-Header").as_deref(), Some("Hello World"));

    src_record.set_bytes_payload(new_record_bytes_content.to_vec());
    let content_len = new_record_bytes_content.len().to_string();
    assert_eq!(src_record.content_length(), new_record_bytes_content.len() as u64);
    assert_eq!(src_record.headers().get(WarcHeader::ContentLength).as_deref(), Some(content_len.as_str()));

    src_record.set_is_http(true);
    assert_eq!(src_record.headers().get(WarcHeader::ContentType).as_deref(), Some("application/http"));
    src_record.set_record_type(WarcRecordType::Request);
    src_record.set_is_http(true);
    assert_eq!(
        src_record.headers().get(WarcHeader::ContentType).as_deref(),
        Some("application/http; msgtype=request")
    );
    src_record.set_record_type(WarcRecordType::Response);
    src_record.set_is_http(true);
    assert_eq!(
        src_record.headers().get(WarcHeader::ContentType).as_deref(),
        Some("application/http; msgtype=response")
    );

    let payload_start = new_record_bytes_content
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 4)
        .unwrap();
    let payload_digest = format!("sha1:{}", BASE32.encode(&Sha1::digest(&new_record_bytes_content[payload_start..])));
    src_record
        .headers_mut()
        .set_bytes(WarcHeader::WarcPayloadDigest, payload_digest.as_bytes());

    let mut stream = Vec::new();
    src_record.write_with_checksum(&mut stream)?;

    let mut it = ArchiveIterator::new(io::Cursor::new(stream)).with_parse_http(false);
    let rec = it.next().unwrap()?;
    {
        let mut rec = rec.borrow_mut();
        assert_eq!(rec.headers().status_line(), src_record.headers().status_line());
        assert_eq!(rec.headers().get("X-Multiline-Header").as_deref(), Some("Hello World"));
        assert_eq!(
            src_record.headers().get(WarcHeader::ContentType).as_deref(),
            Some("application/http; msgtype=response")
        );
        assert_eq!(rec.headers(), src_record.headers());
        assert_eq!(rec.record_id(), src_record.record_id());
        assert_eq!(rec.record_type(), src_record.record_type());
        assert!(
            rec.verify_block_digest(false)
                .map_err(|e| io::Error::other(e.to_string()))?
        );

        assert!(rec.is_http());
        rec.parse_http()?;
        let http_headers = rec.http_headers().unwrap();
        assert_eq!(http_headers.status_code(), Some(200));
        assert_eq!(http_headers.reason_phrase().as_deref(), Some("OK"));
        assert_eq!(rec.http_content_type().as_deref(), Some("text/html"));
        assert_eq!(rec.http_charset().as_deref(), Some("utf-8"));
        assert_eq!(http_headers.get("X-Multiline-Header").as_deref(), Some("Hello World"));
        assert!(
            rec.verify_payload_digest(false)
                .map_err(|e| io::Error::other(e.to_string()))?
        );
    }

    assert!(it.next().is_none());

    Ok(())
}

#[test]
fn record_init_from_bytes() -> io::Result<()> {
    let record_bytes = warc_record_data("response", "<urn:uuid:42e5b12c-3396-4b7e-b4b3-c88b7000cf43>", None, b"ABC");
    let record = WarcRecord::from_bytes(record_bytes)?;
    assert!(record.is_frozen());
    assert_eq!(record.record_type(), WarcRecordType::Response);
    assert_eq!(record.record_id().as_deref(), Some("<urn:uuid:42e5b12c-3396-4b7e-b4b3-c88b7000cf43>"));
    assert_eq!(record.content_length(), 3);
    Ok(())
}

#[test]
fn record_equality_requires_frozen_identical_contents() -> io::Result<()> {
    let record_bytes = warc_record_data("response", "<urn:uuid:eq>", None, b"ABC");

    let left = WarcRecord::from_bytes(record_bytes.clone())?;
    let right = WarcRecord::from_bytes(record_bytes)?;
    assert_eq!(left, right);

    let different_payload = WarcRecord::from_bytes(warc_record_data("response", "<urn:uuid:eq>", None, b"ABD"))?;
    assert_ne!(left, different_payload);

    let unfrozen =
        WarcRecord::from_reader(io::Cursor::new(warc_record_data("response", "<urn:uuid:eq>", None, b"ABC")))?;
    assert_ne!(left, unfrozen);

    // Test partial reflexivity
    assert_eq!(left, left);
    assert_eq!(right, right);
    assert_ne!(unfrozen, unfrozen);

    let mut http_left = WarcRecord::from_bytes(http_response_warc_data("<urn:uuid:eq-http>", "Hello"))?;
    let mut http_right = WarcRecord::from_bytes(http_response_warc_data("<urn:uuid:eq-http>", "Hello"))?;
    assert_eq!(http_left, http_right);

    http_left.parse_http()?;
    assert_ne!(http_left, http_right);

    http_right.parse_http()?;
    assert_eq!(http_left, http_right);

    let mut http_different = WarcRecord::from_bytes(http_response_warc_data("<urn:uuid:eq-http>", "World"))?;
    http_different.parse_http()?;
    assert_ne!(http_left, http_different);

    Ok(())
}

#[test]
fn record_init_headers_http() -> io::Result<()> {
    let mut record = WarcRecord::new();
    record.init_headers(WarcRecordType::AnyType, Some(b"uuid:494749ad-b14a-4f22-b143-0bab4347884b"));
    assert_eq!(record.record_type(), WarcRecordType::Unknown);
    assert_eq!(record.record_id().as_deref(), Some("<urn:uuid:494749ad-b14a-4f22-b143-0bab4347884b>"));
    assert_eq!(record.headers().status_line().as_deref(), Some("WARC/1.1"));
    assert_eq!(record.content_length(), 0);
    assert_eq!(record.headers().get(WarcHeader::ContentLength).as_deref(), Some("0"));

    record.parse_http()?;
    assert!(!record.is_http_parsed());

    record.set_record_type(WarcRecordType::Request);
    record.set_is_http(true);
    assert!(record.is_http());
    assert_eq!(record.headers().get(WarcHeader::ContentType).as_deref(), Some("application/http; msgtype=request"));
    assert_eq!(record.http_content_type(), None);
    assert_eq!(record.http_charset(), None);

    Ok(())
}

#[test]
fn warc_record_debug_format() -> io::Result<()> {
    let mut non_http = WarcRecord::new();
    non_http.init_headers(WarcRecordType::Resource, Some(b"urn:uuid:debug-resource"));
    assert_eq!(non_http.content_length(), 0);
    assert_eq!(non_http.headers().get(WarcHeader::ContentLength).as_deref(), Some("0"));
    non_http.set_bytes_payload(b"ABC".to_vec());
    assert_eq!(non_http.content_length(), 3);
    assert_eq!(non_http.headers().get(WarcHeader::ContentLength).as_deref(), Some("3"));

    let non_http_debug = format!("{non_http:?}");
    assert!(non_http_debug.contains("WarcRecord"));
    assert!(non_http_debug.contains("record_type: Resource"));
    assert!(non_http_debug.contains("content_length: 3"));
    assert!(non_http_debug.contains("is_http: false"));
    assert!(!non_http_debug.contains("http_charset"));
    assert!(!non_http_debug.contains("http_headers"));

    let mut http = WarcRecord::from_bytes(http_response_warc_data("<urn:uuid:debug-http>", "Hello"))?;
    http.parse_http()?;

    let http_debug = format!("{http:?}");
    assert!(http_debug.contains("record_type: Response"));
    assert!(http_debug.contains("is_http: true"));
    assert!(http_debug.contains("http_charset: Some(\"utf-8\")"));
    assert!(http_debug.contains("http_headers: Some"));

    Ok(())
}

#[test]
fn write_headers() -> io::Result<()> {
    let http_data = "HTTP/1.1 200 OK\r\n\
                              Content-Length: 123\r\n\
                              Content-Encoding: gzip\r\n\
                              Content-Type: text/html; charset=utf-8\r\n\
                              \r\n";

    let mut headers = HeaderMap::new(HeaderEncoding::Latin1);
    headers.set_status_line("HTTP/1.1 200 OK");
    headers.set("Content-Length", "456");
    headers.set("Content-Encoding", "gzip");
    headers.set("Content-Type", "text/html; charset=utf-8");
    headers.set("Content-Length", "123");

    let mut buf = Vec::with_capacity(http_data.len());
    headers.write(&mut buf)?;
    assert_eq!(String::from_utf8_lossy(&buf), http_data);

    Ok(())
}

#[test]
fn write_record() -> io::Result<()> {
    let payload = b"ABCDE".to_vec();

    let mut default_write = WarcRecord::new();
    default_write.init_headers(WarcRecordType::Resource, Some(b"urn:uuid:write-default"));
    assert_eq!(default_write.content_length(), 0);
    default_write.set_bytes_payload(payload.clone());
    assert_eq!(default_write.content_length(), payload.len() as u64);

    let mut default_serialized = Vec::new();
    let default_bytes_written = default_write.write(&mut default_serialized)?;
    assert_eq!(default_bytes_written, default_serialized.len());
    assert!(String::from_utf8_lossy(&default_serialized).contains("WARC-Type: resource\r\n"));
    assert!(default_serialized.ends_with(b"ABCDE\r\n\r\n"));
    assert!(!String::from_utf8_lossy(&default_serialized).contains("WARC-Block-Digest"));

    Ok(())
}

#[test]
fn write_record_with_block_size() -> io::Result<()> {
    let payload = b"ABCDE".to_vec();

    let mut chunked_write = WarcRecord::new();
    chunked_write.init_headers(WarcRecordType::Resource, Some(b"urn:uuid:write-chunked"));
    assert_eq!(chunked_write.content_length(), 0);
    chunked_write.set_bytes_payload(payload.clone());
    assert_eq!(chunked_write.content_length(), payload.len() as u64);

    let mut chunked_serialized = Vec::new();
    let chunked_bytes_written = chunked_write.write_with_block_size(&mut chunked_serialized, 2)?;
    assert_eq!(chunked_bytes_written, chunked_serialized.len());
    assert!(chunked_serialized.ends_with(b"ABCDE\r\n\r\n"));
    assert!(!String::from_utf8_lossy(&chunked_serialized).contains("WARC-Block-Digest"));

    Ok(())
}

#[test]
fn write_record_with_checksum() -> io::Result<()> {
    let mut checksummed = WarcRecord::from_bytes(http_response_warc_data("<urn:uuid:write-checksum>", "Hello"))?;
    checksummed.parse_http()?;

    let http_headers = b"HTTP/1.1 200 OK\r\n\
                         Content-Type: text/plain; charset=utf-8\r\n\
                         Content-Length: 5\r\n\
                         Server: nginx\r\n\
                         \r\n";
    let expected_payload_digest = format!("sha1:{}", BASE32.encode(&Sha1::digest(b"Hello")));

    let mut block_data = http_headers.to_vec();
    block_data.extend_from_slice(b"Hello");
    let expected_block_digest = format!("sha1:{}", BASE32.encode(&Sha1::digest(&block_data)));

    let mut checksummed_serialized = Vec::new();
    let checksummed_bytes_written = checksummed.write_with_checksum(&mut checksummed_serialized)?;
    let checksummed_text = String::from_utf8_lossy(&checksummed_serialized);
    assert_eq!(checksummed_bytes_written, checksummed_serialized.len());
    assert!(checksummed_text.contains(&format!("WARC-Block-Digest: {expected_block_digest}\r\n")));
    assert!(checksummed_text.contains(&format!("WARC-Payload-Digest: {expected_payload_digest}\r\n")));
    assert!(checksummed_text.contains(&format!("Content-Length: {}\r\n", http_headers.len() + 5)));
    assert!(checksummed_text.contains("HTTP/1.1 200 OK\r\n"));
    assert!(checksummed_serialized.ends_with(b"Hello\r\n\r\n"));

    Ok(())
}

#[test]
fn serialize_preserves_byte_identical_headers_until_mutated() -> io::Result<()> {
    let http_data = "HTTP/1.1 200 OK\r\n\
        X-Bar: abc\r\n\
        \x20def\r\n\
        Content-Length: 3000\r\n\r\n";
    let record_data = format!(
        "WARC/1.1\r\n\
        WARC-Type: resource\r\n\
        WARC-Record-ID: <urn:uuid:continuation>\r\n\
        Content-Type: application/http; msgtype=response\r\n\
        X-Foo: one\r\n\
        \x20two\r\n\
        Content-Length: {}\r\n\
        \r\n\
        {}\
        \r\n\r\n",
        http_data.len(),
        http_data
    );
    let record_bytes = record_data.as_bytes();

    let mut record = WarcRecord::from_bytes(record_bytes.to_vec())?;
    assert_eq!(record.headers().get("X-Foo").as_deref(), Some("one two"));

    // Parsing HTTP does not mutate headers.
    record.parse_http()?;
    assert_eq!(record.http_headers.as_ref().unwrap().get_bytes("X-Bar").as_deref(), Some(b"abc def".as_slice()));

    // Record is preserved byte-identical, including continuation lines.
    let mut serialized = Vec::new();
    let bytes_written = record.write(&mut serialized)?;
    assert_eq!(bytes_written, serialized.len());
    assert_eq!(String::from_utf8_lossy(&serialized), String::from_utf8_lossy(record_bytes));

    // Mutating a header marks it as dirty and serialisation is no longer guaranteed to be byte-identical.
    record.headers_mut().set("X-Bar", "baz");

    let mut mutated_serialized = Vec::new();
    let bytes_written = record.write(&mut mutated_serialized)?;
    assert_eq!(bytes_written, mutated_serialized.len());
    assert_ne!(mutated_serialized, record_bytes);

    // Continuation lines have been combined.
    let mutated_text = String::from_utf8_lossy(&mutated_serialized);
    assert!(mutated_text.contains("X-Foo: one two\r\n"));
    assert!(!mutated_text.contains("X-Foo: one\r\n two\r\n"));

    // HTTP header map was not mutated, so it remains identical.
    assert!(!mutated_text.contains("X-Bar: abc def\r\n"));
    assert!(mutated_text.contains("X-Bar: abc\r\n def\r\n"));

    Ok(())
}

#[test]
fn verify_record_digests() -> io::Result<()> {
    let payload = b"ABC".to_vec();
    let mut record = WarcRecord::new();
    record.init_headers(WarcRecordType::Resource, Some(b"urn:uuid:digest-test"));
    assert_eq!(record.content_length(), 0);
    record.set_bytes_payload(payload.clone());
    assert_eq!(record.content_length(), payload.len() as u64);

    let digest = BASE32.encode(&Sha1::digest(&payload));
    record
        .headers_mut()
        .set(WarcHeader::WarcBlockDigest, format!("sha1:{digest}"));

    // `consume = false` should leave the frozen payload reader rewound for later use.
    assert!(record.verify_block_digest(false).unwrap());
    assert_eq!(record.reader_mut().unwrap().stream_position()?, 0);

    let md5_digest = BASE32.encode(&Md5::digest(&payload));
    record
        .headers_mut()
        .set(WarcHeader::WarcBlockDigest, format!("md5:{md5_digest}"));
    assert!(record.verify_block_digest(false).unwrap());
    record
        .headers_mut()
        .set(WarcHeader::WarcBlockDigest, format!("md5:{}", BASE32.encode(b"xxxxxx")));
    assert!(!record.verify_block_digest(false).unwrap());

    let sha256_digest = BASE32.encode(&Sha256::digest(&payload));
    record
        .headers_mut()
        .set(WarcHeader::WarcBlockDigest, format!("sha256:{sha256_digest}"));
    assert!(record.verify_block_digest(false).unwrap());
    record
        .headers_mut()
        .set(WarcHeader::WarcBlockDigest, format!("sha256:{}", BASE32.encode(b"xxxxxx")));
    assert!(!record.verify_block_digest(false).unwrap());

    let sha512_digest = BASE32.encode(&Sha512::digest(&payload));
    record
        .headers_mut()
        .set(WarcHeader::WarcBlockDigest, format!("sha512:{sha512_digest}"));
    assert!(record.verify_block_digest(false).unwrap());
    record
        .headers_mut()
        .set(WarcHeader::WarcBlockDigest, format!("sha512:{}", BASE32.encode(b"xxxxxx")));
    assert!(!record.verify_block_digest(false).unwrap());

    let sha1_hex_digest = HEXLOWER.encode(&Sha1::digest(&payload));
    record
        .headers_mut()
        .set(WarcHeader::WarcBlockDigest, format!("sha1:{sha1_hex_digest}"));
    assert!(record.verify_block_digest(false).unwrap());
    record
        .headers_mut()
        .set(WarcHeader::WarcBlockDigest, format!("sha1:{}", "0".repeat(sha1_hex_digest.len())));
    assert!(!record.verify_block_digest(false).unwrap());

    // Cover formatting failures.
    record.headers_mut().remove(WarcHeader::WarcBlockDigest);
    assert!(matches!(record.verify_block_digest(false), Err(DigestError::Missing(_))));
    record.headers_mut().set(WarcHeader::WarcBlockDigest, "sha999:AAAA");
    assert!(matches!(record.verify_block_digest(false), Err(DigestError::Unsupported(_))));
    record.headers_mut().set(WarcHeader::WarcBlockDigest, "bad-format");
    assert!(matches!(record.verify_block_digest(false), Err(DigestError::FormatError(_))));
    record.headers_mut().set(WarcHeader::WarcBlockDigest, "sha1:_____");
    assert!(matches!(record.verify_block_digest(false), Err(DigestError::FormatError(_))));
    assert!(matches!(record.verify_payload_digest(false), Err(DigestError::NoPayload(_))));

    let mut http_without_payload_digest =
        WarcRecord::from_bytes(http_response_warc_data("<urn:uuid:missing-payload-digest>", "Hello"))?;
    http_without_payload_digest.parse_http()?;
    assert!(matches!(http_without_payload_digest.verify_payload_digest(false), Err(DigestError::Missing(_))));

    let http_payload = "Hello";
    let http_data = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         \r\n\
         {}",
        http_payload.len(),
        http_payload
    );
    let mut http_with_digests = WarcRecord::from_bytes(warc_record_data_with_headers(
        "response",
        "<urn:uuid:payload-digest-consume>",
        Some("application/http; msgtype=response"),
        &format!(
            "WARC-Block-Digest: sha1:{}\r\n\
                 WARC-Payload-Digest: sha1:{}\r\n",
            BASE32.encode(&Sha1::digest(http_data.as_bytes())),
            BASE32.encode(&Sha1::digest(http_payload.as_bytes()))
        ),
        http_data.as_bytes(),
    ))?;
    assert!(http_with_digests.verify_block_digest(false).unwrap());
    http_with_digests.parse_http()?;
    assert!(http_with_digests.verify_payload_digest(true).unwrap());
    assert!(!http_with_digests.verify_block_digest(false).unwrap());

    let mut writable = WarcRecord::new();
    writable.init_headers(WarcRecordType::Resource, Some(b"urn:uuid:write-test"));
    assert_eq!(writable.content_length(), 0);
    writable.set_bytes_payload(payload.clone());
    assert_eq!(writable.content_length(), payload.len() as u64);

    // Serialize and reparse.
    let mut serialized = Vec::new();
    let bytes_written = writable.write_with_block_size_checksum(&mut serialized, 2, true)?;
    assert_eq!(bytes_written, serialized.len());

    let mut reparsed = WarcRecord::from_reader(io::Cursor::new(serialized))?;
    assert!(reparsed.headers().contains_key("WARC-Block-Digest"));
    assert!(reparsed.verify_block_digest(false).unwrap());

    let mut reparsed_payload = Vec::new();
    reparsed.reader_mut().unwrap().read_to_end(&mut reparsed_payload)?;
    assert_eq!(reparsed_payload, b"ABC");

    Ok(())
}

#[test]
fn verify_record_digest_error_kinds() -> io::Result<()> {
    let payload = b"ABC".to_vec();
    let mut record = WarcRecord::new();
    record.init_headers(WarcRecordType::Resource, Some(b"urn:uuid:digest-errors"));
    assert_eq!(record.content_length(), 0);
    record.set_bytes_payload(payload.clone());
    assert_eq!(record.content_length(), payload.len() as u64);

    record.headers_mut().remove(WarcHeader::WarcBlockDigest);
    assert!(matches!(record.verify_block_digest(false), Err(DigestError::Missing(_))));

    record.headers_mut().set(WarcHeader::WarcBlockDigest, "sha999:AAAA");
    assert!(matches!(record.verify_block_digest(false), Err(DigestError::Unsupported(_))));

    record.headers_mut().set(WarcHeader::WarcBlockDigest, "bad-format");
    assert!(matches!(record.verify_block_digest(false), Err(DigestError::FormatError(_))));

    record.headers_mut().set(WarcHeader::WarcBlockDigest, "sha1:_____");
    assert!(matches!(record.verify_block_digest(false), Err(DigestError::FormatError(_))));

    assert!(matches!(record.verify_payload_digest(false), Err(DigestError::NoPayload(_))));

    let mut stream_error_record = WarcRecord::new();
    stream_error_record.init_headers(WarcRecordType::Resource, Some(b"urn:uuid:digest-stream-error"));
    stream_error_record
        .headers_mut()
        .set(WarcHeader::WarcBlockDigest, "sha1:AAAA");
    assert!(matches!(stream_error_record.verify_block_digest(false), Err(DigestError::StreamError(_))));

    Ok(())
}

#[test]
fn record_consume_and_freeze_stream_payload() -> io::Result<()> {
    let record_data1 = warc_record_data("resource", "<urn:uuid:consume-freeze-1>", None, b"ABCDEF");
    let record_data2 = warc_record_data("metadata", "<urn:uuid:consume-freeze-2>", None, b"XYZ");

    let mut consumed =
        WarcRecord::from_reader(io::Cursor::new([record_data1.as_slice(), record_data2.as_slice()].concat()))?;
    assert_eq!(consumed.consume_n(2)?, 2);
    assert_eq!(consumed.reader_mut().unwrap().stream_position()?, 2);
    assert_eq!(consumed.consume()?, 4);
    assert_eq!(consumed.reader_mut().unwrap().stream_position()?, 6);

    let next = consumed.next().unwrap()?;
    assert_eq!(next.record_id().as_deref(), Some("<urn:uuid:consume-freeze-2>"));

    let mut frozen = WarcRecord::from_reader(io::Cursor::new(record_data1))?;
    let mut prefix = [0u8; 2];
    frozen.reader_mut().unwrap().read_exact(&mut prefix)?;
    assert_eq!(&prefix, b"AB");

    frozen.freeze()?;
    assert!(frozen.is_frozen());
    assert_eq!(frozen.content_length(), 4);

    let mut remaining = Vec::new();
    frozen.reader_mut().unwrap().read_to_end(&mut remaining)?;
    assert_eq!(remaining, b"CDEF");

    Ok(())
}

#[test]
fn record_freeze_preserves_payload_from_current_seek_offset() -> io::Result<()> {
    let file = get_fixture_path("warcfile.warc");
    let record = ArchiveIterator::from_path(&file)?
        .with_parse_http(false)
        .find_map(|rec| {
            let rec = rec.ok()?;
            let is_response = rec.borrow().record_type() == WarcRecordType::Response;
            is_response.then_some(rec)
        })
        .unwrap();

    let mut record = record.borrow_mut();
    let mut skipped = [0u8; 13];
    record.reader_mut().unwrap().read_exact(&mut skipped)?;
    let mut expected_remaining = Vec::new();
    record.reader_mut().unwrap().read_to_end(&mut expected_remaining)?;
    record.reader_mut().unwrap().seek(io::SeekFrom::Start(13))?;
    record.freeze()?;
    assert!(record.is_frozen());
    assert_eq!(record.reader_mut().unwrap().stream_position()?, 0);

    let mut frozen_remaining = Vec::new();
    record.reader_mut().unwrap().read_to_end(&mut frozen_remaining)?;
    assert_eq!(frozen_remaining, expected_remaining);

    Ok(())
}

#[test]
fn record_encoded_http_payload() -> io::Result<()> {
    let payload_raw = b"ABCDEF".repeat(2000);

    let read_record = |payload_encoded: &[u8], tenc, cenc, dec_opts| -> io::Result<Vec<u8>> {
        let data = http_response_warc_data_encoded("<urn:uuid:abc>", payload_encoded, tenc, cenc);
        let mut rec = WarcRecord::from_reader(io::Cursor::new(data))?;
        rec.parse_http_with_decode(dec_opts)?;
        let mut buf = Vec::with_capacity(payload_raw.len());
        rec.reader_mut().unwrap().read_to_end(&mut buf)?;
        Ok(buf)
    };

    // Transfer-Encoding: gzip (decode: None)
    let mut w = gzip::GzipWriter::new(Vec::new());
    w.write_all(&payload_raw)?;
    let encoded = w.into_inner()?;
    let decoded = read_record(&encoded, Some("gzip"), None, AutoDecode::None)?;
    assert_eq!(decoded, encoded);

    // Transfer-Encoding: gzip, Content-Encoding: gzip (decode: All)
    let mut w = gzip::GzipWriter::new(Vec::new());
    w.write_all(&payload_raw)?;
    let encoded = w.into_inner()?;
    let mut w = gzip::GzipWriter::new(Vec::new());
    w.write_all(&encoded)?;
    let encoded = w.into_inner()?;
    let decoded = read_record(&encoded, Some("gzip"), Some("gzip"), AutoDecode::All)?;
    assert_eq!(decoded, payload_raw);

    // Transfer-Encoding: gzip, Content-Encoding: br (decode: Transfer)
    let mut w = brotli::BrotliWriter::new(Vec::new());
    w.write_all(&payload_raw)?;
    let encoded = w.into_inner()?;
    let mut w = gzip::GzipWriter::new(Vec::new());
    w.write_all(&encoded)?;
    let encoded = w.into_inner()?;
    let mut w = brotli::BrotliWriter::new(Vec::new());
    w.write_all(&payload_raw)?;
    let encoded_content = w.into_inner()?;
    let decoded = read_record(&encoded, Some("gzip"), Some("br"), AutoDecode::TransferEncoding)?;
    assert_eq!(decoded, encoded_content);

    // Transfer-Encoding: zstd, deflate (decode: Transfer)
    let mut w = zstd::ZstdWriter::new(Vec::new());
    w.write_all(&payload_raw)?;
    let encoded = w.into_inner()?;
    let mut w = gzip::GzipWriter::new_deflate(Vec::new());
    w.write_all(&encoded)?;
    let encoded = w.into_inner()?;
    let decoded = read_record(&encoded, Some("zstd, deflate"), None, AutoDecode::TransferEncoding)?;
    assert_eq!(decoded, payload_raw);

    // Transfer-Encoding: chunked (decode: Transfer)
    let mut w = chunked::ChunkedWriter::new(Vec::new());
    w.write_all(&payload_raw)?;
    let encoded = w.into_inner()?;
    let decoded = read_record(&encoded, Some("chunked"), None, AutoDecode::TransferEncoding)?;
    assert_eq!(decoded, payload_raw);

    // Transfer-Encoding: chunked, Content-Encoding: gzip (decode: All)
    let mut w = gzip::GzipWriter::new(Vec::new());
    w.write_all(&payload_raw)?;
    let encoded = w.into_inner()?;
    let mut w = chunked::ChunkedWriter::new(Vec::new());
    w.write_all(&encoded)?;
    let encoded = w.into_inner()?;
    let decoded = read_record(&encoded, Some("chunked"), Some("gzip"), AutoDecode::All)?;
    assert_eq!(decoded, payload_raw);

    // Transfer-Encoding: identity (decode: Transfer)
    let decoded = read_record(payload_raw.as_slice(), Some("identity"), None, AutoDecode::TransferEncoding)?;
    assert_eq!(decoded, payload_raw);

    // Content-Encoding: unsupported (decode: Transfer)
    let decoded = read_record(payload_raw.as_slice(), None, Some("unsupported"), AutoDecode::TransferEncoding)?;
    assert_eq!(decoded, payload_raw);

    // Content-Encoding: unsupported (decode: Content)
    let decoded = read_record(payload_raw.as_slice(), None, Some("unsupported"), AutoDecode::ContentEncoding);
    assert!(decoded.is_err());

    Ok(())
}

#[test]
fn record_encoded_http_payload_frozen_record() -> io::Result<()> {
    let payload_raw = b"ABCDEF".repeat(2000);

    // Encode payload and check that output starts with Gzip magic bytes.
    let mut w = gzip::GzipWriter::new(Vec::new());
    w.write_all(&payload_raw)?;
    let payload_encoded = w.into_inner()?;
    assert_eq!(payload_encoded[..2], [0x1F, 0x8B]);

    // Create HTTP WARC record with encoded payload.
    let record_data = http_response_warc_data_encoded("<urn:uuid:abc>", &payload_encoded, Some("gzip"), None);
    let mut rec = WarcRecord::from_reader(io::Cursor::new(record_data.clone()))?;

    // Freeze record
    rec.freeze()?;

    // Parse HTTP and decode content.
    rec.parse_http_with_decode(AutoDecode::TransferEncoding)?;
    let mut decoded = Vec::with_capacity(payload_raw.len());
    rec.reader_mut().unwrap().read_to_end(&mut decoded)?;
    assert_eq!(decoded, payload_raw);

    // Rewind frozen stream and read again (same result)
    decoded.clear();
    rec.reader_mut().unwrap().rewind()?;
    rec.reader_mut().unwrap().read_to_end(&mut decoded)?;
    assert_eq!(decoded, payload_raw);

    // Detach and rewind reader.
    let mut detached = rec.detach_reader().unwrap();
    detached.rewind()?;

    // Create new record from stream and HTTP without content decoding.
    let mut rec = WarcRecord::from_reader(detached)?;
    rec.parse_http_with_decode(AutoDecode::None)?;

    // Test that stream begins with Gzip magic bytes
    let mut magic_bytes = [0u8; 2];
    rec.reader_mut().unwrap().read_exact(&mut magic_bytes)?;
    assert_eq!(magic_bytes, payload_encoded[..2]);
    assert_ne!(magic_bytes, payload_raw[..2]);

    // Test freezing and eager decoding don't mess with record boundaries.
    let record_data = [record_data.clone(), record_data].concat();
    let mut count = 0;
    for r in ArchiveIterator::new(io::Cursor::new(record_data)).with_parse_http(false) {
        r?.with_mut(|r| -> io::Result<()> {
            r.freeze()?;
            r.parse_http_with_decode(AutoDecode::TransferEncoding)?;
            let mut buf = Vec::with_capacity(r.content_length() as usize);
            r.reader_mut().unwrap().read_to_end(&mut buf)?;
            assert_eq!(buf, payload_raw);
            count += 1;
            Ok(())
        })?;
    }
    assert_eq!(count, 2);

    Ok(())
}
