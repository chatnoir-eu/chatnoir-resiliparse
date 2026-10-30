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

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::string::PyStringData;
use pyo3::types::{PySet, PyString};
use resiliparse::parse::lang::{self, LanguageTrainer, LanguageVector};

fn language_vector(text: &Bound<'_, PyString>, vec_len: usize) -> PyResult<Vec<u8>> {
    let mut vector = LanguageVector::new(vec_len).map_err(PyValueError::new_err)?;
    // SAFETY: the Python string remains borrowed and the GIL held while its
    // immutable backing storage is read. UCS units preserve lone surrogates;
    // all classification and n-gram processing are handled by the Rust core.
    match unsafe { text.data()? } {
        PyStringData::Ucs1(data) => {
            for &cp in data {
                vector.push_codepoint(cp as u32);
            }
        }
        PyStringData::Ucs2(data) => {
            for &cp in data {
                vector.push_codepoint(cp as u32);
            }
        }
        PyStringData::Ucs4(data) => {
            for &cp in data {
                vector.push_codepoint(cp);
            }
        }
    }
    Ok(vector.finish())
}

/// Perform very fast (linear-time) language detection on the input string.
///
/// The output is a tuple of the detected language code and its calculated
/// out-of-place rank, indicating the distance from the closest language profile.
/// Higher ranks mean less accurate detection; values above 1200 are usually
/// false results. Ranks are integer distances, not confidence probabilities.
///
/// The given Unicode string should be in composed normal form (NFC) for best
/// results. No normalization or case folding is performed automatically. Longer
/// texts generally produce more reliable predictions.
///
/// For texts shorter than 150 Unicode code points, raw ranks above 500 receive
/// a bias of up to 50 toward languages with more Wikipedia users. The length
/// includes ignored characters. The rank is the sum of absolute profile-bin
/// differences plus this bias.
///
/// With ``n_results=1``, no match returns ``("unknown", 0)``. Other values return
/// a list of up to ``n_results`` matches in ascending rank order; zero returns an
/// empty list. The cutoff is inclusive. ``langs=None`` considers all 101 profiles;
/// an empty iterable considers none. Unsupported language codes are ignored, and
/// filter order does not affect ranking. For one result, equal ranks favor the
/// first profile in model order (descending Wikipedia user counts). Multiple
/// results use deterministic heap order for ties. Empty or entirely ignored text
/// can still match a profile.
///
/// :param text: input text
/// :type text: str
/// :param cutoff: inclusive OOP rank cutoff, default 1200
/// :type cutoff: int
/// :param n_results: nonnegative number of best matches to return, default 1
/// :type n_results: int
/// :param langs: restrict detection to these language codes, or None for all
/// :type langs: typing.Iterable[str] | None
/// :return: detected language and rank, or a list of matches
/// :rtype: tuple[str, int] | list[tuple[str, int]]
/// :raises OverflowError: if cutoff or n_results is negative or exceeds the platform's unsigned integer range
/// :raises TypeError: if text is not a str or langs cannot be used as a language filter
#[pyfunction(signature = (text, cutoff=1200, n_results=1, langs=None))]
pub fn detect_fast(
    text: &Bound<'_, PyString>,
    cutoff: usize,
    n_results: usize,
    langs: Option<&Bound<'_, PyAny>>,
) -> PyResult<Py<PyAny>> {
    let py = text.py();
    let vector = language_vector(text, 256)?;
    // Match Python membership behavior, including strings, empty containers,
    // generators, and non-string members, without converting elements to str.
    let allowed = if let Some(langs) = langs {
        let set = if langs.is_truthy()? {
            Some(PySet::new(py, langs.try_iter()?.collect::<PyResult<Vec<_>>>()?)?)
        } else {
            None
        };
        let mut allowed = Vec::new();
        if let Some(set) = &set
            && set.iter().all(|item| item.is_exact_instance_of::<PyString>())
        {
            // Normal language-code filters need only a few conversions, rather
            // than creating and checking all 101 supported codes in Python.
            for item in set.iter() {
                if let Ok(code) = item.cast::<PyString>()?.to_str() {
                    allowed.push(code.to_owned());
                }
            }
        } else {
            // Preserve Python membership for custom objects and str subclasses.
            for code in lang::supported_langs() {
                let contains = match &set {
                    Some(set) => set.contains(code)?,
                    None => langs.contains(code)?,
                };
                if contains {
                    allowed.push(code.to_owned());
                }
            }
        }
        Some(allowed)
    } else {
        None
    };
    let allowed_refs = allowed
        .as_ref()
        .map(|codes| codes.iter().map(String::as_str).collect::<Vec<_>>());
    let matches = lang::rank_vector(
        vector.as_slice().try_into().unwrap(),
        text.code_point_len()?,
        cutoff,
        n_results,
        allowed_refs.as_deref(),
    );
    if n_results == 1 {
        let result = matches.first().map(|m| (m.language, m.rank)).unwrap_or(("unknown", 0));
        Ok(result.into_pyobject(py)?.into_any().unbind())
    } else {
        let result: Vec<_> = matches.iter().map(|m| (m.language, m.rank)).collect();
        Ok(result.into_pyobject(py)?.into_any().unbind())
    }
}

/// Get all languages supported by the fast language detector.
///
/// Returns a fresh list of 101 language codes sorted lexicographically. These
/// codes can be passed to :func:`detect_fast` through its ``langs`` parameter.
///
/// :return: sorted list of supported languages
/// :rtype: list[str]
#[pyfunction]
pub fn supported_langs() -> Vec<&'static str> {
    lang::supported_langs()
}

/// Train a language vector for fast detection on an iterable of example texts.
///
/// Each text is converted to a normalized character n-gram vector, and the
/// resulting vectors are averaged with equal weight, truncating fractions.
/// Each example bin is its n-gram count multiplied by 256, divided by the number
/// of retained letters and collapsed spaces, truncated and capped at 255.
/// Empty strings contribute zero vectors and count toward the average.
///
/// Letters retain their case; consecutive whitespace collapses to an ASCII
/// space. Other characters are ignored without breaking n-grams, but interrupt
/// whitespace collapsing. Normalize to NFC before training for best results.
///
/// The returned list has exactly ``vec_len`` elements. Use 256 bins for the
/// built-in detector's dimensions. This function does not modify the built-in profiles.
///
/// :param examples: example texts for this language
/// :type examples: typing.Iterable[str]
/// :param vec_len: positive output vector length, default 256
/// :type vec_len: int
/// :return: vector of trained values
/// :rtype: list[int]
/// :raises ValueError: if examples is empty, vec_len is zero, or the 32-bit example
///                     counter wraps to zero
/// :raises OverflowError: if vec_len is negative or exceeds the platform's unsigned integer range
/// :raises TypeError: if examples is not iterable or contains a non-string value
#[pyfunction(signature = (examples, vec_len=256))]
pub fn train_language_examples(examples: &Bound<'_, PyAny>, vec_len: usize) -> PyResult<Vec<u32>> {
    let mut trainer = LanguageTrainer::new(vec_len).map_err(PyValueError::new_err)?;
    for text in examples.try_iter()? {
        let text = text?;
        trainer.add(&language_vector(text.cast::<PyString>()?, vec_len)?);
    }
    Ok(trainer
        .finish()
        .map_err(PyValueError::new_err)?
        .into_iter()
        .map(u32::from)
        .collect())
}

/// Fast character n-gram language detection for 101 languages and profile training.
///
/// Use :func:`detect_fast` to detect languages, :func:`supported_langs` to list
/// their codes, and :func:`train_language_examples` to train a separate vector.
/// Normalize input text to NFC for best results. Ranks measure profile distance;
/// lower values indicate closer matches, and values above 1200 are usually
/// unreliable. Training does not change the built-in profiles.
#[pymodule(name = "lang", module = "resiliparse.parse")]
pub mod _lang {
    #[pymodule_export]
    pub use super::{detect_fast, supported_langs, train_language_examples};
}
