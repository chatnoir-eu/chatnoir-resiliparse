cdef extern from "<cstring>" namespace "std" nogil:
    char* strerror(int errnum)
