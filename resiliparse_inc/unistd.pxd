cdef extern from "<unistd.h>" nogil:
    ctypedef int pid_t
    pid_t getpid()
    int getpagesize()
    int usleep(size_t usec)
