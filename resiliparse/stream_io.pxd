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
    cdef unused_data

    cpdef void open(self, const char* path, const char* mode=*)
    cpdef void open_from_fstream(self, FileStream fstream, const char* mode=*)
    cpdef open_from_pystream(self, pystream, mode=*)
    cdef void close(self)
    cdef int errnum(self)
    cdef string error(self)
    cpdef string read(self, size_t size=*)


cdef class BufferedReader:
    cdef IOStream stream
    cdef string buf

    cdef bint fill_buf(self, size_t buf_size=*)
    cpdef string read(self, size_t size, size_t buf_size=*, bint skip=*)
    cpdef string readline(self, size_t max_line_len=*, size_t buf_size=*)


cdef class LimitedBufferedReader(BufferedReader):
    cdef size_t max_len
    cdef size_t len_consumed