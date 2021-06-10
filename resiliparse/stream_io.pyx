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

cdef extern from "zlib.h" nogil:
    ctypedef void* gzFile
    ctypedef size_t z_off_t

    int gzclose(gzFile fp)
    gzFile gzopen(char* path, char* mode)
    gzFile gzdopen(int fd, char* mode)
    int gzread(gzFile fp, void* buf, unsigned int n)
    char* gzerror(gzFile fp, int* errnum)

cdef size_t BUFF_SIZE = 16384
cdef size_t STR_NPOS = <size_t> -1


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
    cdef FILE* fp

    def __init__(self):
        self.fp = NULL

    cpdef void open(self, char* path, char* mode=b'rb'):
        self.fp = fopen(path, mode)

    cdef void close(self):
        fclose(self.fp)

    cdef bint flush(self):
        fflush(self.fp)

    cdef size_t tell(self):
        return ftell(self.fp)

    cdef void fseek(self, size_t pos):
        fseek(self.fp, pos, SEEK_SET)

    cdef string read(self, size_t size=BUFF_SIZE):
        cdef string buf
        buf.resize(size)
        cdef size_t c = fread(&buf.front(), sizeof(char*), size, self.fp)
        buf.resize(c)
        return buf

    cdef size_t write(self, char* data, size_t size):
        return fwrite(data, sizeof(char*), size, self.fp)


cdef class GZipStream(IOStream):
    cdef gzFile fp

    def __init__(self):
        self.fp = NULL

    def __dealloc__(self):
        self.close()

    cpdef void open(self, char* path, char* mode=b'rb'):
        self.fp = gzopen(path, mode)

    cpdef void open_from_stream(self, FileStream raw, char* mode=b'rb'):
        self.fp = gzdopen(fileno(raw.fp), mode)

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

    cdef string read(self, size_t size=BUFF_SIZE):
        cdef string buf
        buf.resize(size)
        cdef int l = gzread(self.fp, &buf.front(), size)
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
