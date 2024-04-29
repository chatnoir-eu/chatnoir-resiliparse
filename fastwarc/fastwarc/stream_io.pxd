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

from libc.stdio cimport FILE
from libcpp.string cimport string

from resiliparse_inc.zlib cimport *
from resiliparse_inc.lz4hc cimport *
from resiliparse_inc.lz4frame cimport *
from resiliparse_inc.string_view cimport string_view


cdef class IOStream:
    cdef size_t read_(self, char* out, size_t size) except -1
    cdef size_t write_(self, const char* data, size_t size) except -1
    cpdef size_t tell(self) except -1
    cpdef void seek(self, size_t offset) except *
    cpdef void flush(self)  except *
    cpdef void close(self) except *


cdef class PythonIOStreamAdapter(IOStream):
    cdef object py_stream


cdef class BytesIOStream(IOStream):
    cdef string buffer
    cdef size_t pos

    cpdef string getvalue(self)


cdef class FileStream(IOStream):
    cdef FILE* fp

    cdef void open_(self, const char* path, char* mode=*) except *


cdef class CompressingStream(IOStream):
    cpdef size_t begin_member(self)
    cpdef size_t end_member(self)


cdef extern from * nogil:
    """
    /* Helper enum for keeping track of current compressing stream state. */
    enum CompressingStreamState {
        UNINIT = 0,
        DECOMPRESSING = 1,
        COMPRESSING = 2
    };
    """
    ctypedef enum CompressingStreamState:
        UNINIT,
        DECOMPRESSING,
        COMPRESSING


cdef class GZipStream(CompressingStream):
    cdef IOStream raw_stream
    cdef string working_buf
    cdef unsigned int working_buf_filled
    cdef z_stream zst
    cdef int window_bits
    cdef char stream_state
    cdef size_t stream_pos
    cdef bint member_started
    cdef int compression_level

    cpdef void prepopulate(self, bint deflate, const string& initial_data)
    cdef void _init_z_stream(self, bint zlib) noexcept nogil
    cdef void _free_z_stream(self) noexcept nogil
    cdef void _reinit_z_stream(self, bint deflate) noexcept nogil
    cdef bint _refill_working_buf(self, size_t size) except -1


cdef class LZ4Stream(CompressingStream):
    cdef IOStream raw_stream
    cdef LZ4F_cctx* cctx
    cdef LZ4F_dctx* dctx
    cdef LZ4F_preferences_t prefs
    cdef string working_buf
    cdef size_t working_buf_read
    cdef size_t working_buf_filled
    cdef bint frame_started
    cdef size_t stream_pos

    cpdef void prepopulate(self, const string& initial_data)
    cdef void _free_ctx(self) noexcept nogil


cdef class BrotliStream(CompressingStream):
    cdef IOStream raw_stream
    cdef size_t quality
    cdef size_t lgwin
    cdef size_t lgblock
    cdef object compressor
    cdef string working_buf
    cdef CompressingStreamState stream_state


cdef class BufferedReader:
    cdef IOStream stream
    cdef string errstr
    cdef string buf
    cdef string_view buf_view
    cdef string_view limited_buf_view
    cdef size_t last_read_size
    cdef size_t stream_pos
    cdef size_t limit
    cdef size_t limit_consumed
    cdef bint negotiate_stream
    cdef bint stream_started
    cdef bint stream_is_compressed

    cdef inline void set_limit(self, size_t offset) noexcept nogil
    cdef inline void reset_limit(self) noexcept nogil
    cdef bint detect_stream_type(self) except 0

    # read() is mainly used in user code and therefore returns bytes.
    # readline() is mainly used in internal code, hence we keep it string for efficiency purposes
    cpdef bytes read(self, size_t size=*)
    cpdef string readline(self, bint crlf=*, size_t max_line_len=*)  except *

    cpdef size_t tell(self) except -1
    cpdef size_t consume(self, size_t size=*) except -1
    cpdef void close(self) except *

    cdef bint _fill_buf(self) except -1
    cdef inline string_view* _get_buf(self) noexcept nogil
    cdef inline size_t _consume_buf(self, size_t size) noexcept nogil
