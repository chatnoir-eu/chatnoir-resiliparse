from libcpp.string cimport string

cdef extern from "<string_view>" namespace "std" nogil:
    cdef cppclass string_view:
        string_view()
        string_view(const string_view& other)
        string_view(const char* s, size_t count)
        string_view& operator=(const string_view& view)
        bint empty() const
        size_t size() const
        string_view substr(size_t pos, size_t count) const
        string_view substr(size_t pos) const
        string_view substr() const
        size_t find(const char* s, size_t pos)
        size_t find(const char* s)
        const char& operator[](size_t pos) const
        const char* data() const
        const char& front()
        const char& back()
        string_view remove_prefix(size_t n)
        string_view remove_suffix(size_t n)

    bint operator==(const string_view& lhs, const string_view& rhs)
    bint operator==(const char* lhs, const string_view& rhs)
    bint operator==(const string_view& lhs, const char* rhs)
    bint operator==(const string_view& lhs, const string& rhs)
    bint operator==(const string& lhs, const string_view& rhs)

    bint operator!=(const string_view& lhs, const string_view& rhs)
    bint operator!=(const char* lhs, const string_view& rhs)
    bint operator!=(const string_view& lhs, const char* rhs)
    bint operator!=(const string_view& lhs, const string& rhs)
    bint operator!=(const string& lhs, const string_view& rhs)
