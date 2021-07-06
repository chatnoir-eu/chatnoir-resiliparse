cdef extern from "<cstdlib>" namespace "std" nogil:
    long strtol(const char* str, char** endptr, int base)
