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

from fastwarc.stream_io cimport IOStream, GZipStream, LZ4Stream, FileStream, PythonIOStreamAdapter
from fastwarc.stream_io import StreamError
from fastwarc.warc cimport ArchiveIterator, WarcRecordType


cpdef enum CompressionAlg:
    gzip,
    lz4,
    uncompressed,
    auto


def detect_compression_algorithm(infile_name):
    """
    Try to detect the used compression algorithm from the given filename.

    :param infile_name: input filename
    :return: compression algorithm
    """
    filename = None
    if type(infile_name) is str:
        filename = infile_name
    elif hasattr(infile_name, 'name'):
        filename = infile_name.name

    if type(filename) is str and filename.endswith('.gz'):
        return CompressionAlg.gzip
    elif type(filename) is str and filename.endswith('.lz4'):
        return CompressionAlg.lz4
    elif type(filename) is str and filename.endswith('.warc'):
        return CompressionAlg.uncompressed
    else:
        # Unknown, let stream negotiation try to determine the stream format
        return CompressionAlg.auto


def wrap_warc_stream(warc_in, mode='rb', CompressionAlg comp_alg=auto, **comp_args):
    """
    Wrap WARC stream based on type and desired compression algorithm for use with
    :class:`fastwarc.warc.ArchiveIterator`.

    :param warc_in: input stream or filename
    :param mode: stream open mode
    :param comp_alg: compression algorithm to use
    :param comp_args: optional arguments to pass on to the compressing stream
    :return: wrapped WARC stream
    """
    if type(warc_in) is str:
        stream = FileStream.__new__(FileStream, warc_in, mode)
    elif isinstance(warc_in, IOStream):
        stream = <IOStream>warc_in
    else:
        attr = 'write' if 'w' in mode else 'read'
        if not hasattr(warc_in, attr):
            raise AttributeError(f"Object of type '{type(warc_in).__name__}' has no attribute '{attr}'.")
        stream = PythonIOStreamAdapter.__new__(PythonIOStreamAdapter, warc_in)

    if comp_alg == gzip:
        stream = GZipStream.__new__(GZipStream, stream, **comp_args)
    elif comp_alg == lz4:
        stream = LZ4Stream.__new__(LZ4Stream, stream, **comp_args)

    return stream


def recompress_warc_interactive(warc_in, warc_out, CompressionAlg comp_alg_in=auto,
                                CompressionAlg comp_alg_out=auto, **comp_args):
    """
    Recompress WARC file.

    Currently supported compression algorithms: GZip and LZ4.

    :param warc_in: input file name or stream object
    :param warc_out: output file name or stream object
    :param comp_alg_in: compression algorithm of input file
    :param comp_alg_out: compression algorithm to use for output file
    :param comp_args: keyword arguments pass on to the compressor
    :return: generator of records and bytes written in each iteration
    """

    cdef IOStream in_stream
    cdef IOStream out_stream

    if comp_alg_in == auto:
        comp_alg_in = detect_compression_algorithm(warc_in)
    if comp_alg_out == auto:
        if type(warc_in) is str:
            comp_alg_out = detect_compression_algorithm(warc_out)
        else:
            raise StreamError('Illegal output compression algorithm: auto')

    in_stream = wrap_warc_stream(warc_in, 'rb', comp_alg_in)
    out_stream = wrap_warc_stream(warc_out, 'wb', comp_alg_out, **comp_args)

    num = 0
    # noinspection PyTypeChecker
    for record in ArchiveIterator.__new__(ArchiveIterator, in_stream,
                                          parse_http=False, record_types=WarcRecordType.any_type):
        bytes_written = record.write(out_stream)
        yield record, bytes_written

    in_stream.close()
    out_stream.close()


def recompress_warc(warc_in, warc_out, CompressionAlg comp_alg_in=auto, CompressionAlg comp_alg_out=auto, **comp_args):
    """
    Recompress WARC file.

    Currently supported compression algorithms: GZip and LZ4.
    This is a Non-interactive version of :func:`recompress_warc_interactive`.

    :param warc_in: input file name or stream object
    :param warc_out: output file name or stream object
    :param comp_alg_in: compression algorithm of input file
    :param comp_alg_out: compression algorithm to use for output file
    :param comp_args: keyword arguments pass on to the compressor
    :return: number of records recompressed and number of bytes written
    """

    bytes_written_total = 0
    num = 0
    for record, bytes_written in recompress_warc_interactive(warc_in, warc_out, comp_alg_in, comp_alg_out, **comp_args):
        bytes_written_total += bytes_written
        num += 1

    return num, bytes_written


def verify_digests(warc_in, bint verify_payloads=False, CompressionAlg comp_alg=auto):
    """
    Verify block or (optionally) payload digests of all records in a WARC.

    Returns a generator of dicts containing the following structure:
    ```
    {
        "record_id": <ID>,
        "block_digest_ok">: <OK_VAL>
        [ "payload_digest_ok">: <OK_VAL> ]
    }
    ```

    `<OK_VAL>` is either `True` or `False` or `None` if the record has no digest.

    :param warc_in: input WARC file
    :param verify_payloads: verify payload digests
    :param comp_alg: WARC compression algorithm
    :return: generator of dicts containing verification result data
    """

    if comp_alg == auto:
        comp_alg = detect_compression_algorithm(warc_in)

    in_stream = wrap_warc_stream(warc_in, 'rb', comp_alg)

    # noinspection PyTypeChecker
    for record in ArchiveIterator.__new__(ArchiveIterator, in_stream,
                                          parse_http=False, record_types=WarcRecordType.any_type):
        consume = not verify_payloads or not record.is_http
        res = {
            'record_id': record.record_id,
            'block_digest_ok': record.verify_block_digest(consume) if 'WARC-Block-Digest' in record.headers else None
        }

        if verify_payloads:
            if 'WARC-Payload-Digest' in record.headers:
                record.parse_http()
                res['payload_digest_ok'] = record.verify_payload_digest(True)
            else:
                res['payload_digest_ok'] = None

        yield res
