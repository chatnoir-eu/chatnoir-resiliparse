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

use std::path::PathBuf;

// ===========================================================
// Test helpers
// ===========================================================

// noinspection DuplicatedCode
/// Helper for getting path to external test fixtures.
pub fn get_fixture_path(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../test-data");
    path.push(name);
    path
}

/// Test fixture: WARC record as String.
pub fn warc_record_data(record_type: &str, record_id: &str, content_type: Option<&str>, payload: &[u8]) -> Vec<u8> {
    warc_record_data_with_headers(record_type, record_id, content_type, "", payload)
}

/// Test fixture: WARC record with custom headers as String.
pub fn warc_record_data_with_headers(
    record_type: &str,
    record_id: &str,
    content_type: Option<&str>,
    extra_headers: &str,
    payload: &[u8],
) -> Vec<u8> {
    let mut bytes = format!(
        "WARC/1.1\r\n\
         WARC-Type: {}\r\n\
         WARC-Record-ID: {}\r\n\
         {}\
         {}\
         Content-Length: {}\r\n\
         \r\n",
        record_type,
        record_id,
        content_type
            .map(|value| format!("Content-Type: {value}\r\n"))
            .unwrap_or_default(),
        extra_headers,
        payload.len()
    )
    .into_bytes();
    bytes.extend_from_slice(payload);
    bytes.extend_from_slice(b"\r\n\r\n");
    bytes
}

/// Test fixture: WARC record of an HTTP response record as String.
pub fn http_response_warc_data(record_id: &str, payload: &str) -> Vec<u8> {
    let http_data = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Server: nginx\r\n\
         \r\n\
         {}",
        payload.len(),
        payload
    );
    warc_record_data("response", record_id, Some("application/http; msgtype=response"), http_data.as_bytes())
}

/// Test fixture: WARC record of an HTTP response record as String.
pub fn http_response_warc_data_encoded(
    record_id: &str,
    payload: &[u8],
    transfer_encoding: Option<&str>,
    content_encoding: Option<&str>,
) -> Vec<u8> {
    let mut transfer_enc_header = "".to_string();
    let mut content_enc_header = "".to_string();
    let mut content_len_header = format!("Content-Length: {}\r\n", payload.len());

    if let Some(enc) = transfer_encoding {
        transfer_enc_header = format!("Transfer-Encoding: {}\r\n", enc);
        if enc == "chunked" {
            content_len_header = "".to_string();
        }
    };
    if let Some(enc) = content_encoding {
        content_enc_header = format!("Content-Encoding: {}\r\n", enc);
    };
    let http_data = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\
         Server: nginx\r\n\
         {}{}{}\
         \r\n",
        transfer_enc_header, content_enc_header, content_len_header
    );
    let payload = [http_data.as_bytes(), payload].concat();
    warc_record_data("response", record_id, Some("application/http; msgtype=response"), payload.as_slice())
}
