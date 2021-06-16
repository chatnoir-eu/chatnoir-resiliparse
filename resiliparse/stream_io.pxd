from libc.stdio cimport FILE
from libcpp.string cimport string


cdef extern from "<zlib.h>" nogil:
    ctypedef unsigned char Bytef
    ctypedef struct z_stream:
        Bytef* next_in
        size_t avail_in
        size_t total_in
        Bytef* next_out
        size_t avail_out
        size_t total_out
        const char* msg
        void* zalloc
        void* zfree
        void* opaque

    const void* Z_NULL
    const int Z_OK
    const int Z_STREAM_END
    const int Z_STREAM_ERROR
    const int Z_DATA_ERROR
    const int Z_SYNC_FLUSH
    const int MAX_WBITS

    ctypedef void* gzFile
    int inflateInit2(z_stream* strm, int window_bits)
    int inflateEnd(z_stream* strm)
    int inflate(z_stream* strm, int flush)

    int gzclose(gzFile fp)
    gzFile gzopen(const char* path, const char* mode)
    gzFile gzdopen(int fd, const char* mode)
    int gzread(gzFile fp, void* buf, unsigned long n)
    size_t gztell(gzFile fp)


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
    cdef string read(self, size_t size)
    cdef size_t write(self, char* data, size_t size)
    cdef size_t tell(self)


cdef class FileStream(IOStream):
    cdef FILE* fp

    cpdef void open(self, const char* path, const char* mode=*)
    cpdef void close(self)
    cdef bint flush(self)
    cdef void seek(self, size_t offset)
    cdef string read(self, size_t size)
    cdef size_t write(self, const char* data, size_t size)
    cdef size_t tell(self)


cdef class GZipStream(IOStream):
    cdef IOStream raw_stream
    cdef string in_buf
    cdef z_stream zst
    cdef int stream_status

    cdef void close(self)
    cdef string read(self, size_t size)
    cdef size_t tell(self)


cdef class BufferedReader:
    cdef IOStream stream
    cdef string buf
    cdef size_t buf_size
    cdef size_t stream_pos
    cdef size_t limit
    cdef size_t limit_consumed

    cdef inline void set_limit(self, size_t offset) nogil
    cdef inline void reset_limit(self) nogil

    cpdef string read(self, size_t size)
    cpdef string readline(self, size_t max_line_len=*)
    cpdef void consume(self, size_t size=*)
    cpdef size_t tell(self)

    cdef bint _fill_buf(self)
    cdef inline string_view _get_buf(self) nogil
    cdef inline void _consume_buf(self, size_t size) nogil
