# Copyright 2021 Janek Bevendorff
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

# distutils: language = c++

import typing as t

from libc.stdint cimport uint32_t, uint8_t
from libcpp.algorithm cimport pop_heap, push_heap
from cpython.unicode cimport Py_UNICODE_ISALPHA, Py_UNICODE_ISSPACE

cdef lang_vec_t str_to_vec(str train_text, size_t vec_len=LANG_VEC_SIZE):
    cdef long hash2
    cdef long hash3
    cdef long hash4
    cdef long hash5
    cdef Py_UCS4[2] ngram2 = [0, 0]
    cdef Py_UCS4[3] ngram3 = [0, 0, 0]
    cdef Py_UCS4[4] ngram4 = [0, 0, 0, 0]
    cdef Py_UCS4[5] ngram5 = [0, 0, 0, 0, 0]

    cdef vector[uint32_t] count_vec32
    count_vec32.resize(vec_len)

    cdef bint prev_is_space = False
    cdef size_t i = 0
    cdef Py_UCS4 uchar
    for uchar in train_text:
        if Py_UNICODE_ISALPHA(uchar):
            prev_is_space = False
        elif Py_UNICODE_ISSPACE(uchar):
            if prev_is_space:
                continue
            prev_is_space = True
        else:
            prev_is_space = False
            continue

        # Shift n-gram buffers
        if i > 0:
            ngram2[0] = ngram2[1]
            shiftleft(ngram3, 3)
            shiftleft(ngram4, 4)
            shiftleft(ngram5, 5)
        ngram2[1] = uchar
        ngram3[2] = uchar
        ngram4[3] = uchar
        ngram5[4] = uchar

        count_vec32[hash(&uchar, 1) % count_vec32.size()] += 1
        if i >= 1:
            count_vec32[hash(ngram2, 2) % count_vec32.size()] += 1
        if i >= 2:
            count_vec32[hash(ngram3, 3) % count_vec32.size()] += 1
        if i >= 3:
            count_vec32[hash(ngram4, 4) % count_vec32.size()] += 1
        if i >= 4:
            count_vec32[hash(ngram5, 5) % count_vec32.size()] += 1

        i += 1

    # Normalize vector
    cdef size_t j
    cdef lang_vec_t lang_vec
    lang_vec.resize(count_vec32.size())
    if i > 0:
        for j in range(count_vec32.size()):
            lang_vec[j] = min(255u, count_vec32[j] * 256u // i)

    return lang_vec


cdef size_t cmp_oop_ranks(const uint8_t* vec1, const uint8_t* vec2, size_t size):
    cdef size_t rank = 0
    cdef size_t i
    for i in range(size):
        if vec1[i] > vec2[i]:
            rank += vec1[i] - vec2[i]
        else:
            rank += vec2[i] - vec1[i]
    return rank


ctypedef struct lang_rank_t:
    uint32_t rank
    const char* lang


cdef inline bint lang_rank_greater(const lang_rank_t& a, const lang_rank_t& b):
    return a.rank > b.rank


cpdef detect_fast(str text, size_t cutoff=1200, size_t n_results=1, langs=None):
    """
    detect_fast(text, cutoff=1000, num_results=1, restrict_langs=None)
    
    Perform a very fast (linear-time) language detection on the input string.
    
    The output is a tuple of the detected language name and the calculated
    out-of-place rank, which indicates how far the given text is from the closest-matching
    language profile. The higher the rank, the less accurate the detection is. Values
    above 1000 are usually false results.
    
    The given Unicode string should be in composed normal form (NFC) for the best results.
    
    :param text: input text
    :type text: str
    :param cutoff: OOP rank cutoff after which to return ``"unknown"``
    :type cutoff: int
    :param n_results: if this is greater than one, a list of the ``n_results`` best matches will be returned
    :type n_results: int
    :param langs: restrict detection to these languages
    :type langs: list[str]
    :return: tuple of the detected language (or ``"unknown"``) and its out-of-place rank
    :rtype: (str, int) | list[(str, int)]
    """
    cdef lang_vec_t text_vec = str_to_vec(text, LANG_VEC_SIZE)
    cdef size_t min_rank = <size_t>-1
    cdef const char* lang = NULL
    cdef vector[lang_rank_t] predicted
    cdef size_t i
    cdef size_t rank

    if langs:
        langs = set(langs)

    for i in range(N_LANGS):
        if langs is not None and LANGS[i].lang.decode() not in langs:
            continue

        # Bias rank by position in the language list as tie-breaker between close matches
        rank = cmp_oop_ranks(text_vec.data(), LANGS[i].vec, LANG_VEC_SIZE) + min(i, 20u)
        if rank > cutoff:
            continue

        if n_results == 1 and rank < min_rank:
            min_rank = rank
            lang = LANGS[i].lang
        elif n_results > 1:
            predicted.push_back([rank, LANGS[i].lang])
            push_heap(predicted.begin(), predicted.end(), &lang_rank_greater)

    if n_results == 1:
        if lang == NULL:
            return "unknown", 0
        return lang.decode(), min_rank

    result_list = []
    for i in range(min(n_results, predicted.size())):
        pop_heap(predicted.begin(), predicted.end(), &lang_rank_greater)
        result_list.append((predicted.back().lang.decode(), predicted.back().rank))
        predicted.pop_back()

    return result_list


def supported_langs():
    """
    supported_langs()

    Get a list of all languages that are supported by the fast language detector.

    :return: list of supported languages
    :rtype: list[str]
    """

    cdef size_t i
    langs = []
    for i in range(N_LANGS):
        langs.append(LANGS[i].lang.decode())
    return sorted(langs)


cpdef train_language_examples(examples, size_t vec_len=LANG_VEC_SIZE):
    """
    train_language_examples(examples, vec_len=200)

    Train a language vector for fast language detection on a list of example texts.

    :param examples: list of example texts for this language
    :type examples: t.Iterable[str]
    :param vec_len: output vector length
    :type vec_len: int
    :return: vector of trained values
    :rtype: List[int]
    """
    cdef vector[uint32_t] agg_vec
    agg_vec.resize(vec_len)

    cdef lang_vec_t tmp_vec
    cdef size_t example_count = 0
    cdef size_t i
    for text in examples:
        tmp_vec = str_to_vec(text, vec_len)
        for i in range(tmp_vec.size()):
            agg_vec[i] += tmp_vec[i]
        example_count += 1

    cdef lang_vec_t agg_vec8
    agg_vec8.resize(agg_vec.size())
    for i in range(agg_vec.size()):
        agg_vec8[i] = agg_vec[i] // example_count

    return agg_vec8
