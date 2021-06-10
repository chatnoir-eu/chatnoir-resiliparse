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
    FILE* fmemopen(void* buf, size_t size, const char* mode);

cdef extern from "<zlib.h>" nogil:
    ctypedef void* gzFile

    int gzclose(gzFile fp)
    gzFile gzopen(const char* path, const char* mode)
    gzFile gzdopen(int fd, const char* mode)
    int gzread(gzFile fp, void* buf, unsigned long n)
    char* gzerror(gzFile fp, int* errnum)

cdef size_t BUFF_SIZE = 16384
cdef size_t STR_NPOS = <size_t> -1


cdef class IOStream:
    def __dealloc__(self):
        self.close()

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
    cdef FILE* fp

    def __init__(self):
        self.fp = NULL

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


cdef class PythonIOStreamAdapter(IOStream):
    cdef py_stream

    def __init__(self, py_stream):
        super().__init__()
        self.py_stream = py_stream  # type: RawIOBase

    cdef void close(self):
        self.py_stream.close()

    cdef bint flush(self):
        self.py_stream.close()

    cdef size_t tell(self):
        return self.py_stream.tell()

    cdef void seek(self, size_t offset):
        self.py_stream.seek(offset)

    cdef string read(self, size_t size=BUFF_SIZE):
        pbuf = self.py_stream.read(size)
        return pbuf[:len(pbuf)]

    cdef size_t write(self, const char* data, size_t size):
        return self.py_stream.write(data[:size])


cdef class GZipStream(IOStream):
    cdef gzFile fp
    cdef py_stream
    cdef decomp_obj

    def __init__(self):
        self.fp = NULL
        self.py_stream = None   # type: RawIOBase
        self.decomp_obj = None  # type: zlib.Decompress

    cpdef void open(self, const char* path, const char* mode=b'rb'):
        self.fp = gzopen(path, mode)

    cpdef void open_from_fstream(self, FileStream fstream, const char* mode=b'rb'):
        self.fp = gzdopen(fileno(fstream.fp), mode)

    cpdef open_from_pystream(self, pystream, mode='rb'):
        if self.fp != NULL:
            self.close()

        self.decomp_obj = zlib.decompressobj(16 + zlib.MAX_WBITS)
        self.py_stream = pystream

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
        elif self.py_stream is not None:
            data = b''
            if self.decomp_obj.unused_data:
                data = self.decomp_obj.unused_data
            if <unsigned long>len(data) < size:
                data += self.py_stream.read(size)
            self.decomp_obj = zlib.decompressobj(16 + zlib.MAX_WBITS)
            return self.decomp_obj.decompress(data)

        return buf.substr(0, l)


cdef class LineParser:
    cdef IOStream stream
    cdef string buf

    def __init__(self, IOStream stream):
        self.stream = stream
        self.buf = string(b'')

    cdef bint _fill_buf(self, size_t buf_size):
        if self.buf.size() >= buf_size:
            return True

        cdef string tmp_buf
        tmp_buf = self.stream.read(buf_size)
        if tmp_buf.empty():
            return False
        self.buf.append(tmp_buf)
        return True

    cdef string unused_data(self):
        return self.buf

    cpdef string readline(self, size_t max_line_len=4096, size_t buf_size=BUFF_SIZE):
        cdef string line

        if not self._fill_buf(buf_size):
            return line

        cdef size_t pos = self.buf.find(b'\n')

        while pos == STR_NPOS and not self.buf.empty():
            if line.size() < max_line_len:
                line.append(self.buf.substr(0u, min(self.buf.size(), max_line_len - line.size())))
            # Consume rest of line
            self.buf.clear()
            if not self._fill_buf(buf_size):
                break
            pos = self.buf.find(b'\n')

        line.append(self.buf.substr(0, min(pos + 1, max_line_len - line.size())))
        self.buf = self.buf.substr(min(pos + 1, self.buf.size() - 1))

        return line
