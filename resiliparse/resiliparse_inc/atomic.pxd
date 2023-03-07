from libc.stdint cimport uint64_t

cdef extern from "<atomic>" namespace "std" nogil:
    cdef cppclass atomic[T]:
        atomic()
        T load() const
        void store(T desired)
        T fetch_add(T arg)
    ctypedef atomic atomic_bool[bint]
    ctypedef atomic atomic_size_t[size_t]
    ctypedef atomic atomic_uint64_t[uint64_t]
