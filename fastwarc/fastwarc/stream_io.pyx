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
from cython.operator cimport preincrement as preinc
from libc.string cimport memchr, memcmp, memcpy
from libcpp.string cimport npos as strnpos, string

from resiliparse_inc.cstring cimport strerror
from resiliparse_inc.errno cimport errno
from resiliparse_inc.stdio cimport fclose, ferror, fflush, fopen, fread, fseek, ftell, fwrite, SEEK_SET

import brotli


class FastWARCError(Exception):
    """Generic FastWARC exception."""


class StreamError(FastWARCError):
    """FastWARC stream error."""


class ReaderStaleError(FastWARCError):
    """FastWARC reader stale error."""


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

    def read(self, size_t size):
        """
        read(self, size)

        Read ``size`` bytes from stream.

        :param size: bytes to read
        :type size: int
        :return: read bytes
        :rtype: bytearray
        """
        cdef bytearray buf = bytearray(size)
        size = self.read_(<char*>buf, size)
        return buf[:size]

    cdef size_t read_(self, char* out, size_t size) except -1:
        """Internal C interface for :meth:`read`."""
        pass

    cdef size_t write_(self, const char* data, size_t size) except -1:
        """Internal C interface for :meth:`write`."""
        pass

    def write(self, bytes data):
        """
        write(self, data)

        Write bytes to stream.

        :param data: data to write
        :type data: bytes
        :return: number of bytes written
        :rtype: int
        """
        size = self.write_(<const char*>data, len(data))
        return size

    cpdef void seek(self, size_t offset) except *:
        """
        seek(self, offset)
        
        Seek to specified offset.
        
        :param offset: seek offset
        :type offset: int
        """
        pass

    cpdef size_t tell(self) except -1:
        """
        tell(self)
        
        Return current stream offset.
        
        :return: stream offset
        :rtype: int
        """
        pass

    cpdef void flush(self) except *:
        """
        flush(self)
        
        Flush stream buffer.
        """
        pass

    cpdef void close(self) except *:
        """
        close(self)
        
        Close the stream.
        """
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

    cpdef size_t tell(self) except -1:
        if self.pos == strnpos:
            raise ValueError('Trying I/O on closed file.')
        return self.pos

    cpdef void seek(self, size_t offset) except *:
        if self.pos == strnpos:
            raise ValueError('Trying I/O on closed file.')
        self.pos = min(self.buffer.size(), offset)

    cdef size_t read_(self, char* out, size_t size) except -1:
        if self.pos == strnpos:
            raise ValueError('Trying I/O on closed file.')
        if self.pos >= self.buffer.size():
            return 0
        if self.pos + size > self.buffer.size():
            size = self.buffer.size() - self.pos
        memcpy(out, self.buffer.data() + self.pos, size)
        self.seek(self.pos + size)
        return size

    cdef size_t write_(self, const char* data, size_t size) except -1:
        if self.pos == strnpos:
            raise ValueError('Trying I/O on closed file.')
        if self.pos + size > self.buffer.size():
            self.buffer.resize(self.pos + size)
        memcpy(self.buffer.data() + self.pos, data, size)
        self.pos += size
        return size

    cpdef void close(self) except *:
        self.buffer.clear()
        self.pos = strnpos

    cpdef string getvalue(self):
        """
        getvalue(self)
        
        Get buffer value.
        
        :return: buffer value
        :rtype: bytes
        """
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

    def __cinit__(self, str filename, str mode='rb'):
        self.fp = NULL
        if filename:
            self.open_(filename.encode(), mode.encode())

    def __dealloc__(self):
        self.close()

    cdef void open_(self, char* path, char* mode=b'rb') except *:
        if self.fp != NULL:
            self.close()

        self.fp = fopen(path, mode)
        if self.fp == NULL:
            raise StreamError(strerror(errno).decode())

    cpdef void seek(self, size_t offset) except *:
        if self.fp == NULL:
            raise ValueError('Trying I/O on closed file.')
        fseek(self.fp, <unsigned long>offset, SEEK_SET)

    cpdef size_t tell(self) except -1:
        if self.fp == NULL:
            raise ValueError('Trying I/O on closed file.')
        return ftell(self.fp)

    cdef size_t read_(self, char* out, size_t size) except -1:
        if self.fp == NULL:
            raise ValueError('Trying I/O on closed file.')
        cdef size_t bytes_read
        with nogil:
            bytes_read = fread(out, sizeof(char), size, self.fp)
            if bytes_read < size and ferror(self.fp):
                with gil:
                    raise StreamError('Error reading file')
            return bytes_read

    cdef size_t write_(self, const char* data, size_t size) except -1:
        if self.fp == NULL:
            raise ValueError('Trying I/O on closed file.')
        cdef size_t w = fwrite(data, sizeof(char), size, self.fp)
        if ferror(self.fp):
            raise StreamError('Error writing file')
        return w

    cpdef void flush(self) except *:
        if self.fp == NULL:
            raise ValueError('Trying I/O on closed file.')
        fflush(self.fp)

    cpdef void close(self) except *:
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

    cdef size_t read_(self, char* out, size_t size) except -1:
        cdef bytes data = self.py_stream.read(size)
        size = min(<size_t>len(data), size)
        memcpy(out, <char*>data, size * sizeof(char))
        return size

    cdef size_t write_(self, const char* data, size_t size) except -1:
        return self.py_stream.write(data[:size])

    cpdef size_t tell(self) except -1:
        return self.py_stream.tell()

    cpdef void seek(self, size_t offset) except *:
        self.py_stream.seek(offset)

    cpdef void flush(self) except *:
        self.py_stream.flush()

    cpdef void close(self) except *:
        if self.py_stream is None:
            return
        try:
            self.py_stream.close()
        except ValueError:
            # Ignore if already closed
            pass


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

    cpdef size_t begin_member(self):
        """
        begin_member(self)
        
        Begin compression member / frame (if not already started).
        
        :return: bytes written
        :rtype: int
        """
        return 0

    cpdef size_t end_member(self):
        """
        end_member(self)
        
        End compression member / frame (if one has been started).
        
        :return: bytes written
        :rtype: int
        """
        return 0


# noinspection PyAttributeOutsideInit
@cython.auto_pickle(False)
cdef class GZipStream(CompressingStream):
    """
    __init__(self, raw_stream, compression_level=9, zlib=False)

    GZip :class:`IOStream` implementation.

    :param raw_stream: raw data stream
    :param compression_level: GZip compression level (for compression only)
    :type compression_level: int
    :param zlib: use raw deflate / zlib format instead of gzip
    :type zlib: bool
    """

    def __init__(self, *args, **kwargs):
        pass

    def __cinit__(self, raw_stream, compression_level=Z_BEST_COMPRESSION, bint zlib=False):
        self.raw_stream = wrap_stream(raw_stream)
        self.member_started = False
        self.working_buf_filled = 0u
        self.stream_state = CompressingStreamState.UNINIT
        self.stream_pos = self.raw_stream.tell()
        self.compression_level = compression_level
        self.window_bits = MAX_WBITS if zlib else 16 + MAX_WBITS

    def __dealloc__(self):
        self.close()

    cpdef size_t tell(self) except -1:
        return self.stream_pos

    cdef void _init_z_stream(self, bint deflate) noexcept nogil:
        """
        Reset internal state and initialize ``z_stream``.
        
        :param deflate: ``True`` for compression context, ``False`` for decompression context.
        """

        if self.stream_state != CompressingStreamState.UNINIT:
            return

        self.zst.opaque = Z_NULL
        self.zst.zalloc = Z_NULL
        self.zst.zfree = Z_NULL
        self.zst.next_in = NULL
        self.zst.next_out = NULL
        self.zst.total_in = 0u
        self.zst.total_out = 0u
        self.zst.avail_in = 0u
        self.zst.avail_out = 0u
        self.working_buf.clear()

        if deflate:
            deflateInit2(&self.zst, self.compression_level, Z_DEFLATED, self.window_bits, 9, Z_DEFAULT_STRATEGY)
            self.stream_state = CompressingStreamState.COMPRESSING
        else:
            inflateInit2(&self.zst, self.window_bits)
            self.stream_state = CompressingStreamState.DECOMPRESSING

    cpdef void prepopulate(self, bint deflate, const string& initial_data):
        """
        prepopulate(self, initial_data)
        
        Fill internal working buffer with initial data.
        Use if some initial data of the stream have already been consumed (e.g., for stream content negotiation).
        Has to be called before the first :meth:`read()`.
        
        :param deflate: ``True`` if ``data`` is uncompressed, ``False`` if ``data`` is compressed GZip data. 
        :type deflate: int
        :param initial_data: data to pre-populate
        :type initial_data: bytes
        """
        self._init_z_stream(deflate)
        self.working_buf.append(initial_data)
        self.working_buf_filled += <unsigned int>initial_data.size()
        self.zst.next_in = <Bytef*>self.working_buf.data()
        self.zst.avail_in = self.working_buf_filled
        self.stream_pos = max(0u, self.raw_stream.tell() - self.working_buf_filled)

    cdef void _free_z_stream(self) noexcept nogil:
        """Release internal state and reset working buffer."""
        if self.stream_state == CompressingStreamState.UNINIT:
            return
        if self.stream_state == CompressingStreamState.COMPRESSING:
            deflateEnd(&self.zst)
        elif self.stream_state == CompressingStreamState.DECOMPRESSING:
            inflateEnd(&self.zst)
        self.working_buf.clear()
        self.working_buf_filled = 0u
        self.stream_state = CompressingStreamState.UNINIT

    cdef void _reinit_z_stream(self, bint deflate) noexcept nogil:
        """Re-initialize zstream, but retain working buffer. Use to restart stream with different window parameters."""

        cdef string working_buf_tmp = self.working_buf.substr(0, self.working_buf_filled)
        if self.stream_state != CompressingStreamState.UNINIT:
            self.zst.next_out = NULL
            self.zst.avail_out = 0u
            self._free_z_stream()
        with gil:
            self.prepopulate(deflate, working_buf_tmp)

    cdef bint _refill_working_buf(self, size_t size) except -1:
        if self.zst.avail_in > 0:
            return True

        if self.working_buf.size() < size:
            self.working_buf.resize(size)

        self.working_buf_filled = <unsigned int>self.raw_stream.read_(self.working_buf.data(), size)
        if self.working_buf_filled == 0:
            # EOF
            self._free_z_stream()
            return False

        self.zst.next_in = <Bytef*>self.working_buf.data()
        self.zst.avail_in = self.working_buf_filled
        return True

    cdef size_t read_(self, char* out, size_t size) except -1:
        if self.stream_state == CompressingStreamState.COMPRESSING:
            raise StreamError('Compression in progress.')

        # Init stream if this is the first read
        if self.stream_state == CompressingStreamState.UNINIT:
            self._init_z_stream(False)

            # If reader is in deflate / zlib format mode, check if first bytes are a zlib header
            if self.window_bits == MAX_WBITS:
                if not self._refill_working_buf(size) or self.working_buf.size() < 2:
                    return 0
                if not (self.working_buf[0] == b'\x78' and self.working_buf[1] in [b'\x01', b'\xef', b'\x9c', b'\xda']):
                    # Re-init as "raw" deflate stream if no zlib header found
                    self.window_bits = -MAX_WBITS
                    self._reinit_z_stream(False)

        if self.zst.avail_in == 0 or self.working_buf.empty():
            if not self._refill_working_buf(size):
                return 0

        self.zst.next_out = <Bytef*>out
        self.zst.avail_out = <unsigned int>size
        cdef int stream_read_status = Z_STREAM_END
        cdef size_t read_so_far = self.zst.total_in

        while True:
            stream_read_status = inflate(&self.zst, Z_NO_FLUSH)
            if self.zst.avail_out == 0 or (stream_read_status != Z_OK and stream_read_status != Z_BUF_ERROR):
                break
            if self.zst.avail_in == 0 and not self._refill_working_buf(size):
                break
        self.stream_pos += self.zst.total_in - read_so_far

        # Error
        if stream_read_status < 0 and stream_read_status != Z_BUF_ERROR:
            self._free_z_stream()
            raise StreamError('Not a valid GZip stream')

        # Member end
        if stream_read_status == Z_STREAM_END:
            inflateReset(&self.zst)

        return size - self.zst.avail_out

    cdef size_t write_(self, const char* data, size_t size) except -1:
        if self.stream_state == CompressingStreamState.DECOMPRESSING:
            raise StreamError('Decompression in progress')

        if not self.stream_state:
            self._init_z_stream(True)

        self.zst.next_in = <Bytef*>data
        self.zst.avail_in = <unsigned int>size

        self.begin_member()
        cdef size_t written = 0
        cdef size_t bound = max(8192u, deflateBound(&self.zst, <unsigned long>size))
        if self.working_buf.size() < bound:
            self.working_buf.resize(bound)
        self.zst.next_out = <Bytef*>self.working_buf.data()
        self.zst.avail_out = <unsigned int>self.working_buf.size()

        cdef int status = Z_OK
        cdef size_t written_so_far = self.zst.total_out

        while self.zst.avail_in > 0 and status == Z_OK:
            status = deflate(&self.zst, Z_NO_FLUSH)
            if self.zst.avail_in > 0 and self.zst.avail_out == 0:
                # Out buffer fully filled, but in buffer still holding data
                self.working_buf.resize(self.working_buf.size() + 4096u)
                self.zst.next_out = <Bytef*>self.working_buf.data() + self.working_buf.size() - 4096u
                self.zst.avail_out = 4096u

        written += self.zst.total_out - written_so_far
        self.stream_pos += written
        if written == 0:
            return 0

        return self.raw_stream.write_(self.working_buf.data(), written)

    cpdef size_t begin_member(self):
        self.member_started = True
        return 0

    cpdef size_t end_member(self):
        if not self.member_started:
            return 0

        self.zst.avail_in = 0u
        self.zst.next_in = NULL
        self.zst.next_out = <Bytef*>self.working_buf.data()
        self.zst.avail_out = <unsigned int>self.working_buf.size()

        cdef size_t written_so_far = self.zst.total_out
        cdef int status = deflate(&self.zst, Z_FINISH)
        while status == Z_OK or status == Z_BUF_ERROR:
            if self.zst.avail_out == 0:
                # Need larger output buffer (unlikely to ever happen at this point)
                self.working_buf.resize(self.working_buf.size() + 4096u)
                self.zst.next_out = <Bytef*>self.working_buf.data() + self.working_buf.size() - 4096u
                self.zst.avail_out = 4096u
            status = deflate(&self.zst, Z_FINISH)

        cdef size_t written = self.zst.total_out - written_so_far
        deflateReset(&self.zst)
        self.stream_pos += written
        self.member_started = False

        if written == 0:
            return 0
        return self.raw_stream.write_(self.working_buf.data(), written)

    cpdef void flush(self) except *:
        self.end_member()
        self.raw_stream.flush()

    cpdef void close(self) except *:
        self.end_member()
        self._free_z_stream()


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
        self.working_buf_read = 0u
        self.working_buf_filled = 0u
        self.frame_started = False
        self.prefs.compressionLevel = compression_level
        self.prefs.favorDecSpeed = favor_dec_speed
        self.stream_pos = self.raw_stream.tell()

    def __dealloc__(self):
        self.close()

    cpdef size_t tell(self) except -1:
        return self.stream_pos

    cpdef void prepopulate(self, const string& initial_data):
        """
        prepopulate(self, initial_data)
        
        Fill internal working buffer with initial data.
        Use if some initial data of the stream have already been consumed (e.g., for stream content negotiation).
        Has to be called before the first :meth:`read()`.
        
        :param initial_data: data to pre-populate
        :type initial_data: bytes
        """
        self.working_buf.append(initial_data)
        self.working_buf_filled += <unsigned int>initial_data.size()
        self.stream_pos = max(0u, self.raw_stream.tell() - self.working_buf_filled)

    cdef size_t read_(self, char* out, size_t size) except -1:
        if self.cctx != NULL:
            raise StreamError('Compression in progress.')

        cdef size_t ret
        cdef size_t bytes_in = 0, bytes_out = 0
        cdef size_t out_buf_consumed = 0

        if self.dctx == NULL:
            LZ4F_createDecompressionContext(&self.dctx, LZ4F_VERSION)

        if self.working_buf.size() < size:
            self.working_buf.resize(size)

        while True:
            if self.working_buf_filled == 0 or self.working_buf_read == self.working_buf_filled:
                self.working_buf_filled = <unsigned int>self.raw_stream.read_(self.working_buf.data(), size)
                self.working_buf_read = 0

                if self.working_buf_filled == 0:
                    # EOF
                    self._free_ctx()
                    return out_buf_consumed

            bytes_in = self.working_buf_filled - self.working_buf_read
            bytes_out = size - out_buf_consumed
            ret = LZ4F_decompress(self.dctx, out + out_buf_consumed, &bytes_out,
                                  self.working_buf.data() + self.working_buf_read, &bytes_in, NULL)
            self.stream_pos += bytes_in
            self.working_buf_read += bytes_in
            out_buf_consumed += bytes_out

            if ret == 0 or out_buf_consumed == size or LZ4F_isError(ret):
                break

        if LZ4F_isError(ret):
            self._free_ctx()
            raise StreamError(f'Not a valid LZ4 stream: {LZ4F_getErrorName(ret).decode()}')

        return out_buf_consumed

    cpdef size_t begin_member(self):
        if self.frame_started:
            return 0

        cdef size_t ret = LZ4F_createCompressionContext(&self.cctx, LZ4F_VERSION)
        if LZ4F_isError(ret):
            raise StreamError(f'Failed to create compression context: {LZ4F_getErrorName(ret).decode()}')

        if self.working_buf.size() < LZ4F_HEADER_SIZE_MAX:
            self.working_buf.resize(LZ4F_HEADER_SIZE_MAX + 8192u)
        cdef size_t written = LZ4F_compressBegin(self.cctx, self.working_buf.data(),
                                                 self.working_buf.size(), &self.prefs)
        self.frame_started = True

        return self.raw_stream.write_(self.working_buf.data(), written)

    cpdef size_t end_member(self):
        if self.cctx == NULL or not self.frame_started:
            return 0

        cdef size_t written
        cdef size_t buf_needed = LZ4F_compressBound(0, &self.prefs)
        with nogil:
            if self.working_buf.size() < buf_needed:
                self.working_buf.resize(buf_needed)
            written = LZ4F_compressEnd(self.cctx, self.working_buf.data(), self.working_buf.size(), NULL)
            self.frame_started = False
        return self.raw_stream.write_(self.working_buf.data(), written)

    cdef size_t write_(self, const char* data, size_t size) except -1:
        if self.dctx != NULL:
            raise StreamError('Decompression in progress.')

        cdef size_t buf_needed = max(8192u, LZ4F_compressBound(size, &self.prefs))
        if self.working_buf.size() < buf_needed:
            self.working_buf.resize(buf_needed)

        cdef size_t header_bytes_written = self.begin_member()
        cdef size_t written = LZ4F_compressUpdate(self.cctx, self.working_buf.data(), self.working_buf.size(),
                                                  data, size, NULL)
        self.stream_pos += written
        return self.raw_stream.write_(self.working_buf.data(), written) + header_bytes_written

    cpdef void flush(self) except *:
        if self.cctx == NULL:
            return

        cdef size_t buf_needed = LZ4F_compressBound(0, &self.prefs)
        if self.working_buf.size() < buf_needed:
            self.working_buf.resize(buf_needed)

        cdef size_t written = LZ4F_flush(self.cctx, self.working_buf.data(), self.working_buf.size(), NULL)
        self.raw_stream.write_(self.working_buf.data(), written)
        self.stream_pos += written
        self.raw_stream.flush()

    cpdef void close(self) except *:
        if self.cctx != NULL:
            self.end_member()

        self._free_ctx()

    cdef void _free_ctx(self) noexcept nogil:
        if self.cctx != NULL:
            LZ4F_freeCompressionContext(self.cctx)
            self.cctx = NULL

        if self.dctx != NULL:
            LZ4F_freeDecompressionContext(self.dctx)
            self.dctx = NULL

        if not self.working_buf.empty():
            self.working_buf.clear()
        self.working_buf_filled = 0u



# noinspection PyAttributeOutsideInit
@cython.auto_pickle(False)
cdef class BrotliStream(CompressingStream):
    """
    __init__(self, raw_stream, quality=11, lgwin=22, lgblock=0)

    Brotli :class:`IOStream` implementation.

    Implementation relies on Google's ``brotli`` Python package, will be ported to native
    C version in a later version.

    :param raw_stream: raw data stream
    :param quality: compression quality (higher quality means better compression, but less speed)
    :type quality: int
    :param lgwin: Base 2 logarithm of the sliding window size in the range 16 to 24
    :type lgwin: int
    :param lgblock: Base 2 logarithm of the maximum input block size in the range 16 to 24
                    (will be set based on quality of value is 0)
    :type lgblock: int
    """
    def __init__(self, *args, **kwargs):
        pass

    def __cinit__(self, raw_stream, size_t quality=11, size_t lgwin=22, size_t lgblock=0):
        self.raw_stream = raw_stream
        self.quality = quality
        self.lgwin = lgwin
        self.lgblock = lgblock
        self.stream_state = CompressingStreamState.UNINIT
        self.compressor = None

    cpdef size_t begin_member(self):
        pass

    cpdef size_t end_member(self):
        pass

    cdef size_t read_(self, char* out, size_t size) except -1:
        if self.stream_state == CompressingStreamState.COMPRESSING:
            raise StreamError('Compression in progress.')
        elif self.stream_state == 0:
            self.compressor = brotli.Decompressor()
            self.stream_state = CompressingStreamState.DECOMPRESSING

        cdef string tmp_buffer
        tmp_buffer.resize(size)
        cdef size_t bytes_read = self.raw_stream.read_(tmp_buffer.data(), size)
        tmp_buffer.resize(bytes_read)

        try:
            self.working_buf.append(<string>self.compressor.process(tmp_buffer))
        except brotli.error:
            raise StreamError(f'Not a valid Brotli stream')

        bytes_read = min(self.working_buf.size(), size)
        memcpy(out, self.working_buf.data(), bytes_read)
        self.working_buf = self.working_buf.substr(bytes_read)
        return bytes_read


    cdef size_t write_(self, const char* data, size_t size) except -1:
        if self.stream_state == CompressingStreamState.DECOMPRESSING:
            raise StreamError('Decompression in progress.')
        elif self.stream_state == 0:
            self.compressor = brotli.Compressor()
            self.stream_state = CompressingStreamState.COMPRESSING
        cdef bytes compressed = self.compressor.process(<bytes>data[:size])
        return self.raw_stream.write_(<char*>compressed, len(compressed))

    cpdef size_t tell(self) except -1:
        return self.raw_stream.tell()

    cpdef void seek(self, size_t offset) except *:
        self.raw_stream.seek(offset)

    cpdef void flush(self)  except *:
        if self.stream_state == CompressingStreamState.COMPRESSING:
            compressed = self.compressor.flush()
            self.raw_stream.write_(<char*>compressed, len(compressed))
            self.raw_stream.flush()

    cpdef void close(self) except *:
        if self.stream_state == CompressingStreamState.COMPRESSING:
            compressed = self.compressor.finish()
            self.raw_stream.write_(<char*>compressed, len(compressed))
        self.compressor = None
        self.stream_state = CompressingStreamState.UNINIT
        self.working_buf.clear()


# noinspection PyAttributeOutsideInit
@cython.auto_pickle(False)
cdef class BufferedReader:
    """
    __init__(self, stream, buf_size=65536, negotiate_stream=True)

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

    def __cinit__(self, IOStream stream, size_t buf_size=65536, bint negotiate_stream=True):
        self.stream = stream
        self.buf = string()
        self.buf.resize(max(4096u, buf_size))
        self.buf_view = string_view()
        self.limited_buf_view = string_view()
        self.last_read_size = self.buf.size()
        self.limit = strnpos
        self.limit_consumed = 0
        self.stream_is_compressed = isinstance(stream, CompressingStream)
        self.stream_started = False
        self.negotiate_stream = negotiate_stream and not self.stream_is_compressed

    # noinspection PyAttributeOutsideInit
    cdef bint detect_stream_type(self) except 0:
        """
        Try to auto-detect stream type (GZip, LZ4, or uncompressed).
        """
        if not self.negotiate_stream or self.stream_started:
            return True

        self.negotiate_stream = False
        self._fill_buf()

        if self.buf_view.size() > 2 and self.buf_view[0] == <char>0x1f and self.buf_view[1] == <char>0x8b:
            self.stream = GZipStream.__new__(GZipStream, self.stream)
            (<GZipStream> self.stream).prepopulate(False, <string>self.buf_view)
        elif self.buf_view.size() > 4 and memcmp(self.buf_view.data(), <const char*>b'\x04\x22\x4d\x18', 4) == 0:
            self.stream = LZ4Stream.__new__(LZ4Stream, self.stream)
            (<LZ4Stream> self.stream).prepopulate(<string>self.buf_view)
        elif self.buf_view.size() > 5 and memcmp(self.buf_view.data(), <const char*>b'WARC/', 5) == 0:
            # Stream is uncompressed: bail out, don't mess with buffers
            self.stream_is_compressed = False
            return True
        else:
            self.stream_is_compressed = False
            raise StreamError('Not a valid WARC stream')

        self.stream_is_compressed = isinstance(self.stream, CompressingStream)
        self.buf_view.remove_prefix(self.buf_view.size())
        self.stream_started = False
        return True

    cdef bint _fill_buf(self) except -1:
        """
        Refill internal buffer.
        
        :return: ``True`` if refill was successful, ``False`` otherwise (EOF)
        """
        self.stream_started = True
        if self.buf_view.size() > 0:
            return True if self.limit == strnpos else self.limit > self.limit_consumed

        cdef size_t bytes_read = self.stream.read_(self.buf.data(), self.buf.size())
        self.buf_view = string_view(self.buf.data(), bytes_read)

        if self.buf_view.size() == 0:
            return False
        elif self.limit != strnpos:
            return self.limit > self.limit_consumed

        return True

    cdef string_view* _get_buf(self) noexcept nogil:
        """
        Get buffer contents. Does take a set limit into account.
        
        Returns a pointer, since Cython does not support returning lvalue references.
        
        :return: available buffer contents
        """
        if self.limit == strnpos:
            return &self.buf_view

        cdef size_t remaining = self.limit - self.limit_consumed
        self.limited_buf_view = string_view(self.buf_view.data(), self.buf_view.size())
        if self.limited_buf_view.size() > remaining:
            self.limited_buf_view.remove_suffix(self.limited_buf_view.size() - remaining)
        return &self.limited_buf_view

    cdef size_t _consume_buf(self, size_t size) noexcept nogil:
        """
        Consume up to ``size`` bytes from internal buffer. Takes a set limit into account.
        
        :param size: number of bytes to read
        :return: bytes consumed
        """
        cdef size_t consumed
        if self.limit == strnpos and size >= self.buf_view.size():
            consumed = self.buf_view.size()
            self.buf_view.remove_prefix(consumed)
            return consumed

        if self.limit != strnpos:
            if size > self.limit - self.limit_consumed:
                size = self.limit - self.limit_consumed
            self.limit_consumed += size

        if size > self.buf_view.size():
            size = self.buf_view.size()
        self.buf_view.remove_prefix(size)
        return size

    cdef inline void set_limit(self, size_t offset) noexcept nogil:
        """
        Set a stream limit in bytes. Any read beyond this limit will act as if the stream reached EOF.
        A set limit can be reset by calling :meth:`reset_limit()`.
        
        :param offset: limit in bytes
        """
        self.limit = offset
        self.limit_consumed = 0

    cdef inline void reset_limit(self) noexcept nogil:
        """Reset any previously set stream limit."""
        self.limit = strnpos

    cpdef bytes read(self, size_t size=strnpos):
        """
        read(self, size=-1)
        
        Read up to ``size`` bytes from the input stream.
        
        :param size: number of bytes to read (default means read remaining stream)
        :type size: int
        :return: consumed buffer contents as bytes (or empty string if EOF)
        :rtype: bytes
        """
        cdef string data_read
        data_read.reserve(size if size != strnpos else self.last_read_size)
        cdef size_t remaining = size
        cdef string_view buf_sub
        cdef string_view* buf = NULL

        while (size == strnpos or data_read.size() < size) and self._fill_buf():
            buf = self._get_buf()
            remaining = size - data_read.size() if size != strnpos else strnpos
            buf_sub = self._get_buf().substr(0, remaining)
            data_read.append(buf_sub.data(), buf_sub.size())
            self._consume_buf(buf_sub.size())
        self.last_read_size = data_read.size()
        return data_read

    cpdef string readline(self, bint crlf=True, size_t max_line_len=8192) except *:
        """
        readline(self, crlf=True, max_line_len=8192)
        
        Read a single line from the input stream.
        
        :param crlf: whether lines are separated by CRLF or LF
        :type crlf: bool
        :param max_line_len: maximum line length (longer lines will still be consumed, but the return
                             value will not be larger than this)
        :type max_line_len: int
        :return: line contents (or empty string if EOF)
        :rtype: bytes
        """

        cdef string_view* buf = NULL
        cdef size_t capacity_remaining = max_line_len
        cdef bint last_was_cr = False

        cdef string line
        line.reserve(128)

        cdef size_t lf_pos = 0
        cdef char* lf_ptr = NULL

        while lf_ptr == NULL and self._fill_buf():
            buf = self._get_buf()

            if crlf:
                if last_was_cr and buf.front() == b'\n':
                    line.push_back(b'\n')
                    self._consume_buf(1)
                    return line
                last_was_cr = False

                lf_ptr = <char*>memchr(buf.data(), <int>b'\r', buf.size())
                while lf_ptr != NULL:
                    lf_pos = lf_ptr - buf.data() + 1
                    if lf_pos == buf.size():
                        last_was_cr = True
                        lf_ptr = NULL
                        break
                    preinc(lf_ptr)
                    if lf_ptr[0] == b'\n':
                        break
                    lf_ptr = <char*>memchr(lf_ptr, <int>b'\r', buf.size() - lf_pos)
            else:
                lf_ptr = <char*>memchr(buf.data(), <int>b'\n', buf.size())
                if lf_ptr != NULL:
                    lf_pos = lf_ptr - buf.data()

            if lf_ptr == NULL:
                lf_pos = buf.size() - 1

            preinc(lf_pos)
            line.append(buf.data(), min(lf_pos, capacity_remaining))
            capacity_remaining = max_line_len - line.size()
            self._consume_buf(lf_pos)

        return line

    cpdef size_t tell(self) except -1:
        """
        tell(self)
        
        Offset on the input stream.
        
        :return: offset
        :rtype: int
        """
        if not self.stream_is_compressed and not self.stream_started:
            return 0

        if self.limit != strnpos:
            return self.limit_consumed

        if self.stream_is_compressed:
            # Use position as returned by the decompressor.
            # The BufferedReader position inside a decompressed stream is meaningless.
            return self.stream.tell()

        return self.stream.tell() - self.buf_view.size()

    cpdef size_t consume(self, size_t size=strnpos) except -1:
        """
        consume(self, size=-1)
        
        Consume up to ``size`` bytes from the input stream without allocating a buffer for it.
        
        :param size: number of bytes to read (default means read remaining stream)
        :type size: int
        :return: number of bytes consumed
        :rtype: int
        """
        cdef string_view* buf
        cdef size_t consumed = 0

        while size > consumed and self._fill_buf():
            buf = self._get_buf()
            if buf.empty():
                break

            if size != strnpos:
                consumed += self._consume_buf(size - consumed)
            else:
                consumed += self._consume_buf(buf.size())

        return consumed

    cpdef void close(self) except *:
        """
        close(self)
        
        Close stream.
        """
        if self.stream is not None:
            self.stream.close()


def _buf_reader_py_test_detect_stream_type(BufferedReader buf):
    """
    _buf_reader_py_test_detect_stream_type(buf):

    Test interface for :meth:`BufferedReader.detect_stream_type`
    """
    buf.detect_stream_type()


def _buf_reader_py_test_set_limit(BufferedReader buf, size_t limit):
    """
    _buf_reader_py_test_detect_set_limit(buf, limit):

    Test interface for :meth:`BufferedReader.set_limit`
    """
    buf.set_limit(limit)


def _buf_reader_py_test_reset_limit(BufferedReader buf):
    """
    _buf_reader_py_test_reset_limit(buf, limit):

    Test interface for :meth:`BufferedReader.reset_limit`
    """
    buf.reset_limit()
