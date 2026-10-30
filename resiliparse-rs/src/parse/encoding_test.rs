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

#[test]
fn detector_results_reset_the_stream() {
    let mut detector = EncodingDetector::new().unwrap();
    detector.update(b"Hello ");
    detector.update(b"world");
    assert_eq!(detector.encoding(), "ASCII");
    assert_eq!(detector.encoding(), "");
    detector.update(b"\xff\xfeH\0i\0");
    detector.reset();
    detector.update(b"new stream");
    assert_eq!(detector.encoding(), "ASCII");
}

#[test]
fn meta_prescan_uses_the_first_declaration_within_the_limit() {
    assert_eq!(
        html_meta_label(b"<meta charset='windows-1252'><meta charset='utf-8'>").as_deref(),
        Some("windows-1252")
    );
    let mut html = vec![b' '; 1024];
    html.extend_from_slice(b"<meta charset='utf-8'>");
    assert_eq!(html_meta_label(&html), None);
}

#[test]
fn mime_signature_precedence_and_binary_threshold() {
    assert_eq!(detect_mime(b"RIFF....WAVE", 0.05), "image/webp");
    assert_eq!(detect_mime(b" \n%PDF-1.7", 0.05), "application/pdf");
    assert_eq!(detect_mime(b"1234567890123456789\0", 0.05), "text/plain");
    assert_eq!(detect_mime(b"123456789012345678\0\0", 0.05), "application/octet-stream");
}

#[test]
fn html5_labels_and_sampled_detection() {
    assert_eq!(map_encoding_to_html5(" ASCII ", false), Some("windows-1252"));
    assert_eq!(map_encoding_to_html5("csisolatin9", false), Some("iso-8859-15"));
    assert_eq!(map_encoding_to_html5("not-an-encoding", false), None);
    assert_eq!(map_encoding_to_html5("not-an-encoding", true), Some("utf-8"));
    assert_eq!(detect_encoding("Zażółć gęślą jaźń".as_bytes(), 0).as_deref(), Some("UTF-8"));
}

#[test]
fn html5_names_use_standard_encoding_labels() {
    for (label, expected) in [
        ("cp866", "ibm866"),
        ("cp1250", "windows-1250"),
        ("ISO-8859-8", "iso-8859-8"),
        ("windows-874", "iso-8859-11"),
        ("euc_jp", "euc-jp"),
        ("euc_kr", "euc-kr"),
        ("iso2022_jp", "iso-2022-jp"),
        ("mac-roman", "macintosh"),
        ("utf-16-le", "utf-16le"),
        ("utf-16-be", "utf-16be"),
        ("shift_jis", "shift_jis"),
    ] {
        assert_eq!(map_encoding_to_html5(label, false), Some(expected), "{label}");
    }
    for &(label, expected) in ENCODING_LABELS {
        assert_eq!(map_encoding_to_html5(expected, false), Some(expected), "{label}");
    }
}

#[test]
fn mime_detection_covers_common_document_signatures() {
    for (bytes, expected) in [
        (b"%PDF-1.5\n".as_slice(), "application/pdf"),
        (b"\x89PNG\r\n\x1a\n".as_slice(), "image/png"),
        (b"<!doctype html>".as_slice(), "text/html"),
        (b"<?xml version='1.0'?>".as_slice(), "application/xml"),
    ] {
        assert_eq!(detect_mime(bytes, 0.05), expected);
    }
}

#[test]
fn meta_declarations_ignore_invalid_utf8() {
    // Discard malformed bytes, including a truncated sequence, without losing valid text.
    assert_eq!(html_meta_label(b"<meta charset='ut\xfff-8'>").as_deref(), Some("utf-8"));
    assert_eq!(utf8_ignore(b"A\xffB\xe2\x82"), "AB");
    assert_eq!(html_meta_label(b"<meta name='description' content='charset=utf-8'>"), None);
    assert_eq!(
        html_meta_label(
            b"<!--<meta charset=utf-8>--><meta http-equiv=content-type content='text/html; charset=windows-1251'>"
        )
        .as_deref(),
        Some("windows-1251")
    );
    // Prescanning returns the first declaration even when its label is unsupported.
    assert_eq!(html_meta_label(b"<meta charset=unknown><meta charset=utf-8>").as_deref(), Some("unknown"));
}

#[test]
fn detection_samples_both_ends() {
    // The sampled ends contain UTF-8; the invalid middle must not reach the detector.
    let edge = "Zażółć gęślą jaźń. ".repeat(8);
    let mut data = edge.as_bytes().to_vec();
    data.extend_from_slice(&[0xff; 32]);
    data.extend_from_slice(edge.as_bytes());
    // Odd limits round each half up, so both limits select the same bytes.
    for limit in [edge.len() * 2 - 1, edge.len() * 2] {
        assert_eq!(detect_encoding(&data, limit).as_deref(), Some("UTF-8"));
    }
    assert_eq!(detect_encoding(b"", 0).as_deref(), Some(""));
}

#[test]
fn xml_mime_subtypes() {
    assert_eq!(
        detect_mime(b"<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.0 Strict//EN\">", 0.05),
        "application/xhtml+xml"
    );
    assert_eq!(detect_mime(b"<?xml version='1.0'?><svg xmlns='http://www.w3.org/2000/svg'>", 0.05), "image/svg+xml");
    assert_eq!(detect_mime(b"<?xml version='1.0'?><root/>", 0.05), "application/xml");
}
