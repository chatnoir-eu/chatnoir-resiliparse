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
use std::io;

#[test]
fn test_new_empty_header_map() {
    let headers = HeaderMap::new(HeaderEncoding::Unicode);
    assert_eq!(headers.len(), 0);
    assert!(headers.is_empty());
    assert_eq!(headers.status_code(), None);
}

#[test]
fn test_set_get_remove_header() {
    let mut headers = HeaderMap::new(HeaderEncoding::Latin1);
    headers.set("Content-Type", "text/plain");
    assert_eq!(headers.get("Content-Type").as_deref(), Some("text/plain"));
    assert_eq!(headers.len(), 1);

    // Override existing
    headers.set("Content-Type", "text/html");
    assert_eq!(headers.get("Content-Type").as_deref(), Some("text/html"));
    assert_eq!(headers.len(), 1);

    // Add new
    headers.set("Content-Length", "10");
    assert_eq!(headers.get("Content-Length").as_deref(), Some("10"));
    assert_eq!(headers.len(), 2);

    // Set and get as bytes
    headers.set_bytes(b"Content-Type", b"text/plain");
    assert_eq!(headers.get("Content-Type").as_deref(), Some("text/plain"));
    assert_eq!(headers.get_bytes(b"Content-Type").as_deref(), Some(b"text/plain".as_slice()));
    assert_eq!(headers.len(), 2);

    // Header does not exist
    assert_eq!(headers.get("Missing-Header"), None);

    // Remove (case-insensitive)
    headers.remove("CONTENT-TYPE");
    assert_eq!(headers.get("Content-Type"), None);
    assert_eq!(headers.len(), 1);
}

#[test]
fn test_duplicate_header() {
    let mut headers = HeaderMap::new(HeaderEncoding::Latin1);
    assert_eq!(headers.len(), 0);
    assert_eq!(headers.get_multiple("Content-Type"), Vec::<&str>::new());

    // Set
    headers.set("Content-Type", "text/plain");
    assert_eq!(headers.get("Content-Type").as_deref(), Some("text/plain"));
    assert_eq!(headers.len(), 1);
    assert_eq!(headers.get_multiple("Content-Type"), vec!["text/plain"]);

    // Set again
    headers.set("Content-Type", "text/html");
    assert_eq!(headers.get("Content-Type").as_deref(), Some("text/html"));
    assert_eq!(headers.len(), 1);
    assert_eq!(headers.get_multiple("Content-Type"), vec!["text/html"]);

    // Append duplicate
    headers.append("Content-Type", "text/plain");
    assert_eq!(headers.get("Content-Type").as_deref(), Some("text/html"));
    assert_eq!(headers.len(), 2);
    assert_eq!(headers.get_multiple("Content-Type"), vec!["text/html", "text/plain"]);

    // Remove (case-insensitive)
    headers.remove("CONTENT-TYPE");
    assert_eq!(headers.get("Content-Type"), None);
    assert_eq!(headers.len(), 0);
}

#[test]
fn test_header_case_insensitive() {
    let mut headers = HeaderMap::new(HeaderEncoding::Latin1);
    headers.set("Content-Type", "text/html");
    assert!(headers.keys().any(|k| k == "Content-Type"));
    assert_eq!(headers.get("CONTENT-TYPE").as_deref(), Some("text/html"));
    assert_eq!(headers.get("content-type").as_deref(), Some("text/html"));
    assert_eq!(headers.get("CoNtEnT-TyPe").as_deref(), Some("text/html"));

    headers.set("CONTENT-TYPE", "text/html");
    assert!(headers.keys().any(|k| k == "CONTENT-TYPE"));
}

#[test]
fn test_new_empty_header_encoding() -> io::Result<()> {
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

#[test]
fn test_parse_warc_headers() -> io::Result<()> {
    let record_data1 = "WARC/1.1\r\n\
                             WARC-Type: request\r\n\
                             WARC-Record-ID: <urn:uuid:259bd4e8-b820-4a11-b14b-8f25e573f071>\r\n\
                             Content-Length: 3\r\n\
                             \r\n\
                             ABC\r\n\r\n";
    let record_data2 = "WARC/1.1\r\n\
                             WARC-Type: response\r\n\
                             WARC-Record-ID: <urn:uuid:e480bf84-e412-461e-9e24-9081daa79945>\r\n\
                             Content-Length: 6\r\n\
                             \r\n\
                             DEFGHI\r\n\r\n";
    let warc_data = format!("{}{}", record_data1, record_data2).as_bytes().to_vec();

    let reader = Box::new(io::Cursor::new(warc_data));
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
    assert!(!record1.is_http());
    assert_eq!(headers.get("WARC-Type").as_deref(), Some("request"));
    assert_eq!(headers.get_bytes(b"WARC-Type").as_deref(), Some(b"request".as_slice()));
    assert_eq!(record1.record_id().as_deref(), Some("<urn:uuid:259bd4e8-b820-4a11-b14b-8f25e573f071>"));
    assert_eq!(headers.get("WARC-Record-ID").as_deref(), Some("<urn:uuid:259bd4e8-b820-4a11-b14b-8f25e573f071>"));
    assert_eq!(
        headers.get_bytes(b"WARC-Record-ID").as_deref(),
        Some(b"<urn:uuid:259bd4e8-b820-4a11-b14b-8f25e573f071>".as_slice())
    );
    assert_eq!(headers.get("Content-Length").as_deref(), Some("3"));
    assert_eq!(headers.get_bytes(b"Content-Length").as_deref(), Some(b"3".as_slice()));

    let mut buf = Vec::new();
    record1.reader_mut().unwrap().read_to_end(&mut buf)?;
    assert_eq!(buf, "ABC".as_bytes());

    // Parse second record (construct directly from stream)
    let reader = record1.detach_reader().unwrap();
    let mut record2 = WarcRecord::from_reader(reader)?;

    assert_eq!(record2.stream_pos(), record_data1.len());
    assert_eq!(record2.content_length(), 6);
    assert_eq!(record2.record_type(), WarcRecordType::Response);

    buf.clear();
    record2.reader_mut().unwrap().read_to_end(&mut buf)?;
    assert_eq!(buf, "DEFGHI".as_bytes());

    Ok(())
}

#[test]
fn test_parse_http_headers() -> io::Result<()> {
    let http_payload = "Hello World";
    let http_data = format!(
        "HTTP/1.1 200 OK\r\n\
            Content-Type: text/plain; charset=utf-8\r\n\
            Content-Length: {}\r\n\
            Server: Apache/2.4\r\n\
            \r\n\
            {}",
        http_payload.len(),
        http_payload
    );

    let warc_data = format!(
        "WARC/1.1\r\n\
            WARC-Type: response\r\n\
            WARC-Record-ID: <urn:uuid:259bd4e8-b820-4a11-b14b-8f25e573f071>\r\n\
            Content-Type: application/http; msgtype=response\r\n\
            Content-Length: {} \r\n\
            \r\n\
            {}",
        http_data.len(),
        http_data
    )
    .as_bytes()
    .to_vec();

    let reader = Box::new(io::Cursor::new(warc_data));
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
    assert_eq!(http_headers.get("Content-Type").as_deref(), Some("text/plain; charset=utf-8"));
    assert_eq!(record.http_charset(), Some("utf-8"));
    assert_eq!(record.http_content_type().as_deref(), Some("text/plain"));

    let mut buf = Vec::new();
    record.reader_mut().unwrap().read_to_end(&mut buf)?;
    assert_eq!(buf, http_payload.as_bytes());

    Ok(())
}

// #[test]
// fn test_parse_headers_with_continuation_lines() {
//     let http_data = b"HTTP/1.1 200 OK\r\n\
//                      Content-Type: text/html;\r\n\
//                       charset=utf-8\r\n\
//                      Accept: text/html,\r\n\
//                       application/json,\r\n\
//                       text/plain\r\n\
//                      \r\n";
//
//     let mut cursor = Cursor::new(http_data);
//     let headers = HeaderMap::from_http_reader(&mut cursor).unwrap();
//
//     assert_eq!(headers.get("Content-Type"), Some("text/html; charset=utf-8"));
//     assert_eq!(headers.get("Accept"), Some("text/html, application/json, text/plain"));
// }
//
// #[test]
// fn test_write_warc_headers() {
//     let mut headers = HeaderMap::new(HeaderEncoding::Unicode);
//     headers.set("WARC-Type", "response");
//     headers.set("WARC-Record-ID", "<urn:uuid:12345678-1234-1234-1234-123456789abc>");
//     headers.set("Content-Length", "1234");
//
//     let mut buffer = Vec::new();
//     headers.write_warc(&mut buffer).unwrap();
//
//     let output = String::from_utf8(buffer).unwrap();
//     assert!(output.contains("WARC/1.0\r\n"));
//     assert!(output.contains("WARC-Type: response\r\n"));
//     assert!(output.contains("WARC-Record-ID: <urn:uuid:12345678-1234-1234-1234-123456789abc>\r\n"));
//     assert!(output.contains("Content-Length: 1234\r\n"));
//     assert!(output.ends_with("\r\n"));
// }
//
// #[test]
// fn test_write_http_headers() {
//     let mut headers = HeaderMap::new(HeaderEncoding::Unicode);
//     headers.set("Content-Type", "text/html");
//     headers.set("Content-Length", "5678");
//     headers.set("Server", "Apache/2.4");
//
//     let mut buffer = Vec::new();
//     headers.write_http(&mut buffer, "HTTP/1.1 200 OK").unwrap();
//
//     let output = String::from_utf8(buffer).unwrap();
//     assert!(output.starts_with("HTTP/1.1 200 OK\r\n"));
//     assert!(output.contains("Content-Type: text/html\r\n"));
//     assert!(output.contains("Content-Length: 5678\r\n"));
//     assert!(output.contains("Server: Apache/2.4\r\n"));
//     assert!(output.ends_with("\r\n"));
// }
//
// #[test]
// fn test_get_all_values() {
//     let mut headers = HeaderMap::new(HeaderEncoding::Unicode);
//     headers.append("Cache-Control", "no-cache");
//     headers.append("Cache-Control", "no-store");
//     headers.set("Content-Type", "text/html");
//
//     let cache_values: Vec<&str> = headers.get_all("Cache-Control").collect();
//     assert_eq!(cache_values.len(), 2);
//
//     let type_values: Vec<&str> = headers.get_all("Content-Type").collect();
//     assert_eq!(type_values.len(), 1);
//
//     let missing_values: Vec<&str> = headers.get_all("Missing-Header").collect();
//     assert_eq!(missing_values.len(), 0);
// }
//
// #[test]
// fn test_contains_header() {
//     let mut headers = HeaderMap::new(HeaderEncoding::Unicode);
//     headers.set("Content-Type", "text/html");
//
//     assert!(headers.contains("Content-Type"));
//     assert!(headers.contains("content-type"));
//     assert!(headers.contains("CONTENT-TYPE"));
//     assert!(!headers.contains("Content-Length"));
// }
//
// #[test]
// fn test_iterate_headers() {
//     let mut headers = HeaderMap::new(HeaderEncoding::Unicode);
//     headers.set("Content-Type", "text/html");
//     headers.set("Content-Length", "1234");
//     headers.append("Set-Cookie", "cookie1=value1");
//     headers.append("Set-Cookie", "cookie2=value2");
//
//     let mut count = 0;
//     for (name, value) in headers.iter() {
//         count += 1;
//         assert!(!name.is_empty());
//         assert!(!value.is_empty());
//     }
//     assert_eq!(count, 4);
// }
//
// #[test]
// fn test_clear_headers() {
//     let mut headers = HeaderMap::new(HeaderEncoding::Unicode);
//     headers.set("Content-Type", "text/html");
//     headers.set("Content-Length", "1234");
//     assert_eq!(headers.len(), 2);
//
//     headers.clear();
//     assert_eq!(headers.len(), 0);
//     assert!(headers.is_empty());
//     assert_eq!(headers.get("Content-Type"), None);
// }
