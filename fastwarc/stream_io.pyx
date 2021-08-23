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

from resiliparse_inc.cstring cimport strerror
from resiliparse_inc.errno cimport errno
from resiliparse_inc.stdio cimport fclose, fflush, fopen, fread, fseek, ftell, fwrite, SEEK_SET
from resiliparse_inc.string cimport npos as strnpos, string


class FastWARCError(Exception):
    """Generic FastWARC exception."""


class StreamError(FastWARCError):
    """FastWARC stream error."""


@cython.auto_pickle(False)
cdef class IOStream:
    """IOStream base class."""

    def __enter__(self) -> IOStream:
        """
        :rtype: IOStream
        """
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()

    cdef size_t read(self, string& out, size_t size) except -1:
        pass

    cdef size_t write(self, const char* data, size_t size) except -1:
        pass

    cdef size_t tell(self) except -1:
        pass

    cdef void flush(self) except *:
        pass

    cdef void close(self) except *:
        pass


# noinspection PyAttributeOutsideInit
@cython.auto_pickle(False)
cdef class BytesIOStream(IOStream):
    """
    __init__(self, initial_data=None)

    IOStream that uses an in-memory buffer.

    :param initial_data: fill internal buffer with this initial data
    :type initial_data: bytes
    """
    def __init__(self, *args, **kwargs):
        pass

    def __cinit__(self, bytes initial_data=None):
        self.pos = 0
        if initial_data is not None:
            self.buffer = initial_data

    cdef inline size_t tell(self) except -1:
        return self.pos

    cdef inline void seek(self, size_t offset) except *:
        self.pos = min(self.buffer.size(), offset)

    cdef size_t read(self, string& out, size_t size) except -1:
        if self.pos >= self.buffer.size():
            out.clear()
            return 0
        out.assign(self.buffer.substr(self.pos, size))
        self.seek(self.pos + size)
        return out.size()

    cdef size_t write(self, const char* data, size_t size) except -1:
        if self.pos + size > self.buffer.size():
            self.buffer.resize(self.pos + size)
        cdef size_t i
        for i in range(size):
            self.buffer[self.pos + i] = data[i]
        self.pos += size
        return size

    cdef inline void close(self) except *:
        self.buffer.clear()

    cdef inline string getvalue(self):
        return self.buffer


# noinspection PyAttributeOutsideInit
@cython.auto_pickle(False)
cdef class FileStream(IOStream):
    """
    __init__(self, filename=None, mode='rb')

    Fast alternative to Python file objects for local files.

    :param filename: input filename
    :type filename: str
    :param mode: file open mode
    :type mode: str
    """

    def __init__(self, *args, **kwargs):
        pass

    def __cinit__(self, str filename=None, str mode='rb'):
        self.fp = NULL
        if filename:
            self.open(filename.encode(), mode.encode())

    def __dealloc__(self):
        self.close()

    cdef void open(self, char* path, char* mode=b'rb') except *:
        if self.fp != NULL:
            self.close()

        self.fp = fopen(path, mode)
        if self.fp == NULL:
            raise StreamError(strerror(errno).decode())

    cdef void seek(self, size_t offset) except *:
        fseek(self.fp, offset, SEEK_SET)

    cdef size_t tell(self) except -1:
        return ftell(self.fp)

    cdef size_t read(self, string& out, size_t size) except -1:
        cdef size_t c
        with nogil:
            if out.size() < size:
                out.resize(size)
            c = fread(out.data(), sizeof(char), size, self.fp)
            if errno:
                out.clear()
                with gil:
                    raise StreamError(strerror(errno).decode())
            out.resize(c)
            return out.size()

    cdef size_t write(self, const char* data, size_t size) except -1:
        cdef size_t w = fwrite(data, sizeof(char), size, self.fp)
        if errno:
            raise StreamError(strerror(errno).decode())
        return w

    cdef void flush(self) except *:
        fflush(self.fp)

    cdef void close(self) except *:
        if self.fp != NULL:
            fclose(self.fp)
            self.fp = NULL


@cython.auto_pickle(False)
cdef class PythonIOStreamAdapter(IOStream):
    """
    __init__(self, py_stream)

    IOStream adapter for file-like Python objects.

    :param py_stream: input Python stream object
    """

    def __init__(self, *args, **kwargs):
        pass

    def __cinit__(self, py_stream):
        self.py_stream = py_stream


cpdef IOStream wrap_stream(raw_stream):
    """
    wrap_stream(raw_stream)
    
    Wrap ``raw_stream`` into a :class:`PythonIOStreamAdapter` if it is a Python object or
    return ``raw_stream`` unmodified if it is a :class:`IOStream` already.
    
    :param raw_stream: stream to wrap
    :return: wrapped stream
    :rtype: IOStream
    """
    if isinstance(raw_stream, IOStream):
        return raw_stream
    elif isinstance(raw_stream, object) and hasattr(raw_stream, 'read'):
        return PythonIOStreamAdapter.__new__(PythonIOStreamAdapter, raw_stream)
    else:
        raise ValueError(f"Object of type '{type(raw_stream).__name__}' is not a valid stream.")


@cython.auto_pickle(False)
cdef class CompressingStream(IOStream):
    """Base class for compressed :class:`IOStream` types."""

    cdef size_t begin_member(self):
        """Begin compression member/frame (if not already started)."""
        return 0

    cdef size_t end_member(self):
        """End compression member/frame (if one has been started)."""
        return 0


cdef char _GZIP_DEFLATE = 1
cdef char _GZIP_INFLATE = 2


# noinspection PyAttributeOutsideInit
@cython.auto_pickle(False)
cdef class GZipStream(CompressingStream):
    """
    __init__(self, raw_stream, compression_level=9)

    GZip :class:`IOStream` implementation.

    :param raw_stream: raw data stream
    :param compression_level: GZip compression level (for compression only)
    :type compression_level: int
    """

    def __init__(self, *args, **kwargs):
        pass

    def __cinit__(self, raw_stream, compression_level=Z_BEST_COMPRESSION):
        self.raw_stream = wrap_stream(raw_stream)
        self.member_started = False
        self.working_buf = string()
        self.initialized = 0
        self.stream_read_status = Z_STREAM_END
        self.stream_pos = self.raw_stream.tell()
        self.compression_level = compression_level

    def __dealloc__(self):
        self.close()

    cdef size_t tell(self) except -1:
        if self.initialized == _GZIP_INFLATE:
            return self.stream_pos
        return self.raw_stream.tell()

    cdef void _init_z_stream(self, bint deflate) nogil:
        """
        Reset internal state and initialize ``z_stream``.
        
        :param deflate: ``True`` for compression context, ``False`` for decompression context.
        """

        if self.initialized:
            return

        self.zst.opaque = Z_NULL
        self.zst.zalloc = Z_NULL
        self.zst.zfree = Z_NULL
        self.zst.next_in = NULL
        self.zst.next_out = NULL
        self.zst.avail_in = 0
        self.zst.avail_out = 0
        self.stream_read_status = Z_STREAM_END
        self.working_buf.clear()

        if deflate:
            deflateInit2(&self.zst, self.compression_level, Z_DEFLATED, 16 + MAX_WBITS, 9, Z_DEFAULT_STRATEGY)
            self.initialized = _GZIP_DEFLATE
        else:
            inflateInit2(&self.zst, 16 + MAX_WBITS)
            self.initialized = _GZIP_INFLATE

    cdef void prepopulate(self, bint deflate, const string& initial_data):
        """
        Fill internal working buffer with initial data.
        Use if some initial data of the stream have already been consumed (e.g., for stream content negotiation).
        Has to be called before the first :meth:`read()`.
        
        :param deflate: ``True`` if ``data`` is uncompressed, ``False`` if ``data`` is compressed GZip data. 
        :param initial_data: data to pre-populate
        """
        self._init_z_stream(deflate)
        self.working_buf.append(initial_data)
        self.zst.next_in = <Bytef*>self.working_buf.data()
        self.zst.avail_in = self.working_buf.size()

    cdef void _free_z_stream(self) nogil:
        """Release internal state and reset working buffer."""
        if not self.initialized:
            return
        if self.initialized == _GZIP_DEFLATE:
            deflateEnd(&self.zst)
        elif self.initialized == _GZIP_INFLATE:
            inflateEnd(&self.zst)
        self.working_buf.clear()
        self.initialized = 0

    cdef bint _refill_working_buf(self, size_t size) nogil except -1:
        self.working_buf.erase(0, self.working_buf.size())
        cdef string raw_data
        with gil:
            self.raw_stream.read(raw_data, max(1024u, size))
            self.working_buf.append(raw_data)
            if self.working_buf.empty():
                # EOF
                self._free_z_stream()
                self.stream_pos = self.raw_stream.tell()
                return False

        self.zst.next_in = <Bytef*>self.working_buf.data()
        self.zst.avail_in = self.working_buf.size()
        return True

    cdef size_t read(self, string& out, size_t size) except -1:
        if self.initialized == _GZIP_DEFLATE:
            raise StreamError('Compression in progress.')

        if not self.initialized:
            self._init_z_stream(False)

        if self.zst.avail_in == 0 or self.working_buf.empty():
            if not self._refill_working_buf(size):
                out.clear()
                return 0

        out.resize(size)
        self.zst.next_out = <Bytef*>out.data()
        self.zst.avail_out = out.size()

        with nogil:
            self.stream_read_status = inflate(&self.zst, Z_NO_FLUSH)
            while self.zst.next_out == <Bytef*>out.data() and (self.stream_read_status == Z_OK or
                                                               self.stream_read_status == Z_BUF_ERROR):
                if self.zst.avail_in == 0:
                    if not self._refill_working_buf(size):
                        break

                self.stream_read_status = inflate(&self.zst, Z_NO_FLUSH)

        # Error
        if self.stream_read_status < 0 and self.stream_read_status != Z_BUF_ERROR:
            self._free_z_stream()
            raise StreamError('Not a valid GZip stream')

        if self.stream_read_status == Z_STREAM_END:
            # Member end
            self.stream_pos = self.raw_stream.tell() - self.working_buf.size() + \
                              (self.zst.next_in - <Bytef*>self.working_buf.data())
            inflateReset(&self.zst)

        if self.zst.avail_out > 0:
            out.resize(self.zst.next_out - <Bytef*>out.data())

        if out.empty():
            # We may have hit a member boundary, try again
            return self.read(out, size)

        return out.size()

    cdef size_t write(self, const char* data, size_t size) except -1:
        if self.initialized == _GZIP_INFLATE:
            raise StreamError('Decompression in progress')

        if not self.initialized:
            self._init_z_stream(True)

        self.zst.next_in = <Bytef*>data
        self.zst.avail_in = size

        self.begin_member()
        cdef size_t written = 0
        cdef size_t bound = max(4096u, deflateBound(&self.zst, size))
        if self.working_buf.size() < bound or self.working_buf.size() / 8 > bound:
            self.working_buf.resize(bound)
        self.zst.next_out = <Bytef*>self.working_buf.data()
        self.zst.avail_out = self.working_buf.size()

        cdef int status = Z_OK
        cdef size_t written_so_far = self.zst.total_out
        with nogil:
            while self.zst.avail_in > 0 and status == Z_OK:
                status = deflate(&self.zst, Z_NO_FLUSH)
                if self.zst.avail_in > 0 and self.zst.avail_out == 0:
                    # Out buffer fully filled, but in buffer still holding data
                    self.working_buf.append(4096u, <char>0)
                    self.zst.next_out = <Bytef*>&self.working_buf.back() - 4096u
                    self.zst.avail_out = 4096u

        written += self.zst.total_out - written_so_far
        if written == 0:
            return 0

        return self.raw_stream.write(self.working_buf.data(), written)

    cdef size_t begin_member(self):
        self.member_started = True
        return 0

    cdef size_t end_member(self):
        if not self.member_started:
            return 0

        self.zst.avail_in = 0
        self.zst.next_in = NULL
        self.zst.next_out = <Bytef*>self.working_buf.data()
        self.zst.avail_out = self.working_buf.size()

        cdef size_t written_so_far = self.zst.total_out
        cdef int status = deflate(&self.zst, Z_FINISH)
        while status == Z_OK or status == Z_BUF_ERROR:
            if self.zst.avail_out == 0:
                # Need larger output buffer (unlikely to ever happen at this point)
                self.working_buf.resize(self.working_buf.size() + 4096u)
                self.zst.next_out = <Bytef*>&self.working_buf.back() - 4096u
                self.zst.avail_out = 4096u
            status = deflate(&self.zst, Z_FINISH)

        cdef size_t written = self.zst.total_out - written_so_far
        deflateReset(&self.zst)
        self.member_started = False

        if written == 0:
            return 0
        return self.raw_stream.write(self.working_buf.data(), written)

    cdef void flush(self) except *:
        self.end_member()
        self.raw_stream.flush()

    cdef void close(self) except *:
        self.end_member()
        self._free_z_stream()
        if self.raw_stream is not None:
            self.raw_stream.close()


# noinspection PyAttributeOutsideInit
@cython.auto_pickle(False)
cdef class LZ4Stream(CompressingStream):
    """
    __init__(self, raw_stream, compression_level=12, favor_dec_speed=True)

    LZ4 :class:`IOStream` implementation.

    :param raw_stream: raw data stream
    :param compression_level: LZ4 compression level (for compression only)
    :type compression_level: int
    :param favor_dec_speed: favour decompression speed over compression speed and size
    :type favor_dec_speed: bool
    """

    def __init__(self, *args, **kwargs):
        pass

    def __cinit__(self, raw_stream, compression_level=LZ4HC_CLEVEL_MAX, favor_dec_speed=True):
        self.raw_stream = wrap_stream(raw_stream)
        self.cctx = NULL
        self.dctx = NULL
        self.working_buf = string()
        self.frame_started = False
        self.prefs.compressionLevel = compression_level
        self.prefs.favorDecSpeed = favor_dec_speed
        self.stream_pos = self.raw_stream.tell()

    def __dealloc__(self):
        self.close()

    cdef size_t tell(self) except -1:
        if self.dctx != NULL:
            return self.stream_pos
        return self.raw_stream.tell()

    cdef void prepopulate(self, const string& initial_data):
        """
        Fill internal working buffer with initial data.
        Use if some initial data of the stream have already been consumed (e.g., for stream content negotiation).
        Has to be called before the first :meth:`read()`.
        
        :param initial_data: data to pre-populate
        """
        self.working_buf.append(initial_data)

    cdef size_t read(self, string& out, size_t size) except -1:
        if self.cctx != NULL:
            raise StreamError('Compression in progress.')

        self.raw_stream.read(self.working_buf, size)
        if self.working_buf.empty():
            if self.working_buf.empty():
                # EOF
                self._free_ctx()
                return 0

        out.resize(size)
        cdef size_t ret, bytes_read, bytes_written
        with nogil:
            bytes_read = self.working_buf.size()
            bytes_written = out.size()

            if self.dctx == NULL:
                LZ4F_createDecompressionContext(&self.dctx, LZ4F_VERSION)

            ret = LZ4F_decompress(self.dctx, out.data(), &bytes_written,
                                  self.working_buf.data(), &bytes_read, NULL)
            while ret != 0 and bytes_written == 0 and not LZ4F_isError(ret):
                with gil:
                    self.working_buf.erase(0, bytes_read)
                    self.raw_stream.read(self.working_buf, size)
                if self.working_buf.empty():
                    # EOF
                    self._free_ctx()
                    out.clear()
                    return 0
                bytes_read = self.working_buf.size()
                bytes_written = out.size()
                ret = LZ4F_decompress(self.dctx, out.data(), &bytes_written,
                                      self.working_buf.data(), &bytes_read, NULL)

            if ret == 0:
                # Frame end
                with gil:
                    self.stream_pos = self.raw_stream.tell() - self.working_buf.size() + bytes_read
            elif LZ4F_isError(ret):
                self._free_ctx()
                with gil:
                    raise StreamError('Not a valid LZ4 stream')
            self.working_buf.erase(0, bytes_read)

            if out.size() != bytes_written:
                out.resize(bytes_written)

        if out.empty():
            # Everything OK, we may have hit a frame boundary
            return self.read(out, size)

        return out.size()

    cdef size_t begin_member(self):
        cdef size_t written
        with nogil:
            if self.cctx == NULL:
                LZ4F_isError(LZ4F_createCompressionContext(&self.cctx, LZ4F_VERSION))

            if self.frame_started:
                return 0

            if self.working_buf.size() < LZ4F_HEADER_SIZE_MAX:
                self.working_buf.resize(LZ4F_HEADER_SIZE_MAX)
            written = LZ4F_compressBegin(self.cctx, self.working_buf.data(), self.working_buf.size(), &self.prefs)
            self.frame_started = True

        return self.raw_stream.write(self.working_buf.data(), written)

    cdef size_t end_member(self):
        cdef size_t written
        with nogil:
            if self.cctx == NULL or not self.frame_started:
                return 0
            written = LZ4F_compressEnd(self.cctx, self.working_buf.data(), self.working_buf.size(), NULL)
            self.frame_started = False
        return self.raw_stream.write(self.working_buf.data(), written)

    cdef size_t write(self, const char* data, size_t size) except -1:
        if self.dctx != NULL:
            raise StreamError('Decompression in progress.')

        cdef size_t buf_needed, written
        cdef size_t header_bytes_written = self.begin_member()
        with nogil:
            buf_needed = max(4096u, LZ4F_compressBound(size, &self.prefs))
            if self.working_buf.size() < buf_needed or self.working_buf.size() / 8 > buf_needed:
                self.working_buf.resize(buf_needed)

            written = LZ4F_compressUpdate(self.cctx, self.working_buf.data(), self.working_buf.size(),
                                          data, size, NULL)
        return self.raw_stream.write(self.working_buf.data(), written) + header_bytes_written

    cdef void flush(self) except *:
        cdef size_t written
        cdef size_t buf_needed
        if self.cctx != NULL:
            buf_needed = LZ4F_compressBound(0, &self.prefs)
            if self.working_buf.size() < buf_needed:
                self.working_buf.resize(buf_needed)

            written = LZ4F_flush(self.cctx, self.working_buf.data(), self.working_buf.size(), NULL)
            self.raw_stream.write(self.working_buf.data(), written)
        self.raw_stream.flush()

    cdef void close(self) except *:
        if self.cctx != NULL:
            self.end_member()

        self._free_ctx()
        if self.raw_stream is not None:
            self.raw_stream.close()

    cdef void _free_ctx(self) nogil:
        if self.cctx != NULL:
            LZ4F_freeCompressionContext(self.cctx)
            self.cctx = NULL

        if self.dctx != NULL:
            LZ4F_freeDecompressionContext(self.dctx)
            self.dctx = NULL

        if not self.working_buf.empty():
            self.working_buf.clear()


# noinspection PyAttributeOutsideInit
@cython.auto_pickle(False)
cdef class BufferedReader:
    """
    __init__(self, stream, buf_size=16384, negotiate_stream=True)

    Buffered reader operating on an :class:`IOStream` instance.

    :param stream: stream to operate on
    :type stream: IOStream
    :param buf_size: internal buffer size
    :type buf_size: int
    :param negotiate_stream: whether to auto-negotiate stream type
    :type negotiate_stream: bool
    """

    def __init__(self, *args, **kwargs):
        pass

    def __cinit__(self, IOStream stream, size_t buf_size=16384, bint negotiate_stream=True):
        self.stream = stream
        self.buf_size = max(1024u, buf_size)
        self.buf = string()
        self.limit = strnpos
        self.limit_consumed = 0
        self.stream_is_compressed = isinstance(stream, CompressingStream)
        self.stream_started = False
        self.negotiate_stream = negotiate_stream and not self.stream_is_compressed

    cdef bint detect_stream_type(self) except 0:
        """
        Try to auto-detect stream type (GZip, LZ4, or uncompressed).
        """
        if not self.negotiate_stream or self.stream_started:
            return True

        self.negotiate_stream = False
        if self.buf.empty():
            self._fill_buf()

        if self.buf.size() > 2 and self.buf[0] == <char> 0x1f and self.buf[1] == <char> 0x8b:
            self.stream = GZipStream.__new__(GZipStream, self.stream)
            (<GZipStream> self.stream).prepopulate(False, self.buf)
        elif self.buf.size() > 4 and self.buf.substr(0, 4) == <char*>b'\x04\x22\x4d\x18':
            self.stream = LZ4Stream.__new__(LZ4Stream, self.stream)
            (<LZ4Stream> self.stream).prepopulate(self.buf)
        elif self.buf.size() > 5 and self.buf.substr(0, 5) == <char*>b'WARC/':
            # Stream is uncompressed: bail out, dont' mess with buffers
            self.stream_is_compressed = False
            return True
        else:
            self.stream_is_compressed = False
            raise StreamError('Not a valid WARC stream')

        self.stream_is_compressed = isinstance(self.stream, CompressingStream)
        self.buf.clear()
        cdef string raw_data
        self.stream.read(raw_data, self.buf_size)
        self.buf.append(raw_data)
        self.stream_started = False
        return True

    cdef bint _fill_buf(self) except -1:
        """
        Refill internal buffer.
        
        :return: ``True`` if refill was successful, ``False`` otherwise (EOF)
        """
        self.stream_started = True
        if self.buf.size() > 0:
            return True if self.limit == strnpos else self.limit > self.limit_consumed

        cdef string stream_data
        self.stream.read(stream_data, self.buf_size)
        self.buf.append(stream_data)

        if self.buf.size() == 0:
            return False
        elif self.limit != strnpos:
            return self.limit > self.limit_consumed
        return True

    cdef string_view _get_buf(self) nogil:
        """
        Get buffer contents. Does take a set limit into account.
        
        :return: available buffer contents
        """
        cdef string_view v = string_view(self.buf.c_str(), self.buf.size())
        cdef size_t remaining
        if self.limit != strnpos:
            remaining = self.limit - self.limit_consumed
            if v.size() > remaining:
                v.remove_suffix(v.size() - remaining)
        return v

    cdef void _consume_buf(self, size_t size) nogil:
        """
        Consume up to ``size`` bytes from internal buffer. Takes a set limit into account.
        
        :param size: number of bytes to read
        """
        if self.limit == strnpos and size >= self.buf.size():
            self.buf.clear()
            return

        if self.limit != strnpos:
            if size > self.limit - self.limit_consumed:
                size = self.limit - self.limit_consumed
            self.limit_consumed += size

        self.buf.erase(0, size)

    cdef inline void set_limit(self, size_t offset) nogil:
        """
        Set a stream limit in bytes. Any read beyond this limit will act as if the stream reached EOF.
        A set limit can be reset by calling :meth:`reset_limit()`.
        
        :param offset: limit in bytes
        """
        self.limit = offset
        self.limit_consumed = 0

    cdef inline void reset_limit(self) nogil:
        """Reset any previously set stream limit."""
        self.limit = strnpos

    cpdef string read(self, size_t size=strnpos) except *:
        """
        read(self, size=-1)
        
        Read up to ``size`` bytes from the input stream.
        
        :param size: number of bytes to read (default means read remaining stream)
        :type size: int
        :return: consumed buffer contents as bytes (or empty string if EOF)
        :rtype: bytes
        """
        cdef string data_read
        cdef size_t missing = size
        cdef string_view buf_sub

        while (size == strnpos or data_read.size() < size) and self._fill_buf():
            missing = size - data_read.size()
            buf_sub = self._get_buf().substr(0, missing)
            data_read.append(<string>buf_sub)
            self._consume_buf(buf_sub.size())
        return data_read

    cpdef string readline(self, bint crlf=True, size_t max_line_len=4096) except *:
        """
        readline(self, crlf=True, max_line_len=4096)
        
        Read a single line from the input stream.
        
        :param crlf: whether lines are separated by CRLF or LF
        :type crlf: bool
        :param max_line_len: maximum line length (longer lines will still be consumed, but the return
                             value will not be larger than this)
        :type max_line_len: int
        :return: line contents (or empty string if EOF)
        :rtype: bytes
        """
        cdef string line

        if not self._fill_buf():
            return string()

        cdef size_t capacity_remaining = max_line_len
        cdef string_view buf = self._get_buf()
        cdef char* newline_sep = b'\r\n' if crlf else '\n'
        cdef short newline_offset = 1 if crlf else 0
        cdef size_t pos = buf.find(newline_sep)
        cdef bint last_was_cr = False

        with nogil:
            while pos == strnpos:
                if capacity_remaining > 0:
                    line.append(<string>buf.substr(0, min(buf.size(), capacity_remaining)))
                    capacity_remaining -= line.size()

                # CRLF may be split
                last_was_cr = crlf and buf.back() == <char>b'\r'

                # Consume rest of line
                self._consume_buf(buf.size())

                with gil:
                    if not self._fill_buf():
                        break
                buf = self._get_buf()
                if last_was_cr and buf.front() == <char>b'\n':
                    if capacity_remaining:
                        line.append(<char*>b'\n')
                    self._consume_buf(1)
                    pos = strnpos
                    break

                pos = buf.find(newline_sep)

            if not buf.empty() and pos != strnpos:
                if capacity_remaining > 0:
                    line.append(<string>buf.substr(0, min(pos + 1 + newline_offset, capacity_remaining)))
                self._consume_buf(pos + 1 + newline_offset)

        return line

    cpdef size_t tell(self) except -1:
        """
        tell(self)
        
        Offset on the input stream.
        
        :return: offset
        :rtype: int
        """
        if not self.stream_started:
            return 0

        if self.limit != strnpos:
            return self.limit_consumed

        if self.stream_is_compressed:
            # Compressed streams advance their position only on block boundaries.
            # The reader position inside the stream is meaningless.
            return self.stream.tell()

        return self.stream.tell() - self.buf.size()

    cpdef bint consume(self, size_t size=strnpos) except 0:
        """
        consume(self, size=-1)
        
        Consume up to ``size`` bytes from the input stream without allocating a buffer for it.
        
        :param size: number of bytes to read (default means read remaining stream)
        :type size: int
        """
        cdef string_view buf
        cdef size_t bytes_to_consume

        while size > 0 and self._fill_buf():
            buf = self._get_buf()
            if buf.empty():
                break

            if size != strnpos:
                bytes_to_consume = min(buf.size(), size)
                self._consume_buf(bytes_to_consume)
                size -= bytes_to_consume
            else:
                self._consume_buf(buf.size())

        return True

    cpdef void close(self) except *:
        """
        close(self)
        
        Close stream.
        """
        if self.stream is not None:
            self.stream.close()
