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

import io

from libc.stdint cimport uint16_t
from libcpp.string cimport string
from libcpp.utility cimport pair
from libcpp.vector cimport vector

from stream_io cimport IOStream, BufferedReader


cdef extern from "<cctype>" namespace "std" nogil:
    int isspace(int c)
    int tolower(int c)

cdef extern from "<string>" namespace "std" nogil:
    int stoi(const string& s)

cdef size_t strnpos = -1

cdef string strip_str(const string& s) nogil:
    cdef size_t start = 0
    cdef size_t end = s.size()

    for start in range(0, s.size()):
        if not isspace(s[start]):
            break

    for end in reversed(range(s.size())):
        if not isspace(s[end]):
            break

    return s.substr(start, end - start + 1)


cdef string str_to_lower(string s) nogil:
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


# noinspection PyAttributeOutsideInit
cdef class WarcHeaderMap:
    cdef string _status_line
    cdef vector[pair[string, string]] _headers
    cdef str _encoding

    def __init__(self, encoding='utf-8'):
        self._encoding = encoding

    def __getitem__(self, header_key):
        cdef string header_key_lower = header_key.lower().encode(self._encoding, errors='ignore')
        for h in self._headers:
            if str_to_lower(h.first) == header_key_lower:
                return h.second.decode(self._encoding, errors='ignore')

    def __setitem__(self, header_key, header_value):
        cdef string header_key_lower = header_key.lower().encode(self._encoding, errors='ignore')
        for h in self._headers:
            if str_to_lower(h.first) == header_key_lower:
                h.second = header_value.encode(self._encoding, errors='ignore')
                return
        self._headers.push_back(pair[string, string](
            <string>header_key.encode(self._encoding, errors='ignore'),
            <string>header_value.encode(self._encoding, errors='ignore')))

    def __iter__(self):
        return self.items()

    def __repr__(self):
        return str(self.asdict())

    @property
    def status_line(self):
        return self._status_line.decode(self._status_code, errors='ignore')

    @status_line.setter
    def status_line(self, status_line):
        self._status_line = status_line.encode(self._encoding, errors='ignore')

    @property
    def status_code(self):
        if self._status_line.find(b'HTTP/') != 0:
            return None
        s = self._status_line.split(b' ')
        if len(s) != 3 or not s[1].isdigit():
            return None
        return int(s)

    def items(self):
        for h in self._headers:
            yield h.first.decode(self._encoding, errors='ignore'), h.second.decode(self._encoding, errors='ignore')

    def keys(self):
        for h in self._headers:
            yield h.first.decode(self._encoding, errors='ignore')

    def values(self):
        for h in self._headers:
            yield h.second.decode(self._encoding, errors='ignore')

    def asdict(self):
        return {h.first.decode(self._encoding, errors='ignore'): h.second.decode(self._encoding, errors='ignore')
                for h in self._headers}

    cdef void write(self, IOStream stream):
        if not self._status_line.empty():
            stream.write(self._status_line.data(), self._status_line.size())
            stream.write(b'\r\n', 2)

        for h in self._headers:
            if not h.first.empty():
                stream.write(h.first.data(), h.first.size())
                stream.write(b': ', 2)
            stream.write(h.second.data(), h.second.size())
            stream.write(b'\r\n', 2)

    cdef void set_status_line(self, const string& status_line):
        self._status_line = status_line

    cdef void append_header(self, const string header_key, const string& header_value):
        self._headers.push_back(pair[string, string](header_key, header_value))

    cdef void add_continuation(self, const string& header_continuation_value):
        if not self._headers.empty():
            self._headers.back().second.append(b'\n')
            self._headers.back().second.append(header_continuation_value)
        else:
            # This should no happen, but what can we do?!
            self.append_header(b'', header_continuation_value)


# noinspection PyAttributeOutsideInit,PyProtectedMember
cdef class WarcRecord:
    cdef WarcRecordType _record_type
    cdef WarcHeaderMap _headers
    cdef bint _is_http
    cdef WarcHeaderMap _http_headers
    cdef size_t _content_length
    cdef BufferedReader _reader

    def __init__(self):
        self._record_type = unknown
        self._is_http = False
        self._content_length = 0
        self._headers = WarcHeaderMap('utf-8')
        self._http_headers = WarcHeaderMap('iso-8859-15')

    @property
    def record_id(self):
        return self._headers['WARC-Record-ID']

    @property
    def record_type(self):
        return self._record_type

    @property
    def headers(self):
        return self._headers

    @property
    def is_http(self):
        return self._is_http

    @property
    def http_headers(self):
        return self._http_headers

    @property
    def content_length(self):
        return self._content_length

    @property
    def reader(self):
        return self._reader

    cpdef void set_bytes_content(self, bytes b):
        self._reader = BufferedReader(io.BytesIO(b))
        self._content_length = len(b)

    cpdef parse_http(self):
        cdef size_t num_bytes = parse_header_block(self.reader, self._http_headers, True)
        self._content_length = self._content_length - num_bytes

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
        else:
            self._record_type = unknown


# noinspection PyProtectedMember
cdef size_t parse_header_block(BufferedReader reader, WarcHeaderMap target, bint has_status_line=False):
    cdef string line
    cdef string header_key, header_value
    cdef size_t delim_pos = 0
    cdef size_t bytes_consumed = 0

    while True:
        line = reader.readline()
        bytes_consumed += line.size()
        if line == b'\r\n' or line == b'':
            break

        if isspace(line[0]):
            # Continuation line
            target.add_continuation(strip_str(line))
            continue

        if has_status_line:
            target.set_status_line(strip_str(line))
            has_status_line = False
            continue

        delim_pos = line.find(b':')
        if delim_pos == strnpos:
            # Invalid header, try to preserve it as best we can
            header_key = string()
            header_value = strip_str(line)
        else:
            header_key = strip_str(line.substr(0, delim_pos))
            header_value = strip_str(line.substr(delim_pos + 1))
        target.append_header(header_key, header_value)

    return bytes_consumed


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
                self.reader.close()
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

        self.record = WarcRecord()
        parse_header_block(self.reader, self.record._headers)

        cdef string hkey
        cdef size_t parse_count = 0
        for h in self.record._headers._headers:
            hkey = str_to_lower(h.first)
            if hkey == b'content-length':
                self.record._content_length = stoi(h.second)
                parse_count += 1
            elif hkey == b'warc-type':
                self.record._set_record_type(h.second)
                parse_count += 1
            elif hkey == b'content-type' and h.second.find(b'application/http') == 0:
                self.record._is_http = True
                parse_count += 1

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
