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
