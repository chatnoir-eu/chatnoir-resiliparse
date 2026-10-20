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
use rstest::rstest;
use std::borrow::Cow;
use std::io::{self, Read};

#[test]
fn record_type_and_header_map_helpers() {
    let mut headers = HeaderMap::new(HeaderEncoding::Unicode);
    headers.set_status_line("HTTP/1.1 204");
    headers.append("Set-Cookie", "a=1");
    headers.append("set-cookie", "b=2");

    assert_eq!(headers.encoding(), HeaderEncoding::Unicode);
    assert_eq!(headers.status_code(), Some(204));
    assert_eq!(headers.reason_phrase(), None);
    assert!(headers.contains_key_bytes(b"SET-cookie"));

    let merged = headers.to_map();
    assert_eq!(merged.get(&ci("set-cookie")), Some(&"a=1,b=2".to_string()));

    headers.remove_bytes(b"set-cookie");
    assert!(!headers.contains_key("Set-Cookie"));
    headers.clear();
    assert!(headers.is_empty());
    assert_eq!(headers.status_line(), None);
}

#[test]
fn new_empty_header_map() {
    let headers = HeaderMap::new(HeaderEncoding::Unicode);
    assert_eq!(headers.encoding(), HeaderEncoding::Unicode);
    assert_eq!(headers.len(), 0);
    assert!(headers.is_empty());
    assert_eq!(headers.status_code(), None);
}

#[test]
fn set_get_remove_header() {
    let mut headers = HeaderMap::new(HeaderEncoding::Latin1);
    headers.set(WarcHeader::ContentType, "text/plain");
    assert_eq!(headers.encoding(), HeaderEncoding::Latin1);
    assert_eq!(headers.get(WarcHeader::ContentType).as_deref(), Some("text/plain"));
    assert_eq!(headers.len(), 1);

    // Override existing
    headers.set(WarcHeader::ContentType, "text/html");
    assert_eq!(headers.get(WarcHeader::ContentType).as_deref(), Some("text/html"));
    assert_eq!(headers.len(), 1);

    // Add new
    headers.set(WarcHeader::ContentLength, "10");
    assert_eq!(headers.get(WarcHeader::ContentLength).as_deref(), Some("10"));
    assert_eq!(headers.len(), 2);

    // Set and get as bytes
    headers.set_bytes(WarcHeader::ContentType, b"text/plain");
    assert_eq!(headers.get(WarcHeader::ContentType).as_deref(), Some("text/plain"));
    assert_eq!(headers.get_bytes(WarcHeader::ContentType).as_deref(), Some(b"text/plain".as_slice()));
    assert_eq!(headers.len(), 2);

    // Header does not exist
    assert_eq!(headers.get("Missing-Header"), None);

    // Remove (case-insensitive)
    headers.remove("CONTENT-TYPE");
    assert_eq!(headers.get(WarcHeader::ContentType), None);
    assert_eq!(headers.len(), 1);
}

#[test]
fn duplicate_header() {
    let mut headers = HeaderMap::new(HeaderEncoding::Latin1);
    assert_eq!(headers.len(), 0);
    assert_eq!(headers.get_multiple(WarcHeader::ContentType), Vec::<&str>::new());

    // Set
    headers.set(WarcHeader::ContentType, "text/plain");
    assert_eq!(headers.get(WarcHeader::ContentType).as_deref(), Some("text/plain"));
    assert_eq!(headers.len(), 1);
    assert_eq!(headers.get_multiple(WarcHeader::ContentType), vec!["text/plain"]);

    // Set again
    headers.set(WarcHeader::ContentType, "text/html");
    assert_eq!(headers.get(WarcHeader::ContentType).as_deref(), Some("text/html"));
    assert_eq!(headers.len(), 1);
    assert_eq!(headers.get_multiple(WarcHeader::ContentType), vec!["text/html"]);

    // Append duplicate
    headers.append(WarcHeader::ContentType, "text/plain");
    assert_eq!(headers.get(WarcHeader::ContentType).as_deref(), Some("text/html"));
    assert_eq!(headers.len(), 2);
    assert_eq!(headers.get_multiple(WarcHeader::ContentType), vec!["text/html", "text/plain"]);
    assert_eq!(
        headers.get_bytes_multiple(WarcHeader::ContentType),
        vec![b"text/html".as_slice(), b"text/plain".as_slice()]
    );

    // Remove (case-insensitive)
    headers.remove("CONTENT-TYPE");
    assert_eq!(headers.get(WarcHeader::ContentType), None);
    assert_eq!(headers.len(), 0);
}

#[test]
fn header_case_insensitive_key() {
    let mut headers = HeaderMap::new(HeaderEncoding::Unicode);
    headers.set(WarcHeader::ContentType, "text/html");
    assert!(headers.keys().any(|k| k == "Content-Type"));
    assert_eq!(headers.get("CONTENT-TYPE").as_deref(), Some("text/html"));
    assert_eq!(headers.get("content-type").as_deref(), Some("text/html"));
    assert_eq!(headers.get("CoNtEnT-TyPe").as_deref(), Some("text/html"));

    headers.set("CONTENT-TYPE", "text/html");
    assert!(headers.keys().any(|k| k == "CONTENT-TYPE"));
}

#[test]
fn header_case_insensitive_key_from_helpers() {
    let borrowed_key = CaseInsensitiveKey::from(WarcHeader::ContentType.as_str());
    let owned_key = CaseInsensitiveKey::from("content-type".to_string());
    let cow_key = CaseInsensitiveKey::from(Cow::Borrowed("CONTENT-TYPE"));
    assert_eq!(borrowed_key, "content-type");
    assert_eq!("CONTENT-TYPE", owned_key);
    assert_eq!(String::from(cow_key), "CONTENT-TYPE");
}

#[test]
fn header_case_insensitive_key_eq_and_deref() {
    let borrowed_key = CaseInsensitiveKey::from(WarcHeader::ContentType.as_str());
    let owned_key = CaseInsensitiveKey::from("content-type".to_string());

    assert_eq!(borrowed_key, owned_key);
    assert_eq!(borrowed_key, *"CONTENT-TYPE");
    assert_eq!(borrowed_key, "content-type");
    assert_eq!(*"CONTENT-TYPE", borrowed_key);
    assert_eq!("content-type", borrowed_key);
    assert_ne!(borrowed_key, "Accept");

    assert_eq!(borrowed_key.as_ref(), WarcHeader::ContentType.as_str());
}

#[test]
fn iterate_headers() {
    let tuples = [
        (WarcHeader::ContentType.as_str(), "text/html"),
        (WarcHeader::ContentLength.as_str(), "1234"),
        ("Set-Cookie", "cookie1=value1"),
        ("Set-Cookie", "cookie2=value2"),
    ];

    let mut headers = HeaderMap::new(HeaderEncoding::Latin1);
    headers.set(tuples[0].0, tuples[0].1);
    headers.set(tuples[1].0, tuples[1].1);
    headers.append(tuples[2].0, tuples[2].1);
    headers.append(tuples[3].0, tuples[3].1);

    let mut count = 0;
    for ((k, v), (kb, vb)) in headers.items().zip(headers.items_bytes()) {
        assert_eq!(k, tuples[count].0);
        assert_eq!(kb, tuples[count].0.as_bytes());
        assert_eq!(v, tuples[count].1);
        assert_eq!(vb, tuples[count].1.as_bytes());
        count += 1;
    }
    assert_eq!(count, 4);

    count = 0;
    for (k, kb) in headers.keys().zip(headers.keys_bytes()) {
        assert_eq!(k, tuples[count].0);
        assert_eq!(kb, tuples[count].0.as_bytes());
        count += 1;
    }
    assert_eq!(count, 4);

    count = 0;
    for (v, vb) in headers.values().zip(headers.values_bytes()) {
        assert_eq!(v, tuples[count].1);
        assert_eq!(vb, tuples[count].1.as_bytes());
        count += 1;
    }
    assert_eq!(count, 4);
}

#[test]
fn header_sanitization() {
    let mut headers = HeaderMap::new(HeaderEncoding::Unicode);
    headers.set("Content-Type:", "text/html; charset=utf-8");
    headers.set("Foo:bar", "bar:baz");
    headers.set("new\r\nline", "new\t\nline");

    assert_eq!(headers.get(WarcHeader::ContentType).as_deref(), Some("text/html; charset=utf-8"));
    assert_eq!(headers.get("Foobar").as_deref(), Some("bar:baz"));
    assert_eq!(headers.get("new line").as_deref(), Some("new\t line"));
}

#[test]
fn parse_headers_with_empty_values() -> io::Result<()> {
    let header_data = b"HTTP/1.1 200\r\n\
                                  alt-svc: h3=\":443\"; ma=86400\r\n\
                                  cache-control: max-age=0, private, must-revalidate\r\n\
                                  cf-cache-status: DYNAMIC\r\n\
                                  cf-ray: a09215a55df4ec03-IAD\r\n\
                                  content-security-policy: \r\n\
                                  \r\n";

    let mut headers = HeaderMap::new(HeaderEncoding::Latin1);
    let mut reader = io::Cursor::new(header_data);
    headers.parse(&mut reader, true)?;
    // No slicing errors etc.
    assert_eq!(headers.get("content-security-policy").as_deref(), Some(""));
    Ok(())
}

#[test]
fn parse_headers_with_continuation_lines() -> io::Result<()> {
    let http_data = b"HTTP/1.1 200 OK\r\n\
                              Content-Length: 123\r\n\
                              Content-Encoding     :     gzip    \r\n\
                              Content-Type: text/html;\r\n  charset=utf-8\r\n\
                              Invalid-Header-Ignored\r\n\
                              Accept: text/html,\r\n\tapplication/json,\r\n\ttext/plain\r\n\
                              \r\n";

    let mut headers = HeaderMap::new(HeaderEncoding::Latin1);
    let mut reader = io::Cursor::new(http_data);
    headers.parse(&mut reader, true)?;

    assert_eq!(headers.get("Content-Length").as_deref(), Some("123"));
    assert_eq!(headers.get("Content-Encoding").as_deref(), Some("gzip"));
    assert_eq!(headers.get("Content-Type").as_deref(), Some("text/html; charset=utf-8"));
    assert_eq!(headers.get("Accept").as_deref(), Some("text/html, application/json, text/plain"));

    assert!(!headers.keys().any(|k| k == "Invalid-Header-Ignored"));
    assert!(!headers.contains_key("Invalid-Header-Ignored"));
    assert!(!headers.values().any(|k| k == "Invalid-Header-Ignored"));

    Ok(())
}

#[rstest]
#[case::lf("\n")]
#[case::crlf("\r\n")]
fn parse_headers_with_split_eoh_marker(#[case] sep: &str) -> io::Result<()> {
    // EOH markers split at buffer boundaries should not cause issues.
    let http_headers = format!("HTTP/1.1 200 OK{sep}Content-Length: 123{sep}{sep}");
    let http_data = [http_headers.as_bytes(), b"payload"].concat();

    let mut headers = HeaderMap::new(HeaderEncoding::Latin1);
    let mut reader = io::BufReader::with_capacity(1, io::Cursor::new(http_data));
    let quirks_mode = sep == "\n";
    let bytes_read = headers.parse_with_opts(&mut reader, true, 8192, quirks_mode)?;

    assert_eq!(bytes_read, http_headers.len());
    assert_eq!(headers.status_line().as_deref(), Some("HTTP/1.1 200 OK"));
    assert_eq!(headers.get("Content-Length").as_deref(), Some("123"));

    let mut payload = Vec::new();
    reader.read_to_end(&mut payload)?;
    assert_eq!(payload, b"payload");

    Ok(())
}

#[test]
fn parse_headers_with_partial_crlf_prefix_before_split_eoh_marker() -> io::Result<()> {
    // Stray \r before CRLF EOH marker should not cause issues even if EOH is split.
    let http_headers = b"HTTP/1.1 200 OK\r\nX-Stray: value\r\r\n\r\n";
    let http_data = [http_headers.as_slice(), b"payload"].concat();

    let mut headers = HeaderMap::new(HeaderEncoding::Latin1);
    let mut reader = io::BufReader::with_capacity(1, io::Cursor::new(http_data));
    let bytes_read = headers.parse(&mut reader, true)?;

    assert_eq!(bytes_read, http_headers.len());
    assert_eq!(headers.status_line().as_deref(), Some("HTTP/1.1 200 OK"));
    assert_eq!(headers.get("X-Stray").as_deref(), Some("value"));

    let mut payload = Vec::new();
    reader.read_to_end(&mut payload)?;
    assert_eq!(payload, b"payload");

    Ok(())
}

#[test]
fn parse_headers_with_lf_line_endings_in_quirks_mode() -> io::Result<()> {
    // HTTP headers with mixed LF and CRLF line endings.
    let http_headers = "HTTP/1.1 200 OK\nContent-Length: 123\nContent-Type: text/plain\r\nContent-Encoding: gzip\n\n";
    let http_data = format!("{}payload", http_headers).as_bytes().to_vec();

    let mut strict_headers = HeaderMap::new(HeaderEncoding::Latin1);
    let mut strict_reader = io::Cursor::new(http_data.clone());
    strict_headers.parse_with_opts(&mut strict_reader, true, 8192, false)?;
    // Incorrect status line split, only Content-Encoding separated with CRLF.
    assert!(strict_headers.status_line().as_deref().unwrap().ends_with("text/plain"));
    assert!(strict_headers.get("Content-Length").is_none());
    assert!(strict_headers.get("Content-Type").is_none());
    assert!(strict_headers.get("Content-Encoding").is_some());

    let mut quirks_headers = HeaderMap::new(HeaderEncoding::Latin1);
    let mut quirks_reader = io::Cursor::new(http_data.clone());
    let bytes_read = quirks_headers.parse_with_opts(&mut quirks_reader, true, 8192, true)?;

    assert_eq!(bytes_read, http_headers.len());
    assert_eq!(quirks_headers.status_line().as_deref(), Some("HTTP/1.1 200 OK"));
    assert_eq!(quirks_headers.get("Content-Length").as_deref(), Some("123"));
    assert_eq!(quirks_headers.get("Content-Type").as_deref(), Some("text/plain"));
    assert_eq!(quirks_headers.get("Content-Encoding").as_deref(), Some("gzip"));
    assert_eq!(quirks_reader.position(), bytes_read as u64);

    Ok(())
}

#[test]
fn write_preserves_non_standard_or_broken_headers_if_not_modified() -> io::Result<()> {
    // Non-standard line endings, continuations, broken Unicode header, no value
    let http_headers = b"HTTP/1.1 200 OK\n\
        Content-Length: 123\n\
        X-Foo: test\n\
        X-Cont: abc\n\
        \x20def\n\
        X-Broken-\xFD-Unicode: test\xFE\n\
        No-Key\n\
        \n";

    let mut headers = HeaderMap::new(HeaderEncoding::Unicode);
    let mut reader = io::Cursor::new(http_headers);
    let bytes_read = headers.parse_with_opts(&mut reader, true, 8192, true)?;

    assert_eq!(bytes_read, http_headers.len());
    assert_eq!(headers.get_bytes(b"X-Foo").as_deref(), Some(b"test".as_slice()));

    // Continuations are sanitised here, but writing the header out should preserve them.
    assert_eq!(headers.get_bytes(b"X-Cont").as_deref(), Some(b"abc def".as_slice()));

    // Broken Unicode is preserved
    assert_eq!(headers.get_bytes(b"X-Broken-\xFD-Unicode").as_deref(), Some(b"test\xFE".as_slice()));

    // Header without key missing from map.
    assert!(!headers.contains_key_bytes(b"No-Key"));

    // Header map was not mutated, output should be byte-identical.
    let mut serialized = Vec::new();
    let bytes_written = headers.write(&mut serialized)?;

    assert_eq!(bytes_written, http_headers.len());
    assert_eq!(serialized, http_headers);

    Ok(())
}

#[test]
fn new_empty_header_encoding() -> io::Result<()> {
    let mut headers_unicode = HeaderMap::new(HeaderEncoding::Unicode);
    let mut headers_latin1 = HeaderMap::new(HeaderEncoding::Latin1);

    let utf8_value = "abcäöü";
    let latin1_bytes = WINDOWS_1252.encode(utf8_value, EncoderTrap::Ignore).unwrap_or_default();

    // Test Unicode encoding
    headers_unicode.set("X-Utf8", utf8_value);
    assert_eq!(headers_unicode.get("X-Utf8").as_deref(), Some(utf8_value));
    assert_eq!(headers_unicode.get_bytes(b"X-Utf8").as_deref(), Some(utf8_value.as_bytes()));

    // Test Latin1 encoding
    headers_latin1.set("X-Latin1", utf8_value);
    assert_eq!(headers_latin1.get("X-Latin1").as_deref(), Some(utf8_value));
    assert_eq!(headers_latin1.get_bytes(b"X-Latin1").as_deref(), Some(latin1_bytes.as_slice()));

    // Incorrect decodings
    let latin_value_utf8_dec_lossy = "abc���";
    let utf8_value_latin_dec = "abcÃ¤Ã¶Ã¼";

    // Test incorrect UTF-8 decoding of Latin bytes (irreversible)
    headers_unicode.set_bytes(b"X-Latin1-Utf8", latin1_bytes.as_slice());
    assert_eq!(headers_unicode.get("X-Latin1-Utf8").as_deref(), Some(latin_value_utf8_dec_lossy));
    assert_eq!(headers_unicode.get_bytes(b"X-Latin1-Utf8").as_deref(), Some(latin1_bytes.as_slice()));

    // Test incorrect Latin1 decoding of UTF-8 bytes (reversible)
    headers_latin1.set_bytes(b"X-Utf8-Latin1", utf8_value.as_bytes());
    assert_eq!(headers_latin1.get("X-Utf8-Latin1").as_deref(), Some(utf8_value_latin_dec));
    assert_eq!(headers_latin1.get_bytes(b"X-Utf8-Latin1").as_deref(), Some(utf8_value.as_bytes()));

    // Invalid UTF-8 sequence
    let invalid_utf8 = b"abc\xff\xfedef";
    let invalid_utf8_dec_lossy = "abc��def";
    let invalid_utf8_latin_dec = "abcÿþdef";

    // Test UTF-8 decoding with invalid UTF-8 sequence
    headers_unicode.set_bytes(b"X-Invalid", invalid_utf8);
    // Bytes should be the same
    assert_eq!(headers_unicode.get_bytes(b"X-Invalid").as_deref(), Some(invalid_utf8.as_ref()));
    // Decoding is lossy
    assert_eq!(headers_unicode.get("X-Invalid").as_deref(), Some(invalid_utf8_dec_lossy));

    // Test Latin decoding with invalid UTF-8 sequence
    headers_latin1.set_bytes(b"X-Invalid", invalid_utf8);
    // Bytes should be the same
    assert_eq!(headers_latin1.get_bytes(b"X-Invalid").as_deref(), Some(invalid_utf8.as_ref()));
    // Decodes to strange characters
    assert_eq!(headers_latin1.get("X-Invalid").as_deref(), Some(invalid_utf8_latin_dec));

    Ok(())
}
