cdef extern from * namespace "stdx" nogil:
    """
    #include <type_traits>

    namespace stdx {
    template <typename T> typename std::remove_reference<T>::type&& move(T& t) noexcept { return std::move(t); }
    template <typename T> typename std::remove_reference<T>::type&& move(T&& t) noexcept { return std::move(t); }
    }
    """
    cdef T move[T](T)
