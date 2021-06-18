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
    const int Z_SYNC_FLUSH
    const int Z_NO_FLUSH
    const int Z_FINISH

    const int Z_STREAM_END
    const int Z_BUF_ERROR
    const int Z_STREAM_ERROR
    const int Z_DATA_ERROR

    const int Z_BEST_SPEED
    const int Z_BEST_COMPRESSION
    const int Z_DEFAULT_STRATEGY
    const int Z_DEFLATED
    const int MAX_WBITS

    ctypedef void* gzFile

    int deflateInit2(z_stream* strm, int level, int method, int windowBits, int memLevel, int strategy)
    int deflate(z_stream* strm, int flush)
    int deflateReset(z_stream* strm)
    int deflateEnd(z_stream* strm)
    unsigned long deflateBound(z_stream* strm, unsigned long sourceLen)

    int inflateInit2(z_stream* strm, int windowBits)
    int inflate(z_stream* strm, int flush)
    int inflateReset(z_stream* strm)
    int inflateEnd(z_stream* strm)


cdef extern from "<lz4hc.h>" nogil:
    const int LZ4HC_CLEVEL_MIN
    const int LZ4HC_CLEVEL_DEFAULT
    const int LZ4HC_CLEVEL_OPT_MIN
    const int LZ4HC_CLEVEL_MAX


cdef extern from "<lz4frame.h>" nogil:
    const int LZ4F_VERSION
    const int LZ4F_HEADER_SIZE_MAX

    ctypedef struct LZ4F_cctx
    ctypedef struct LZ4F_dctx
    ctypedef struct LZ4F_compressOptions_t
    ctypedef struct LZ4F_decompressOptions_t
    ctypedef struct LZ4F_preferences_t:
        int compressionLevel
        unsigned autoFlush
        unsigned favorDecSpeed

    bint LZ4F_isError(size_t code)

    size_t LZ4F_createCompressionContext(LZ4F_cctx** cctxPtr, unsigned version)
    size_t LZ4F_freeCompressionContext(LZ4F_cctx* dctx)
    size_t LZ4F_compressBound(size_t srcSize, const LZ4F_preferences_t* prefsPtr)
    size_t LZ4F_compressBegin(LZ4F_cctx* cctx,
                              void* dstBuffer, size_t dstCapacity,
                              const LZ4F_preferences_t* prefsPtr);
    size_t LZ4F_compressUpdate(LZ4F_cctx * cctx,
                               void* dstBuffer, size_t dstCapacity,
                               const void* srcBuffer, size_t srcSize,
                               const LZ4F_compressOptions_t* cOptPtr)
    size_t LZ4F_flush(LZ4F_cctx* cctx,
                      void * dstBuffer, size_t dstCapacity,
                      const LZ4F_compressOptions_t* cOptPtr)
    size_t LZ4F_compressEnd(LZ4F_cctx* cctx,
                            void* dstBuffer, size_t dstCapacity,
                            const LZ4F_compressOptions_t* cOptPtr)


    size_t LZ4F_createDecompressionContext(LZ4F_dctx** dctxPtr, unsigned version)
    size_t LZ4F_freeDecompressionContext(LZ4F_dctx* dctx)
    size_t LZ4F_decompress(LZ4F_dctx* dctx,
                           void* dstBuffer, size_t* dstSizePtr,
                           const void* srcBuffer, size_t* srcSizePtr,
                           const LZ4F_decompressOptions_t* dOptPtr)


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
    cdef string read(self, size_t size)
    cdef size_t write(self, char* data, size_t size)
    cdef size_t tell(self)
    cdef void flush(self)
    cdef void close(self)


cdef class PythonIOStreamAdapter(IOStream):
    cdef object py_stream

    cdef void seek(self, size_t offset)


cdef class FileStream(IOStream):
    cdef FILE* fp

    cpdef void open(self, char* path, char* mode=*)
    cdef void seek(self, size_t offset)
    cpdef void close(self)


cdef class CompressingStream(IOStream):
    cdef size_t begin_member(self)
    cdef size_t end_member(self)


cdef class GZipStream(CompressingStream):
    cdef IOStream raw_stream
    cdef string working_buf
    cdef z_stream zst
    cdef bint initialized
    cdef int stream_read_status
    cdef bint member_started
    cdef int compression_level

    cdef void _init_z_stream(self, bint deflate) nogil
    cdef void _free_z_stream(self) nogil


cdef class LZ4Stream(CompressingStream):
    cdef IOStream raw_stream
    cdef LZ4F_cctx* cctx
    cdef LZ4F_dctx* dctx
    cdef LZ4F_preferences_t prefs
    cdef string working_buf
    cdef bint frame_started

    cdef void _free_ctx(self) nogil


cdef class BufferedReader:
    cdef IOStream stream
    cdef string buf
    cdef size_t buf_size
    cdef size_t stream_pos
    cdef size_t limit
    cdef size_t limit_consumed

    cdef inline void set_limit(self, size_t offset) nogil
    cdef inline void reset_limit(self) nogil

    cpdef string read(self, size_t size=*)
    cpdef string readline(self, bint crlf=*, size_t max_line_len=*)
    cpdef size_t tell(self)
    cpdef void consume(self, size_t size=*)
    cpdef void close(self)

    cdef bint _fill_buf(self)
    cdef inline string_view _get_buf(self) nogil
    cdef inline void _consume_buf(self, size_t size) nogil
