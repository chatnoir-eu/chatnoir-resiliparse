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

from resiliparse_inc.zlib cimport *
from resiliparse_inc.lz4hc cimport *
from resiliparse_inc.lz4frame cimport *
from resiliparse_inc.string cimport string
from resiliparse_inc.string_view cimport string_view


cdef class IOStream:
    cdef string errstr

    cdef string read(self, size_t size)
    cdef size_t write(self, const char* data, size_t size)
    cdef size_t tell(self)
    cdef void flush(self)
    cdef void close(self)
    cdef string error(self)


# noinspection PyAttributeOutsideInit
cdef class PythonIOStreamAdapter(IOStream):
    """IOStream adapter for Python file-like objects."""

    cdef object py_stream
    cdef Exception exc

    cdef inline size_t tell(self):
        try:
            return self.py_stream.tell()
        except Exception as e:
            self.errstr = str(e).encode()
            self.exc = e
            return 0

    cdef inline string read(self, size_t size):
        try:
            return self.py_stream.read(size)[:size]
        except Exception as e:
            self.errstr = str(e).encode()
            self.exc = e
            return string()

    cdef inline size_t write(self, const char* data, size_t size):
        try:
            return self.py_stream.write(data[:size])
        except Exception as e:
            self.errstr = str(e).encode()
            self.exc = e
            return 0

    cdef inline void flush(self):
        try:
            self.py_stream.flush()
        except Exception as e:
            self.errstr = str(e).encode()
            self.exc = e

    cdef inline void close(self):
        if self.py_stream is None:
            return

        # noinspection PyBroadException
        try:
            self.py_stream.close()
        except:
            # Ignore exceptions during close()
            pass


cdef class BytesIOStream(IOStream):
    cdef string buffer
    cdef size_t pos

    cdef void seek(self, size_t offset)
    cdef string getvalue(self)


cdef class FileStream(IOStream):
    cdef FILE* fp

    cdef void open(self, const char* path, char* mode=*) except *
    cdef void seek(self, size_t offset)


cdef class CompressingStream(IOStream):
    cdef size_t begin_member(self)
    cdef size_t end_member(self)


cdef class GZipStream(CompressingStream):
    cdef IOStream raw_stream
    cdef string working_buf
    cdef z_stream zst
    cdef int stream_read_status
    cdef char initialized
    cdef size_t stream_pos
    cdef bint member_started
    cdef int compression_level

    cdef void prepopulate(self, bint deflate, const string& initial_data)
    cdef void _init_z_stream(self, bint deflate) nogil
    cdef void _free_z_stream(self) nogil
    cdef bint _refill_working_buf(self, size_t size) nogil


cdef class LZ4Stream(CompressingStream):
    cdef IOStream raw_stream
    cdef LZ4F_cctx* cctx
    cdef LZ4F_dctx* dctx
    cdef LZ4F_preferences_t prefs
    cdef string working_buf
    cdef bint frame_started
    cdef size_t stream_pos

    cdef void prepopulate(self, const string& initial_data)
    cdef void _free_ctx(self) nogil


cdef class BufferedReader:
    cdef IOStream stream
    cdef string errstr
    cdef string buf
    cdef size_t buf_size
    cdef size_t stream_pos
    cdef size_t limit
    cdef size_t limit_consumed
    cdef bint negotiate_stream
    cdef bint stream_started
    cdef bint stream_is_compressed

    cdef inline void set_limit(self, size_t offset) nogil
    cdef inline void reset_limit(self) nogil
    cdef void detect_stream_type(self)

    cpdef string read(self, size_t size=*)
    cpdef string readline(self, bint crlf=*, size_t max_line_len=*)
    cpdef size_t tell(self)
    cpdef void consume(self, size_t size=*)
    cpdef void close(self)
    cpdef string error(self)

    cdef bint _fill_buf(self)
    cdef inline string_view _get_buf(self) nogil
    cdef inline void _consume_buf(self, size_t size) nogil
