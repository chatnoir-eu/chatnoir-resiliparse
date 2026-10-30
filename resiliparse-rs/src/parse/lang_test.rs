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

#[test]
fn language_detection_and_training() {
    let english = "The quick brown fox jumps over the lazy dog. This is a longer sentence about language detection.";
    assert_eq!(detect_fast(english, 1200, 1, None)[0].language, "en");
    assert_eq!(detect_fast(english, 1200, 1, Some(&["fr", "en"]))[0].language, "en");
    assert!(detect_fast(english, 1200, 10, Some(&["xx"])).is_empty());

    let one = train_language_examples(["abc"], 8).unwrap();
    let averaged = train_language_examples(["abc", ""], 8).unwrap();
    assert_eq!(one, [85, 0, 85, 85, 0, 85, 85, 85]);
    assert_eq!(averaged, [42, 0, 42, 42, 0, 42, 42, 42]);
    assert_eq!(train_language_examples(["a \t\n b"], 8), train_language_examples(["a b"], 8));
    assert_ne!(train_language_examples(["A a"], 8), train_language_examples(["a a"], 8));
}

#[test]
fn incremental_vectors_and_dimensions() {
    let text = "A α\u{2003}中!B";
    let mut vector = LanguageVector::new(512).unwrap();
    for c in text.chars() {
        vector.push_codepoint(c as u32);
    }
    let result = vector.finish();
    // Extra bins do not change the 256-bin profile and must remain zero.
    assert_eq!(&result[..256], language_vector(text, 256).unwrap());
    assert_eq!(&result[256..], &[0; 256]);
    assert_eq!(language_vector("!?123", 8).unwrap(), [0; 8]);
    assert_eq!(LanguageVector::new(8).unwrap().finish(), [0; 8]);
    assert!(LanguageTrainer::new(0).is_err());
    assert!(LanguageTrainer::new(8).unwrap().finish().is_err());
}

#[test]
fn ranking_limits_and_order() {
    let vector: [u8; 256] = language_vector("A long English article about language detection.", 256)
        .unwrap()
        .try_into()
        .unwrap();
    // Request every profile to exercise heap ordering beyond the single-best path.
    let ranked = rank_vector(&vector, 200, usize::MAX, usize::MAX, None);
    assert_eq!(ranked.len(), supported_langs().len());
    assert!(ranked.windows(2).all(|pair| pair[0].rank <= pair[1].rank));
    let unique: std::collections::HashSet<_> = ranked.iter().map(|item| item.language).collect();
    assert_eq!(unique.len(), ranked.len());
    assert!(rank_vector(&vector, 200, usize::MAX, 0, None).is_empty());
    let filtered = rank_vector(&vector, 200, usize::MAX, 10, Some(&["en", "de", "unknown"]));
    // Filter order, duplicates, and unsupported codes must not change the ranking.
    let reversed = rank_vector(&vector, 200, usize::MAX, 10, Some(&["de", "en", "en"]));
    assert_eq!(filtered, reversed);
    assert_eq!(filtered.len(), 2);
    // A cutoff equal to the best rank includes it; lowering the cutoff by one excludes it.
    let best = ranked[0];
    assert_eq!(rank_vector(&vector, 200, best.rank as usize, 1, None), [best]);
    if best.rank > 0 {
        assert!(rank_vector(&vector, 200, best.rank as usize - 1, 1, None).is_empty());
    }
}
