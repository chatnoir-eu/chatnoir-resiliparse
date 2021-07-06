cdef extern from "<sys/time.h>" nogil:
    ctypedef long int time_t
    ctypedef long int suseconds_t
    cdef struct timeval:
        long int tv_sec
    cdef struct timezone
    int gettimeofday(timeval* tv, timezone* tz)
