cdef extern from "<cctype>" namespace "std" nogil:
    int isspace(int c)
    int tolower(int c)
