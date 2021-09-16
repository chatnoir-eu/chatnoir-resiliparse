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

from libc.stdint cimport uint32_t, int32_t, uint8_t
from libcpp.vector cimport vector

cdef extern from "lang_profiles.h" nogil:
    cdef const size_t LANG_VEC_SIZE
    ctypedef const uint8_t lang_rawvec_t[LANG_VEC_SIZE]

    ctypedef struct lang_t:
        const char* lang
        const lang_rawvec_t vec

    cdef const size_t N_LANGS
    cdef const lang_t LANGS[LANG_VEC_SIZE]

ctypedef vector lang_vec_t[uint8_t]

cdef inline uint8_t hash(Py_UCS4* ustr, int order):
    """
    FNV-1a hash (32-bit, 8-bit folded).
    Reference: http://www.isthe.com/chongo/tech/comp/fnv/
    """
    cdef uint32_t h = 2166136261
    cdef int i
    for i in range(order):
        h = h ^ <uint32_t>ustr[i]
        h = h * 16777619
    return <uint8_t>(((h >> 8) ^ h) & ((<uint32_t>1 << 8) - 1))


cdef inline void shiftleft(Py_UCS4* ustr, int size):
    cdef int i
    for i in range(size - 1):
        ustr[i] = ustr[i + 1]


cdef lang_vec_t str_to_vec(str train_text, size_t vec_len=*)
cdef size_t cmp_oop_ranks(const uint8_t* vec1, const uint8_t* vec2, size_t size)
cpdef detect_fast(str text, size_t cutoff=*, size_t n_results=*, langs=*)
