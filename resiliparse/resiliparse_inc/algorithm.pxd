cdef extern from "<algorithm>" namespace "std" nogil:
    void replace[Iter, T](Iter first, Iter last, const T& old_value, const T& new_value)
    int count[Iter, T](Iter first, Iter last, const T& val)
