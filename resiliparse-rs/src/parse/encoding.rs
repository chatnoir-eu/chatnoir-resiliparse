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

//! Character encoding detection and lightweight MIME sniffing.
//!
//! Use [`EncodingDetector`] to feed data incrementally or [`detect_encoding`] to
//! sample a byte slice in one call. Both return raw uchardet labels. For web
//! content, [`html_meta_label`] can supply a declared charset, and
//! [`map_encoding_to_html5`] maps labels to the API's Python-compatible WHATWG
//! subset. Detection and label mapping do not decode the input bytes.
//!
//! [`detect_mime`] provides a quick check based on magic bytes and the proportion
//! of unprintable bytes, rather than full document or media parsing.

use super::encoding_tables::{ENCODING_LABELS, MIME_SIGNATURES};
use crate::third_party::lexbor::*;
use crate::third_party::uchardet::*;
use std::collections::HashMap;
use std::ffi::CStr;
use std::ptr::NonNull;
use std::sync::LazyLock;

static LABEL_MAP: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| ENCODING_LABELS.iter().copied().collect());

/// Universal character encoding detector based on uchardet.
///
/// uchardet is a C wrapper and continuation of Mozilla's Universal Charset
/// Detector library. Call [`update`](Self::update) repeatedly to feed a document
/// in blocks. Calling [`encoding`](Self::encoding) finalizes detection and resets
/// the internal state so the same detector can process another document.
///
/// Results are raw uchardet labels, such as `"ASCII"`. Use
/// [`map_encoding_to_html5`] to obtain the Python-compatible names used for web
/// content, including the remapping of ASCII and ISO-8859-1 to Windows-1252.
///
/// # Examples
///
/// ```
/// use resiliparse::parse::encoding::{EncodingDetector, map_encoding_to_html5};
///
/// let mut detector = EncodingDetector::new().expect("allocate detector");
/// detector.update(b"Hello ");
/// detector.update(b"world");
/// let label = detector.encoding();
/// assert_eq!(map_encoding_to_html5(&label, true), Some("cp1252"));
/// // Retrieving the result has reset the detector.
/// assert_eq!(detector.encoding(), "");
/// ```
pub struct EncodingDetector(NonNull<uchardet>);

// Each instance exclusively owns its uchardet state, which has no thread affinity.
unsafe impl Send for EncodingDetector {}

impl EncodingDetector {
    /// Create an empty detector ready to receive input bytes.
    ///
    /// # Returns
    ///
    /// A new detector, or `None` if uchardet cannot allocate its state. The native
    /// state is released automatically when the detector is dropped.
    pub fn new() -> Option<Self> {
        NonNull::new(unsafe { uchardet_new() }).map(Self)
    }

    /// Update charset detection with another block of input bytes.
    ///
    /// Blocks belong to the same document until [`encoding`](Self::encoding) or
    /// [`reset`](Self::reset) is called. More data can improve detection accuracy;
    /// uchardet shortcuts processing once it has enough data to reach certainty,
    /// so callers do not need to stop feeding blocks themselves.
    ///
    /// # Arguments
    ///
    /// * `data` - The next block of raw text bytes, in document order.
    pub fn update(&mut self, data: &[u8]) {
        unsafe { uchardet_handle_data(self.0.as_ptr(), data.as_ptr().cast(), data.len()) };
    }

    /// Finalize detection, return the raw encoding label, and reset the detector.
    ///
    /// Returns an empty string if no encoding could be determined, including
    /// when no data was supplied. The result is neither canonicalized through
    /// Python's codec registry nor remapped according to WHATWG. Use
    /// [`map_encoding_to_html5`] for the latter; its `fallback_utf8` argument
    /// controls whether unknown labels, including the empty string, become UTF-8.
    ///
    /// The reset happens even if detection fails. Subsequent updates start a new
    /// document rather than extending the input used for this result.
    pub fn encoding(&mut self) -> String {
        unsafe {
            uchardet_data_end(self.0.as_ptr());
            let result = CStr::from_ptr(uchardet_get_charset(self.0.as_ptr()))
                .to_string_lossy()
                .into_owned();
            self.reset();
            result
        }
    }

    /// Manually discard all data supplied so far and start a new detection operation.
    ///
    /// Use this to abandon a document without retrieving a result. Calling
    /// [`encoding`](Self::encoding) already resets the state automatically.
    pub fn reset(&mut self) {
        unsafe { uchardet_reset(self.0.as_ptr()) };
    }
}

impl Drop for EncodingDetector {
    fn drop(&mut self) {
        unsafe { uchardet_delete(self.0.as_ptr()) };
    }
}

/// Map an encoding label to a Python-compatible subset of HTML5 encoding names.
///
/// The [WHATWG encoding mapping](https://encoding.spec.whatwg.org/#names-and-labels)
/// reduces encoding aliases to canonical names while accounting for common
/// mislabelling on the web. For example, ISO-8859-1 and ASCII are mapped to
/// Windows-1252 (`"cp1252"`), which is more likely to match actual web content.
/// Labels extracted from HTTP headers, HTML meta tags, and automatic detection
/// can all be passed to this function.
///
/// Leading and trailing whitespace is stripped and the label is lowercased.
/// Lookup uses the adjusted WHATWG table rather than Python's codec alias
/// registry. The input need not be a valid Python encoding name, while every
/// returned label is one. A successful mapping does not guarantee that the
/// input bytes can be decoded with the resulting encoding.
///
/// The mapping differs from WHATWG in these details:
///
/// * ISO-8859-8-I is mapped to ISO-8859-8.
/// * Windows-874 is mapped to ISO-8859-11.
/// * `x-mac-cyrillic` and `x-user-defined` are unsupported.
/// * No `"replacement"` mapping is used for the 7-bit ISO/IEC 2022 encodings.
///
/// # Arguments
///
/// * `label` - The encoding name or alias to map.
/// * `fallback_utf8` - Return `Some("utf-8")` for unknown labels when `true`, or
///   `None` when `false`.
///
/// # Examples
///
/// ```
/// use resiliparse::parse::encoding::map_encoding_to_html5;
///
/// assert_eq!(map_encoding_to_html5(" ISO-8859-1 ", true), Some("cp1252"));
/// assert_eq!(map_encoding_to_html5("csisolatin9", true), Some("iso8859-15"));
/// assert_eq!(map_encoding_to_html5("unknown", true), Some("utf-8"));
/// assert_eq!(map_encoding_to_html5("unknown", false), None);
/// ```
pub fn map_encoding_to_html5(label: &str, fallback_utf8: bool) -> Option<&'static str> {
    let label = label.trim().to_lowercase();
    LABEL_MAP
        .get(label.as_str())
        .copied()
        .or(if fallback_utf8 { Some("utf-8") } else { None })
}

/// Read the first charset label declared by an HTML meta tag in the initial 1024 bytes.
///
/// Uses Lexbor's HTML encoding prescan to recognize charset declarations,
/// including `http-equiv="Content-Type"` declarations. Only the first detected
/// entry is returned. The label is decoded as UTF-8, ignoring invalid byte
/// sequences, and is returned without checking or remapping the encoding name.
///
/// Returns `None` if there is no declaration within the prescan window or if the
/// prescan cannot be initialized or completed. An unknown first label is still
/// returned; the function does not search later declarations for a valid name.
/// Call [`map_encoding_to_html5`] with `fallback_utf8 = false` to check the label,
/// then fall back to [`detect_encoding`] if it is missing or unsupported.
///
/// # Arguments
///
/// * `data` - The raw HTML document. At most its first 1024 bytes are inspected.
///
/// # Examples
///
/// ```
/// use resiliparse::parse::encoding::{html_meta_label, map_encoding_to_html5};
///
/// let label = html_meta_label(b"<meta charset='iso-8859-1'><p>Hello</p>").unwrap();
/// assert_eq!(label, "iso-8859-1");
/// assert_eq!(map_encoding_to_html5(&label, false), Some("cp1252"));
/// ```
pub fn html_meta_label(data: &[u8]) -> Option<String> {
    let data = &data[..data.len().min(1024)];
    unsafe {
        let mut detector: lxb_html_encoding_t = std::mem::zeroed();
        if lxb_html_encoding_init(&mut detector) != lexbor_status_t::LXB_STATUS_OK as lxb_status_t {
            return None;
        }
        let result = if lxb_html_encoding_determine(&mut detector, data.as_ptr(), data.as_ptr().add(data.len()))
            == lexbor_status_t::LXB_STATUS_OK as lxb_status_t
        {
            let entry = lxb_html_encoding_meta_entry_noi(&mut detector, 0);
            if let Some(entry) = entry.as_ref() {
                let len = entry.end.offset_from(entry.name) as usize;
                // Legacy ignores invalid UTF-8 bytes in the declared label.
                Some(utf8_ignore(std::slice::from_raw_parts(entry.name, len)))
            } else {
                None
            }
        } else {
            None
        };
        lxb_html_encoding_destroy(&mut detector, false);
        result
    }
}

fn utf8_ignore(mut bytes: &[u8]) -> String {
    let mut output = String::new();
    loop {
        match std::str::from_utf8(bytes) {
            Ok(valid) => {
                output.push_str(valid);
                return output;
            }
            Err(error) => {
                output.push_str(std::str::from_utf8(&bytes[..error.valid_up_to()]).unwrap());
                match error.error_len() {
                    Some(length) => bytes = &bytes[error.valid_up_to() + length..],
                    None => return output,
                }
            }
        }
    }
}

/// Detect the encoding of a byte slice using uchardet.
///
/// This is a convenience wrapper around [`EncodingDetector`]. Sampling limits
/// the amount of data fed to the detector to keep processing time and temporary
/// memory use low. If the input exceeds a nonzero `max_len`, bytes from its start
/// and end are concatenated into one sample. Increase the limit or pass zero to
/// feed more data when greater detection accuracy is needed.
///
/// Each end contributes `ceil(max_len / 2)` bytes, so odd limits produce a sample
/// one byte larger than `max_len`, preserving the legacy sampling behavior.
/// The Python API uses a default limit of 131072 bytes; Rust callers supply the
/// desired limit explicitly.
///
/// This function performs automatic detection only. To prefer an HTML charset
/// declaration, call [`html_meta_label`] first. The returned label is raw; use
/// [`map_encoding_to_html5`] to obtain the API's names for web encodings.
///
/// # Arguments
///
/// * `data` - The input bytes whose encoding should be detected.
/// * `max_len` - Sampling limit in bytes. Zero disables sampling and feeds all data.
///
/// # Returns
///
/// The raw label in `Some`, or `Some("")` if detection fails. Returns `None` only
/// if the native detector could not be allocated.
///
/// # Examples
///
/// ```
/// use resiliparse::parse::encoding::{detect_encoding, map_encoding_to_html5};
///
/// let label = detect_encoding(b"Hello world", 131072).expect("allocate detector");
/// assert_eq!(map_encoding_to_html5(&label, true), Some("cp1252"));
/// ```
pub fn detect_encoding(data: &[u8], max_len: usize) -> Option<String> {
    let mut detector = EncodingDetector::new()?;
    if max_len != 0 && data.len() > max_len {
        let half = max_len.div_ceil(2);
        let mut sample = Vec::with_capacity(half * 2);
        sample.extend_from_slice(&data[..half]);
        sample.extend_from_slice(&data[data.len() - half..]);
        detector.update(&sample);
    } else {
        detector.update(data);
    }
    Some(detector.encoding())
}

/// Detect common internet MIME types from magic bytes and the ratio of unprintable bytes.
///
/// This is a basic, fast check rather than a full MIME detection engine such as
/// Apache Tika. It recognizes selected document, image, archive, audio, video,
/// font, and executable signatures. Signatures are checked against both the raw
/// input and its first 128 bytes with leading ASCII whitespace removed. The
/// first matching signature wins, preserving legacy precedence for overlapping
/// signatures such as `RIFF`.
///
/// HTML and XML matches are additionally checked within that 128-byte window
/// for XHTML or SVG markers. If no signature matches, bytes in `0x00..=0x1f` or
/// `0x7f..=0xff` are counted as unprintable. This includes control characters and
/// non-ASCII text bytes; it does not validate Unicode or attempt text decoding.
/// If their count exceeds `floor(data.len() * max_unprintable)`, the result is
/// `"application/octet-stream"`; otherwise it is `"text/plain"`. Empty input is
/// therefore classified as plain text.
///
/// # Arguments
///
/// * `data` - The input bytes to inspect.
/// * `max_unprintable` - Maximum allowed fraction of unprintable bytes for the
///   plain-text fallback. A typical value is `0.05`, the Python API's default.
///   A recognized magic signature takes precedence over this threshold.
///
/// # Examples
///
/// ```
/// use resiliparse::parse::encoding::detect_mime;
///
/// assert_eq!(detect_mime(b"%PDF-1.5\n", 0.05), "application/pdf");
/// assert_eq!(detect_mime(b"Hello world", 0.05), "text/plain");
/// assert_eq!(detect_mime(b"Hello \0\x0f\0world", 0.05), "application/octet-stream");
/// ```
pub fn detect_mime(data: &[u8], max_unprintable: f32) -> &'static str {
    let peek = &data[..data.len().min(128)];
    let start = peek
        .iter()
        .position(|c| !matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))
        .unwrap_or(peek.len());
    let stripped = &peek[start..];
    let contains = |needle: &[u8]| stripped.windows(needle.len()).any(|s| s == needle);
    for (magic, mime) in MIME_SIGNATURES {
        if data.starts_with(magic) || stripped.starts_with(magic) {
            if matches!(*mime, "text/html" | "application/xml") {
                if contains(b"\"-//W3C//DTD XHTML") {
                    return "application/xhtml+xml";
                }
                if *mime == "application/xml" && (contains(b"<svg ") || contains(b"\"-//W3C//DTD SVG")) {
                    return "image/svg+xml";
                }
            }
            return mime;
        }
    }
    // Cython's float argument and multiplication are single precision.
    let limit = (data.len() as f32 * max_unprintable) as usize;
    let mut unprintable = 0;
    for &byte in data {
        if byte <= 0x1f || byte >= 0x7f {
            unprintable += 1;
            if unprintable > limit {
                return "application/octet-stream";
            }
        }
    }
    "text/plain"
}

#[cfg(test)]
mod tests {
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
}
