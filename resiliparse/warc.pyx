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

from stream_io cimport IOStream, BufferedLineReader

from libcpp.string cimport string
from libcpp.unordered_map cimport unordered_map

cdef extern from "<cctype>" namespace "std" nogil:
    int isspace(int c)
    int tolower(int c)

cdef extern from "<string>" namespace "std" nogil:
    int stoi(const string& s)

cdef size_t strnpos = -1

cdef string strip_str(const string& s):
    cdef size_t start = 0
    cdef size_t end = s.size()

    for start in range(0, s.size()):
        if not isspace(s[start]):
            break

    for end in reversed(range(s.size())):
        if not isspace(s[end]):
            break

    return s.substr(start, end - start + 1)


cdef string str_to_lower(string s):
    for i in range(s.size()):
        s[i] = tolower(s[i])
    return s


cdef bint str_equal_ci(const string& a, const string& b):
    return str_to_lower(a) == str_to_lower(b)


cdef class ArchiveIterator:
    cdef IOStream stream
    cdef BufferedLineReader line_reader
    cdef string unused_data

    def __init__(self, IOStream stream):
        self.stream = stream
        self.line_reader = BufferedLineReader(self.stream)

    def __iter__(self):
        return self

    def __next__(self):
        if not self.read_next_record():
            raise StopIteration

        return 0

    cdef bint read_next_record(self):
        cdef string version_line

        while True:
            version_line = self.line_reader.readline()
            if version_line.empty():
                # EOF
                return False

            version_line = strip_str(version_line)
            if version_line.empty():
                # Consume empty lines
                pass
            elif version_line == b'WARC/1.0' or version_line == b'WARC/1.1':
                # OK, continue with parsing headers
                break
            else:
                # Not a WARC file or unsupported version
                return False

        cdef string line
        cdef unordered_map[string, string] headers

        cdef string header_key, header_value

        cdef size_t delim_pos = 0
        cdef size_t content_length = 0

        while True:
            line = self.line_reader.readline()
            if line == b'\r\n':
                break

            delim_pos = line.find(b':')
            if delim_pos == strnpos:
                delim_pos = line.size() - 1

            header_key = strip_str(line.substr(0, delim_pos))
            header_value = strip_str(line.substr(delim_pos + 1))
            headers[header_key] = header_value

            if str_equal_ci(header_key, b'Content-Length'):
                content_length = stoi(header_value)

        cdef string content = self.line_reader.read_block(content_length + 2)
        return True
