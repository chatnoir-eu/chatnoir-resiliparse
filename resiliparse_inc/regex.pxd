from libcpp.string cimport string

cdef extern from "<regex>" namespace "std" nogil:
    cdef cppclass basic_regex[T]:
        basic_regex()
        basic_regex(const basic_regex& rgx) except +
        basic_regex(basic_regex& & rgx)
        basic_regex(const char* str)
        basic_regex(const char* str, size_t len)
    ctypedef basic_regex[char] regex

    string regex_replace(const string& s, const regex, const char* fmt)
    string regex_replace(const string& s, const regex, const string& fmt)

    string regex_replace[OutputIterator, BidirectionalIterator](
            OutputIterator out, BidirectionalIterator first, BidirectionalIterator last,
            const regex, const char* fmt)

    string regex_replace[OutputIterator, BidirectionalIterator](
            OutputIterator out, BidirectionalIterator first, BidirectionalIterator last,
            const regex, const string& fmt)
