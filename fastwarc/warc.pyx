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

cimport cython
from cython.operator cimport dereference as deref, preincrement as inc
from libc.stdint cimport uint16_t
from libcpp.vector cimport vector

import base64
import datetime
import hashlib
import typing as t
import uuid
import warnings

from resiliparse_inc.cctype cimport isspace, tolower
from resiliparse_inc.cstdlib cimport strtol
from resiliparse_inc.string cimport npos as strnpos, string, to_string

from fastwarc.stream_io cimport BufferedReader, BytesIOStream, CompressingStream, IOStream, PythonIOStreamAdapter
from fastwarc.stream_io import StreamError


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


cdef const char* _enum_record_type_to_str(WarcRecordType record_type) nogil:
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


cdef WarcRecordType _str_record_type_to_enum(string record_type) nogil:
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


class CaseInsensitiveStr(str):
    """Case insensitive str implementation for use as dict key."""

    def __hash__(self):
        return hash(self.casefold())

    def __eq__(self, other):
        return isinstance(other, str) and self.casefold() == other.casefold()


class CaseInsensitiveStrDict(dict):
    """Case-insensitive str dict."""

    def __getitem__(self, str key not None):
        return super().__getitem__(CaseInsensitiveStr(key))

    def __setitem__(self, str key not None, str value not None):
        super().__setitem__(CaseInsensitiveStr(key), value)

    def __contains__(self, str key not None):
        return super().__contains__(CaseInsensitiveStr(key))

    def get(self, str key not None, str value=None):
        return super().get(CaseInsensitiveStr(key), value)

    def setdefault(self, str key not None, str value=None):
        return super().setdefault(CaseInsensitiveStr(key), value)

    def pop(self, str key not None):
        return super().pop(CaseInsensitiveStr(key))

    def update(self, it=None, **kwargs):
        if hasattr(it, 'items'):
            it = it.items()
        if it is not None:
            for k, v in it:
                super().__setitem__(CaseInsensitiveStr(k), v)
        for k, v in kwargs.items():
            super().__setitem__(CaseInsensitiveStr(k), v)


# noinspection PyAttributeOutsideInit
cdef class WarcHeaderMap:
    """
    __init__(self, encoding='utf-8')

    Dict-like type representing a WARC or HTTP header block.

    :param encoding: header source encoding
    :type encoding: str
    """

    def __init__(self, *args, **kwargs):
        pass

    def __cinit__(self, str encoding='utf-8'):
        self._enc = encoding
        self._dict_cache = None
        self._dict_cache_stale = True

    def __getitem__(self, header_key):
        return self.asdict()[header_key]

    def __setitem__(self, header_key, header_value):
        self.set_header(header_key.encode(self._enc, errors='ignore'),
                        header_value.encode(self._enc, errors='ignore'))

    def __iter__(self):
        yield from self.items()

    def __repr__(self):
        return str(self.asdict())

    def __contains__(self, item):
        return item in self.asdict()

    @property
    def status_line(self) -> str:
        """
        Header status line.

        :rtype: str
        """
        return self._status_line.decode(self._enc, errors='ignore')

    @status_line.setter
    def status_line(self, str status_line not None):
        """
        Set status line contents.

        :param status_line: new status line
        :type status_line: str
        """
        self._status_line = status_line.encode(self._enc, errors='ignore')

    @property
    def status_code(self) -> t.Optional[int]:
        """
        HTTP status code (unset if header block is not an HTTP header block).

        :rtype: int | None
        """
        if self._status_line.find(<char*>b'HTTP/') != 0:
            return None
        s = self._status_line.split(b' ', 2)
        if len(s) != 3 or not s[1].isdigit():
            return None
        return int(s)

    def append(self, str key not None, str value not None):
        """
        append(self, key, value)

        Append header (use if header name is not unique).

        :param key: header key
        :type key: str
        :param value: header value
        :type value: str
        """
        self.append_header(key.encode(self._enc), value.encode(self._enc))

    def get(self, str key not None, str default=None) -> str:
        """
        get(self, key, default=None)

        Get header value or ``default``.

        If multiple headers have the same key, only the last occurrence will be returned.

        :param key: header key
        :type key: str
        :param default: default value if ``key`` not found
        :type default: str
        :rtype: str
        """
        return self.asdict().get(key, default)

    def items(self):
        """
        items(self)

        Item view of keys and values.

        If multiple headers have the same key, only the last occurrence will be returned.

        :rtype: t.Iterable[(str, str)]
        """
        return self.asdict().items()

    def keys(self):
        """
        keys(self)

        Iterable of header keys.

        If multiple headers have the same key, only the last occurrence will be returned.

        :rtype: t.Iterable[str]
        """
        return self.asdict().keys()

    def values(self):
        """
        values(self)

        Iterable of header values.

        If multiple headers have the same key, only the last occurrence will be returned.

        :rtype: t.Iterable[str]
        """
        return self.asdict().values()

    def asdict(self) -> t.Dict[str, str]:
        """
        asdict(self)

        Headers as Python dict.

        If multiple headers have the same key, only the last occurrence will be returned.

        :rtype: t.Dict[str, str]
        """
        cdef str_pair h
        if self._dict_cache_stale:
            self._dict_cache = <dict>CaseInsensitiveStrDict({
                CaseInsensitiveStr(h[0].decode(self._enc, errors='ignore')): h[1].decode(self._enc, errors='ignore')
                for h in self._headers})
            self._dict_cache_stale = False
        return self._dict_cache

    def astuples(self) -> t.List[t.Tuple[str, str]]:
        """
        astuples(self)

        Headers as list of tuples, including multiple headers with the same key.
        Use this over :meth:`items` if header keys are not necessarily unique.

        :rtype: list[(str, str)]
        """
        cdef str_pair h
        return [(h[0].decode(self._enc, errors='ignore'), h[1].decode(self._enc, errors='ignore'))
                for h in self._headers]


    cdef size_t write(self, IOStream stream):
        """Write header block into stream."""
        cdef size_t bytes_written = 0
        if not self._status_line.empty():
            bytes_written += stream.write(self._status_line.data(), self._status_line.size())
            bytes_written += stream.write(<char*>b'\r\n', 2)

        cdef vector[str_pair].iterator it = self._headers.begin()
        while it != self._headers.end():
            if not deref(it)[0].empty():
                bytes_written += stream.write(deref(it)[0].data(), deref(it)[0].size())
                bytes_written += stream.write(<char*>b': ', 2)
            bytes_written += stream.write(deref(it)[1].data(), deref(it)[1].size())
            bytes_written += stream.write(<char*>b'\r\n', 2)
            inc(it)
        return bytes_written

    cdef inline void clear(self):
        """Clear headers."""
        self._headers.clear()
        self._dict_cache_stale = True

    cdef inline void set_status_line(self, const string& status_line):
        """Set status line string."""
        self._status_line = status_line

    cdef string find_header(self, const string& header_key, const string& default):
        """Return value of first header occurrence or ``default`` (linear complexity)."""
        cdef string header_key_lower = str_to_lower(header_key)
        cdef vector[str_pair].iterator it = self._headers.begin()
        while it != self._headers.end():
            if str_to_lower(deref(it)[0]) == header_key_lower:
                return deref(it)[1]
            inc(it)

        return default

    cdef void set_header(self, const string& header_key, const string& header_value):
        """Set new header or overwrite existing header if it exists."""
        self._dict_cache_stale = True
        cdef string header_key_lower = str_to_lower(header_key)
        cdef vector[str_pair].iterator it = self._headers.begin()
        while it != self._headers.end():
            if str_to_lower(deref(it)[0]) == header_key_lower:
                deref(it)[1] = header_value
                return
            inc(it)

        self._headers.push_back((header_key, header_value))

    cdef inline void append_header(self, const string& header_key, const string& header_value):
        """Append header (use if header key is not unique)."""
        self._headers.push_back((header_key, header_value))
        self._dict_cache_stale = True

    cdef void add_continuation(self, const string& header_continuation_value):
        """Append value to previous header."""
        if not self._headers.empty():
            self._headers.back()[1].append(<char*>b' ')
            self._headers.back()[1].append(header_continuation_value)
        else:
            # This should no happen, but what can we do?!
            self._headers.push_back((<char*>b'', header_continuation_value))
        self._dict_cache_stale = True


# noinspection PyProtectedMember, PyAttributeOutsideInit
cdef class WarcRecord:
    """
    __init__(self)

    A WARC record.
    """

    def __cinit__(self):
        self._record_type = unknown
        self._is_http = False
        self._http_parsed = False
        self._content_length = 0
        self._headers = WarcHeaderMap.__new__(WarcHeaderMap, 'utf-8')
        self._http_headers = None
        self._stream_pos = 0

    @property
    def record_id(self) -> str:
        """
        Record ID (same as ``headers['WARC-Record'ID']``.

        :rtype: str
        """
        return self._headers['WARC-Record-ID']

    @property
    def record_type(self) -> WarcRecordType:
        """
        Record type (same as ``headers['WARC-Type']``.

        :rtype: WarcRecordType
        """
        return self._record_type

    @record_type.setter
    def record_type(self, WarcRecordType record_type):
        """
        record_type(self, record_type)

        Set record type.

        :param record_type: record type
        :type record_type: WarcRecordType
        """
        self._record_type = record_type
        self._headers['WARC-Type'] = _enum_record_type_to_str(record_type)

    @property
    def headers(self) -> WarcHeaderMap:
        """
        WARC record headers.

        :rtype: WarcHeaderMap
        """
        return self._headers

    @property
    def is_http(self) -> bool:
        """
        Whether record is an HTTP record.

        :rtype: bool
        """
        return self._is_http

    @property
    def is_http_parsed(self) -> bool:
        """
        Whether HTTP headers have been parsed.

        :rtype: bool
        """
        return self._http_parsed

    @property
    def http_headers(self) -> t.Optional[WarcHeaderMap]:
        """
        HTTP headers if record is an HTTP record and HTTP headers have been parsed yet.

        :rtype: WarcHeaderMap | None
        """
        return self._http_headers

    @property
    def http_content_type(self) -> t.Optional[str]:
        """
        Plain HTTP Content-Type without additional fields such as ``charset=``.

        :rtype: str | None
        """
        if not self._http_parsed:
            return None

        cdef string content_type = self._http_headers.find_header(<char*>b'content-type', <char*>b'')
        if content_type.empty():
            return None

        content_type = strip_str(content_type.substr(0, content_type.find(<char*>b';')))
        return content_type.decode(self._http_headers._enc, errors='ignore')

    @property
    def http_charset(self) -> t.Optional[str]:
        """
        HTTP charset/encoding as returned by the server or ``None`` if no valid charset is set.
        A returned string is guaranteed to be a valid Python encoding name.

        :rtype: str | None
        """
        if not self._http_parsed or self._http_charset == <char*>b'_':
            return None

        if not self._http_charset.empty():
            return self._http_charset.decode()

        cdef string content_type = self._http_headers.find_header(<char*>b'content-type', <char*>b'')
        cdef size_t pos = content_type.find(<char*>b'charset=')
        if pos == strnpos:
            return None

        pos += 8
        cdef size_t pos_end = content_type.find(<char*>b';', pos)
        if pos_end != strnpos:
            pos_end = pos_end - pos

        self._http_charset = str_to_lower(strip_str(content_type.substr(pos, pos_end)))
        try:
            ''.encode(encoding=self._http_charset.decode())
        except LookupError:
            self._http_charset = <char*>b'_'
            return None

        return self._http_charset.decode(self._http_headers._enc, errors='ignore')

    @property
    def content_length(self) -> int:
        """
        Remaining WARC length in bytes (not necessarily the same as the ``Content-Length`` header).

        :rtype: int
        """
        return self._content_length

    @property
    def reader(self) -> BufferedReader:
        """
        Reader for the remaining WARC record content.

        :rtype: BufferedReader
        """
        return self._reader

    @property
    def stream_pos(self) -> int:
        """
        WARC record start offset in the original (uncompressed) stream.

        :rtype: int
        """
        return self._stream_pos

    cpdef void init_headers(self, size_t content_length, WarcRecordType record_type=no_type, bytes record_urn=None):
        """
        init_headers(self, content_length, record_type=no_type, record_urn=None)
        
        Initialize mandatory headers in a fresh :class:`WarcRecord` instance.
        
        :param content_length: WARC record body length in bytes
        :type content_length: int
        :param record_type: WARC-Type
        :type record_type: WarcRecordType
        :param record_urn: WARC-Record-ID as URN without ``'<'``, ``'>'`` (if unset, a random URN will be generated)
        :type record_urn: bytes
        """
        if record_urn is None:
            record_urn = uuid.uuid4().urn.encode()

        if record_type == no_type:
            record_type = self.record_type
        if record_type == any_type or record_type == no_type:
            record_type = unknown
        self.record_type = record_type

        self._headers.clear()
        self._headers.set_status_line(<char*>b'WARC/1.1')
        self._headers.append_header(<char*>b'WARC-Type', _enum_record_type_to_str(record_type))
        self._headers.append_header(<char*>b'WARC-Date', datetime.datetime.utcnow().strftime('%Y-%m-%dT%H:%M:%SZ').encode())
        self._headers.append_header(<char*>b'WARC-Record-ID', b''.join((b'<', record_urn, b'>')))
        self._headers.append_header(<char*>b'Content-Length', to_string(<long int>content_length))

    cpdef void set_bytes_content(self, bytes b):
        """
        set_bytes_content(self, b)
        
        Set WARC body.
        
        :param b: body as bytes
        :type b: bytes
        """
        self._reader = BufferedReader.__new__(BufferedReader, BytesIOStream(b))
        self._content_length = len(b)

    cpdef void parse_http(self):
        """
        parse_http(self)
        
        Parse HTTP headers and advance content reader.
        
        It is safe to call this method multiple times, even if the record is not an HTTP record.
        """
        if self._http_parsed or not self._is_http:
            return
        self._http_headers = WarcHeaderMap.__new__(WarcHeaderMap, 'iso-8859-15')
        cdef size_t num_bytes = parse_header_block(self.reader, self._http_headers, True)
        self._content_length = self._content_length - num_bytes
        self._http_parsed = True

    # noinspection PyTypeChecker
    cpdef size_t write(self, stream, bint checksum_data=False, size_t chunk_size=16384):
        """
        write(self, stream, checksum_data=False, chunk_size=16384)
        
        Write WARC record onto a stream.
        
        :param stream: output stream
        :param checksum_data: add block and payload digest headers
        :type checksum_data: bool
        :param chunk_size: write block size
        :type chunk_size: int
        :return: number of bytes written
        :rtype: int
        """
        # If the raw byte content hasn't been parsed, we can simply pass it through
        if not checksum_data and not self._http_parsed:
            return self._write_impl(self.reader, stream, True, chunk_size)

        # Otherwise read everything into memory for content-length correction and checksumming
        cdef BytesIOStream block_buf = BytesIOStream()

        block_digest = hashlib.sha1() if checksum_data else None
        payload_digest = hashlib.sha1() if checksum_data and self._http_parsed else None

        if self._http_parsed:
            self._http_headers.write(block_buf)
            block_buf.write(<char*>b'\r\n', 2)

            if checksum_data:
                block_digest.update(block_buf.getvalue())

        cdef string payload_data
        while True:
            payload_data = self.reader.read(chunk_size)
            if payload_data.empty():
                break

            if checksum_data:
                block_digest.update(payload_data.data()[:payload_data.size()])
                if payload_digest is not None:
                    payload_digest.update(payload_data.data()[:payload_data.size()])
            block_buf.write(payload_data.data(), payload_data.size())

        self._headers.set_header(<char*>b'Content-Length', to_string(<long int>block_buf.tell()))
        if checksum_data:
            if payload_digest is not None:
                self._headers.set_header(<char*>b'WARC-Payload-Digest', b'sha1:' + base64.b32encode(payload_digest.digest()))
            self._headers.set_header(<char*>b'WARC-Block-Digest', b'sha1:' + base64.b32encode(block_digest.digest()))

        block_buf.seek(0)
        return self._write_impl(block_buf, stream, False, chunk_size)

    cdef size_t _write_impl(self, in_reader, out_stream, bint write_payload_headers, size_t chunk_size):
        cdef IOStream out_stream_wrapped
        cdef size_t bytes_written = 0
        cdef bint compress_member_started = False

        if isinstance(out_stream, IOStream):
            out_stream_wrapped = <IOStream>out_stream

            if isinstance(out_stream, CompressingStream):
                bytes_written = (<CompressingStream>out_stream_wrapped).begin_member()
                compress_member_started = True

        elif isinstance(out_stream, object) and hasattr(out_stream, 'write'):
            out_stream_wrapped = PythonIOStreamAdapter.__new__(PythonIOStreamAdapter, out_stream)
        else:
            warnings.warn(f"Object of type '{type(out_stream).__name__}' is not a valid stream.", RuntimeWarning)
            return 0

        bytes_written += self._headers.write(out_stream_wrapped)
        bytes_written += out_stream_wrapped.write(b'\r\n', 2)

        if write_payload_headers and self._http_parsed:
            bytes_written += self._http_headers.write(out_stream_wrapped)
            bytes_written += out_stream_wrapped.write(b'\r\n', 2)

        cdef string data
        while True:
            data = in_reader.read(chunk_size)
            if data.empty():
                break
            bytes_written += out_stream_wrapped.write(data.data(), data.size())
        bytes_written += out_stream_wrapped.write(b'\r\n', 2)

        if compress_member_started:
            bytes_written += (<CompressingStream> out_stream_wrapped).end_member()

        return bytes_written

    cdef bint _verify_digest(self, const string& base32_digest, bint consume):
        cdef size_t sep_pos = base32_digest.find(b':')
        if sep_pos == strnpos:
            return False

        cdef string alg = base32_digest.substr(0, sep_pos)
        cdef bytes digest = base64.b32decode(base32_digest.substr(sep_pos + 1))

        if alg == b'sha1':
            h = hashlib.sha1()
        elif alg == b'md5':
            h = hashlib.md5()
        elif alg == b'sha256':
            h = hashlib.sha256()
        else:
            warnings.warn(f'Unsupported hash algorithm "{alg.decode()}".')
            return False

        cdef string block
        cdef BytesIOStream tee_stream
        cdef bint consume_override = consume

        if isinstance(self._reader.stream, BytesIOStream):
            # Stream is already a BytesIOStream, so we don't need to create another copy
            consume_override = True
            tee_stream = <BytesIOStream>self._reader.stream
        elif not consume:
            tee_stream = BytesIOStream()

        while True:
            block = self._reader.read(1024)
            if block.empty():
                break
            h.update(block)
            if not consume_override:
                tee_stream.write(block.data(), block.size())

        if not consume:
            tee_stream.seek(0)
            self._reader = BufferedReader.__new__(BufferedReader, tee_stream)

        return h.digest() == digest

    cpdef bint verify_block_digest(self, bint consume=False):
        """
        verify_block_digest(self, consume=False)
        
        Verify whether record block digest is valid.
        
        :param consume: do not create an in-memory copy of the record stream
                        (will fully consume the rest of the record)
        :type consume: bool
        :return: ``True`` if digest exists and is valid
        :rtype: bool
        """
        return self._verify_digest(self._headers.find_header(<char*>b'WARC-Block-Digest', <char*>b''), consume)

    cpdef bint verify_payload_digest(self, bint consume=False):
        """
        verify_payload_digest(self, consume=False)
        
        Verify whether record payload digest is valid.
        
        :param consume: do not create an in-memory copy of the record stream
                        (will fully consume the rest of the record)
        :type consume: bool
        :return: ``True`` if record is HTTP record and digest exists and is valid
        :rtype: bool
        """
        if not self._http_parsed:
            return False
        return self._verify_digest(self._headers.find_header(<char*>b'WARC-Payload-Digest', <char*>b''), consume)


# noinspection PyProtectedMember
cdef size_t parse_header_block(BufferedReader reader, WarcHeaderMap target, bint has_status_line=False):
    """
    parse_header_block(reader, target, has_status_line=False)
    
    Helper function for parsing WARC or HTTP header blocks.
    
    :param reader: input reader
    :type reader: BufferedReader
    :param target: header map to fill
    :type reader: WarcHeaderMap
    :param has_status_line: whether first line is a status line or already a header
    :type has_status_line: bool
    :return: number of bytes read from `reader`
    :rtype: int
    """
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
            target.add_continuation(strip_str(line))
        else:
            header_key = strip_str(line.substr(0, delim_pos))
            if delim_pos >= line.size():
                header_value = string()
            else:
                header_value = strip_str(line.substr(delim_pos + 1))
            target.append_header(header_key, header_value)

    return bytes_consumed


# noinspection PyProtectedMember, PyAttributeOutsideInit
@cython.auto_pickle(False)
cdef class ArchiveIterator:
    """
    __init__(self, stream, record_types=any_type, parse_http=True, min_content_length=-1, max_content_length=-1, \
             func_filter=None, verify_digests=False)

    WARC record stream iterator.

    :param stream: input stream (preferably an :class:`~fastwarc.stream_io.IOStream`,
                   but any file-like Python object is fine)
    :param parse_http: whether to parse HTTP records automatically (disable for better performance if not needed)
    :type parse_http: bool
    :param record_types: bitmask of :class:`WarcRecordType` record types to return (others will be skipped)
    :type record_types: int
    :param min_content_length: skip records with Content-Length less than this
    :type min_content_length: int
    :param max_content_length: skip records with Content-Length large than this
    :type max_content_length: int
    :param func_filter: Python callable taking a :class:`WarcRecord` and returning a ``bool``
                        for further record filtering
    :type func_filter: Callable
    :param verify_digests: skip records which have no or an invalid block digest
    :type verify_digests: bool
    """

    def __init__(self, *args, **kwargs):
        pass

    def __cinit__(self, stream, uint16_t record_types=any_type, bint parse_http=True,
                  size_t min_content_length=strnpos, size_t max_content_length=strnpos,
                  func_filter=None, bint verify_digests=False):
        self._set_stream(stream)
        self.record = None
        self.parse_http = parse_http
        self.verify_digests = verify_digests
        self.min_content_length = min_content_length
        self.max_content_length = max_content_length
        self.func_filter = func_filter
        self.record_type_filter = record_types

    def __iter__(self) -> t.Iterable[WarcRecord]:
        """
        :rtype: t.Iterable[WarcRecord]
        """
        cdef _NextRecStatus status

        while True:
            status = self._read_next_record()
            if status == has_next:
                yield self.record
            elif status == eof:
                self.reader.close()
                return

    cdef _NextRecStatus _read_next_record(self) except _NextRecStatus.error:
        self.reader.detect_stream_type()

        if self.record is not None:
            self.reader.consume()
            self.reader.reset_limit()

        self.record = WarcRecord.__new__(WarcRecord)
        self.record._stream_pos = self.reader.tell()

        cdef string version_line
        while True:
            version_line = self.reader.readline()

            if version_line.empty():
                # EOF
                return eof

            if version_line == b'\r\n' or version_line == b'\n':
                # Consume empty lines
                if not self.reader.stream_is_compressed:
                    self.record._stream_pos += version_line.size()
                continue

            version_line = strip_str(version_line)
            if version_line == b'WARC/1.0' or version_line == b'WARC/1.1':
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
        cdef bint skip = False
        for h in self.record._headers._headers:
            hkey = str_to_lower(h[0])
            if hkey == b'warc-type':
                self.record._record_type = _str_record_type_to_enum(h[1])
                parse_count += 1
            elif hkey == b'content-length':
                self.record._content_length = strtol(h[1].c_str(), NULL, 10)
                parse_count += 1
            elif hkey == b'content-type' and h[1].find(b'application/http') == 0:
                self.record._is_http = True
                parse_count += 1

            if parse_count >= 3:
                break

        # Check if record is to be skipped
        skip |= (self.record._record_type & self.record_type_filter) == 0
        skip |= self.max_content_length != strnpos and self.record._content_length > self.max_content_length
        skip |= self.min_content_length != strnpos and self.record._content_length < self.max_content_length
        if not skip and self.func_filter is not None:
            # Execute expensive filters last and only if skip is not already true
            skip |= not self.func_filter(self.record)

        if skip:
            self.reader.consume(self.record._content_length)
            self.record = None
            return skip_next

        self.reader.set_limit(self.record._content_length)
        self.record._reader = self.reader

        if self.verify_digests and not self.record.verify_block_digest(False):
            self.reader.reset_limit()
            self.reader.consume(self.record._content_length)
            self.record = None
            return skip_next

        if self.parse_http and self.record._is_http:
            self.record.parse_http()

        return has_next

    cpdef bint _set_stream(self, stream) except 0:
        """
        _set_stream(self, stream)
        
        Replace underlying input stream.
        
        This method is for internal use and should not be called by external users.
        """
        if not isinstance(stream, IOStream):
            for attr in ('read', 'tell', 'close'):
                if not hasattr(stream, attr):
                    raise AttributeError(f"Object of type '{type(stream).__name__}' has no attribute '{attr}'.")
            stream = PythonIOStreamAdapter.__new__(PythonIOStreamAdapter, stream)

        cdef IOStream stream_ = <IOStream>stream
        self.reader = BufferedReader.__new__(BufferedReader, stream_)
        self.record = None
        return True


# noinspection PyProtectedMember
cpdef bint is_warc_10(WarcRecord record):
    """
    is_warc_10(record)
    
    Filter predicate for checking if record is a WARC/1.0 record.
    
    :param record: WARC record
    :type record: WarcRecord
    :rtype: bool
    """
    return record._headers._status_line == <char*>b'WARC/1.0'


# noinspection PyProtectedMember
cpdef bint is_warc_11(WarcRecord record):
    """
    is_warc_11(record)
    
    Filter predicate for checking if record is a WARC/1.1 record.
    
    :param record: WARC record
    :type record: WarcRecord
    :rtype: bool
    """
    return record._headers._status_line == <char*>b'WARC/1.1'


# noinspection PyProtectedMember
cpdef bint has_block_digest(WarcRecord record):
    """
    has_block_digest(record)
    
    Filter predicate for checking if record has a block digest.
    
    :param record: WARC record
    :type record: WarcRecord
    :rtype: bool
    """
    return not record._headers.find_header(<char*>b'WARC-Block-Digest', <char*>b'').empty()


# noinspection PyProtectedMember
cpdef bint has_payload_digest(WarcRecord record):
    """
    has_payload_digest(record)
    
    Filter predicate for checking if record has a payload digest.
    
    :param record: WARC record
    :type record: WarcRecord
    :rtype: bool
    """
    return not record._headers.find_header(<char*>b'WARC-Payload-Digest', <char*>b'').empty()


# noinspection PyProtectedMember
cpdef bint is_http(WarcRecord record):
    """
    is_http(record)
    
    Filter predicate for checking if record is an HTTP record.
    
    :param record: WARC record
    :type record: WarcRecord
    :rtype: bool
    """
    return record._is_http


# noinspection PyProtectedMember
cpdef bint is_concurrent(WarcRecord record):
    """
    is_concurrent(record)
    
    Filter predicate for checking if record is concurrent to another record.
    
    :param record: WARC record
    :type record: WarcRecord
    :rtype: bool
    """
    return not record._headers.find_header(<char*>b'WARC-Concurrent-To', <char*>b'').empty()
