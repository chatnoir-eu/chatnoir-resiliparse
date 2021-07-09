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

import chardet

include 'parse_selectolax.pxi'


cpdef str detect_encoding(bytes raw_bytes, size_t max_len=800, bint from_html_meta=False, bint prefer_speed=True):
    """
    detect_encoding(raw_bytes, max_len=800, from_html_meta=False, prefer_speed=True)
    
    Detect the encoding of a byte string.
    
    The detection relies on Modest Engine's MyENCODING (via :mod:`selectolax`) as the primary detection
    engine and falls back onto the more accurate but also much slower :mod:`chardet` engine if
    MyENCODING fails to detect the encoding. Set ``prefer_speed=False`` to always use :mod:`chardet`
    if accuracy matters more than execution time and memory footprint.
    
    The string that is scanned by the engine will not be longer than ``max_len`` bytes to prevent massive
    slow-downs and excessive memory use. If the string is longer than this limit, only the ``max_len / 2``
    bytes from the start and from the end of the string will be used.
    
    :param raw_bytes: input string for which to detect encoding
    :type raw_bytes: bytes
    :param max_len: maximum number of bytes to scan
    :type max_len: int
    :param from_html_meta: if string is an HTML document, use meta tag info instead of auto-detection
    :type from_html_meta: bool
    :param prefer_speed: prefer speed over accuracy
    :type prefer_speed: bool
    :return: detected encoding
    :rtype: str
    """
    cdef myencoding_t encoding = MyENCODING_DEFAULT
    cdef size_t raw_bytes_len = len(raw_bytes)
    cdef const char* raw_bytes_cstr
    if raw_bytes_len > max_len:
        raw_bytes = raw_bytes[:max_len // 2] + raw_bytes[-(max_len // 2):]
        raw_bytes_len = len(raw_bytes)
    raw_bytes_cstr = <const char*>raw_bytes

    if from_html_meta:
        encoding = __slx.myencoding_prescan_stream_to_determine_encoding(raw_bytes_cstr, raw_bytes_len)
        if encoding != MyENCODING_DEFAULT and encoding != MyENCODING_NOT_DETERMINED:
            __slx.myencoding_name_by_id(encoding, NULL).decode().lower()

    if not prefer_speed:
        return chardet.detect(raw_bytes)['encoding'].lower()

    if not __slx.myencoding_detect_bom(raw_bytes_cstr, raw_bytes_len, &encoding):
        if not __slx.myencoding_detect(raw_bytes_cstr, raw_bytes_len, &encoding):
            return chardet.detect(raw_bytes)['encoding'].lower()

    return __slx.myencoding_name_by_id(encoding, NULL).decode().lower()
