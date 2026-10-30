# Copyright 2026 Janek Bevendorff
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

import pytest

from resiliparse.parse import lang


def test_supported_languages_returns_a_fresh_python_list():
    languages = lang.supported_langs()
    assert isinstance(languages, list)
    assert languages == sorted(set(languages))
    languages.clear()
    assert len(lang.supported_langs()) == 101


def test_detect_fast_python_return_shapes_and_iterable_candidates():
    single = lang.detect_fast("English text for language detection.")
    multiple = lang.detect_fast("English text for language detection.", n_results=2)
    assert isinstance(single, tuple) and len(single) == 2
    assert isinstance(multiple, list) and len(multiple) == 2

    for candidates in (['en', 'fr'], ('en', 'fr'), iter(['en', 'fr'])):
        assert lang.detect_fast("English text for language detection.", langs=candidates)[0] == 'en'


def test_detect_fast_python_candidate_membership_and_empty_results():
    class LanguageCode(str):
        pass

    text = "English text for language detection."
    assert lang.detect_fast(text, langs=[]) == ('unknown', 0)
    assert lang.detect_fast(text, langs=[LanguageCode('en')])[0] == 'en'
    assert lang.detect_fast(text, langs=['en', 1])[0] == 'en'
    assert lang.detect_fast(text, n_results=0) == []


def test_detect_fast_python_argument_type_errors():
    with pytest.raises(TypeError):
        lang.detect_fast(b'English text')
    with pytest.raises(TypeError):
        lang.detect_fast('English text', langs=42)
    assert lang.detect_fast('English text', langs=[object()]) == ('unknown', 0)


@pytest.mark.parametrize('options', [{'cutoff': -1}, {'n_results': -1}])
def test_detect_fast_rejects_negative_python_integer_arguments(options):
    with pytest.raises(OverflowError):
        lang.detect_fast("text", **options)


def test_train_language_examples_returns_python_byte_values():
    result = lang.train_language_examples(iter(['abc']), vec_len=8)
    assert isinstance(result, list)
    assert len(result) == 8
    assert all(type(value) is int and 0 <= value <= 255 for value in result)


def test_train_language_examples_python_input_errors():
    with pytest.raises(TypeError):
        lang.train_language_examples(42)
    with pytest.raises(TypeError):
        lang.train_language_examples(['not text', object()])


@pytest.mark.parametrize('suffix', ['\ud800', '\U0001f600'])
def test_language_vector_python_unicode_storage(suffix):
    # Surrogates and emoji force CPython's two- and four-byte storage, respectively.
    # Both are ignored by the language vector, so the ASCII vector must be preserved.
    assert lang.train_language_examples(['abc' + suffix]) == lang.train_language_examples(['abc'])
    result = lang.detect_fast('English text for language detection.' + suffix)
    assert isinstance(result, tuple) and result[0] == 'en'
