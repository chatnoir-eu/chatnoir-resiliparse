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


from resiliparse_inc.string cimport string
from resiliparse_inc.uchardet cimport uchardet_t

cdef class EncodingDetector:
    cdef uchardet_t d

    cpdef void update(self, const string& data)
    cpdef str encoding(self, bint html5_compatible=*)

cpdef str detect_encoding(bytes data, size_t max_len=*, bint html5_compatible=*, bint from_html_meta=*)
cpdef str bytes_to_str(bytes data, str encoding=*, str errors=*, fallback_encodings=*, bint strip_bom=*)
cpdef bytes read_http_chunk(reader)
