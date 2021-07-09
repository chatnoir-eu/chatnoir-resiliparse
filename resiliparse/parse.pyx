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

from resiliparse_inc.string cimport string
from resiliparse_inc.uchardet cimport uchardet_t, uchardet_new, uchardet_delete, uchardet_handle_data, \
    uchardet_data_end, uchardet_reset, uchardet_get_charset

include 'parse_selectolax.pxi'


cdef class CharsetDetector:
    """
    Charset (encoding) detector based on `uchardet`.

    `uchardet` is a C wrapper and a continuation of Mozilla's `Universal Charset Detector` library.
    """
    cdef uchardet_t d

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

    cdef string encoding_str(self):
        """
        Get an iconv-compatible name of the encoding that was detected and reset the detector.
        
        :return: detected encoding
        """
        uchardet_data_end(self.d)
        cdef string enc = uchardet_get_charset(self.d)
        uchardet_reset(self.d)
        return enc

    cpdef str encoding(self):
        """
        encoding(self)

        Get an iconv-compatible name of the encoding that was detected and reset the detector.

        :return: detected encoding
        :rtype: str
        """
        return self.encoding_str().decode()

    def __dealloc__(self):
        if self.d != NULL:
            uchardet_delete(self.d)
            self.d = NULL


cdef CharsetDetector __chardet = None

@atexit.register
def __chardet_exit():
    global __chardet
    __chardet = None


def detect_encoding(bytes data, size_t max_len=4096, bint from_html_meta=False):
    """
    detect_encoding(data, max_len=4096, from_html_meta=False)

    Detect the encoding of a byte string. This is a convenience wrapper around :class:`CharsetDetector`
    that uses a single global instance.

    The string that is passed to the :class:`'CharsetDetector` will be no longer than ``max_len`` bytes to
    prevent slow-downs and keep memory usage low. If the string is longer than this limit, only the
    ``max_len / 2`` bytes from the start and from the end of the string will be used. This is a tradeoff
    between performance and accuracy. If you need higher accuracy, increase the limit to feed more data
    into the :class:`CharsetDetector`.

    The :class:`CharsetDetector` relies on `uchardet` as its encoding detection engine. If the input string
    is an HTML document, you can also use the available information from the HTML meta tags instead. With
    ``from_html_meta=True``, :func:`detect_encoding` will try to use the charset metadata contained in the
    HTML string if available and readable with an ASCII-compatible single-byte encoding or else fall back
    to auto-detection with `uchardet`.

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
        __chardet = CharsetDetector.__new__(CharsetDetector)
    __chardet.update(<string>data)
    return __chardet.encoding()
