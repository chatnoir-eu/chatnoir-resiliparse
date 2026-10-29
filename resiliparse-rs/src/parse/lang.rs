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

//! Fast, linear-time language detection using hashed character n-gram profiles.
//!
//! The detector compares 256-byte profiles for 101 languages. Lower out-of-place
//! ranks indicate closer matches, not probabilities; ranks above 1200 are usually
//! unreliable. Input should be in composed normal form (NFC) for best results.
//! Neither Unicode normalization nor case folding is performed automatically.
//!
//! Use [`detect_fast`] to rank candidate languages, [`supported_langs`] to list
//! their codes, and [`train_language_examples`] to train a separate profile.
//! Longer texts generally give more reliable results than short snippets.
//! Training does not change the built-in model.

use super::lang_profiles::LANGS;

mod unicode_tables {
    include!(concat!(env!("OUT_DIR"), "/lang_unicode.rs"));
}
use unicode_tables::{ALPHABETIC_BITS, ALPHABETIC_PAGES};

/// Classification of a Unicode code point for language-profile training.
#[derive(Clone, Copy, Debug)]
pub enum CharacterClass {
    /// A Unicode alphabetic character, retained with its original case and code point.
    Letter,
    /// Whitespace, collapsed to an ASCII space when consecutive.
    Whitespace,
    /// An ignored character, which interrupts whitespace collapsing.
    Other,
}

/// Incrementally construct a language vector with fixed memory per vector.
///
/// Hashes one- through five-character n-grams with 32-bit FNV-1a folded to eight
/// bits. Ignored characters do not break n-grams. Vector lengths below 256 fold
/// hash buckets modulo the length; lengths above 256 leave trailing bins zero.
/// Letter code points retain their case. Consecutive whitespace is replaced by
/// one ASCII space; other characters are ignored and reset whitespace collapsing.
/// Each bin is normalized by the number of retained characters, scaled by 256,
/// truncated and capped at 255. Memory usage is proportional to the vector length
/// and independent of the amount of text.
pub struct LanguageVector {
    counts: Vec<u32>,
    hashes: [u32; 5],
    accepted: usize,
    previous_space: bool,
}

impl LanguageVector {
    /// Create a vector with `vec_len` bins, initially containing zero counts.
    ///
    /// Use 256 bins for profiles compared with the built-in language model.
    /// Returns an error if `vec_len` is zero.
    pub fn new(vec_len: usize) -> Result<Self, &'static str> {
        if vec_len == 0 {
            return Err("vec_len must be greater than zero");
        }
        Ok(Self {
            counts: vec![0; vec_len],
            hashes: [2166136261; 5],
            accepted: 0,
            previous_space: false,
        })
    }

    /// Add a code point, using the caller's Unicode character classification.
    ///
    /// `class` determines whether `codepoint` is retained as a letter, converted
    /// to an ASCII space, or ignored. Ignored input leaves the n-gram history
    /// intact but resets whitespace collapsing. The caller supplies classification
    /// so bindings can use their runtime's Unicode tables. No normalization or
    /// case folding is applied.
    #[inline]
    pub fn push(&mut self, mut codepoint: u32, class: CharacterClass) {
        match class {
            CharacterClass::Letter => self.previous_space = false,
            CharacterClass::Whitespace => {
                if self.previous_space {
                    return;
                }
                self.previous_space = true;
                codepoint = 32;
            }
            CharacterClass::Other => {
                self.previous_space = false;
                return;
            }
        }
        // Extend each previous suffix hash by one code point. Updating longest
        // first preserves the previous suffixes and needs only five FNV steps.
        for index in (0..5.min(self.accepted.saturating_add(1))).rev() {
            let previous = if index == 0 { 2166136261 } else { self.hashes[index - 1] };
            let hash = (previous ^ codepoint).wrapping_mul(16777619);
            self.hashes[index] = hash;
            let bucket = (((hash >> 8) ^ hash) & 255) as usize;
            let bucket = if self.counts.len() >= 256 {
                bucket
            } else {
                bucket % self.counts.len()
            };
            self.counts[bucket] = self.counts[bucket].wrapping_add(1);
        }
        self.accepted += 1;
    }

    /// Add a Unicode code point using Rust's Unicode character properties.
    ///
    /// Letters are retained, whitespace is collapsed, and other input is ignored.
    /// Surrogates and values above U+10FFFF are ignored as non-letter input.
    /// See [`classify_codepoint`] for the character definitions.
    #[inline]
    pub fn push_codepoint(&mut self, codepoint: u32) {
        self.push(codepoint, classify_codepoint(codepoint));
    }

    /// Normalize counts to bytes, truncating division and saturating at 255.
    ///
    /// The denominator is the number of retained letters and collapsed spaces,
    /// rather than the number of n-grams. Empty or entirely ignored input yields
    /// an all-zero vector. Returns exactly the configured number of bins, with
    /// each value calculated as `min(255, count * 256 / retained_characters)`.
    /// Counters and multiplication use wrapping 32-bit arithmetic.
    pub fn finish(self) -> Vec<u8> {
        self.counts
            .into_iter()
            .map(|n| {
                if self.accepted == 0 {
                    0
                } else {
                    (n.wrapping_mul(256) as usize / self.accepted).min(255) as u8
                }
            })
            .collect()
    }
}

/// Classify a code point using Rust's Unicode character properties.
///
/// Alphabetic characters are identified with [`char::is_alphabetic`], including
/// Unicode letters, letter numbers, and marks with the Other_Alphabetic property.
/// Whitespace is identified with [`char::is_whitespace`]. Surrogates and values
/// above U+10FFFF are classified as [`CharacterClass::Other`]. Definitions follow
/// the Unicode version supplied by the Rust toolchain. Alphabetic properties
/// are compiled into compact bitmap pages at build time, with no runtime cache.
#[inline]
pub fn classify_codepoint(codepoint: u32) -> CharacterClass {
    let Some(c) = char::from_u32(codepoint) else {
        return CharacterClass::Other;
    };
    let alphabetic = if c.is_ascii() {
        c.is_ascii_alphabetic()
    } else {
        let page = ALPHABETIC_PAGES[(codepoint >> 8) as usize] as usize;
        ALPHABETIC_BITS[page][((codepoint >> 6) & 3) as usize] & (1u64 << (codepoint & 63)) != 0
    };
    if alphabetic {
        CharacterClass::Letter
    } else if c.is_whitespace() {
        CharacterClass::Whitespace
    } else {
        CharacterClass::Other
    }
}

/// Build a normalized n-gram vector from a Unicode string.
///
/// Unicode letters retain their case; consecutive whitespace collapses to one
/// ASCII space. Punctuation, digits and non-alphabetic marks are ignored without
/// breaking n-grams, but interrupt whitespace collapsing. Letters are identified
/// using Rust's alphabetic and whitespace properties. Alphabetic combining marks
/// and letter numbers are retained. See [`classify_codepoint`] for the definitions.
/// Normalize `text` to NFC before calling for best results.
///
/// `vec_len` specifies the number of returned bins. Use 256 for the built-in
/// detector; smaller lengths fold hash buckets modulo the length, and larger
/// lengths leave trailing bins zero. Values range from 0 to 255; see
/// [`LanguageVector::finish`] for normalization. Empty or entirely ignored input
/// yields a zero vector. Returns an error if `vec_len` is zero.
pub fn language_vector(text: &str, vec_len: usize) -> Result<Vec<u8>, &'static str> {
    let mut vector = LanguageVector::new(vec_len)?;
    for c in text.chars() {
        vector.push_codepoint(c as u32);
    }
    Ok(vector.finish())
}

/// Train a language vector by averaging normalized example vectors.
///
/// Each example has equal weight, regardless of text length. A zero vector from
/// an empty text still contributes to the example count. Accumulation uses
/// wrapping 32-bit sums; memory usage depends only on the configured vector length.
pub struct LanguageTrainer {
    sums: Vec<u32>,
    examples: u32,
}

impl LanguageTrainer {
    /// Create a trainer whose output contains `vec_len` bins.
    ///
    /// Use 256 bins to train a profile with the built-in detector's dimensions.
    /// Returns an error if `vec_len` is zero.
    pub fn new(vec_len: usize) -> Result<Self, &'static str> {
        if vec_len == 0 {
            return Err("vec_len must be greater than zero");
        }
        Ok(Self {
            sums: vec![0; vec_len],
            examples: 0,
        })
    }

    /// Add one already normalized example vector, including empty examples.
    ///
    /// Each call counts as one example. Values should come from
    /// [`language_vector`] or [`LanguageVector::finish`].
    ///
    /// # Panics
    ///
    /// Panics if `vector` does not have the trainer's configured length.
    pub fn add(&mut self, vector: &[u8]) {
        assert_eq!(self.sums.len(), vector.len());
        for (sum, &value) in self.sums.iter_mut().zip(vector) {
            *sum = sum.wrapping_add(value as u32);
        }
        self.examples = self.examples.wrapping_add(1);
    }

    /// Return a trained vector with the configured number of bins.
    ///
    /// Each bin is the sum of the example values divided by the number of
    /// examples, truncating fractions and capping at 255. Returns an error if
    /// no examples were added or the 32-bit example counter wrapped to zero.
    pub fn finish(self) -> Result<Vec<u8>, &'static str> {
        if self.examples == 0 {
            return Err("at least one training example is required");
        }
        Ok(self
            .sums
            .into_iter()
            .map(|sum| (sum / self.examples).min(255) as u8)
            .collect())
    }
}

/// Train a language vector for fast language detection on example texts.
///
/// `examples` accepts any iterable of strings for one language. Each text is
/// converted with [`language_vector`], then the normalized vectors are averaged
/// with equal weight, truncating fractions. Empty strings contribute zero vectors
/// and count toward the average. Normalize texts to NFC for best results; no
/// automatic normalization or case folding is performed.
///
/// `vec_len` is the positive output length; use 256 for the built-in detector's
/// dimensions. Lengths below 256 fold hash buckets modulo the length; lengths
/// above 256 leave trailing bins zero. Returns exactly `vec_len` values in
/// `0..=255`. Training does not modify the built-in profiles.
///
/// Returns an error if `examples` is empty, `vec_len` is zero, or the 32-bit
/// example counter wraps to zero.
pub fn train_language_examples<I, S>(examples: I, vec_len: usize) -> Result<Vec<u8>, &'static str>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut trainer = LanguageTrainer::new(vec_len)?;
    for text in examples {
        trainer.add(&language_vector(text.as_ref(), vec_len)?);
    }
    trainer.finish()
}

/// Get all languages supported by the fast language detector.
///
/// Returns a newly allocated list of 101 language codes in lexicographic order.
/// Pass these codes to [`detect_fast`] or [`rank_vector`] to restrict detection.
pub fn supported_langs() -> Vec<&'static str> {
    let mut langs: Vec<_> = LANGS.iter().map(|&(code, _)| code).collect();
    langs.sort_unstable();
    langs
}

/// One detected language and its integer out-of-place distance (lower is better).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LanguageMatch {
    /// Supported language code identifying the matched profile.
    pub language: &'static str,
    /// Sum of absolute profile-bin differences, including any short-text bias.
    /// Lower values indicate closer matches; this is not a probability.
    pub rank: u32,
}

// Reproduce the original C++ heap's tie ordering, including selection of the
// right child on equal ranks during pop_heap. Stable sorting changes results.
fn push_heap(heap: &mut Vec<LanguageMatch>, value: LanguageMatch) {
    let mut hole = heap.len();
    heap.push(value);
    while hole > 0 {
        let parent = (hole - 1) / 2;
        if heap[parent].rank <= value.rank {
            break;
        }
        heap[hole] = heap[parent];
        hole = parent;
    }
    heap[hole] = value;
}

fn pop_heap(heap: &mut Vec<LanguageMatch>) -> LanguageMatch {
    let result = heap[0];
    let value = heap.pop().unwrap();
    if heap.is_empty() {
        return result;
    }
    let mut hole = 0;
    while hole * 2 + 1 < heap.len() {
        let mut child = hole * 2 + 1;
        if child + 1 < heap.len() && heap[child + 1].rank <= heap[child].rank {
            child += 1;
        }
        heap[hole] = heap[child];
        hole = child;
    }
    while hole > 0 {
        let parent = (hole - 1) / 2;
        if heap[parent].rank <= value.rank {
            break;
        }
        heap[hole] = heap[parent];
        hole = parent;
    }
    heap[hole] = value;
    result
}

/// Rank a 256-byte vector against the built-in profiles.
///
/// `vector` must be the normalized 256-bin profile of the input text. Each raw
/// out-of-place rank is the sum of absolute differences from a language profile.
/// `text_len` is the original Unicode code-point count, including ignored
/// characters. Texts shorter than 150 code points receive a profile-order bias
/// of `min(50, profile_index * 3)` when their raw rank exceeds 500. Model order
/// follows descending Wikipedia user counts. `cutoff` is the maximum accepted
/// rank after this bias; 1200 is recommended, and the cutoff is inclusive.
///
/// `langs=None` permits all languages; an empty slice permits none. Unsupported
/// codes are ignored, and filter order does not affect ranking. Returns up to
/// `n_results` matches in ascending rank order. With one result, ties favor the
/// first profile in model order; multiple results use deterministic heap order.
/// No match or zero requested results yields an empty vector.
pub fn rank_vector(
    vector: &[u8; 256],
    text_len: usize,
    cutoff: usize,
    n_results: usize,
    langs: Option<&[&str]>,
) -> Vec<LanguageMatch> {
    let mut heap = Vec::new();
    let mut best: Option<LanguageMatch> = None;
    for (index, (language, profile)) in LANGS.iter().enumerate() {
        if langs.is_some_and(|langs| !langs.contains(language)) {
            continue;
        }
        let mut rank: u32 = vector.iter().zip(profile).map(|(&a, &b)| a.abs_diff(b) as u32).sum();
        if rank > 500 && text_len < 150 {
            rank += (index as u32 * 3).min(50);
        }
        if rank as usize > cutoff {
            continue;
        }
        let value = LanguageMatch { language, rank };
        if n_results == 1 {
            if best.is_none_or(|best| rank < best.rank) {
                best = Some(value);
            }
        } else if n_results > 1 {
            push_heap(&mut heap, value);
        }
    }
    if n_results == 1 {
        return best.into_iter().collect();
    }
    (0..n_results.min(heap.len())).map(|_| pop_heap(&mut heap)).collect()
}

/// Perform very fast, linear-time language detection on a Unicode string.
///
/// `text` is converted to a normalized 256-bin character n-gram vector and
/// compared with 101 language profiles. The calculated out-of-place rank
/// indicates how far the text is from a matching profile: higher ranks mean
/// less accurate detection, and values above 1200 are usually false results.
/// Longer texts generally produce more reliable predictions. The given Unicode
/// string should be in composed normal form (NFC) for best results; no Unicode
/// normalization or case folding is performed automatically.
///
/// `cutoff` is the inclusive maximum accepted rank; 1200 is recommended.
/// `n_results` is the maximum number of matches to return. `langs=None` considers
/// all supported language codes; an empty slice considers none, and unsupported
/// codes are ignored. Restricting candidates reduces profile comparisons.
///
/// Returns up to `n_results` matches in ascending rank order. No permitted match
/// within the cutoff, or `n_results=0`, returns an empty vector. Empty or entirely
/// ignored text still produces a vector and can match a profile. Short uncertain
/// texts receive a bias toward more common languages; [`rank_vector`] describes
/// the bias and tie ordering. Ranks are integer distances, not probabilities.
///
/// ```
/// use resiliparse::parse::lang::detect_fast;
/// let result = detect_fast("This is a synthetic English sentence about language detection.", 1200, 1, None);
/// assert_eq!(result[0].language, "en");
/// assert_eq!(result[0].rank, 848);
/// ```
pub fn detect_fast(text: &str, cutoff: usize, n_results: usize, langs: Option<&[&str]>) -> Vec<LanguageMatch> {
    let vector = language_vector(text, 256).unwrap();
    rank_vector(vector.as_slice().try_into().unwrap(), text.chars().count(), cutoff, n_results, langs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_character_definitions() {
        // Alphabetic marks and letter numbers are retained alongside letters.
        for cp in [0x41, 0xe9, 0x345, 0x2160, 0x10400, 0x20000] {
            assert!(matches!(classify_codepoint(cp), CharacterClass::Letter), "{cp:x}");
        }
        for cp in [0x09, 0x20, 0x85, 0xa0, 0x1680, 0x2000, 0x2028, 0x3000] {
            assert!(matches!(classify_codepoint(cp), CharacterClass::Whitespace), "{cp:x}");
        }
        for cp in [0x1c, 0x301, 0xd800, 0x10ffff, 0x110000, u32::MAX] {
            assert!(matches!(classify_codepoint(cp), CharacterClass::Other), "{cp:x}");
        }
        // Verify the generated lookup against the toolchain that defines it.
        for c in (0..=0x10ffff).filter_map(char::from_u32) {
            assert_eq!(matches!(classify_codepoint(c as u32), CharacterClass::Letter), c.is_alphabetic());
            assert_eq!(matches!(classify_codepoint(c as u32), CharacterClass::Whitespace), c.is_whitespace());
        }
    }

    #[test]
    fn frozen_training_vectors() {
        assert_eq!(train_language_examples(["abc"], 8).unwrap(), [85, 0, 85, 85, 0, 85, 85, 85]);
        assert_eq!(train_language_examples(["a ! b"], 8).unwrap(), [192, 64, 0, 64, 0, 192, 0, 128]);
        assert_eq!(
            train_language_examples(["é e\u{301}\u{a0}Ω\u{2003}中"], 16).unwrap(),
            [146, 0, 109, 36, 36, 36, 73, 0, 109, 36, 73, 109, 36, 36, 73, 0]
        );
        assert_eq!(train_language_examples(["abc", ""], 8).unwrap(), [42, 0, 42, 42, 0, 42, 42, 42]);
        assert!(train_language_examples(Vec::<&str>::new(), 8).is_err());
        assert!(language_vector("abc", 0).is_err());
    }

    #[test]
    fn cutoff_and_filters() {
        assert_eq!(supported_langs().len(), 101);
        assert!(detect_fast("", 1014, 1, None).is_empty());
        assert_eq!(
            detect_fast("", 1015, 1, None),
            [LanguageMatch {
                language: "en",
                rank: 1015
            }]
        );
        assert!(detect_fast("", 1200, 1, Some(&[])).is_empty());
        assert_eq!(
            detect_fast("", 1200, 3, Some(&["en", "fr", "de"])),
            [
                LanguageMatch {
                    language: "en",
                    rank: 1015
                },
                LanguageMatch {
                    language: "fr",
                    rank: 1026
                },
                LanguageMatch {
                    language: "de",
                    rank: 1027
                },
            ]
        );
    }
}
