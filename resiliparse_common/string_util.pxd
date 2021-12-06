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

from cython.operator cimport dereference as deref, predecrement as dec, preincrement as inc
from libcpp.string cimport string
from resiliparse_inc.cctype cimport isspace, tolower


cdef inline size_t lstrip_c_str(const char** s_ptr, size_t l) nogil:
    """Strip leading white space from a C string."""
    cdef const char* end = deref(s_ptr) + l
    while deref(s_ptr) < end and isspace(deref(s_ptr)[0]):
        inc(deref(s_ptr))
    return end - deref(s_ptr)


cdef inline size_t rstrip_c_str(const char** s_ptr, size_t l) nogil:
    """Strip trailing white space from a C string."""
    cdef const char* end = deref(s_ptr) + l
    while end > deref(s_ptr) and isspace((end - 1)[0]):
        dec(end)
    return end - deref(s_ptr)


cdef inline size_t strip_c_str(const char** s_ptr, size_t l) nogil:
    """Strip leading and trailing white space from a C string."""
    return rstrip_c_str(s_ptr, lstrip_c_str(s_ptr, l))


# cdef inline string lstrip_str(const string& s) nogil:
#     """Strip leading white space from a C++ string."""
#     cdef const char* start = s.data()
#     cdef size_t l = lstrip_c_str(&start, s.size())
#     if start == s.data():
#         return s
#     return string(start, l)


cdef inline string rstrip_str(const string& s) nogil:
    """Strip leading white space from a C++ string."""
    cdef const char* start = s.data()
    cdef size_t l = rstrip_c_str(&start, s.size())
    if l == s.size():
        return s
    return string(start, l)


cdef inline string strip_str(const string& s) nogil:
    """Strip leading and trailing white space from a C++ string."""
    cdef const char* start = s.data()
    cdef size_t l = strip_c_str(&start, s.size())
    return string(start, l)


cdef inline string str_to_lower(string s) nogil:
    """Convert a C++ string to lower-case characters."""
    cdef size_t i
    for i in range(s.size()):
        s[i] = tolower(s[i])
    return s
