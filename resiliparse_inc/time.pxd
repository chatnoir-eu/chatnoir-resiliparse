from libc.stdint cimport uint64_t

cdef extern from '<time.h>' nogil:
    ctypedef uint64_t time_t
    ctypedef struct timespec:
        time_t tv_sec
        long tv_nsec
    cdef int CLOCK_MONOTONIC
    cdef int CLOCK_REALTIME
    cdef int clock_gettime(int clk_id, timespec *tp)
