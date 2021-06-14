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

from io import RawIOBase
import zlib

from libc.stdio cimport fclose, FILE, fflush, fopen, fread, fseek, ftell, fwrite, SEEK_SET
from libcpp.string cimport string

cdef extern from "<stdio.h>" nogil:
    int fileno(FILE*)

cdef extern from "<zlib.h>" nogil:
    int gzclose(gzFile fp)
    gzFile gzopen(const char* path, const char* mode)
    gzFile gzdopen(int fd, const char* mode)
    int gzread(gzFile fp, void* buf, unsigned long n)
    char* gzerror(gzFile fp, int* errnum)


cdef size_t BUFF_SIZE = 16384
cdef size_t strnpos = -1

cdef class IOStream:
    cdef void close(self):
        pass

    cdef bint flush(self):
        pass

    cdef size_t tell(self):
        pass

    cdef int errnum(self):
        return 0

    cdef string error(self):
        pass

    cdef string read(self, size_t size=BUFF_SIZE):
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

    cdef bint flush(self):
        fflush(self.fp)

    cdef size_t tell(self):
        return ftell(self.fp)

    cdef void seek(self, size_t offset):
        fseek(self.fp, offset, SEEK_SET)

    cdef string read(self, size_t size=BUFF_SIZE):
        cdef string buf
        buf.resize(size)
        cdef size_t c = fread(&buf[0], sizeof(char*), size, self.fp)
        buf.resize(c)
        return buf

    cdef size_t write(self, const char* data, size_t size):
        return fwrite(data, sizeof(char*), size, self.fp)


cdef class GZipStream(IOStream):
    def __init__(self):
        self.fp = NULL
        self.py_stream = None   # type: RawIOBase
        self.decomp_obj = None  # type: zlib.Decompress
        self.unused_data = string(b'')

    def __dealloc__(self):
        self.close()

    cpdef void open(self, const char* path, const char* mode=b'rb'):
        self.fp = gzopen(path, mode)

    cpdef void open_stream(self, stream, const char* mode=b'rb'):
        if isinstance(stream, FileStream):
            self.fp = gzdopen(fileno((<FileStream>stream).fp), mode)
        else:
            self.py_stream = stream

    cdef void close(self):
        if self.fp:
            gzclose(self.fp)
            self.fp = NULL

    cdef int errnum(self):
        cdef int errnum
        gzerror(self.fp, &errnum)
        return errnum

    cdef string error(self):
        cdef int errnum
        return gzerror(self.fp, &errnum)

    cpdef string read(self, size_t size=BUFF_SIZE):
        cdef string buf
        cdef unsigned long l = 0

        if self.fp != NULL:
            buf.resize(size)
            l = gzread(self.fp, &buf[0], buf.size())
            return buf.substr(0, l)

        elif self.py_stream is not None:
            if not self.decomp_obj or self.decomp_obj.eof:
                # New member, so we need a new decompressor
                self.decomp_obj = zlib.decompressobj(16 + zlib.MAX_WBITS)

            data = self.unused_data
            if <unsigned long>len(data) < size:
                data += self.py_stream.read(size)

            if not data:
                return b''

            decomp_data = self.decomp_obj.decompress(data)
            self.unused_data =  self.decomp_obj.unconsumed_tail + self.decomp_obj.unused_data
            return decomp_data


cdef class BufferedReader:
    def __init__(self, IOStream stream):
        self.stream = stream
        self.buf = string(b'')

    cdef bint fill_buf(self, size_t buf_size=BUFF_SIZE):
        if self.buf.size() >= buf_size:
            return True

        self.buf.append(self.stream.read(buf_size))
        return self.buf.size() > 0

    cpdef string read(self, size_t size, size_t buf_size=BUFF_SIZE, bint skip=False):
        cdef string data_read
        cdef size_t missing = size
        while data_read.size() < size and self.fill_buf(buf_size):
            missing = size - data_read.size()
            if not skip:
                data_read.append(self.buf.substr(0, missing))
            self.buf = self.buf.substr(min(self.buf.size(), missing))
        return data_read

    cpdef string readline(self, size_t max_line_len=4096, size_t buf_size=BUFF_SIZE):
        cdef string line

        if not self.fill_buf(buf_size):
            return b''

        cdef size_t capacity_remaining = max_line_len
        cdef size_t pos = self.buf.find(b'\n')
        while pos == strnpos:
            if capacity_remaining > 0:
                line.append(self.buf.substr(0, min(self.buf.size(), capacity_remaining)))
                capacity_remaining -= line.size()

            # Consume rest of line
            self.buf.clear()
            if not self.fill_buf(buf_size):
                break
            pos = self.buf.find(b'\n')

        if not self.buf.empty() and pos != strnpos:
            if capacity_remaining > 0:
                line.append(self.buf.substr(0, min(pos + 1, capacity_remaining)))

            self.buf = self.buf.substr(pos + 1)

        return line


cdef class LimitedBufferedReader(BufferedReader):
    def __init__(self, IOStream stream, size_t max_len):
        super().__init__(stream)
        self.max_len = max_len
        self.len_consumed = 0

    cdef bint fill_buf(self, size_t buf_size=BUFF_SIZE):
        if self.buf.size() >= buf_size:
            return True

        cdef size_t capacity = self.max_len - self.len_consumed
        cdef size_t cur_buf_size = self.buf.size()
        self.buf.append(self.stream.read(min(buf_size, capacity)))
        self.len_consumed += (self.buf.size() - cur_buf_size)
        return self.buf.size() > 0
