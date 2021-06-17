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

import base64
from datetime import datetime
import hashlib
import io
import uuid
import warnings

from libc.stdint cimport uint16_t
from libcpp.string cimport string
from libcpp.vector cimport vector

from stream_io cimport IOStream, BufferedReader, PythonIOStreamAdapter


cdef extern from "<cctype>" namespace "std" nogil:
    int isspace(int c)
    int tolower(int c)

cdef extern from "<string>" namespace "std" nogil:
    int stoi(const string& s)
    string to_string(int i)

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


cdef const char* _enum_record_type_to_str(WarcRecordType record_type):
    if record_type == warcinfo:
        return b'warcinfo'
    elif record_type == response:
        return b'response'
    elif record_type == resource:
        return b'resource'
    elif record_type == request:
        return b'request'
    elif record_type == metadata:
        return b'metadata'
    elif record_type == revisit:
        return b'revisit'
    elif record_type == conversion:
        return b'conversion'
    elif record_type == continuation:
        return b'continuation'
    else:
        return b'unknown'


cdef WarcRecordType _str_record_type_to_enum(string record_type):
    record_type = str_to_lower(record_type)
    if record_type == b'warcinfo':
        return warcinfo
    elif record_type == b'response':
        return response
    elif record_type == b'resource':
        return resource
    elif record_type == b'request':
        return request
    elif record_type == b'metadata':
        return metadata
    elif record_type == b'revisit':
        return revisit
    elif record_type == b'conversion':
        return conversion
    elif record_type == b'continuation':
        return continuation
    else:
        return unknown

ctypedef (string, string) str_pair

# noinspection PyAttributeOutsideInit
cdef class WarcHeaderMap:
    cdef string _status_line
    cdef vector[str_pair] _headers
    cdef str _enc
    cdef dict _dict_cache

    def __init__(self, encoding='utf-8'):
        self._enc = encoding
        self._dict_cache = None

    def __getitem__(self, header_key):
        return self.asdict()[header_key]

    def __setitem__(self, header_key, header_value):
        self.set_header(header_key.encode(self._enc, errors='ignore'),
                        header_value.encode(self._enc, errors='ignore'))
        self.invalidate_dict_cache()

    def __iter__(self):
        yield from self.items()

    def __repr__(self):
        return str(self.asdict())

    def __contains__(self, item):
        return item in self.asdict()

    @property
    def status_line(self):
        return self._status_line.decode(self._status_code, errors='ignore')

    @status_line.setter
    def status_line(self, status_line):
        self._status_line = status_line.encode(self._enc, errors='ignore')

    @property
    def status_code(self):
        if self._status_line.find(b'HTTP/') != 0:
            return None
        s = self._status_line.split(b' ')
        if len(s) != 3 or not s[1].isdigit():
            return None
        return int(s)

    def get(self, item, default=None):
        return self.asdict().get(item, default)

    def items(self):
        return self.asdict().items()

    def keys(self):
        return self.asdict().keys()

    def values(self):
        return self.asdict().values()

    def asdict(self):
        cdef str_pair h
        if self._dict_cache is None:
            self._dict_cache = {
                h[0].decode(self._enc, errors='ignore'): h[1].decode(self._enc, errors='ignore')
                for h in self._headers}
        return self._dict_cache

    cdef void write(self, IOStream stream):
        if not self._status_line.empty():
            stream.write(self._status_line.data(), self._status_line.size())
            stream.write(b'\r\n', 2)

        cdef str_pair h
        for h in self._headers:
            if not h[0].empty():
                stream.write(h[0].data(), h[0].size())
                stream.write(b': ', 2)
            stream.write(h[1].data(), h[1].size())
            stream.write(b'\r\n', 2)

    cdef inline void clear(self):
        self._headers.clear()

    cdef inline void set_status_line(self, const string& status_line):
        self._status_line = status_line

    cdef string get_header(self, string header_key):
        header_key = str_to_lower(header_key)
        cdef str_pair h
        for h in self._headers:
            if str_to_lower(h[0]) == header_key:
                return h[1]
        return string()

    cdef void set_header(self, const string& header_key, const string& header_value):
        cdef string header_key_lower = str_to_lower(header_key)
        cdef str_pair h
        for h in self._headers:
            if str_to_lower(h[0]) == header_key_lower:
                h[1] = header_value
                return

        self._headers.push_back((header_key, header_value))

    cdef inline void append_header(self, const string& header_key, const string& header_value):
        self._headers.push_back((header_key, header_value))

    cdef void add_continuation(self, const string& header_continuation_value):
        if not self._headers.empty():
            self._headers.back()[1].append(b'\n')
            self._headers.back()[1].append(header_continuation_value)
        else:
            # This should no happen, but what can we do?!
            self._headers.push_back((b'', header_continuation_value))

    cdef inline void invalidate_dict_cache(self):
        self._dict_cache = None


# noinspection PyAttributeOutsideInit,PyProtectedMember
cdef class WarcRecord:
    cdef WarcRecordType _record_type
    cdef WarcHeaderMap _headers
    cdef bint _is_http
    cdef bint _http_parsed
    cdef WarcHeaderMap _http_headers
    cdef size_t _content_length
    cdef BufferedReader _reader

    def __init__(self):
        self._record_type = unknown
        self._is_http = False
        self._http_parsed = False
        self._content_length = 0
        self._headers = WarcHeaderMap('utf-8')
        self._http_headers = None

    @property
    def record_id(self):
        return self._headers['WARC-Record-ID']

    @property
    def record_type(self):
        return self._record_type

    @record_type.setter
    def record_type(self, WarcRecordType record_type):
        self._record_type = record_type
        self._headers['WARC-Type'] = _enum_record_type_to_str(record_type)

    @property
    def headers(self):
        return self._headers

    @property
    def is_http(self):
        return self._is_http

    @property
    def http_parsed(self):
        return self._http_parsed

    @property
    def http_headers(self):
        return self._http_headers

    @property
    def content_length(self):
        return self._content_length

    @property
    def reader(self):
        return self._reader

    cpdef void init_headers(self, size_t content_length):
        self._headers.clear()
        self._headers.set_status_line(b'WARC/1.1')
        self._headers.append_header(b'WARC-Type', _enum_record_type_to_str(self.record_type))
        self._headers.append_header(b'WARC-Date', datetime.utcnow().strftime('%Y-%m-%dT%H:%M:%SZ').encode())
        self._headers.append_header(b'WARC-Record-ID', b''.join((b'<', uuid.uuid4().urn.encode(), b'>')))
        self._headers.append_header(b'Content-Length', to_string(content_length))
        self._headers.invalidate_dict_cache()

    cpdef void set_bytes_content(self, bytes b):
        self._reader = BufferedReader(io.BytesIO(b))
        self._content_length = len(b)

    cpdef void parse_http(self):
        if self._http_parsed:
            return
        self._http_headers = WarcHeaderMap('iso-8859-15')
        cdef size_t num_bytes = parse_header_block(self.reader, self._http_headers, True)
        self._content_length = self._content_length - num_bytes
        self._http_parsed = True

    # noinspection PyTypeChecker
    cpdef void write(self, stream, bint checksum_data=False, size_t chunk_size=16384):
        if not checksum_data:
            self._write_impl(self.reader, stream, True, chunk_size)
            return

        # Otherwise read everything into memory for checksumming and content-length correction
        block_digest = hashlib.sha1()
        payload_digest = None
        block_buf = io.BytesIO()

        if self._http_parsed:
            payload_digest = hashlib.sha1()
            self._http_headers.write(PythonIOStreamAdapter(block_buf))
            block_buf.write(b'\r\n')
            block_digest.update(block_buf.getvalue())

        cdef string payload_data
        while True:
            payload_data = self.reader.read(chunk_size)
            if payload_data.empty():
                break

            block_digest.update(payload_data.data()[:payload_data.size()])
            if payload_digest is not None:
                payload_digest.update(payload_data.data()[:payload_data.size()])
            block_buf.write(payload_data.data()[:payload_data.size()])

        self._headers.set_header(b'Content-Length', to_string(block_buf.tell()))

        cdef char* prefix = b'sha1:'
        if payload_digest is not None:
            self._headers.set_header(b'WARC-Payload-Digest', prefix + base64.b32encode(payload_digest.digest()))
        self._headers.set_header(b'WARC-Block-Digest', prefix + base64.b32encode(block_digest.digest()))

        block_buf.seek(0)
        self._write_impl(block_buf, stream, False, chunk_size)

    cdef void _write_impl(self, in_reader, out_stream, bint write_payload_headers, size_t chunk_size):
        cdef IOStream out_stream_wrapped

        if isinstance(out_stream, IOStream):
            out_stream_wrapped = <IOStream>out_stream
        elif isinstance(out_stream, object) and hasattr(out_stream, 'write'):
            out_stream_wrapped = PythonIOStreamAdapter(out_stream)
        else:
            warnings.warn(f'Object of type "{type(out_stream).__name__}" is not a valid stream.', RuntimeWarning)
            return

        self._headers.write(out_stream_wrapped)
        out_stream_wrapped.write(b'\r\n', 2)

        if write_payload_headers and self._http_parsed:
            self._http_headers.write(out_stream_wrapped)
            out_stream_wrapped.write(b'\r\n', 2)

        cdef string data
        while True:
            data = in_reader.read(chunk_size)
            if data.empty():
                break
            out_stream_wrapped.write(data.data(), data.size())
        out_stream_wrapped.write(b'\r\n', 2)


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

        self.record = WarcRecord()

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
                self.record._headers.set_status_line(version_line)
                break
            else:
                # Not a WARC file or unsupported version
                return eof

        parse_header_block(self.reader, self.record._headers)

        cdef string hkey
        cdef size_t parse_count = 0
        cdef str_pair h
        for h in self.record._headers._headers:
            hkey = str_to_lower(h[0])
            if hkey == b'content-length':
                self.record._content_length = stoi(h[1])
                parse_count += 1
            elif hkey == b'warc-type':
                self.record._record_type = _str_record_type_to_enum(h[1])
                parse_count += 1
            elif hkey == b'content-type' and h[1].find(b'application/http') == 0:
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
