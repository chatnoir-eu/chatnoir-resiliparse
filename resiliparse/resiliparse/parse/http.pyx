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

import typing as t

from libcpp.string cimport string
from resiliparse_inc.cstdlib cimport strtol


cpdef bytes read_http_chunk(reader):
    """
    read_http_chunk(reader)

    Helper function for reading chunked HTTP payloads.

    Each call to this function will try to read the next chunk. In case of an error
    or EOF, an empty byte string will be returned.

    :param reader: input reader
    :type reader: fastwarc.stream_io.BufferedReader
    :return: contents of the next chunk or empty string if EOF
    :rtype: bytes
    """
    cdef string header_line = reader.readline()
    cdef size_t chunk_size = strtol(header_line.substr(0, header_line.size() - 2).c_str(), NULL, 16)
    return reader.read(chunk_size + 2)[:chunk_size]


def iterate_http_chunks(reader):
    """
    iterate_http_chunks(reader)

    Generator wrapping :func:`read_http_chunk` for fully consuming a chunked HTTP payload.

    :param reader: input reader
    :type reader: fastwarc.stream_io.BufferedReader
    :return: generator of chunks
    :rtype: t.Generator[bytes]
    """
    cdef bytes chunk
    while True:
        chunk = read_http_chunk(reader)
        if len(chunk) == 0:
            return
        yield chunk
