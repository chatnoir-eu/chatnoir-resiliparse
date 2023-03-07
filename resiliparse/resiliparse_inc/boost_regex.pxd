from libcpp.string cimport string

cdef extern from "<boost/regex.hpp>" namespace "boost::regex_constants" nogil:
    ctypedef enum flag_type:
        perl_syntax_group,
        basic_syntax_group,
        literal,
        main_option_type,
        no_bk_refs,
        no_perl_ex,
        no_mod_m,
        mod_x,
        mod_s,
        no_mod_s,
        no_char_classes,
        no_intervals,
        bk_plus_qm,
        bk_vbar,
        emacs_ex,
        no_escape_in_lists,
        newline_alt,
        no_except,
        failbit,
        icase,
        nocollate,
        collate,
        nosubs,
        save_subexpression_location,
        no_empty_expressions,
        optimize,

        basic,
        extended,
        normal,
        emacs,
        awk,
        grep,
        egrep,
        sed,
        perl,
        ECMAScript,
        JavaScript,
        JScript

    ctypedef enum match_flag_type:
        match_default,
        match_not_bol,
        match_not_eol,
        match_not_bob,
        match_not_eob,
        match_not_bow,
        match_not_eow,
        match_not_dot_newline,
        match_not_dot_null,
        match_prev_avail,
        match_init,
        match_any,
        match_not_null,
        match_continuous,
        match_partial,
        match_stop,
        match_not_initial_null,
        match_all,
        match_perl,
        match_posix,
        match_nosubs,
        match_extra,
        match_single_line,
        match_unused1,
        match_unused2,
        match_unused3,
        match_max,
        format_perl,
        format_default,
        format_sed,
        format_all,
        format_no_copy,
        format_first_only,
        format_is_if,
        format_literal,
        match_not_any

from resiliparse_inc.string_view cimport string_view
cdef extern from "<boost/regex.hpp>" namespace "boost" nogil:
    cdef cppclass basic_regex[charT]:
        basic_regex()
        basic_regex(const basic_regex& rgx)
        basic_regex(const charT* p)
        basic_regex(const charT* p, flag_type f)
        basic_regex(const charT* p1, const charT* p2)
        basic_regex(const charT* p1, const charT* p2, flag_type f)
        basic_regex(const charT* p, size_t len, flag_type f)
        basic_regex(const string& p)
        basic_regex(const string& p, flag_type f)
    ctypedef basic_regex[char] regex
    ctypedef struct match_results

    bint regex_match[charT](const charT* s, const regex& rgx)
    bint regex_match[charT](const charT* s, const regex& rgx, match_flag_type flags)
    bint regex_match(const const string& s, const regex& rgx)
    bint regex_match(const const string& s, const regex& rgx, match_flag_type flags)

    bint regex_search[charT](const charT* s, const regex& rgx)
    bint regex_search[charT](const charT* s, const regex& rgx, match_flag_type flags)
    bint regex_search(const const string& s, const regex& rgx)
    bint regex_search(const const string& s, const regex& rgx, match_flag_type flags)

    string regex_replace(const string& s, const regex, const string& fmt)
    string regex_replace(const string& s, const regex, const string& fmt, match_flag_type flags)


cdef extern from * nogil:
    """
    #include <boost/regex.hpp>
    #include <iterator>
    #include <string>

    inline bool regex_match(std::string_view s,
                      const boost::regex& rgx,
                      boost::regex_constants::match_flag_type flags = boost::regex_constants::match_default) {
        return boost::regex_match(s.begin(), s.end(), rgx, flags);
    }

    inline bool regex_search(std::string_view s,
                      const boost::regex& rgx,
                      boost::regex_constants::match_flag_type flags = boost::regex_constants::match_default) {
        return boost::regex_search(s.begin(), s.end(), rgx, flags);
    }

    inline std::string regex_replace(std::string_view s,
                      const boost::regex& rgx,
                      const std::string& fmt,
                      boost::regex_constants::match_flag_type flags = boost::regex_constants::match_default) {
        std::string out;
        boost::regex_replace(std::back_inserter(out), s.begin(), s.end(), rgx, fmt, flags);
        return out;
    }
    """

    bint regex_match(const const string_view& s, const regex& rgx)
    bint regex_match(const const string_view& s, const regex& rgx, match_flag_type flags)

    bint regex_search(const const string_view& s, const regex& rgx)
    bint regex_search(const const string_view& s, const regex& rgx, match_flag_type flags)

    string regex_replace(const string_view& s, const regex, const string& fmt)
    string regex_replace(const string_view& s, const regex, const string& fmt, match_flag_type flags)
