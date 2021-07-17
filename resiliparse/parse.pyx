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

# Encoding name and label map according to https://encoding.spec.whatwg.org/#names-and-labels
# Differences:
#   * ISO-8859-8-I name replaced with ISO-8859-8
#   * WINDOWS-874 name replaced with ISO-8859-11
#   * x-mac-cyrillic is unsupported
#   * x-user-defined is unsupported
#   * No "replacement" mapping for 7-bit versions of ISO/IEC 2022
cdef dict __enc_html5_map = {'866': 'cp866', 'ansi_x3.4-1968': 'cp1252', 'arabic': 'iso8859-6', 'ascii': 'cp1252',
                             'asmo-708': 'iso8859-6', 'big5': 'big5', 'big5-hkscs': 'big5', 'chinese': 'gbk',
                             'cn-big5': 'big5', 'cp1250': 'cp1250', 'cp1251': 'cp1251', 'cp1252': 'cp1252',
                             'cp1253': 'cp1253', 'cp1254': 'cp1254', 'cp1255': 'cp1255', 'cp1256': 'cp1256',
                             'cp1257': 'cp1257', 'cp1258': 'cp1258', 'cp819': 'cp1252', 'cp866': 'cp866',
                             'csbig5': 'big5', 'cseuckr': 'euc_kr', 'cseucpkdfmtjapanese': 'euc_jp', 'csgb2312': 'gbk',
                             'csibm866': 'cp866', 'csiso2022jp': 'iso2022_jp', 'csiso58gb231280': 'gbk',
                             'csiso88596e': 'iso8859-6', 'csiso88596i': 'iso8859-6', 'csiso88598e': 'iso8859-8',
                             'csiso88598i': 'iso8859-8', 'csisolatin1': 'cp1252', 'csisolatin2': 'iso8859-2',
                             'csisolatin3': 'iso8859-3', 'csisolatin4': 'iso8859-4', 'csisolatin5': 'cp1254',
                             'csisolatin6': 'iso8859-10', 'csisolatin9': 'iso8859-15', 'csisolatinarabic': 'iso8859-6',
                             'csisolatincyrillic': 'iso8859-5', 'csisolatingreek': 'iso8859-7',
                             'csisolatinhebrew': 'iso8859-8', 'cskoi8r': 'koi8-r', 'csksc56011987': 'euc_kr',
                             'csmacintosh': 'mac-roman', 'csshiftjis': 'shift_jis', 'csunicode': 'utf-16-le',
                             'cyrillic': 'iso8859-5', 'dos-874': 'iso8859-11', 'ecma-114': 'iso8859-6',
                             'ecma-118': 'iso8859-7', 'elot_928': 'iso8859-7', 'euc-jp': 'euc_jp', 'euc-kr': 'euc_kr',
                             'gb18030': 'gb18030', 'gb2312': 'gbk', 'gb_2312': 'gbk', 'gb_2312-80': 'gbk', 'gbk': 'gbk',
                             'greek': 'iso8859-7', 'greek8': 'iso8859-7', 'hebrew': 'iso8859-8', 'ibm819': 'cp1252',
                             'ibm866': 'cp866', 'iso-10646-ucs-2': 'utf-16-le', 'iso-2022-jp': 'iso2022_jp',
                             'iso-8859-1': 'cp1252', 'iso-8859-10': 'iso8859-10', 'iso-8859-11': 'iso8859-11',
                             'iso-8859-13': 'iso8859-13', 'iso-8859-14': 'iso8859-14', 'iso-8859-15': 'iso8859-15',
                             'iso-8859-16': 'iso8859-16', 'iso-8859-2': 'iso8859-2', 'iso-8859-3': 'iso8859-3',
                             'iso-8859-4': 'iso8859-4', 'iso-8859-5': 'iso8859-5', 'iso-8859-6': 'iso8859-6',
                             'iso-8859-6-e': 'iso8859-6', 'iso-8859-6-i': 'iso8859-6', 'iso-8859-7': 'iso8859-7',
                             'iso-8859-8': 'ISO-8859-8', 'iso-8859-8-e': 'iso8859-8', 'iso-8859-8-i': 'iso8859-8',
                             'iso-8859-9': 'cp1254', 'iso-ir-100': 'cp1252', 'iso-ir-101': 'iso8859-2',
                             'iso-ir-109': 'iso8859-3', 'iso-ir-110': 'iso8859-4', 'iso-ir-126': 'iso8859-7',
                             'iso-ir-127': 'iso8859-6', 'iso-ir-138': 'iso8859-8', 'iso-ir-144': 'iso8859-5',
                             'iso-ir-148': 'cp1254', 'iso-ir-149': 'euc_kr', 'iso-ir-157': 'iso8859-10',
                             'iso-ir-58': 'gbk', 'iso8859-1': 'cp1252', 'iso8859-10': 'iso8859-10',
                             'iso8859-11': 'iso8859-11', 'iso8859-13': 'iso8859-13', 'iso8859-14': 'iso8859-14',
                             'iso8859-15': 'iso8859-15', 'iso8859-2': 'iso8859-2', 'iso8859-3': 'iso8859-3',
                             'iso8859-4': 'iso8859-4', 'iso8859-5': 'iso8859-5', 'iso8859-6': 'iso8859-6',
                             'iso8859-7': 'iso8859-7', 'iso8859-8': 'iso8859-8', 'iso8859-9': 'cp1254',
                             'iso88591': 'cp1252', 'iso885910': 'iso8859-10', 'iso885911': 'iso8859-11',
                             'iso885913': 'iso8859-13', 'iso885914': 'iso8859-14', 'iso885915': 'iso8859-15',
                             'iso88592': 'iso8859-2', 'iso88593': 'iso8859-3', 'iso88594': 'iso8859-4',
                             'iso88595': 'iso8859-5', 'iso88596': 'iso8859-6', 'iso88597': 'iso8859-7',
                             'iso88598': 'iso8859-8', 'iso88599': 'cp1254', 'iso_8859-1': 'cp1252',
                             'iso_8859-15': 'iso8859-15', 'iso_8859-1:1987': 'cp1252', 'iso_8859-2': 'iso8859-2',
                             'iso_8859-2:1987': 'iso8859-2', 'iso_8859-3': 'iso8859-3', 'iso_8859-3:1988': 'iso8859-3',
                             'iso_8859-4': 'iso8859-4', 'iso_8859-4:1988': 'iso8859-4', 'iso_8859-5': 'iso8859-5',
                             'iso_8859-5:1988': 'iso8859-5', 'iso_8859-6': 'iso8859-6', 'iso_8859-6:1987': 'iso8859-6',
                             'iso_8859-7': 'iso8859-7', 'iso_8859-7:1987': 'iso8859-7', 'iso_8859-8': 'iso8859-8',
                             'iso_8859-8:1988': 'iso8859-8', 'iso_8859-9': 'cp1254', 'iso_8859-9:1989': 'cp1254',
                             'koi': 'koi8-r', 'koi8': 'koi8-r', 'koi8-r': 'koi8-r', 'koi8-ru': 'koi8-u',
                             'koi8-u': 'koi8-u', 'koi8_r': 'koi8-r', 'korean': 'euc_kr', 'ks_c_5601-1987': 'euc_kr',
                             'ks_c_5601-1989': 'euc_kr', 'ksc5601': 'euc_kr', 'ksc_5601': 'euc_kr', 'l1': 'cp1252',
                             'l2': 'iso8859-2', 'l3': 'iso8859-3', 'l4': 'iso8859-4', 'l5': 'cp1254',
                             'l6': 'iso8859-10', 'l9': 'iso8859-15', 'latin1': 'cp1252', 'latin2': 'iso8859-2',
                             'latin3': 'iso8859-3', 'latin4': 'iso8859-4', 'latin5': 'cp1254', 'latin6': 'iso8859-10',
                             'logical': 'iso8859-8', 'mac': 'mac-roman', 'macintosh': 'mac-roman', 'ms932': 'shift_jis',
                             'ms_kanji': 'shift_jis', 'shift-jis': 'shift_jis', 'shift_jis': 'shift_jis',
                             'sjis': 'shift_jis', 'sun_eu_greek': 'iso8859-7', 'tis-620': 'iso8859-11',
                             'ucs-2': 'utf-16-le', 'unicode': 'utf-16-le', 'unicode-1-1-utf-8': 'utf-8',
                             'unicode11utf8': 'utf-8', 'unicode20utf8': 'utf-8', 'unicodefeff': 'utf-16-le',
                             'unicodefffe': 'utf-16-be', 'us-ascii': 'cp1252', 'utf-16': 'utf-16-le',
                             'utf-16be': 'utf-16-be', 'utf-16le': 'utf-16-le', 'utf-8': 'utf-8', 'utf8': 'utf-8',
                             'visual': 'iso8859-8', 'windows-1250': 'cp1250', 'windows-1251': 'cp1251',
                             'windows-1252': 'cp1252', 'windows-1253': 'cp1253', 'windows-1254': 'cp1254',
                             'windows-1255': 'cp1255', 'windows-1256': 'cp1256', 'windows-1257': 'cp1257',
                             'windows-1258': 'cp1258', 'windows-31j': 'shift_jis', 'windows-874': 'iso8859-11',
                             'windows-949': 'euc_kr', 'x-cp1250': 'cp1250', 'x-cp1251': 'cp1251', 'x-cp1252': 'cp1252',
                             'x-cp1253': 'cp1253', 'x-cp1254': 'cp1254', 'x-cp1255': 'cp1255', 'x-cp1256': 'cp1256',
                             'x-cp1257': 'cp1257', 'x-cp1258': 'cp1258', 'x-euc-jp': 'euc_jp', 'x-gbk': 'gbk',
                             'x-mac-roman': 'mac-roman', 'x-sjis': 'shift_jis', 'x-unicode20utf8': 'utf-8',
                             'x-x-big5': 'big5'}

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

    cpdef str encoding(self, bint html5_compatible=True):
        """
        encoding(self, html5_compatible=True)

        Get a Python-compatible name of the encoding that was detected and reset the detector.
        
        By default, the detected encoding is remapped based on the `WHATWG encoding specification
        <https://encoding.spec.whatwg.org/#names-and-labels>`_, which is primarily suitable for web
        content. To disable this behaviour, set ``html5_compatible=False``. For more information,
        see: :func:`map_encoding_to_html5`.
        
        If WHATWG remapping is enabled, UTF-8 is returned as a fallback encoding. Otherwise, the
        method returns ``None`` on failure to detect the encoding.

        :param html5_compatible: Remap encoding names according to WHATWG
        :type html5_compatible: bool
        :return: detected encoding or ``None`` on failure
        :rtype: str | None
        """
        uchardet_data_end(self.d)
        cdef str enc = uchardet_get_charset(self.d).decode()
        uchardet_reset(self.d)

        if html5_compatible:
            enc = map_encoding_to_html5(enc)
        else:
            try:
                enc = codecs.lookup(enc).name
            except LookupError:
                return None

        return enc

    def __dealloc__(self):
        if self.d != NULL:
            uchardet_delete(self.d)
            self.d = NULL

cdef EncodingDetector __chardet = None

@atexit.register
def __chardet_exit():
    global __chardet
    __chardet = None


cpdef str detect_encoding(bytes data, size_t max_len=4096, bint html5_compatible=True, bint from_html_meta=False):
    """
    detect_encoding(data, max_len=4096, html5_compatible=True, from_html_meta=False)

    Detect the encoding of a byte string. This is a convenience wrapper around :class:`EncodingDetector`
    that uses a single global instance.

    The string that is passed to the :class:`EncodingDetector` will be no longer than ``max_len``
    bytes to prevent slow-downs and keep memory usage low. If the string is longer than this limit, only
    the ``max_len / 2`` bytes from the start and from the end of the string will be used. This is a tradeoff
    between performance and accuracy. If you need higher accuracy, increase the limit to feed more data
    into the :class:`EncodingDetector`.

    The :class:`EncodingDetector` relies on `uchardet` as its encoding detection engine. If the
    input string is an HTML document, you can also use the available information from the HTML meta tags
    instead. With ``from_html_meta=True``, :func:`detect_encoding` will try to use the charset metadata
    contained in the HTML string if available and readable with an ASCII-compatible single-byte encoding
    or else fall back to auto-detection with `uchardet`.
    
    By default, the detected encoding is remapped based on the `WHATWG encoding specification
    <https://encoding.spec.whatwg.org/#names-and-labels>`_, which is primarily suitable for web content.
    To disable this behaviour, set ``html5_compatible=False``. For more information, see:
    :func:`map_encoding_to_html5`.
    
    If WHATWG remapping is enabled, UTF-8 is returned as a fallback encoding. Otherwise, the method returns
    ``None`` on failure to detect the encoding.

    :param data: input string for which to detect the encoding
    :type data: bytes
    :param max_len: maximum number of bytes to feed to detector (0 for no limit)
    :type max_len: int
    :param html5_compatible: Remap encoding names according to WHATWG
    :type html5_compatible: bool
    :param from_html_meta: if string is an HTML document, use meta tag info
    :type from_html_meta: bool
    :return: detected encoding
    :rtype: str
    """
    if max_len != 0 and <size_t> len(data) > max_len:
        data = data[:(max_len + 1) // 2] + data[-((max_len + 1) // 2):]

    if from_html_meta:
        encoding = __slx.myencoding_prescan_stream_to_determine_encoding(<char *> data, len(data))
        if encoding != MyENCODING_NOT_DETERMINED:
            encoding = __slx.myencoding_name_by_id(encoding, NULL).decode()
            if html5_compatible:
                encoding = map_encoding_to_html5(encoding)
            return encoding

    global __chardet
    if __chardet is None:
        __chardet = EncodingDetector.__new__(EncodingDetector)
    __chardet.update(<string> data)
    return __chardet.encoding(html5_compatible)


cpdef str map_encoding_to_html5(str encoding, bint fallback_utf8=True):
    """
    map_encoding_to_html5(str encoding, fallback_utf8=True)
    
    Map an encoding name to a subset of names allowed by the HTML5 standard.
    
    This function will remap the given name according to the mapping definition given in
    `Section 4.2 <https://encoding.spec.whatwg.org/#names-and-labels>`_ of the WHATWG encoding
    specification. The returned value will always be a valid Python encoding name, but the
    supplied input name does not necessarily have to be.
    
    The WHATWG mapping is designed to boil down the many possible encoding names to a smaller
    subset of canonical names while taking into account common encoding mislabelling practices.
    The main purpose of this function is to map encoding names extracted from HTTP headers or
    websites to their canonical names, but it also makes sense to apply the mapping to an
    auto-detected encoding name, since it remaps some encodings based on observed practices on
    the web, such as the mapping from ISO-8859-1 to Windows-1252, which is more likely to be
    correct, even if both options are possible. :meth:`EncodingDetector.encoding` already remaps
    its detected encodings to the WHATWG set by default.
    
    The mapping does not involve Python's encoding alias names, but instead uses an adjusted
    WHATWG mapping. Inputs not defined in this mapping are remapped to UTF-8. Hence, the function
    always produces a valid output, but the mapped encoding is not guaranteed to be compatible
    with the original encoding. Use :func:`bytes_to_str` to avoid decoding errors. You can also
    set ``fallback_utf8=False`` to return ``None`` instead if the supplied encoding is unknown.
    
    The adjusted encoding mapping differs from the WHATWG spec in the following details:
    
      * ISO-8859-8-I name replaced with ISO-8859-8
      * WINDOWS-874 name replaced with ISO-8859-11
      * x-mac-cyrillic is unsupported
      * x-user-defined is unsupported
      * No "replacement" mapping for 7-bit versions of ISO/IEC 2022
    
    :param encoding: input encoding name
    :type encoding: str
    :param fallback_utf8: Whether to fall back to UTF-8 or return ``None`` for unknown encodings
    :type fallback_utf8: bool
    :return: mapped output encoding name
    :rtype: str | None
    """
    return __enc_html5_map.get(encoding.strip().casefold(), 'utf-8' if fallback_utf8 else None)


cdef str __map_utf(str enc, bytes data, bint strip):
    if not strip:
        return enc

    if enc == 'utf-8' and data.startswith(codecs.BOM_UTF8):
        return 'utf-8-sig'
    if enc.startswith('utf-16-') and (data.startswith(codecs.BOM_UTF16_LE) or data.startswith(codecs.BOM_UTF16_BE)):
        return 'utf-16'
    if enc.startswith('utf-32-') and (data.startswith(codecs.BOM_UTF32_LE) or data.startswith(codecs.BOM_UTF32_BE)):
        return 'utf-32'

    return enc


# noinspection PyTypeChecker
cpdef str bytes_to_str(bytes data, str encoding='utf-8', str errors='ignore',
                       fallback_encodings=('utf-8', 'cp1252'), bint strip_bom=True):
    """
    bytes_to_str(data, encoding='utf-8', errors='ignore', fallback_encodings=('utf-8', 'cp1252'), strip_bom=True)

    Helper for decoding a byte string into a unicode string using a given encoding.
    This encoding should be determined beforehand, e.g., with :func:`detect_encoding`.

    :func:`bytes_to_str` tries to decode the byte string with ``encoding``. If that
    fails, it will fall back to UTF-8 and Windows-1252 (or whichever encodings where
    given in ``fallback_encodings``). If all fallbacks fail as well, the string will be
    double-decoded with ``encoding`` and invalid characters will be treated according
    to ``errors``, which has the same options as for :meth:`bytes.decode` (i.e.,
    ``"ignore"`` or ``"replace"``). The double-decoding step ensures that the resulting
    string is sane and can be re-encoded without errors.
    
    This function also takes care to strip BOMs from the beginning of the string if
    ``strip_bom=True``.

    :param data: input byte string
    :type data: bytes
    :param encoding: desired encoding
    :type encoding: str
    :param errors: error handling for invalid characters
    :type errors: str
    :param fallback_encodings: list of fallback encodings to try if the primary encoding fails
    :type fallback_encodings: t.Iterable[str]
    :param strip_bom: strip BOM sequences from beginning of the string
    :type strip_bom: bool
    :return: decoded string
    :rtype: str
    """

    encoding = codecs.lookup(encoding).name

    for i, e in enumerate((encoding, *fallback_encodings)):
        e = codecs.lookup(e).name
        if i > 0 and e == encoding:
            # No need to try that again
            continue

        try:
            return data.decode(__map_utf(e, data, strip_bom))
        except UnicodeDecodeError:
            pass

    return data.decode(__map_utf(encoding, data, strip_bom), errors=errors).encode(errors=errors).decode()


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
