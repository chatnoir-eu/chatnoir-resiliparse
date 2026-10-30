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

//! Python codec integration and bindings for the native encoding utilities.

use pyo3::exceptions::{PyLookupError, PyMemoryError, PyUnicodeDecodeError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyString, PyTuple};
use resiliparse::parse::encoding as native;
use std::sync::Mutex;

/// Universal character encoding detector based on ``uchardet``.
///
/// ``uchardet`` is a C wrapper and a continuation of Mozilla's Universal Charset
/// Detector library. Call :meth:`update` multiple times to feed the bytes of one
/// document incrementally. Calling :meth:`encoding` returns the predicted encoding
/// name and resets the internal state so the detector can process another document.
///
/// By default, encoding names are remapped to the Python-compatible WHATWG subset
/// described by :func:`map_encoding_to_html5`. Set ``html5_compatible=False`` on
/// :meth:`encoding` to use Python's canonical codec name without WHATWG remapping.
#[pyclass(name = "EncodingDetector", module = "resiliparse.parse.encoding", subclass)]
pub struct EncodingDetectorPy {
    detector: Mutex<native::EncodingDetector>,
}

fn canonical_encoding(py: Python<'_>, label: &str, html5: bool) -> PyResult<Option<String>> {
    if html5 {
        return Ok(native::map_encoding_to_html5(label, true)
            .map(python_encoding_name)
            .map(str::to_owned));
    }
    match py.import("codecs")?.call_method1("lookup", (label,)) {
        Ok(codec) => Ok(Some(codec.getattr("name")?.extract()?)),
        Err(error) if error.is_instance_of::<PyLookupError>(py) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Translate standard encoding labels to the names used by Python's codecs.
fn python_encoding_name(label: &'static str) -> &'static str {
    match label {
        "ibm866" => "cp866",
        "windows-1250" => "cp1250",
        "windows-1251" => "cp1251",
        "windows-1252" => "cp1252",
        "windows-1253" => "cp1253",
        "windows-1254" => "cp1254",
        "windows-1255" => "cp1255",
        "windows-1256" => "cp1256",
        "windows-1257" => "cp1257",
        "windows-1258" => "cp1258",
        "iso-8859-2" => "iso8859-2",
        "iso-8859-3" => "iso8859-3",
        "iso-8859-4" => "iso8859-4",
        "iso-8859-5" => "iso8859-5",
        "iso-8859-6" => "iso8859-6",
        "iso-8859-7" => "iso8859-7",
        "iso-8859-8" => "iso8859-8",
        "iso-8859-10" => "iso8859-10",
        "iso-8859-11" => "iso8859-11",
        "iso-8859-13" => "iso8859-13",
        "iso-8859-14" => "iso8859-14",
        "iso-8859-15" => "iso8859-15",
        "iso-8859-16" => "iso8859-16",
        "euc-jp" => "euc_jp",
        "euc-kr" => "euc_kr",
        "iso-2022-jp" => "iso2022_jp",
        "macintosh" => "mac-roman",
        "utf-16le" => "utf-16-le",
        "utf-16be" => "utf-16-be",
        _ => label,
    }
}

#[pymethods]
impl EncodingDetectorPy {
    /// Create an encoding detector.
    ///
    /// :raise MemoryError: If the encoding detector cannot be allocated
    #[new]
    fn new() -> PyResult<Self> {
        let detector = native::EncodingDetector::new()
            .ok_or_else(|| PyMemoryError::new_err("Cannot allocate encoding detector"))?;
        Ok(Self {
            detector: Mutex::new(detector),
        })
    }

    /// Update charset detection with another block of input bytes.
    ///
    /// The detector shortcuts processing when it has enough data to reach
    /// certainty, so callers do not need to stop feeding input themselves.
    /// Repeated updates belong to the same document until :meth:`encoding` or
    /// :meth:`reset` is called.
    ///
    /// :param data: The next block of input data
    fn update(&self, data: &Bound<'_, PyBytes>) {
        self.detector.lock().unwrap().update(data.as_bytes());
    }

    /// Return a Python-compatible name for the detected encoding and reset the detector.
    ///
    /// By default, the detected label is remapped according to the WHATWG
    /// encoding specification, which is primarily suitable for web content.
    /// Set ``html5_compatible=False`` to use Python's codec registry without
    /// this remapping. See :func:`map_encoding_to_html5` for the adjusted mapping.
    ///
    /// With remapping enabled, unknown encodings fall back to ``'utf-8'``.
    /// Otherwise, detection failure returns ``None``. Retrieving a result always
    /// resets the detector, including on failure; subsequent updates start a new
    /// document.
    ///
    /// :param html5_compatible: Remap encoding names according to WHATWG (default: True)
    /// :return: Detected encoding name, or None if detection fails without remapping
    #[pyo3(signature = (html5_compatible=true))]
    fn encoding(&self, py: Python<'_>, html5_compatible: bool) -> PyResult<Option<String>> {
        let label = self.detector.lock().unwrap().encoding();
        canonical_encoding(py, &label, html5_compatible)
    }

    /// Manually reset the encoding detector state, discarding all supplied data.
    ///
    /// Use this to abandon a document without retrieving a result.
    /// :meth:`encoding` already resets the state automatically.
    fn reset(&self) {
        self.detector.lock().unwrap().reset();
    }
}

/// Detect the encoding of a byte string using ``uchardet``.
///
/// This is a convenience wrapper around :class:`EncodingDetector` that creates a
/// detector for each call.
///
/// Input sampling limits processing time and temporary memory usage. If the string
/// is longer than a nonzero ``max_len``, ``ceil(max_len / 2)`` bytes from each end
/// are concatenated into one sample. An odd limit therefore samples one extra byte.
/// This is a tradeoff
/// between performance and accuracy. If you need higher accuracy, increase the limit to feed more data
/// into the :class:`EncodingDetector` (the default should be more than enough in most cases).
///
/// The :class:`EncodingDetector` relies on `uchardet` as its encoding detection engine. If the
/// input string is an HTML document, you can also use the available information from the HTML meta charset
/// tag instead. With ``from_html_meta=True``, :func:`detect_encoding` will try to use the charset meta
/// tag in the HTML string if one is available and ASCII-readable within the first 1024 bytes. If this fails,
/// it will fall back to auto-detection with `uchardet`.
///
/// By default, the detected encoding name is remapped according to the `WHATWG encoding specification
/// <https://encoding.spec.whatwg.org/#names-and-labels>`_, which is primarily suitable for web content.
/// To disable this behaviour, set ``html5_compatible=False``. For more information, see:
/// :func:`map_encoding_to_html5`. Encodings detected from HTML meta tags are always remapped, no matter the
/// value of ``html5_compatible``, to ensure valid encoding names.
///
/// If WHATWG remapping is enabled, UTF-8 is returned as a fallback encoding. Otherwise, the method returns
/// ``None`` on failure to detect the encoding.
///
/// :param data: Input string for which to detect the encoding
/// :param max_len: Maximum number of bytes to feed to detector (0 for no limit)
/// :param html5_compatible: Remap encoding names according to WHATWG
/// :param from_html_meta: If string is an HTML document, use meta tag info
/// :return: Detected encoding
/// :raise MemoryError: If the encoding detector cannot be allocated
#[pyfunction]
#[pyo3(signature = (data, max_len=131072, html5_compatible=true, from_html_meta=false))]
pub fn detect_encoding(
    py: Python<'_>,
    data: &Bound<'_, PyBytes>,
    max_len: usize,
    html5_compatible: bool,
    from_html_meta: bool,
) -> PyResult<Option<String>> {
    let data = data.as_bytes();
    if from_html_meta && let Some(label) = native::html_meta_label(data) {
        if let Some(encoding) = map_encoding_to_html5(&PyString::new(py, &label), false)? {
            return Ok(Some(encoding.to_owned()));
        }
    }
    let label = native::detect_encoding(data, max_len)
        .ok_or_else(|| PyMemoryError::new_err("Cannot allocate encoding detector"))?;
    canonical_encoding(py, &label, html5_compatible)
}

/// Map an encoding name to a subset of names allowed by the HTML5 standard.
///
/// This function will remap the given name according to the mapping definition given in
/// `Section 4.2 <https://encoding.spec.whatwg.org/#names-and-labels>`_ of the WHATWG encoding
/// specification. The returned value will always be a valid Python encoding name, but the
/// supplied input name does not necessarily have to be.
///
/// The WHATWG mapping is designed to boil down the many possible encoding names to a smaller
/// subset of canonical names while taking into account common encoding mislabelling practices.
/// The main purpose of this function is to map encoding names extracted from HTTP headers or
/// websites to their canonical names, but it also makes sense to apply the mapping to an
/// auto-detected encoding name, since it remaps some encodings based on observed practices on
/// the web, such as the mapping from ISO-8859-1 to Windows-1252, which is more likely to be
/// correct, even if both options are possible. :meth:`EncodingDetector.encoding` already remaps
/// its detected encodings to the WHATWG set by default.
///
/// The mapping does not involve Python's encoding alias names, but instead uses an adjusted
/// WHATWG mapping. Inputs not defined in this mapping are remapped to UTF-8. Hence, the function
/// always produces a valid output, but the mapped encoding is not guaranteed to be compatible
/// with the original encoding. Use :func:`bytes_to_str` to avoid decoding errors. You can also
/// set ``fallback_utf8=False`` to return ``None`` instead if the supplied encoding is unknown.
///
/// The adjusted encoding mapping differs from the WHATWG spec in the following details:
///
///   * ISO-8859-8-I name replaced with ISO-8859-8
///   * WINDOWS-874 name replaced with ISO-8859-11
///   * x-mac-cyrillic is unsupported
///   * x-user-defined is unsupported
///   * No "replacement" mapping for 7-bit versions of ISO/IEC 2022
///
/// :param encoding: Input encoding name
/// :param fallback_utf8: Whether to fall back to UTF-8 or return ``None`` for unknown encodings
/// :return: Mapped output encoding name
#[pyfunction]
#[pyo3(signature = (encoding, fallback_utf8=true))]
pub fn map_encoding_to_html5(encoding: &Bound<'_, PyString>, fallback_utf8: bool) -> PyResult<Option<&'static str>> {
    // Match Python's Unicode strip/casefold semantics, including non-ASCII labels.
    let normalized: String = encoding.call_method0("strip")?.call_method0("casefold")?.extract()?;
    Ok(native::map_encoding_to_html5(&normalized, fallback_utf8).map(python_encoding_name))
}

fn default_fallbacks(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    Ok(PyTuple::new(py, ["utf-8", "cp1252"])?.into_any())
}

fn supplied_fallbacks<'py>(value: &Bound<'py, PyAny>) -> PyResult<Option<Bound<'py, PyAny>>> {
    Ok(Some(value.clone()))
}

fn bom_encoding<'a>(encoding: &'a str, data: &[u8], strip: bool) -> &'a str {
    if strip {
        if encoding == "utf-8" && data.starts_with(b"\xef\xbb\xbf") {
            return "utf-8-sig";
        }
        if encoding.starts_with("utf-16-") && (data.starts_with(b"\xff\xfe") || data.starts_with(b"\xfe\xff")) {
            return "utf-16";
        }
        if encoding.starts_with("utf-32-") && (data.starts_with(b"\xff\xfe\0\0") || data.starts_with(b"\0\0\xfe\xff")) {
            return "utf-32";
        }
    }
    encoding
}

/// Helper for decoding a byte string into a unicode string using a given encoding.
/// This encoding should be determined beforehand, e.g., with :func:`detect_encoding`.
///
/// :func:`bytes_to_str` tries to decode the byte string with ``encoding``. If that
/// fails, it will fall back to UTF-8 and Windows-1252 (or whichever encodings where
/// given in ``fallback_encodings``). If all fallbacks fail as well, the string will be
/// double-decoded with ``encoding`` and invalid characters will be treated according
/// to ``errors``, which has the same options as for :meth:`bytes.decode` (i.e.,
/// ``"ignore"`` or ``"replace"``). The double-decoding step ensures that the resulting
/// string is sane and can be re-encoded without errors.
///
/// This function also takes care to strip BOMs from the beginning of the string if
/// ``strip_bom=True``.
///
/// Exceptions raised by the fallback iterable or a custom codec propagate unchanged.
///
/// :param data: Input byte string
/// :param encoding: Desired encoding
/// :param errors: Error handling for invalid characters
/// :param fallback_encodings: List of fallback encodings to try if the primary encoding fails
/// :param strip_bom: Strip BOM sequences from beginning of the string
/// :return: Decoded string
/// :raise LookupError: If an attempted codec or a required error handler is unknown
/// :raise TypeError: If fallback_encodings is not iterable or an attempted label is not a string
/// :raise UnicodeError: If all decoding attempts fail or re-encoding fails under the selected error policy
#[pyfunction]
#[pyo3(signature = (data, encoding="utf-8", errors="ignore", fallback_encodings=None, strip_bom=true),
    text_signature = "(data, encoding='utf-8', errors='ignore', fallback_encodings=('utf-8', 'cp1252'), strip_bom=True)")]
pub fn bytes_to_str<'py>(
    py: Python<'py>,
    data: &Bound<'py, PyBytes>,
    encoding: &str,
    errors: &str,
    #[pyo3(from_py_with = supplied_fallbacks)] fallback_encodings: Option<Bound<'py, PyAny>>,
    strip_bom: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let codecs = py.import("codecs")?;
    let encoding: String = codecs.call_method1("lookup", (encoding,))?.getattr("name")?.extract()?;
    // Legacy expands the iterable before attempting even the primary encoding.
    let fallback_encodings = match fallback_encodings {
        Some(value) => value,
        None => default_fallbacks(py)?,
    };
    let fallbacks = fallback_encodings.try_iter()?.collect::<PyResult<Vec<_>>>()?;
    let decode = |label: &str| data.call_method1("decode", (bom_encoding(label, data.as_bytes(), strip_bom),));
    match decode(&encoding) {
        Ok(value) => return Ok(value),
        Err(error) if error.is_instance_of::<PyUnicodeDecodeError>(py) => {}
        Err(error) => return Err(error),
    }
    for fallback in fallbacks {
        let label: String = codecs.call_method1("lookup", (fallback,))?.getattr("name")?.extract()?;
        if label == encoding {
            continue;
        }
        match decode(&label) {
            Ok(value) => return Ok(value),
            Err(error) if error.is_instance_of::<PyUnicodeDecodeError>(py) => {}
            Err(error) => return Err(error),
        }
    }
    let decoded = data.call_method1("decode", (bom_encoding(&encoding, data.as_bytes(), strip_bom), errors))?;
    decoded
        .call_method1("encode", ("utf-8", errors))?
        .call_method0("decode")
}

/// Try to detect common internet MIME types based on the initial magic byte sequence of ``data``.
///
/// The check is very basic and only checks the starting bytes as well as the number of unprintable
/// bytes. It does not replace a full-blown MIME type detection engine like Apache Tika at the moment.
///
/// :param data: Input bytes
/// :param max_unprintable: Maximum allowable ratio of unprintable characters for text
/// :return: Detected MIME type
#[pyfunction]
#[pyo3(signature = (data, max_unprintable=0.05))]
pub fn detect_mime(data: &Bound<'_, PyBytes>, max_unprintable: f32) -> &'static str {
    native::detect_mime(data.as_bytes(), max_unprintable)
}

#[pymodule(name = "encoding", module = "resiliparse.parse")]
pub mod _encoding {
    #[pymodule_export]
    pub use super::{EncodingDetectorPy, bytes_to_str, detect_encoding, detect_mime, map_encoding_to_html5};
}
