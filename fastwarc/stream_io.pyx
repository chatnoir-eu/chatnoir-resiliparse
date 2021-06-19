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


from libc.stdio cimport fclose, fflush, fopen, fread, fseek, ftell, fwrite, SEEK_SET
from libcpp.string cimport string

import warnings


cdef size_t strnpos = -1


cdef class IOStream:
    cdef string read(self, size_t size):
        pass

    cdef size_t write(self, const char* data, size_t size):
        pass

    cdef size_t tell(self):
        pass

    cdef void flush(self):
        pass

    cdef void close(self):
        pass


cdef class BytesIOStream(IOStream):
    def __init__(self, bytes initial_data=b''):
        self.pos = 0
        self.buffer = initial_data

    cdef inline size_t tell(self):
        return self.pos

    cdef inline void seek(self, size_t offset):
        self.pos = min(self.buffer.size(), self.pos + offset)

    cdef inline string read(self, size_t size):
        if self.pos >= self.buffer.size():
            return string()
        cdef string substr = self.buffer.substr(self.pos, size)
        self.seek(self.pos + size)
        return substr

    cdef inline size_t write(self, const char* data, size_t size):
        self.pos += size
        self.buffer.append(data, size)
        return size

    cdef inline void close(self):
        self.buffer.clear()

    cdef inline string getvalue(self):
        return self.buffer

cdef class FileStream(IOStream):
    def __init__(self, str filename=None, str mode='rb'):
        self.fp = NULL
        if filename:
            self.open(filename.encode(), mode.encode())

    def __dealloc__(self):
        self.close()

    cpdef bint open(self, char* path, char* mode=b'rb') except 0:
        if self.fp != NULL:
            self.close()

        self.fp = fopen(path, mode)
        if self.fp == NULL:
            raise FileNotFoundError(f"No such file or directory: '{path.decode()}'")
        return True

    cdef void seek(self, size_t offset):
        fseek(self.fp, offset, SEEK_SET)

    cdef size_t tell(self):
        return ftell(self.fp)

    cdef string read(self, size_t size):
        cdef string buf
        cdef size_t c
        with nogil:
            buf.resize(size)
            c = fread(buf.data(), sizeof(char), size, self.fp)
            buf.resize(c)
            return buf

    cdef size_t write(self, const char* data, size_t size):
        return fwrite(data, sizeof(char), size, self.fp)

    cdef void flush(self):
        fflush(self.fp)

    cpdef void close(self):
        if self.fp != NULL:
            fclose(self.fp)
            self.fp = NULL


cdef class PythonIOStreamAdapter(IOStream):
    def __init__(self, py_stream):
        self.py_stream = py_stream


cdef IOStream wrap_stream(raw_stream):
    if isinstance(raw_stream, IOStream):
        return raw_stream
    elif isinstance(raw_stream, object) and hasattr(raw_stream, 'read'):
        return PythonIOStreamAdapter(raw_stream)
    else:
        warnings.warn(f'Object of type "{type(raw_stream).__name__}" is not a valid stream.', RuntimeWarning)
        return None



cdef class CompressingStream(IOStream):
    cdef size_t begin_member(self):
        return 0

    cdef size_t end_member(self):
        return 0


cdef class GZipStream(CompressingStream):
    def __init__(self, raw_stream, compression_level=Z_BEST_COMPRESSION):
        self.raw_stream = wrap_stream(raw_stream)
        self.member_started = False
        self.working_buf = string()
        self.initialized = False
        self.stream_read_status = Z_STREAM_END
        self.stream_pos = self.raw_stream.tell()
        self.compression_level = compression_level

    def __dealloc__(self):
        self.close()

    cdef size_t tell(self):
        if self.stream_read_status != Z_STREAM_END:
            return self.stream_pos
        return self.raw_stream.tell()

    cdef void _init_z_stream(self, bint deflate) nogil:
        if self.initialized:
            return

        self.zst.opaque = Z_NULL
        self.zst.zalloc = Z_NULL
        self.zst.zfree = Z_NULL
        self.zst.next_in = NULL
        self.zst.avail_in = 0
        self.zst.next_out = NULL
        self.zst.avail_out = 0
        self.stream_read_status = Z_STREAM_END
        self.working_buf.clear()

        if deflate:
            deflateInit2(&self.zst, self.compression_level, Z_DEFLATED, 16 + MAX_WBITS, 9, Z_DEFAULT_STRATEGY)
        else:
            inflateInit2(&self.zst, 16 + MAX_WBITS)
        self.initialized = True

    cdef void _free_z_stream(self) nogil:
        if not self.initialized:
            return
        inflateEnd(&self.zst)
        self.working_buf.clear()
        self.initialized = False

    cdef bint _reset_working_buf(self, size_t size) nogil:
        with gil:
            self.working_buf = self.raw_stream.read(size)
            if self.working_buf.empty():
                # EOF
                self._free_z_stream()
                self.stream_pos = self.raw_stream.tell()
                return False

        self.zst.next_in = <Bytef *> self.working_buf.data()
        self.zst.avail_in = self.working_buf.size()
        return True

    cdef string read(self, size_t size):
        if self.member_started:
            # Compression in progress
            return string()

        cdef string out_buf
        cdef size_t written_so_far

        if not self.initialized:
            self._init_z_stream(False)

        # with nogil:
        if self.zst.avail_in == 0 or self.working_buf.empty():
            if not self._reset_working_buf(size):
                return string()

            self.zst.next_in = <Bytef*>self.working_buf.data()
            self.zst.avail_in = self.working_buf.size()

        out_buf = string(size * 8, <char>0)
        self.zst.next_out = <Bytef*>out_buf.data()
        self.zst.avail_out = out_buf.size()

        written_so_far = self.zst.total_out
        self.stream_read_status = inflate(&self.zst, Z_SYNC_FLUSH)

        while self.stream_read_status == Z_OK or self.stream_read_status == Z_BUF_ERROR:
            if self.stream_read_status == Z_BUF_ERROR:
                if self.zst.total_out - written_so_far > 0:
                    # Some output has been produced, that's good enough
                    break

                if self.zst.avail_out == 0:
                    # Grow output buffer if no space left
                    out_buf.resize(out_buf.size() + 4096u)
                    self.zst.next_out = <Bytef*>out_buf.data() + self.zst.total_out - written_so_far
                    self.zst.avail_out += 4096u

                # Input buffer starved and no output produced yet, so we need to read more from stream
                elif self.zst.avail_in == 0 and not self._reset_working_buf(size):
                    return string()

            # There is more, call again
            self.stream_read_status = inflate(&self.zst, Z_SYNC_FLUSH)

        # Error
        if self.stream_read_status < 0 and self.stream_read_status != Z_BUF_ERROR:
            self._free_z_stream()
            return string()

        written_so_far = self.zst.total_out - written_so_far
        if out_buf.size() != written_so_far:
            out_buf.resize(written_so_far)

        if self.stream_read_status == Z_STREAM_END:
            # Member end
            self.stream_pos = self.raw_stream.tell() - (self.zst.next_in - <Bytef*>self.working_buf.data())
            inflateReset(&self.zst)

        if out_buf.empty():
            return self.read(size)

        return out_buf

    cdef size_t write(self, const char* data, size_t size):
        if self.stream_read_status != Z_STREAM_END:
            # Decompression in progress
            return 0

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
                    # Out buffer fully consumed, but in buffer still holding data
                    self.working_buf.append(b'\0', 1024)
                    self.zst.next_out = <Bytef*>&self.working_buf.back() - 1024
                    self.zst.avail_out = 1024

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
            # Need larger output buffer (unlikely to ever happen at this point)
            self.working_buf.resize(self.working_buf.size() + 1024)
            self.zst.next_out = <Bytef*>&self.working_buf.back() - 1024
            self.zst.avail_out = 1024
            status = deflate(&self.zst, Z_FINISH)

        cdef size_t written = self.zst.total_out - written_so_far
        deflateReset(&self.zst)
        self.member_started = False

        if written == 0:
            return 0
        return self.raw_stream.write(self.working_buf.data(), written)

    cdef void flush(self):
        self.end_member()
        self.raw_stream.flush()

    cdef void close(self):
        self.end_member()
        self._free_z_stream()
        self.raw_stream.close()


cdef class LZ4Stream(CompressingStream):
    def __init__(self, raw_stream, compression_level=LZ4HC_CLEVEL_MAX, favor_dec_speed=True):
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

    cdef size_t tell(self):
        if self.dctx != NULL:
            return self.stream_pos
        return self.raw_stream.tell()

    cdef string read(self, size_t size):
        if self.cctx != NULL:
            # Decompression in progress
            return string()

        if self.working_buf.empty():
            self.working_buf = self.raw_stream.read(size)
            if self.working_buf.empty():
                # EOF
                self._free_ctx()
                return string()

        cdef string out_buf
        cdef size_t ret, in_buf_size, out_buf_size, working_buf_size
        with nogil:
            working_buf_size = self.working_buf.size()
            out_buf.resize(size)
            out_buf_size = out_buf.size()

            if self.dctx == NULL:
                LZ4F_createDecompressionContext(&self.dctx, LZ4F_VERSION)

            ret = LZ4F_decompress(self.dctx, out_buf.data(), &out_buf_size,
                                  self.working_buf.data(), &working_buf_size, NULL)

            while ret != 0 and out_buf_size == 0 and not LZ4F_isError(ret):
                with gil:
                    self.working_buf = self.raw_stream.read(size)
                if self.working_buf.empty():
                    # EOF
                    self._free_ctx()
                    break
                working_buf_size = self.working_buf.size()
                out_buf_size = out_buf.size()
                ret = LZ4F_decompress(self.dctx, out_buf.data(), &out_buf_size,
                                      self.working_buf.data(), &working_buf_size, NULL)

            if ret == 0:
                with gil:
                    # Update stream position for tell() on frame boundaries
                    self.stream_pos = self.raw_stream.tell() - self.working_buf.size() + working_buf_size + 1

            if self.working_buf.size() == working_buf_size:
                # Buffer fully consumed
                self.working_buf.clear()
            else:
                self.working_buf = self.working_buf.substr(working_buf_size)

            if out_buf.size() != out_buf_size:
                out_buf.resize(out_buf_size)

        if out_buf.empty() and not LZ4F_isError(ret):
            # Everything OK, but no output produced yet
            return self.read(size)

        return out_buf

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

    cdef size_t write(self, const char* data, size_t size):
        if self.dctx != NULL:
            # Compression in progress
            return 0

        cdef size_t buf_needed, written
        cdef size_t header_bytes_written = self.begin_member()
        with nogil:
            buf_needed = max(4096u, LZ4F_compressBound(size, &self.prefs))
            if self.working_buf.size() < buf_needed or self.working_buf.size() / 8 > buf_needed:
                self.working_buf.resize(buf_needed)

            written = LZ4F_compressUpdate(self.cctx, self.working_buf.data(), self.working_buf.size(),
                                          data, size, NULL)
        return self.raw_stream.write(self.working_buf.data(), written) + header_bytes_written

    cdef void flush(self):
        cdef size_t written
        cdef size_t buf_needed
        if self.cctx != NULL:
            buf_needed = LZ4F_compressBound(0, &self.prefs)
            if self.working_buf.size() < buf_needed:
                self.working_buf.resize(buf_needed)

            written = LZ4F_flush(self.cctx, self.working_buf.data(), self.working_buf.size(), NULL)
            self.raw_stream.write(self.working_buf.data(), written)
        self.raw_stream.flush()

    cdef void close(self):
        if self.cctx != NULL:
            self.end_member()

        self._free_ctx()
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


cdef class BufferedReader:
    def __init__(self, IOStream stream, size_t buf_size=16384):
        self.stream = stream
        self.buf_size = max(1024u, buf_size)
        self.buf = string()
        self.limit = strnpos
        self.limit_consumed = 0

    cdef bint _fill_buf(self):
        if self.buf.size() > 1024u:
            return True if self.limit == strnpos else self.limit > self.limit_consumed

        self.buf.append(self.stream.read(self.buf_size))
        return self.buf.size() > 0 if self.limit == strnpos else self.limit > self.limit_consumed

    cdef inline string_view _get_buf(self) nogil:
        cdef string_view v = string_view(self.buf.c_str(), self.buf.size())
        cdef size_t remaining
        if self.limit != strnpos:
            remaining = self.limit - self.limit_consumed
            if v.size() > remaining:
                v.remove_suffix(v.size() - remaining)
        return v

    cdef inline void _consume_buf(self, size_t size) nogil:
        if self.limit == strnpos and size >= self.buf.size():
            self.buf.clear()
            return

        if self.limit != strnpos:
            if size > self.limit - self.limit_consumed:
                size = self.limit - self.limit_consumed
            self.limit_consumed += size

        strerase(self.buf, 0, size)

    cdef inline void set_limit(self, size_t offset) nogil:
        self.limit = offset
        self.limit_consumed = 0

    cdef inline void reset_limit(self) nogil:
        self.limit = strnpos

    cpdef string read(self, size_t size=strnpos):
        cdef string data_read
        cdef size_t missing = size
        cdef string_view buf_sub

        while (size == strnpos or data_read.size() < size) and self._fill_buf():
            missing = size - data_read.size()
            buf_sub = self._get_buf().substr(0, missing)
            data_read.append(<string>buf_sub)
            self._consume_buf(buf_sub.size())
        return data_read

    cpdef string readline(self, bint crlf=True, size_t max_line_len=4096):
        cdef string line

        if not self._fill_buf():
            return string()

        cdef size_t capacity_remaining = max_line_len
        cdef string_view buf = self._get_buf()
        cdef char* newline_sep = b'\r\n' if crlf else '\n'
        cdef short newline_offset = 1 if crlf else 0
        cdef size_t pos = buf.find(newline_sep) + newline_offset

        with nogil:
            while pos == strnpos:
                if capacity_remaining > 0:
                    line.append(<string>buf.substr(0, min(buf.size(), capacity_remaining)))
                    capacity_remaining -= line.size()

                # Consume rest of line
                self._consume_buf(buf.size())
                with gil:
                    if not self._fill_buf():
                        break

                    buf = self._get_buf()
                pos = buf.find(newline_sep) + newline_offset

            if not buf.empty() and pos != strnpos:
                if capacity_remaining > 0:
                    line.append(<string>buf.substr(0, min(pos + 1, capacity_remaining)))

                self._consume_buf(pos + 1)

        return line

    cpdef size_t tell(self):
        if self.limit != strnpos:
            return self.limit_consumed
        return self.stream.tell() - self.buf.size()

    cpdef void consume(self, size_t size=strnpos):
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

    cpdef void close(self):
        self.stream.close()
