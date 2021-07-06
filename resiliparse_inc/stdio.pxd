from libc.stdio cimport *

cdef extern from "<stdio.h>" nogil:
    FILE* popen(const char* command, const char* type);
    int pclose(FILE* stream);
