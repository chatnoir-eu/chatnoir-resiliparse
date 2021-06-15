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

from stream_io cimport IOStream, BufferedReader

from libc.stdint cimport uint16_t
from libcpp.string cimport string
from libcpp.utility cimport pair
from libcpp.vector cimport vector

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


cpdef enum WarcRecordType:
    warcinfo = 2,
    response = 4,
    resource = 8,
    request = 16,
    metadata = 32,
    revisit = 64,
    conversion = 128,
    continuation = 256,
    unknown = 512,
    any_type = 65535,
    no_type = 0

cdef class WarcRecord:
    cdef WarcRecordType _record_type
    cdef vector[pair[string, string]] _headers
    cdef bint _is_http
    cdef string _http_status_line
    cdef vector[pair[string, string]] _http_headers
    cdef size_t _content_length
    cdef size_t _http_content_length
    cdef BufferedReader _reader

    def __init__(self):
        self._record_type = unknown
        self._is_http = False
        self._content_length = 0
        self._http_content_length = 0

    @property
    def record_type(self):
        return self._record_type

    @property
    def headers(self):
        return self._decode_header_map(self._headers, 'utf-8')

    @property
    def is_http(self):
        return self._is_http

    @property
    def http_status_line(self):
        return self._http_status_line.decode('iso-8859-1', errors='ignore')

    @property
    def http_headers(self):
        return self._decode_header_map(self._http_headers, 'iso-8859-1')

    @property
    def content_length(self):
        return self._content_length

    @property
    def http_content_length(self):
        return self._http_content_length

    @property
    def reader(self):
        return self._reader

    cpdef parse_http(self):
        cdef size_t http_header_bytes = 0
        self._http_headers = parse_header_block(self.reader, True, &http_header_bytes)
        self._http_status_line = self._http_headers[0].second
        self._http_headers.erase(self._http_headers.begin())
        self._http_content_length = http_header_bytes
        self._content_length = self._content_length - http_header_bytes

    cdef _decode_header_map(self, vector[pair[string, string]]& header_map, str encoding):
        return {h.first.decode(encoding, errors='ignore'): h.second.decode(encoding, errors='ignore')
                for h in header_map}

    cdef void _set_record_type(self, string record_type):
        record_type = str_to_lower(record_type)
        if record_type == b'warcinfo':
            self._record_type = warcinfo
        elif record_type == b'response':
            self._record_type = response
        elif record_type == b'resource':
            self._record_type = resource
        elif record_type == b'request':
            self._record_type = request
        elif record_type == b'metadata':
            self._record_type = metadata
        elif record_type == b'revisit':
            self._record_type = revisit
        elif record_type == b'conversion':
            self._record_type = conversion
        elif record_type == b'continuation':
            self._record_type = continuation


cdef vector[pair[string, string]] parse_header_block(BufferedReader reader, bint has_status_line=False,
                                                     size_t* bytes_consumed=NULL):
    cdef vector[pair[string, string]] headers
    cdef string line
    cdef string header_key, header_value
    cdef size_t delim_pos = 0
    cdef size_t byte_counter = 0

    while True:
        line = reader.readline()
        byte_counter += line.size()
        if line == b'\r\n' or line == b'':
            break

        if isspace(line[0]) and not headers.empty():
            # Continuation line
            headers.back().second.append(b'\n')
            headers.back().second.append(strip_str(line))
            continue

        if has_status_line:
            header_key.clear()
            header_value = strip_str(line)
            has_status_line = False
        else:
            delim_pos = line.find(b':')
            if delim_pos == strnpos:
                delim_pos = line.size() - 1
            header_key = strip_str(line.substr(0, delim_pos))
            header_value = strip_str(line.substr(delim_pos + 1))

        headers.push_back(pair[string, string](header_key, header_value))

    if bytes_consumed != NULL:
        bytes_consumed[0] = byte_counter

    return headers


cdef enum _NextRecStatus:
    has_next,
    skip_next,
    eof

# noinspection PyProtectedMember
cdef class ArchiveIterator:
    cdef IOStream stream
    cdef BufferedReader reader
    cdef WarcRecord record
    cdef bint parse_http
    cdef uint16_t record_type_filter

    def __init__(self, IOStream stream, bint parse_http=True, uint16_t record_types=any_type):
        self.stream = stream
        self.reader = BufferedReader(self.stream)
        self.record = None
        self.parse_http = parse_http
        self.record_type_filter = record_types

    def __iter__(self):
        cdef _NextRecStatus status
        while True:
            status = self._read_next_record()
            if status == has_next:
                yield self.record
            elif status == eof:
                return

    cdef _NextRecStatus _read_next_record(self):
        if self.record is not None:
            self.reader.consume()
            self.reader.reset_limit()

        cdef string version_line
        while True:
            version_line = self.reader.readline()
            if version_line.empty():
                # EOF
                return eof

            version_line = strip_str(version_line)
            if version_line.empty():
                # Consume empty lines
                pass
            elif version_line == b'WARC/1.0' or version_line == b'WARC/1.1':
                # OK, continue with parsing headers
                break
            else:
                # Not a WARC file or unsupported version
                return eof

        cdef string hkey
        cdef size_t parse_count = 0

        self.record = WarcRecord()
        self.record._headers = parse_header_block(self.reader)
        for h in self.record._headers:
            hkey = str_to_lower(h.first)
            if hkey == b'content-length':
                self.record._content_length = stoi(h.second)
                parse_count += 1
            elif hkey == b'warc-type':
                self.record._set_record_type(h.second)
                parse_count += 1
            elif hkey == b'content-type' and h.second.find(b'application/http') == 0:
                self.record._is_http = True

            if parse_count >= 3:
                break

        if self.record._record_type & self.record_type_filter == 0:
            self.reader.reset_limit()
            self.reader.consume(self.record._content_length)
            self.record = None
            return skip_next

        self.reader.set_limit(self.record._content_length)
        self.record._reader = self.reader

        if self.parse_http and self.record._is_http:
            self.record.parse_http()

        return has_next
