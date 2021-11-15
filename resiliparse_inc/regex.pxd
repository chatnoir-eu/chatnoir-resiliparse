from libcpp.string cimport string

cdef extern from "<regex>" namespace "std" nogil:
    cdef cppclass basic_regex[T]:
        basic_regex()
        basic_regex(const basic_regex& rgx) except +
        basic_regex(basic_regex& & rgx)
        basic_regex(const char* str)
        basic_regex(const char* str, size_t len)
    ctypedef basic_regex[char] regex
    ctypedef struct match_results

    string regex_replace(const string& s, const regex, const char* fmt)
    string regex_replace(const string& s, const regex, const string& fmt)
    bool regex_match(const char* s, const regex& rgx)
    bool regex_match(const const string& s, const regex& rgx)
