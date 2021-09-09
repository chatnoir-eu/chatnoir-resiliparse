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

from libc.stdint cimport uint16_t
from libcpp.vector cimport vector

from resiliparse_inc.string cimport string
from fastwarc.stream_io cimport IOStream, BufferedReader


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


ctypedef (string, string) str_pair


cdef class WarcHeaderMap:
    cdef string _status_line
    cdef vector[str_pair] _headers
    cdef str _enc
    cdef dict _dict_cache
    cdef bint _dict_cache_stale

    cdef size_t write(self, IOStream stream)
    cdef inline void clear(self)
    cdef inline void set_status_line(self, const string& status_line)
    cdef string find_header(self, const string& header_key, const string& default)
    cdef void set_header(self, const string& header_key, const string& header_value)
    cdef inline void append_header(self, const string& header_key, const string& header_value)
    cdef void add_continuation(self, const string& header_continuation_value)


cdef enum _NextRecStatus:
    has_next,
    skip_next,
    eof,
    error


cdef class WarcRecord:
    cdef WarcRecordType _record_type
    cdef WarcHeaderMap _headers
    cdef bint _is_http
    cdef bint _http_parsed
    cdef string _http_charset
    cdef WarcHeaderMap _http_headers
    cdef size_t _content_length
    cdef BufferedReader _reader
    cdef size_t _stream_pos

    cpdef void init_headers(self, size_t content_length, WarcRecordType record_type=*, bytes record_urn=*)
    cpdef void set_bytes_content(self, bytes b)
    cpdef void parse_http(self)
    cpdef size_t write(self, stream, bint checksum_data=*, size_t chunk_size=*)
    cpdef bint verify_block_digest(self, bint consume=*)
    cpdef bint verify_payload_digest(self, bint consume=*)

    cdef bint _verify_digest(self, const string& base32_digest, bint consume)
    cdef size_t _write_impl(self, in_reader, out_stream, bint write_payload_headers, size_t chunk_size)


cdef class ArchiveIterator:
    cdef BufferedReader reader
    cdef WarcRecord record
    cdef bint parse_http
    cdef bint verify_digests
    cdef size_t min_content_length
    cdef size_t max_content_length
    cdef object func_filter
    cdef uint16_t record_type_filter

    cdef _NextRecStatus _read_next_record(self) except _NextRecStatus.error
    cpdef bint _set_stream(self, stream) except 0


cpdef bint is_warc_10(WarcRecord record)
cpdef bint is_warc_11(WarcRecord record)
cpdef bint has_block_digest(WarcRecord record)
cpdef bint has_payload_digest(WarcRecord record)
cpdef bint is_http(WarcRecord record)
cpdef bint is_concurrent(WarcRecord record)
