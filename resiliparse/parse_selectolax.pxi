# Copyright 2021 Janek Bevendorff
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# distutils: language = c++

from resiliparse_inc.dlfcn cimport dlopen, dlclose, dlsym, RTLD_LAZY

import atexit
import selectolax

# <myencoding/myosi.h>
ctypedef int myencoding_t
cdef myencoding_t MyENCODING_DEFAULT = 0x00
cdef myencoding_t MyENCODING_NOT_DETERMINED = 0x02

cdef class __Selectolax:
    cdef void* handle

    # <myencoding/encoding.h>
    cdef myencoding_t(*myencoding_prescan_stream_to_determine_encoding)(const char*, size_t)
    cdef bint(*myencoding_detect)(const char*, size_t, myencoding_t*)
    cdef bint(*myencoding_detect_bom)(const char*, size_t, myencoding_t*)
    cdef const char* (*myencoding_name_by_id)(myencoding_t, size_t*)

    cdef inline void load(self, void** ptr, const char* name):
        ptr[0] = dlsym(self.handle, name)

    def __cinit__(self):
        import glob, os
        path = os.path.join(glob.glob(os.path.join(os.path.dirname(selectolax.__file__),
                                                   'parser.cpython-*'))[0]).encode()
        self.handle = dlopen(<char*>path, RTLD_LAZY)

        self.load(<void**>&self.myencoding_prescan_stream_to_determine_encoding,
                  b'myencoding_prescan_stream_to_determine_encoding')
        self.load(<void**>&self.myencoding_detect, b'myencoding_detect')
        self.load(<void**>&self.myencoding_detect_bom, b'myencoding_detect_bom')
        self.load(<void**>&self.myencoding_name_by_id, b'myencoding_name_by_id')

    def __dealloc__(self):
        if self.handle != NULL:
            dlclose(self.handle)

cdef __Selectolax __slx = __Selectolax()

@atexit.register
def __exit():
    global __slx
    __slx = None
