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
