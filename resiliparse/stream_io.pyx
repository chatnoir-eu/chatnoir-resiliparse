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

    cdef size_t write(self, char* data, size_t size):
        pass

    cdef size_t tell(self):
        pass

    cdef bint flush(self):
        pass

    cdef void close(self):
        pass


cdef class FileStream(IOStream):
    def __init__(self, str filename=None, str mode='rb'):
        self.fp = NULL
        if filename:
            self.open(filename.encode(), mode.encode())

    def __dealloc__(self):
        self.close()

    cpdef void open(self, char* path, char* mode=b'rb'):
        if self.fp != NULL:
            self.close()
        self.fp = fopen(path, mode)

    cdef void seek(self, size_t offset):
        fseek(self.fp, offset, SEEK_SET)

    cdef size_t tell(self):
        return ftell(self.fp)

    cdef string read(self, size_t size):
        cdef string buf
        cdef size_t c
        with nogil:
            buf.resize(size)
            c = fread(&buf[0], sizeof(char), size, self.fp)
            buf.resize(c)
            return buf

    cdef size_t write(self, char* data, size_t size):
        return fwrite(data, sizeof(char), size, self.fp)

    cdef bint flush(self):
        fflush(self.fp)

    cpdef void close(self):
        if self.fp != NULL:
            fclose(self.fp)
            self.fp = NULL


cdef class PythonIOStreamAdapter(IOStream):
    def __init__(self, py_stream):
        self.py_stream = py_stream

    cdef inline size_t tell(self):
        return self.py_stream.tell()

    cdef inline void seek(self, size_t offset):
        self.py_stream.seek(offset)

    cdef inline string read(self, size_t size):
        return self.py_stream.read(size)[:size]

    cdef inline size_t write(self, char* data, size_t size):
        return self.py_stream.write(data[:size])

    cdef inline bint flush(self):
        self.py_stream.flush()

    cdef inline void close(self):
        self.py_stream.close()


cdef IOStream wrap_stream(raw_stream):
    if isinstance(raw_stream, IOStream):
        return raw_stream
    elif isinstance(raw_stream, object) and hasattr(raw_stream, 'read'):
        return PythonIOStreamAdapter(raw_stream)
    else:
        warnings.warn(f'Object of type "{type(raw_stream).__name__}" is not a valid stream.', RuntimeWarning)
        return None


cdef class GZipStream(IOStream):
    def __init__(self, raw_stream):
        self.raw_stream = wrap_stream(raw_stream)
        self.zst.opaque = Z_NULL
        self.zst.zalloc = Z_NULL
        self.zst.zfree = Z_NULL
        self.zst.next_in = NULL
        self.zst.avail_in = 0
        self.stream_status = Z_STREAM_END

        self.in_buf = string()

    def __dealloc__(self):
        self.close()

    cdef size_t tell(self):
        return self.raw_stream.tell()

    cdef string read(self, size_t size):
        if self.zst.avail_in == 0:
            self.in_buf = self.raw_stream.read(size)
            self.zst.next_in = <Bytef*>&self.in_buf[0]
            self.zst.avail_in = self.in_buf.size()

        cdef string out_buf = string(size, <char>0)
        cdef size_t out_buf_size = out_buf.size()
        with nogil:
            if self.stream_status == Z_STREAM_END:
                inflateInit2(&self.zst, 16 + MAX_WBITS)

            self.zst.next_out = <Bytef*>&out_buf[0]
            self.zst.avail_out = out_buf.size()

            self.stream_status = inflate(&self.zst, Z_SYNC_FLUSH)
            if self.stream_status == Z_DATA_ERROR or self.stream_status == Z_STREAM_ERROR:
                inflateEnd(&self.zst)
                return string()

            if self.stream_status == Z_STREAM_END:
                inflateEnd(&self.zst)

            out_buf_size = <char*>self.zst.next_out - <char*>&out_buf[0]
            if out_buf.size() != out_buf_size:
                out_buf.resize(out_buf_size)
            return out_buf

    cdef void close(self):
        self.raw_stream.close()


cdef class LZ4Stream(IOStream):
    def __init__(self, raw_stream):
        self.raw_stream = wrap_stream(raw_stream)
        self.dctx = NULL
        self.in_buf = string()

    def __dealloc__(self):
        self.close()

    cdef size_t tell(self):
        return self.raw_stream.tell()

    cdef string read(self, size_t size):
        if self.in_buf.empty():
            self.in_buf = self.raw_stream.read(size)
            if self.in_buf.empty():
                # EOF
                self._free_ctx()
                return string()

        cdef size_t in_buf_size
        cdef string out_buf
        cdef size_t out_buf_size
        cdef size_t ret

        with nogil:
            in_buf_size = self.in_buf.size()
            out_buf.resize(size)
            out_buf_size = out_buf.size()

            if self.dctx == NULL:
                LZ4F_createDecompressionContext(&self.dctx, LZ4F_VERSION)

            ret = LZ4F_decompress(self.dctx, &out_buf[0], &out_buf_size, &self.in_buf[0], &in_buf_size, NULL)
            while ret != 0 and out_buf_size == 0 and not LZ4F_isError(ret):
                with gil:
                    self.in_buf = self.raw_stream.read(size)
                if self.in_buf.empty():
                    # EOF
                    self._free_ctx()
                    break
                in_buf_size = self.in_buf.size()
                out_buf_size = out_buf.size()
                ret = LZ4F_decompress(self.dctx, &out_buf[0], &out_buf_size, &self.in_buf[0], &in_buf_size, NULL)

            if self.in_buf.size() == in_buf_size:
                self.in_buf.clear()
            else:
                self.in_buf = self.in_buf.substr(in_buf_size)

            if out_buf.size() != out_buf_size:
                out_buf.resize(out_buf_size)
            return out_buf

    cdef void close(self):
        self._free_ctx()
        self.raw_stream.close()

    cdef void _free_ctx(self) nogil:
        if self.dctx != NULL:
            LZ4F_freeDecompressionContext(self.dctx)
            self.dctx = NULL


cdef class BufferedReader:
    def __init__(self, IOStream stream, size_t buf_size=16384):
        self.stream = stream
        self.buf_size = max(1024u, buf_size)
        self.buf = string()
        self.stream_pos = 0
        self.limit = strnpos
        self.limit_consumed = 0

    cdef bint _fill_buf(self):
        if self.buf.size() >= self.buf_size / 8:
            return True if self.limit == strnpos else self.limit > self.limit_consumed

        self.stream_pos = self.stream.tell()
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

    cpdef string readline(self, size_t max_line_len=4096):
        cdef string line

        if not self._fill_buf():
            return string()

        cdef size_t capacity_remaining = max_line_len
        cdef string_view buf = self._get_buf()
        cdef size_t pos = buf.find(b'\n')

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
                pos = buf.find(b'\n')

            if not buf.empty() and pos != strnpos:
                if capacity_remaining > 0:
                    line.append(<string>buf.substr(0, min(pos + 1, capacity_remaining)))

                self._consume_buf(pos + 1)

            return line

    cpdef size_t tell(self):
        if self.limit != strnpos:
            return self.limit_consumed
        return self.stream_pos

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
