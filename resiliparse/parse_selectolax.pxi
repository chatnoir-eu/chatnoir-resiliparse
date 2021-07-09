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

import selectolax

# <myencoding/myosi.h>
ctypedef int myencoding_t
cdef myencoding_t MyENCODING_DEFAULT = 0x00
cdef myencoding_t MyENCODING_NOT_DETERMINED = 0x02


# noinspection PyAttributeOutsideInit
cdef class __Selectolax:
    cdef void* _sx

    # <myencoding/encoding.h>
    cdef myencoding_t(*myencoding_prescan_stream_to_determine_encoding)(const char*, size_t)
    cdef const char* (*myencoding_name_by_id)(myencoding_t, size_t*)

    cdef inline void load(self, void** ptr, const char* name):
        ptr[0] = dlsym(self._sx, name)

    def __cinit__(self):
        import glob, os
        path = os.path.join(glob.glob(os.path.join(os.path.dirname(selectolax.__file__),
                                                   'parser.*.so'))[0]).encode()
        self._sx = dlopen(<char*>path, RTLD_LAZY)
        if self._sx == NULL:
            return

        self.load(<void**>&self.myencoding_prescan_stream_to_determine_encoding,
                  b'myencoding_prescan_stream_to_determine_encoding')
        self.load(<void**>&self.myencoding_name_by_id, b'myencoding_name_by_id')

    def __dealloc__(self):
        if self._sx != NULL:
            dlclose(self._sx)


cdef __Selectolax __slx = __Selectolax()

@atexit.register
def __slx_exit():
    global __slx
    __slx = None
