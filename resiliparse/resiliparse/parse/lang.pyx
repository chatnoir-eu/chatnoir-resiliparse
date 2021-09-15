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

from libc.stdint cimport int32_t, uint32_t, uint8_t
from libcpp.vector cimport vector
from cpython.unicode cimport Py_UNICODE_ISALPHA, Py_UNICODE_ISSPACE

cdef size_t LANG_VEC_SIZE = 200
ctypedef uint8_t[200] lang_rawvec_t
ctypedef vector[uint8_t] lang_vec_t
cdef lang_rawvec_t VEC_EN = [4, 2, 3, 4, 3, 2, 4, 3, 3, 2, 4, 2, 1, 1, 3, 3, 6, 3, 4, 2, 3, 3, 2, 2, 7, 2, 2, 6, 6, 3,
                             4, 3, 3, 4, 5, 5, 3, 5, 4, 3, 5, 2, 4, 3, 7, 2, 2, 2, 6, 36, 4, 2, 5, 3, 4, 3, 3, 2, 2, 5,
                             2, 3, 3, 2, 4, 2, 3, 3, 2, 3, 4, 2, 3, 2, 2, 3, 2, 2, 4, 3, 3, 3, 4, 3, 3, 4, 4, 2, 3, 3,
                             8, 3, 5, 5, 2, 2, 2, 3, 4, 2, 4, 2, 2, 2, 2, 2, 3, 4, 3, 3, 5, 2, 2, 3, 16, 4, 7, 8, 21, 6,
                             6, 10, 17, 2, 9, 9, 6, 15, 16, 6, 3, 12, 13, 16, 8, 3, 4, 2, 8, 2, 2, 2, 2, 2, 2, 2, 2, 2,
                             2, 3, 1, 3, 3, 2, 2, 8, 3, 2, 4, 3, 4, 2, 4, 2, 2, 3, 4, 2, 3, 4, 2, 2, 5, 4, 3, 3, 4, 2,
                             3, 3, 2, 2, 2, 3, 2, 1, 3, 2, 2, 6, 3, 3, 3, 4, 2, 2, 4, 2, 3, 2]
cdef lang_rawvec_t VEC_ML = [4, 6, 4, 2, 2, 3, 2, 2, 2, 3, 2, 2, 3, 2, 3, 5, 2, 1, 2, 2, 2, 2, 2, 6, 2, 3, 1, 1, 2, 3,
                             1, 3, 2, 4, 2, 2, 1, 3, 4, 2, 2, 2, 3, 4, 3, 2, 4, 2, 2, 31, 3, 3, 4, 2, 3, 4, 2, 2, 2, 3,
                             2, 4, 2, 2, 3, 3, 2, 2, 7, 4, 4, 7, 2, 4, 2, 3, 5, 5, 3, 2, 2, 5, 3, 4, 4, 2, 1, 5, 7, 5,
                             3, 2, 3, 4, 3, 10, 2, 5, 3, 4, 4, 3, 3, 11, 2, 3, 2, 2, 4, 3, 2, 2, 3, 4, 4, 3, 3, 3, 1, 2,
                             3, 2, 4, 2, 4, 3, 5, 3, 1, 3, 2, 3, 3, 3, 2, 3, 4, 4, 2, 4, 4, 2, 2, 3, 2, 3, 3, 3, 1, 3,
                             5, 5, 3, 4, 2, 2, 4, 5, 4, 5, 3, 3, 2, 3, 2, 3, 22, 9, 3, 2, 9, 7, 2, 4, 4, 7, 8, 3, 5, 3,
                             6, 14, 2, 4, 6, 22, 2, 11, 4, 4, 3, 9, 12, 11, 7, 10, 6, 4, 9, 4]
cdef lang_rawvec_t VEC_HI = [5, 3, 3, 2, 4, 4, 4, 2, 3, 5, 3, 4, 3, 2, 3, 4, 2, 2, 2, 3, 4, 1, 2, 7, 2, 2, 3, 4, 5, 5,
                             6, 2, 3, 3, 10, 4, 2, 3, 4, 2, 6, 1, 2, 2, 12, 6, 2, 1, 3, 63, 2, 3, 3, 2, 4, 2, 1, 4, 2,
                             2, 2, 3, 1, 4, 2, 4, 3, 5, 4, 6, 3, 11, 1, 5, 2, 2, 5, 2, 2, 5, 9, 2, 2, 4, 1, 4, 2, 1, 2,
                             1, 3, 2, 1, 3, 1, 3, 3, 3, 3, 6, 2, 2, 1, 2, 2, 2, 2, 3, 2, 2, 2, 2, 6, 1, 3, 3, 3, 2, 3,
                             6, 3, 2, 2, 2, 2, 3, 4, 5, 4, 2, 2, 4, 1, 5, 2, 3, 4, 2, 2, 3, 3, 4, 22, 3, 6, 2, 3, 4, 3,
                             4, 13, 2, 5, 5, 5, 3, 2, 8, 3, 4, 3, 11, 7, 11, 3, 5, 4, 10, 8, 14, 1, 9, 2, 2, 5, 8, 2,
                             11, 16, 3, 1, 2, 3, 2, 1, 3, 3, 2, 2, 2, 3, 1, 2, 2, 1, 2, 8, 5, 2, 2]
cdef lang_rawvec_t VEC_TA = [4, 10, 16, 16, 5, 1, 1, 19, 9, 12, 13, 14, 10, 4, 12, 1, 4, 4, 2, 2, 1, 2, 3, 2, 6, 4, 3,
                             3, 4, 3, 3, 2, 2, 2, 4, 4, 2, 7, 3, 4, 2, 2, 3, 1, 4, 3, 1, 5, 2, 39, 1, 2, 1, 2, 3, 3, 5,
                             1, 2, 4, 2, 3, 3, 3, 2, 3, 3, 2, 6, 4, 6, 2, 5, 3, 5, 2, 2, 5, 3, 6, 3, 3, 3, 2, 6, 2, 2,
                             4, 2, 6, 3, 3, 2, 3, 6, 3, 2, 2, 2, 5, 3, 3, 3, 9, 3, 5, 2, 3, 2, 5, 3, 4, 2, 1, 3, 2, 5,
                             3, 1, 2, 1, 3, 1, 1, 3, 4, 3, 4, 3, 3, 1, 6, 3, 3, 3, 4, 1, 3, 2, 6, 2, 2, 4, 4, 3, 2, 2,
                             2, 1, 3, 3, 2, 2, 2, 2, 1, 2, 1, 1, 3, 2, 2, 4, 2, 2, 4, 4, 3, 7, 2, 3, 2, 4, 6, 2, 5, 2,
                             2, 3, 6, 1, 1, 27, 2, 2, 1, 5, 8, 4, 3, 2, 3, 14, 1, 2, 1, 4, 18, 2, 3]
cdef lang_rawvec_t VEC_PT = [10, 2, 2, 4, 2, 3, 2, 4, 4, 3, 2, 2, 2, 2, 2, 2, 4, 3, 3, 5, 6, 2, 3, 2, 4, 2, 2, 2, 4, 3,
                             6, 5, 4, 2, 7, 3, 3, 6, 2, 3, 2, 3, 4, 3, 5, 2, 2, 3, 6, 35, 4, 3, 3, 5, 3, 4, 2, 3, 5, 3,
                             1, 2, 3, 2, 3, 2, 2, 3, 2, 3, 2, 1, 3, 3, 2, 5, 4, 3, 3, 4, 4, 3, 4, 3, 2, 2, 3, 3, 2, 3,
                             10, 4, 3, 5, 3, 3, 4, 5, 3, 2, 5, 3, 2, 1, 2, 2, 2, 4, 2, 3, 5, 3, 3, 3, 19, 5, 7, 10, 22,
                             3, 5, 3, 14, 2, 9, 7, 9, 11, 23, 8, 4, 11, 16, 9, 10, 5, 1, 3, 6, 2, 1, 2, 2, 2, 4, 3, 2,
                             4, 2, 2, 2, 3, 1, 1, 2, 3, 2, 4, 4, 4, 2, 3, 5, 2, 4, 4, 8, 4, 4, 3, 2, 2, 5, 5, 3, 3, 3,
                             2, 2, 2, 2, 2, 2, 2, 2, 3, 4, 2, 2, 2, 2, 2, 2, 4, 2, 3, 2, 3, 2, 2]
cdef lang_rawvec_t VEC_FR = [4, 2, 2, 4, 2, 3, 2, 3, 3, 3, 3, 2, 1, 2, 4, 3, 4, 3, 4, 6, 4, 2, 4, 2, 4, 2, 3, 5, 5, 3,
                             3, 4, 3, 3, 5, 2, 3, 5, 3, 5, 3, 2, 2, 1, 4, 1, 1, 2, 6, 34, 8, 3, 4, 5, 4, 4, 2, 2, 4, 4,
                             2, 3, 4, 2, 3, 3, 3, 2, 3, 4, 4, 2, 3, 2, 2, 4, 3, 3, 5, 4, 4, 3, 3, 3, 2, 4, 2, 2, 2, 3,
                             15, 4, 3, 6, 3, 3, 5, 5, 3, 2, 3, 2, 3, 2, 2, 3, 3, 5, 2, 2, 3, 2, 2, 3, 14, 4, 7, 8, 26,
                             3, 4, 3, 15, 3, 11, 10, 6, 14, 14, 7, 3, 12, 16, 14, 13, 5, 3, 3, 3, 3, 2, 2, 2, 2, 3, 2,
                             2, 4, 2, 4, 2, 2, 2, 2, 2, 6, 3, 2, 2, 2, 2, 2, 4, 2, 3, 3, 5, 2, 4, 4, 3, 3, 5, 5, 3, 3,
                             5, 2, 2, 3, 3, 1, 2, 2, 2, 1, 4, 2, 2, 3, 2, 3, 2, 4, 2, 4, 2, 3, 2, 2]
cdef lang_rawvec_t VEC_NL = [4, 1, 3, 4, 3, 3, 2, 2, 2, 2, 3, 2, 1, 2, 4, 3, 3, 4, 2, 5, 4, 4, 4, 5, 6, 4, 4, 2, 5, 3,
                             3, 5, 2, 2, 3, 3, 2, 6, 5, 4, 7, 5, 4, 2, 6, 2, 2, 2, 5, 35, 4, 2, 5, 2, 3, 2, 2, 2, 2, 4,
                             2, 3, 3, 1, 2, 1, 2, 2, 2, 3, 2, 3, 1, 1, 3, 4, 5, 3, 2, 4, 3, 3, 5, 4, 2, 3, 4, 2, 4, 4,
                             9, 3, 3, 5, 2, 2, 3, 3, 2, 2, 3, 2, 1, 2, 2, 2, 3, 3, 2, 3, 3, 2, 2, 3, 17, 4, 4, 11, 33,
                             3, 8, 7, 14, 7, 12, 7, 6, 17, 14, 3, 2, 10, 9, 13, 5, 5, 4, 2, 3, 4, 2, 2, 2, 3, 3, 4, 1,
                             2, 2, 2, 3, 3, 3, 1, 2, 9, 2, 3, 3, 5, 3, 4, 3, 3, 2, 3, 7, 3, 8, 9, 2, 2, 6, 3, 6, 2, 4,
                             2, 3, 2, 3, 2, 5, 3, 1, 1, 3, 2, 3, 2, 3, 3, 2, 2, 1, 2, 4, 2, 2, 2]
cdef lang_rawvec_t VEC_ES = [10, 1, 2, 4, 3, 4, 2, 2, 4, 2, 3, 3, 2, 1, 2, 2, 4, 4, 4, 4, 7, 2, 4, 2, 4, 3, 3, 5, 4, 3,
                             4, 5, 4, 3, 8, 3, 3, 5, 3, 2, 4, 2, 5, 3, 3, 2, 3, 2, 6, 35, 3, 3, 2, 5, 4, 4, 2, 2, 5, 3,
                             2, 2, 4, 1, 3, 2, 2, 3, 2, 3, 3, 2, 2, 2, 2, 2, 3, 2, 4, 4, 4, 3, 4, 5, 2, 2, 3, 2, 1, 3,
                             10, 3, 2, 5, 2, 2, 3, 3, 3, 2, 4, 4, 2, 2, 2, 2, 2, 5, 2, 3, 5, 2, 3, 3, 19, 5, 9, 9, 23,
                             3, 5, 3, 13, 2, 9, 9, 6, 14, 21, 7, 5, 12, 15, 10, 10, 4, 1, 3, 4, 2, 2, 1, 2, 2, 4, 3, 3,
                             4, 3, 2, 2, 3, 2, 1, 2, 3, 3, 3, 5, 4, 2, 2, 4, 1, 3, 3, 8, 2, 6, 6, 2, 2, 6, 6, 3, 2, 3,
                             2, 3, 2, 2, 2, 2, 3, 2, 2, 4, 2, 2, 3, 2, 3, 1, 4, 2, 3, 2, 3, 2, 2]
cdef lang_rawvec_t VEC_EL = [1, 3, 3, 4, 1, 1, 3, 2, 5, 3, 3, 1, 1, 4, 3, 4, 3, 3, 3, 2, 3, 4, 2, 4, 4, 2, 5, 3, 3, 3,
                             3, 4, 4, 3, 2, 3, 3, 4, 2, 2, 3, 3, 3, 2, 1, 2, 5, 2, 2, 31, 2, 3, 1, 3, 8, 2, 2, 4, 3, 4,
                             3, 3, 3, 3, 5, 2, 3, 4, 5, 2, 2, 2, 2, 5, 5, 5, 4, 3, 2, 4, 2, 2, 5, 6, 3, 4, 2, 3, 4, 4,
                             2, 5, 3, 2, 2, 1, 2, 2, 2, 5, 2, 5, 8, 3, 2, 3, 3, 3, 3, 2, 3, 2, 2, 2, 3, 2, 5, 2, 3, 3,
                             2, 2, 4, 4, 5, 3, 3, 2, 2, 3, 2, 3, 2, 2, 2, 2, 4, 2, 3, 4, 1, 6, 1, 2, 4, 3, 2, 3, 2, 2,
                             1, 3, 2, 4, 2, 2, 3, 6, 7, 5, 10, 4, 20, 6, 7, 6, 17, 2, 6, 4, 16, 7, 7, 7, 12, 6, 13, 9,
                             12, 10, 10, 20, 7, 5, 6, 3, 6, 3, 2, 6, 3, 4, 3, 3, 2, 1, 2, 1, 4, 2]
cdef lang_rawvec_t VEC_RU = [3, 5, 4, 8, 5, 5, 5, 3, 6, 3, 5, 3, 2, 2, 2, 3, 4, 4, 2, 3, 2, 2, 4, 1, 2, 5, 2, 2, 3, 2,
                             2, 2, 3, 2, 3, 2, 2, 2, 2, 7, 3, 4, 6, 3, 3, 2, 3, 2, 6, 31, 3, 2, 2, 4, 2, 2, 2, 2, 2, 4,
                             2, 4, 3, 3, 3, 2, 2, 4, 2, 3, 4, 3, 4, 2, 4, 2, 3, 3, 3, 2, 3, 3, 3, 3, 3, 2, 3, 3, 3, 17,
                             6, 10, 5, 9, 18, 4, 7, 15, 5, 7, 9, 9, 12, 21, 7, 9, 11, 14, 7, 3, 7, 2, 6, 3, 3, 2, 5, 6,
                             3, 3, 7, 3, 3, 2, 2, 3, 2, 3, 3, 2, 2, 2, 4, 2, 3, 3, 2, 2, 3, 2, 2, 4, 2, 2, 2, 1, 5, 2,
                             2, 6, 2, 3, 5, 2, 2, 4, 3, 2, 3, 3, 4, 2, 3, 6, 2, 2, 3, 2, 3, 2, 2, 3, 4, 3, 3, 2, 4, 3,
                             2, 3, 4, 2, 2, 3, 2, 2, 3, 3, 2, 3, 3, 6, 3, 4, 4, 4, 4, 5, 4, 6]
cdef lang_rawvec_t VEC_DK = [4, 2, 3, 3, 2, 2, 1, 2, 2, 2, 3, 2, 3, 2, 4, 2, 4, 4, 2, 7, 6, 6, 4, 5, 5, 3, 5, 2, 6, 4,
                             3, 4, 2, 3, 6, 3, 4, 6, 2, 2, 4, 2, 3, 2, 5, 1, 3, 3, 6, 36, 4, 3, 3, 2, 3, 2, 2, 2, 3, 4,
                             1, 2, 2, 2, 3, 3, 4, 3, 1, 2, 2, 3, 2, 2, 2, 1, 2, 3, 3, 2, 2, 2, 5, 4, 5, 4, 4, 2, 3, 3,
                             13, 3, 3, 8, 3, 2, 3, 3, 3, 1, 3, 2, 2, 2, 2, 2, 5, 3, 2, 2, 3, 2, 1, 2, 13, 3, 2, 15, 30,
                             5, 13, 6, 13, 4, 9, 8, 6, 13, 13, 4, 3, 14, 10, 12, 7, 6, 3, 2, 4, 2, 2, 3, 2, 2, 3, 3, 1,
                             2, 2, 3, 1, 3, 5, 1, 2, 7, 1, 1, 3, 2, 1, 4, 4, 1, 5, 3, 5, 3, 6, 5, 2, 2, 8, 3, 7, 2, 6,
                             2, 3, 3, 3, 2, 4, 4, 2, 2, 4, 1, 2, 2, 4, 3, 2, 3, 1, 2, 3, 1, 3, 3]
cdef lang_rawvec_t VEC_IT = [9, 3, 2, 5, 3, 3, 2, 4, 3, 2, 3, 2, 2, 2, 9, 3, 4, 3, 5, 5, 6, 2, 3, 2, 5, 2, 3, 4, 5, 3,
                             2, 5, 3, 3, 9, 2, 4, 5, 3, 3, 2, 2, 4, 3, 5, 2, 2, 2, 6, 33, 4, 3, 5, 5, 5, 3, 3, 1, 4, 3,
                             1, 3, 3, 3, 4, 2, 3, 2, 1, 3, 3, 1, 2, 2, 2, 3, 3, 3, 5, 3, 3, 3, 3, 3, 3, 2, 5, 1, 3, 3,
                             11, 4, 3, 5, 3, 4, 2, 4, 3, 2, 6, 2, 2, 2, 2, 2, 3, 5, 2, 3, 4, 2, 2, 4, 18, 4, 9, 8, 21,
                             4, 6, 4, 22, 2, 5, 10, 6, 15, 20, 8, 3, 12, 13, 13, 9, 5, 2, 2, 3, 3, 1, 1, 1, 2, 2, 3, 3,
                             2, 3, 3, 1, 4, 3, 2, 3, 2, 2, 2, 3, 2, 2, 2, 4, 2, 3, 3, 9, 3, 4, 4, 3, 2, 6, 4, 3, 2, 4,
                             2, 2, 1, 3, 1, 2, 3, 2, 1, 3, 2, 1, 3, 2, 3, 2, 3, 1, 2, 2, 3, 3, 2]
cdef lang_rawvec_t VEC_TR = [4, 1, 2, 5, 2, 2, 2, 4, 3, 2, 4, 2, 3, 3, 6, 2, 4, 6, 2, 3, 4, 3, 3, 3, 7, 2, 3, 2, 3, 2,
                             3, 2, 4, 2, 5, 3, 3, 4, 3, 2, 4, 3, 5, 2, 6, 2, 3, 2, 8, 27, 3, 3, 3, 4, 4, 3, 4, 3, 2, 2,
                             2, 3, 3, 3, 3, 2, 3, 3, 2, 6, 5, 1, 1, 2, 2, 3, 5, 2, 4, 2, 2, 2, 5, 2, 5, 3, 3, 2, 2, 3,
                             9, 4, 7, 6, 3, 3, 5, 3, 4, 2, 2, 3, 2, 4, 5, 2, 3, 2, 3, 2, 1, 1, 2, 2, 20, 7, 2, 9, 19, 2,
                             6, 3, 27, 3, 10, 12, 11, 15, 8, 5, 2, 14, 10, 8, 8, 4, 2, 3, 10, 5, 2, 4, 1, 2, 3, 3, 1, 2,
                             1, 3, 2, 3, 3, 2, 2, 3, 2, 3, 3, 3, 2, 2, 2, 2, 2, 4, 8, 3, 8, 6, 3, 3, 7, 3, 3, 5, 6, 2,
                             3, 4, 4, 2, 3, 4, 2, 2, 4, 2, 2, 5, 2, 2, 1, 3, 1, 2, 3, 4, 2, 2]
cdef lang_rawvec_t VEC_SV = [3, 2, 3, 5, 2, 2, 2, 3, 2, 2, 2, 4, 3, 2, 3, 2, 4, 5, 2, 6, 5, 4, 2, 4, 6, 3, 4, 2, 6, 3,
                             4, 3, 2, 4, 7, 3, 3, 6, 2, 4, 4, 4, 3, 2, 6, 6, 5, 2, 6, 34, 4, 2, 4, 2, 3, 2, 2, 2, 3, 2,
                             2, 2, 3, 3, 3, 1, 4, 5, 2, 1, 3, 3, 2, 2, 1, 2, 3, 3, 4, 3, 2, 2, 4, 2, 3, 4, 3, 2, 2, 3,
                             7, 2, 4, 7, 4, 2, 4, 3, 3, 2, 4, 2, 3, 4, 2, 4, 4, 2, 2, 4, 4, 2, 1, 3, 19, 3, 4, 10, 19,
                             5, 11, 5, 13, 4, 10, 10, 6, 14, 11, 5, 3, 16, 13, 16, 6, 7, 3, 2, 4, 2, 2, 4, 2, 2, 4, 2,
                             1, 2, 2, 3, 1, 2, 4, 1, 2, 8, 2, 2, 5, 3, 3, 3, 3, 1, 4, 4, 7, 2, 6, 5, 3, 2, 6, 2, 5, 2,
                             5, 3, 2, 3, 4, 2, 2, 5, 2, 2, 3, 2, 2, 3, 4, 3, 2, 3, 2, 2, 3, 2, 3, 3]
cdef lang_rawvec_t VEC_AR = [7, 5, 7, 2, 6, 4, 5, 4, 3, 3, 10, 3, 3, 4, 2, 3, 2, 3, 7, 6, 10, 20, 13, 11, 6, 9, 6, 17,
                             3, 4, 4, 3, 3, 2, 4, 3, 3, 3, 6, 3, 4, 4, 2, 3, 2, 3, 5, 3, 2, 36, 4, 3, 4, 3, 2, 2, 4, 3,
                             2, 2, 3, 4, 4, 2, 2, 3, 2, 2, 2, 5, 2, 2, 2, 2, 2, 2, 2, 3, 2, 2, 2, 2, 3, 4, 2, 4, 3, 3,
                             5, 2, 5, 2, 4, 2, 8, 5, 2, 4, 2, 3, 3, 2, 3, 3, 2, 4, 3, 3, 3, 2, 2, 2, 4, 2, 6, 2, 3, 2,
                             7, 3, 4, 3, 3, 5, 5, 3, 4, 5, 2, 4, 5, 3, 2, 2, 2, 3, 2, 3, 3, 3, 2, 2, 3, 3, 2, 6, 2, 3,
                             3, 2, 3, 3, 2, 3, 3, 2, 9, 2, 4, 2, 4, 2, 2, 2, 3, 3, 3, 3, 4, 1, 2, 2, 2, 2, 2, 2, 3, 3,
                             2, 3, 1, 2, 3, 3, 8, 3, 3, 3, 7, 2, 4, 3, 24, 8, 6, 10, 4, 5, 4, 5]
cdef lang_rawvec_t VEC_DE = [2, 3, 4, 3, 2, 5, 3, 4, 5, 2, 3, 3, 1, 1, 2, 5, 4, 5, 2, 5, 3, 4, 3, 3, 5, 3, 4, 2, 4, 2,
                             2, 1, 2, 2, 4, 2, 4, 6, 7, 3, 6, 2, 3, 4, 4, 4, 1, 3, 5, 35, 3, 2, 4, 3, 3, 2, 3, 2, 2, 3,
                             2, 1, 2, 4, 4, 2, 3, 4, 2, 3, 4, 2, 2, 1, 2, 3, 2, 2, 3, 3, 3, 5, 3, 6, 4, 4, 4, 3, 3, 3,
                             10, 2, 5, 6, 3, 2, 4, 5, 2, 2, 10, 2, 1, 2, 2, 3, 4, 2, 2, 3, 3, 1, 2, 2, 11, 5, 8, 9, 29,
                             3, 6, 12, 17, 4, 8, 7, 6, 20, 7, 2, 1, 10, 18, 12, 8, 3, 5, 2, 3, 4, 2, 2, 1, 1, 3, 3, 1,
                             1, 2, 3, 2, 3, 2, 3, 3, 7, 2, 2, 2, 4, 3, 4, 4, 7, 3, 2, 3, 3, 9, 8, 2, 2, 6, 5, 3, 3, 3,
                             2, 2, 2, 4, 3, 1, 6, 3, 4, 3, 2, 2, 2, 3, 3, 3, 4, 1, 3, 2, 3, 2, 3]
cdef lang_rawvec_t VEC_KN = [3, 4, 5, 2, 1, 3, 2, 11, 3, 3, 3, 1, 1, 4, 3, 2, 3, 2, 2, 2, 4, 2, 6, 3, 3, 3, 5, 1, 2, 3,
                             2, 3, 3, 3, 3, 3, 2, 3, 9, 3, 12, 2, 1, 3, 2, 2, 2, 5, 5, 40, 5, 2, 6, 16, 2, 21, 3, 28, 2,
                             6, 2, 6, 5, 10, 8, 14, 2, 11, 8, 3, 11, 6, 8, 9, 8, 4, 2, 4, 4, 7, 1, 6, 4, 3, 2, 2, 2, 4,
                             3, 4, 2, 4, 2, 2, 2, 4, 1, 3, 2, 3, 3, 2, 2, 2, 2, 1, 1, 2, 1, 4, 1, 1, 2, 2, 1, 1, 2, 3,
                             1, 3, 4, 3, 3, 1, 5, 3, 3, 6, 3, 2, 2, 2, 3, 3, 2, 4, 2, 8, 3, 2, 5, 2, 3, 5, 2, 4, 3, 1,
                             3, 2, 3, 5, 3, 5, 1, 4, 2, 3, 3, 9, 2, 4, 3, 3, 2, 6, 2, 3, 2, 6, 2, 1, 2, 2, 2, 8, 6, 2,
                             1, 2, 1, 2, 4, 1, 2, 3, 2, 2, 1, 2, 4, 3, 2, 3, 2, 2, 1, 2, 1, 10]


cdef struct lang_t:
    const char* lang,
    uint8_t* vec


cdef size_t N_LANGS = 17
cdef lang_t[17] LANGS = [
    [b'en', VEC_EN],
    [b'ml', VEC_ML],
    [b'hi', VEC_HI],
    [b'ta', VEC_TA],
    [b'pt', VEC_PT],
    [b'fr', VEC_FR],
    [b'nl', VEC_NL],
    [b'es', VEC_ES],
    [b'el', VEC_EL],
    [b'ru', VEC_RU],
    [b'dk', VEC_DK],
    [b'it', VEC_IT],
    [b'tr', VEC_TR],
    [b'sv', VEC_SV],
    [b'ar', VEC_AR],
    [b'de', VEC_DE],
    [b'kn', VEC_KN]]


cdef inline long hash(Py_UCS4* ustr, int order):
    cdef long h = 7
    cdef int i
    for i in range(order):
        h = 31 * h + <int32_t>ustr[i]
    return h


cdef inline void shiftleft(Py_UCS4* ustr, int size):
    cdef int i
    for i in range(size - 1):
        ustr[i] = ustr[i + 1]


cdef lang_vec_t str_to_vec(str train_text, size_t vec_len=N_LANGS):
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
            lang_vec[j] = count_vec32[j] * count_vec32.size() // i

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


cpdef detect_fast(str text, size_t cutoff=1000):
    """
    detect_fast(text)
    
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
    :return: tuple of the detected language (or ``"unknown"``) and its out-of-place rank
    :rtype: (str, int)
    """

    cdef lang_vec_t text_vec = str_to_vec(text, LANG_VEC_SIZE)
    cdef size_t min_rank = <size_t>-1
    cdef const char* lang = NULL
    cdef size_t i
    cdef size_t rank
    for i in range(N_LANGS):
        rank = cmp_oop_ranks(text_vec.data(), LANGS[i].vec, LANG_VEC_SIZE)
        if rank < min_rank:
            min_rank = rank
            lang = LANGS[i].lang

    if lang == NULL or min_rank > cutoff:
        return 'unknown', min_rank

    return lang.decode(), min_rank


def _train_language_examples(examples, size_t vec_len=N_LANGS):
    """
    train_language_examples(examples, vec_len=200)

    Train a language vector on a list of example texts.

    :param examples: list of example texts for this language
    :type examples: t.Iterable[str]
    :param vec_len: output vector length
    :type vec_len: int
    :return: list with trained values
    :rtype: List[int]
    """
    cdef lang_vec_t agg_vec
    agg_vec.resize(vec_len)

    cdef lang_vec_t tmp_vec
    cdef size_t i
    for text in examples:
        tmp_vec = str_to_vec(text, vec_len)
        for i in range(tmp_vec.size()):
            agg_vec[i] += tmp_vec[i]

    for i in range(agg_vec.size()):
        agg_vec[i] = agg_vec[i] // len(examples)

    return agg_vec


def _train_language_examples_cython_str(lang, examples, size_t vec_len=LANG_VEC_SIZE):
    """
    _train_language_examples_and_print(examples, vec_len=200)

    Train a language vector on a list of example texts using :func:`_train_language_examples`
    and return a Cython ``cdef`` string representation of it for copy and paste.

    :param lang: language code
    :type lang: str
    :param examples: list of example texts for this language
    :type examples: t.Iterable[str]
    :param vec_len: output vector length
    :type vec_len: int
    """

    vec = _train_language_examples(examples, vec_len)
    return f'cdef lang_rawvec_t VEC_{lang.upper()} = {vec}'
