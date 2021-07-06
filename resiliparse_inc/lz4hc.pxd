cdef extern from "<lz4hc.h>" nogil:
    const int LZ4HC_CLEVEL_MIN
    const int LZ4HC_CLEVEL_DEFAULT
    const int LZ4HC_CLEVEL_OPT_MIN
    const int LZ4HC_CLEVEL_MAX
