cdef extern from "<dlfcn.h>" nogil:
    const int RTLD_LAZY
    const int RTLD_NOW
    void* dlopen(const char* filename, int flags)
    void* dlsym(void* handle, const char* symbol);
    int dlclose(void* handle)
