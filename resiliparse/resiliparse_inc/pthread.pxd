cdef extern from "<pthread.h>" nogil:
    ctypedef unsigned long pthread_t
    pthread_t pthread_self()
    int pthread_kill(pthread_t thread, int sig)
