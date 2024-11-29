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

from libcpp.string cimport string
from resiliparse_inc.string_view cimport string_view
from resiliparse_inc.cctype cimport tolower


cdef extern from * nogil:
    """
    #include <cctype>

    /**
     * Strip leading white space from a C string.
     */
    inline size_t lstrip_c_str(const char** s_ptr, size_t l) {
        const char* end = *s_ptr + l;
        while (*s_ptr < end && std::isspace((*s_ptr)[0])) {
            ++(*s_ptr);
        }
        return end - *s_ptr;
    }

    /**
     * Strip trailing white space from a C string.
     */
    inline size_t rstrip_c_str(const char** s_ptr, size_t l) {
        const char* end = *s_ptr + l;
        while (end > *s_ptr && std::isspace((end - 1)[0])) {
            --end;
        }
        return end - *s_ptr;
    }

    /**
     * Strip leading and trailing white space from a C string.
     */
    inline size_t strip_c_str(const char** s_ptr, size_t l) {
        return rstrip_c_str(s_ptr, lstrip_c_str(s_ptr, l));
    }

    /**
     * Strip leading white space from a C++ string.
     */
    inline std::string lstrip_str(const std::string& s) {
        const char* start = s.data();
        size_t l = lstrip_c_str(&start, s.size());
        return l != s.size() ? std::string(start, l) : s;
    }

    /**
     * Strip trailing white space from a C++ string.
     */
    inline std::string rstrip_str(std::string&& s) {
        const char* start = s.data();
        size_t l = rstrip_c_str(&start, s.size());
        if (l != s.size()) {
            s.resize(l);
        }
        return s;
    }

    /**
     * Strip leading and trailing white space from a C++ string.
     */
    inline std::string strip_str(std::string&& s) {
        const char* start = s.data();
        size_t l = strip_c_str(&start, s.size());
        if (l != s.size()) {
            memmove(s.data(), start, l);
            s.resize(l);
        }
        return s;
    }
    """

    cdef size_t rstrip_c_str(const char** s_ptr, size_t l)
    cdef size_t lstrip_c_str(const char** s_ptr, size_t l)
    cdef size_t strip_c_str(const char** s_ptr, size_t l)

    cdef string lstrip_str(string s)
    cdef string rstrip_str(string s)
    cdef string strip_str(string s)


cdef inline string_view strip_sv(string_view s) noexcept nogil:
    """Strip leading and trailing white space from a C++ string_view."""
    cdef const char* start = s.data()
    cdef size_t l = strip_c_str(&start, s.size())
    if start != s.data():
        s.remove_prefix(start - s.data())
    if l != s.size():
        s.remove_suffix(s.size() - l)
    return s


cdef inline string str_to_lower(string s) noexcept nogil:
    """Convert a C++ string to lower-case characters."""
    cdef size_t i
    for i in range(s.size()):
        s[i] = tolower(s[i])
    return s
