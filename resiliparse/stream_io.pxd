from libc.stdio cimport FILE
from libcpp.string cimport string


cdef extern from "<zlib.h>" nogil:
    ctypedef void* gzFile


cdef class IOStream:
    cdef void close(self)
    cdef bint flush(self)
    cdef size_t tell(self)
    cdef int errnum(self)
    cdef string error(self)
    cdef string read(self, size_t size=*)
    cdef size_t write(self, char* data, size_t size)


cdef class FileStream(IOStream):
    cdef FILE* fp

    cpdef void open(self, const char* path, const char* mode=*)
    cdef void close(self)
    cdef bint flush(self)
    cdef size_t tell(self)
    cdef void seek(self, size_t offset)
    cdef string read(self, size_t size=*)
    cdef size_t write(self, const char* data, size_t size)


cdef class PythonIOStreamAdapter(IOStream):
    cdef py_stream

    cdef void close(self)
    cdef bint flush(self)
    cdef size_t tell(self)
    cdef void seek(self, size_t offset)
    cdef string read(self, size_t size=*)
    cdef size_t write(self, const char* data, size_t size)


cdef class GZipStream(IOStream):
    cdef gzFile fp
    cdef py_stream
    cdef decomp_obj

    cpdef void open(self, const char* path, const char* mode=*)
    cpdef void open_from_fstream(self, FileStream fstream, const char* mode=*)
    cpdef open_from_pystream(self, pystream, mode=*)
    cdef void close(self)
    cdef int errnum(self)
    cdef string error(self)
    cpdef string read(self, size_t size=*)


cdef class LineParser:
    cdef IOStream stream
    cdef string buf

    cdef bint _fill_buf(self, size_t buf_size)
    cdef string unused_data(self)
    cpdef string readline(self, size_t max_line_len=*, size_t buf_size=*)
