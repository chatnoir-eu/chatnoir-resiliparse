cdef extern from "<atomic>" namespace "std" nogil:
    cdef cppclass atomic[T]:
        atomic()
        T load() const
        void store(T desired)
        T fetch_add(T arg)
    ctypedef atomic[bint] atomic_bool
    ctypedef atomic[size_t] atomic_size_t
