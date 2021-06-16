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


from libc.stdio cimport fclose, FILE, fflush, fopen, fread, fseek, ftell, fwrite, SEEK_SET
from libcpp.string cimport string


cdef extern from "<stdio.h>" nogil:
    int fileno(FILE*)


cdef size_t strnpos = -1

cdef class IOStream:
    cdef void close(self):
        pass

    cdef bint flush(self):
        pass

    cdef size_t tell(self):
        pass

    cdef string read(self, size_t size):
        pass

    cdef size_t write(self, char* data, size_t size):
        pass


cdef class FileStream(IOStream):
    def __init__(self):
        self.fp = NULL

    def __dealloc__(self):
        self.close()

    cpdef void open(self, const char* path, const char* mode=b'rb'):
        if self.fp != NULL:
            self.close()
        self.fp = fopen(path, mode)

    cdef void close(self):
        if self.fp != NULL:
            fclose(self.fp)
            self.fp = NULL

    cdef bint flush(self):
        fflush(self.fp)

    cdef size_t tell(self):
        return ftell(self.fp)

    cdef void seek(self, size_t offset):
        fseek(self.fp, offset, SEEK_SET)

    cpdef string read(self, size_t size):
        cdef string buf
        buf.resize(size)
        cdef size_t c = fread(&buf[0], sizeof(char), size, self.fp)
        buf.resize(c)
        return buf

    cdef size_t write(self, const char* data, size_t size):
        return fwrite(data, sizeof(char*), size, self.fp)


cdef class PythonIOStreamAdapter(IOStream):
    cdef object py_stream

    def __init__(self, py_stream):
        self.py_stream = py_stream  # type: RawIOBase

    cdef void close(self):
        self.py_stream.close()

    cdef bint flush(self):
        self.py_stream.close()

    cdef size_t tell(self):
        return self.py_stream.tell()

    cdef void seek(self, size_t offset):
        self.py_stream.seek(offset)

    cdef string read(self, size_t size):
        return self.py_stream.read(size)[:size]

    cdef size_t write(self, const char* data, size_t size):
        return self.py_stream.write(data[:size])


cdef class GZipStream(IOStream):
    def __init__(self, raw_stream):
        if isinstance(raw_stream, IOStream):
            self.raw_stream = raw_stream
        elif isinstance(raw_stream, object):
            self.raw_stream = PythonIOStreamAdapter(raw_stream)
        else:
            raise TypeError('Invalid stream object.')

        self.zst.opaque = Z_NULL
        self.zst.zalloc = Z_NULL
        self.zst.zfree = Z_NULL
        self.zst.next_in = NULL
        self.zst.avail_in = 0
        self.stream_status = Z_STREAM_END

        self.in_buf = string()

    def __dealloc__(self):
        self.close()

    cdef void close(self):
        self.raw_stream.close()

    cpdef string read(self, size_t size):
        if self.zst.avail_in == 0:
            self.in_buf = self.raw_stream.read(size)
            self.zst.next_in = <Bytef*>&self.in_buf[0]
            self.zst.avail_in = self.in_buf.size()

        if self.stream_status == Z_STREAM_END:
            inflateInit2(&self.zst, 16 + MAX_WBITS)

        cdef string out_buf = string(size, <char>0)
        cdef size_t out_buf_size = out_buf.size()
        self.zst.next_out = <Bytef*>&out_buf[0]
        self.zst.avail_out = out_buf.size()

        self.stream_status = inflate(&self.zst, Z_SYNC_FLUSH)
        if self.stream_status == Z_DATA_ERROR or self.stream_status == Z_STREAM_ERROR:
            inflateEnd(&self.zst)
            return string()

        if self.stream_status == Z_STREAM_END:
            inflateEnd(&self.zst)

        out_buf.resize(<char*>self.zst.next_out - <char*>&out_buf[0])
        return out_buf

    cpdef size_t tell(self):
        return self.raw_stream.tell()

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

    cdef inline string_view _get_buf(self):
        cdef string_view v = string_view(self.buf.c_str(), self.buf.size())
        cdef size_t remaining
        if self.limit != strnpos:
            remaining = self.limit - self.limit_consumed
            if v.size() > remaining:
                v.remove_suffix(v.size() - remaining)
        return v

    cdef inline void _consume_buf(self, size_t size):
        if self.limit == strnpos and size >= self.buf.size():
            self.buf.clear()
            return

        if size > self.limit - self.limit_consumed:
            size = self.limit - self.limit_consumed
        self.limit_consumed += size
        strerase(self.buf, 0, size)

    cdef inline void set_limit(self, size_t offset):
        self.limit = offset
        self.limit_consumed = 0

    cdef inline void reset_limit(self):
        self.limit = strnpos

    cpdef string read(self, size_t size):
        cdef string data_read
        cdef size_t missing = size
        while data_read.size() < size and self._fill_buf():
            missing = size - data_read.size()
            data_read.append(<string>self._get_buf().substr(0, missing))
            self._consume_buf(missing)
        return data_read

    cpdef string readline(self, size_t max_line_len=4096):
        cdef string line

        if not self._fill_buf():
            return string()

        cdef size_t capacity_remaining = max_line_len
        cdef string_view buf = self._get_buf()
        cdef size_t pos = buf.find(b'\n')
        while pos == strnpos:
            if capacity_remaining > 0:
                line.append(<string>buf.substr(0, min(buf.size(), capacity_remaining)))
                capacity_remaining -= line.size()

            # Consume rest of line
            self._consume_buf(buf.size())
            if not self._fill_buf():
                break

            buf = self._get_buf()
            pos = buf.find(b'\n')

        if not buf.empty() and pos != strnpos:
            if capacity_remaining > 0:
                line.append(<string>buf.substr(0, min(pos + 1, capacity_remaining)))

            self._consume_buf(pos + 1)

        return line

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

    cpdef size_t tell(self):
        if self.limit != strnpos:
            return self.limit_consumed
        return self.stream_pos
