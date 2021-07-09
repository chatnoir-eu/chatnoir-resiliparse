cdef extern from "<uchardet/uchardet.h>" nogil:
    ctypedef struct uchardet
    ctypedef uchardet* uchardet_t

    uchardet_t uchardet_new()
    void uchardet_delete(uchardet_t ud)
    int uchardet_handle_data(uchardet_t ud, const char* data, size_t len)
    void uchardet_data_end(uchardet_t ud)
    void uchardet_reset(uchardet_t ud)
    const char * uchardet_get_charset(uchardet_t ud)
