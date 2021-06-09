# distutils: language = c++

from libcpp.string cimport string

cdef extern from "zlib.h":
    ctypedef void *gzFile
    ctypedef size_t z_off_t

    int gzclose(gzFile fp)
    gzFile gzopen(char* path, char* mode)
    int gzread(gzFile fp, void* buf, unsigned int n)
    char* gzerror(gzFile fp, int* errnum)

cdef size_t BUFF_SIZE = 16384

cdef class CompressedStream:
    cdef void open(self, string file, string mode):
        pass

    cdef void close(self):
        pass

    cdef int errnum(self):
        return 0

    cdef string error(self):
        pass

    cdef string read(self, size_t size=BUFF_SIZE):
        pass

cdef class GZipStream(CompressedStream):
    cdef gzFile fp

    def __init__(self, string filename, string mode):
        self.fp = NULL
        self.open(filename, mode)

    def __dealloc__(self):
        self.close()

    cdef void open(self, string file, string mode):
        self.fp = gzopen(file.c_str(), mode.c_str())

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
    cdef CompressedStream stream
    cdef string buf

    def __init__(self, CompressedStream stream):
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

    cpdef string readline(self, size_t max_line_len=4096, size_t buf_size=BUFF_SIZE):
        cdef string line
        cdef size_t pos
        cdef size_t npos = <size_t>-1
        cdef string tmp_buf

        if not self._fill_buf(buf_size):
            return line

        pos = self.buf.find(b'\n')
        while pos == npos and not self.buf.empty():
            if line.size() < max_line_len:
                line.append(self.buf.substr(0u, min(self.buf.size(), max_line_len - line.size())))
            # Consume rest of line
            self.buf.clear()
            if not self._fill_buf(buf_size):
                break
            pos = self.buf.find(b'\n')

        line.append(self.buf.substr(0u, min(pos + 1, max_line_len - line.size())))
        self.buf = self.buf.substr(min(pos + 1, self.buf.size() - 1))

        return line
