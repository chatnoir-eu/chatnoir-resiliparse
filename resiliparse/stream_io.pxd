from libc.stdio cimport FILE
from libcpp.string cimport string


cdef extern from "<zlib.h>" nogil:
    ctypedef void* gzFile


cdef extern from "<string_view>" namespace "std" nogil:
    cdef cppclass string_view:
        string_view()
        string_view(const string_view& other)
        string_view(const char* s, size_t count)
        bint empty() const
        size_t size() const
        string_view substr(size_t pos, size_t count) const
        string_view substr(size_t pos) const
        string_view substr() const
        size_t find(const char* s, size_t pos)
        size_t find(const char* s)
        string_view remove_prefix(size_t n)
        string_view remove_suffix(size_t n)


cdef extern from * nogil:
    '''
    #include <string>
    inline std::string& strerase(std::string& s, size_t index, size_t count) {
        return s.erase(index, count);
    }
    '''
    string& strerase(string& s, size_t index, size_t count)


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


cdef class GZipStream(IOStream):
    cdef gzFile fp
    cdef py_stream
    cdef decomp_obj
    cdef unused_data

    cpdef void open(self, const char* path, const char* mode=*)
    cpdef void open_stream(self, stream, const char * mode=*)
    cdef void close(self)
    cdef int errnum(self)
    cdef string error(self)
    cpdef string read(self, size_t size=*)


cdef class BufferedReader:
    cdef IOStream stream
    cdef string buf
    cdef size_t limit
    cdef size_t limit_consumed

    cdef inline void set_limit(self, size_t offset)
    cdef inline void reset_limit(self)
    cpdef string read(self, size_t size, size_t buf_size=*)
    cpdef string readline(self, size_t max_line_len=*, size_t buf_size=*)
    cpdef void consume(self, size_t size=*, size_t buf_size=*)

    cdef bint _fill_buf(self, size_t buf_size=*)
    cdef inline string_view _get_buf(self)
    cdef inline void _consume_buf(self, size_t size)
