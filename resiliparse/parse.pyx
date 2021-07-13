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

import atexit
import codecs
import typing as t

from resiliparse_inc.cstdlib cimport strtol
from resiliparse_inc.string cimport string
from resiliparse_inc.uchardet cimport uchardet_new, uchardet_delete, uchardet_handle_data, \
    uchardet_data_end, uchardet_reset, uchardet_get_charset

include 'parse_selectolax.pxi'


cdef class EncodingDetector:
    """
    Universal character encoding detector based on `uchardet`.

    `uchardet` is a C wrapper and a continuation of Mozilla's `Universal Charset Detector` library.
    """

    def __cinit__(self):
        self.d = uchardet_new()

    cpdef void update(self, const string& data):
        """
        update(self, data)
        
        Update charset detector with more data.
        
        The detector will shortcut processing when it has enough data to reach certainty, so you don't
        need to worry too much about limiting the input data.
        
        :param data: input data
        :type data: bytes
        """
        uchardet_handle_data(self.d, data.data(), data.size())

    cpdef str encoding(self):
        """
        encoding(self)

        Get a Python-compatible name of the encoding that was detected and reset the detector.

        :return: detected encoding or ``None`` on failure
        :rtype: str | None
        """
        uchardet_data_end(self.d)
        cdef str enc = uchardet_get_charset(self.d).decode()
        uchardet_reset(self.d)
        try:
            codecs.lookup(enc)
        except LookupError:
            return None

        return enc if enc != '' else None

    def __dealloc__(self):
        if self.d != NULL:
            uchardet_delete(self.d)
            self.d = NULL


cdef EncodingDetector __chardet = None

@atexit.register
def __chardet_exit():
    global __chardet
    __chardet = None


cpdef str detect_encoding(bytes data, size_t max_len=4096, bint from_html_meta=False):
    """
    detect_encoding(data, max_len=4096, from_html_meta=False)

    Detect the encoding of a byte string. This is a convenience wrapper around :class:`EncodingDetector`
    that uses a single global instance.

    The string that is passed to the :class:`'EncodingDetector` will be no longer than ``max_len``
    bytes to prevent slow-downs and keep memory usage low. If the string is longer than this limit, only
    the ``max_len / 2`` bytes from the start and from the end of the string will be used. This is a tradeoff
    between performance and accuracy. If you need higher accuracy, increase the limit to feed more data
    into the :class:`EncodingDetector`.

    The :class:`EncodingDetector` relies on `uchardet` as its encoding detection engine. If the
    input string is an HTML document, you can also use the available information from the HTML meta tags
    instead. With ``from_html_meta=True``, :func:`detect_encoding` will try to use the charset metadata
    contained in the HTML string if available and readable with an ASCII-compatible single-byte encoding
    or else fall back to auto-detection with `uchardet`.

    :param data: input string for which to detect the encoding
    :type data: bytes
    :param max_len: maximum number of bytes to feed to detector (0 for no limit)
    :type max_len: int
    :param from_html_meta: if string is an HTML document, use meta tag info
    :type from_html_meta: bool
    :return: detected encoding
    :rtype: str
    """
    if max_len != 0 and <size_t>len(data) > max_len:
        data = data[:(max_len + 1) // 2] + data[-((max_len + 1) // 2):]

    if from_html_meta:
        encoding = __slx.myencoding_prescan_stream_to_determine_encoding(<char*>data, len(data))
        if encoding != MyENCODING_NOT_DETERMINED:
            return __slx.myencoding_name_by_id(encoding, NULL).decode()

    global __chardet
    if __chardet is None:
        __chardet = EncodingDetector.__new__(EncodingDetector)
    __chardet.update(<string>data)
    return __chardet.encoding()


cpdef str bytes_to_str(bytes data, str encoding='utf-8', str errors='ignore',
                       fallback_encodings=('utf-8', 'iso-8859-15', 'iso-8859-1')):
    """
    bytes_to_str(data, encoding='utf-8', errors='ignore', \
                 fallback_encodings=('utf-8', 'iso-8859-15', 'iso-8859-1'))

    Helper for decoding a byte string into a unicode string using a given encoding.
    This encoding should be determined beforehand, e.g., with :func:`detect_encoding`.

    :func:`bytes_to_str` tries to decode the byte string with ``encoding``. If that
    fails, it will fall back to UTF-8 and Latin-1 (or whichever encodings where given
    in ``fallback_encodings``). If all fallbacks fail as well, the string will be
    double-decoded with ``encoding`` and invalid characters will be treated according
    to ``errors``, which has the same options as for :meth:`bytes.decode` (i.e.,
    ``"ignore"`` or ``"replace"``). The double-decoding step ensures that the resulting
    string is sane and can be re-encoded without errors.

    :param data: input byte string
    :type data: bytes
    :param encoding: desired encoding
    :type encoding: str
    :param errors: error handling for invalid characters
    :type errors: str
    :param fallback_encodings: list of fallback encodings to try if the primary encoding fails
    :type fallback_encodings: t.Iterable[str]
    :return: decoded string
    :rtype: str
    """

    for i, e in enumerate((encoding, *fallback_encodings)):
        if i > 0 and e == encoding:
            # No need to try that again
            continue

        try:
            return data.decode(e)
        except UnicodeDecodeError:
            pass

    return data.decode(encoding, errors=errors).encode(errors=errors).decode()


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
