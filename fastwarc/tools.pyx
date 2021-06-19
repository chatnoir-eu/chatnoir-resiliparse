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

from stream_io cimport IOStream, GZipStream, LZ4Stream, FileStream, PythonIOStreamAdapter
from warc cimport ArchiveIterator, WarcRecordType


cpdef enum CompressionAlg:
    gzip,
    lz4,
    uncompressed,
    auto


def _detect_comp_alg(in_obj):
    filename = None
    if type(in_obj) is str:
        filename = in_obj
    elif hasattr(in_obj, 'name'):
        filename = in_obj.name

    if type(filename) is str and filename.endswith('.gz'):
        return CompressionAlg.gzip
    elif type(filename) is str and filename.endswith('.lz4'):
        return CompressionAlg.lz4
    elif type(filename) is str and filename.endswith('.warc'):
        return CompressionAlg.uncompressed
    else:
        raise ValueError('Cannot auto-detect compression algorithm.')


def recompress_warc(warc_in, warc_out, CompressionAlg comp_alg_in=auto, CompressionAlg comp_alg_out=auto, **comp_args):
    """
    Recompress WARC file.

    Currently supported compression algorithms: GZip and LZ4

    :param warc_in: input file name or stream object
    :param warc_out: output file name or stream object
    :param comp_alg_in: compression algorithm of input file
    :param comp_alg_out: compression algorithm to use for output file
    :param comp_args: keyword arguments pass on to the compressor
    :return: number of records recompressed and number of bytes written
    """
    cdef IOStream in_stream
    cdef IOStream out_stream

    if comp_alg_in == auto:
        comp_alg_in = _detect_comp_alg(warc_in)
    if comp_alg_out == auto:
        if type(warc_in) is str:
            comp_alg_out = _detect_comp_alg(warc_out)
        else:
            raise ValueError('Illegal output compression algorithm: auto')

    # Prepare input stream
    if type(warc_in) is str:
        in_stream = FileStream(warc_in, 'rb')
    elif isinstance(warc_in, IOStream):
        in_stream = <IOStream>warc_in
    elif hasattr(warc_in, 'read'):
        in_stream = PythonIOStreamAdapter(warc_in)
    else:
        raise TypeError(f'Object of type {type(warc_in).__name__} is not a valid input stream object')

    if comp_alg_in == gzip:
        in_stream = GZipStream(in_stream)
    elif comp_alg_in == lz4:
        in_stream = LZ4Stream(in_stream)

    # Prepare output stream
    if type(warc_out) is str:
        out_stream = FileStream(warc_out, 'wb')
    elif isinstance(warc_out, IOStream):
        out_stream = <IOStream>warc_out
    elif hasattr(warc_out, 'write'):
        out_stream = PythonIOStreamAdapter(warc_out)
    else:
        raise TypeError(f'Object of type {type(warc_out).__name__} is not a valid output stream object')

    if comp_alg_out == gzip:
        out_stream = GZipStream(out_stream, **comp_args)
    elif comp_alg_out == lz4:
        out_stream = LZ4Stream(out_stream, **comp_args)

    num = 0
    bytes_written = 0
    for record in ArchiveIterator(in_stream, parse_http=False, record_types=WarcRecordType.any_type):
        bytes_written += record.write(out_stream)
        # print(record.record_id, record.stream_pos)
        # print(record.record_id)
        num += 1

    in_stream.close()
    out_stream.close()

    return num, bytes_written
