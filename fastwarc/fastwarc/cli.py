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

import getpass
import importlib
from itertools import chain
import json
import os
import sys
import time
import urllib.request

import click
from tqdm import tqdm

from fastwarc.stream_io import FileStream, StreamError, FastWARCError, PythonIOStreamAdapter
from fastwarc.warc import ArchiveIterator, WarcRecordType
from fastwarc.tools import CompressionAlg, detect_compression_algorithm, wrap_warc_stream, \
    recompress_warc_interactive, verify_digests


def exception_handler(exctype, value, _):
    if exctype == FastWARCError:
        click.echo(str(value), err=True)
        sys.exit(1)

    raise value


sys.excepthook = exception_handler


@click.group(context_settings=dict(help_option_names=['-h', '--help']))
def main():
    """FastWARC Command Line Interface."""
    return 0


def _human_readable_bytes(byte_num):
    for unit in ['bytes', 'KiB', 'MiB', 'GiB', 'TiB']:
        if byte_num <= 1024 or unit == 'TiB':
            if int(byte_num) == byte_num:
                return f'{byte_num:d} {unit}'
            return f'{byte_num:.2f} {unit}'
        byte_num /= 1024


@main.command()
@click.argument('infile', type=click.Path(dir_okay=False, exists=True))
@click.argument('outfile', type=click.Path(dir_okay=False, exists=False))
@click.option('-c', '--compress-alg', type=click.Choice(['gzip', 'lz4', 'uncompressed', 'auto']),
              default='auto', show_default=True, help='Compression algorithm to use for output file')
@click.option('-d', '--decompress-alg', type=click.Choice(['gzip', 'lz4', 'uncompressed', 'auto']),
              default='auto', show_default=True,
              help='Decompression algorithm for decoding input file (auto tries to detect based on file extension)')
@click.option('-l', '--compress-level', type=int, default=None, help='Compression level (defaults to max)')
@click.option('-q', '--quiet', is_flag=True, help='Do not print progress information')
def recompress(infile, outfile, compress_alg, decompress_alg, compress_level, quiet):
    """
    Recompress a WARC file.

    This command allows you to recompress a WARC file if it is uncompressed or not compressed
    properly at the record-level if or you want to recompress a GZip WARC as LZ4 or vice versa.
    """

    compress_alg = getattr(CompressionAlg, compress_alg)
    decompress_alg = getattr(CompressionAlg, decompress_alg)

    comp_args = {}
    if compress_level is None:
        if compress_alg == CompressionAlg.gzip:
            comp_args['compression_level'] = 9
        elif compress_alg == CompressionAlg.lz4:
            comp_args['compression_level'] = 12

    bytes_written = 0
    num = 0
    start = time.time()
    try:
        for _, b in tqdm(recompress_warc_interactive(infile, outfile, decompress_alg, compress_alg, **comp_args),
                         desc='Recompressing WARC file', unit=' record(s)', leave=False,
                         disable=quiet, mininterval=0.2):
            num += 1
            bytes_written += b

        if not quiet:
            click.echo('Recompression completed.')
    except KeyboardInterrupt:
        if not quiet:
            click.echo('Recompression aborted.')
            sys.exit(1)
    finally:
        if not quiet and num > 0:
            click.echo(f'  - Records recompressed: {num}')
            click.echo(f'  - Bytes written: {_human_readable_bytes(bytes_written)}')
            click.echo(f'  - Completed in: {time.time() - start:.02f} seconds')


@main.command()
@click.argument('infile', type=click.Path(dir_okay=False, exists=True))
@click.option('-d', '--decompress-alg', type=click.Choice(['gzip', 'lz4', 'uncompressed', 'auto']),
              default='auto', show_default=True, help='Decompression algorithm')
@click.option('-p', '--verify-payloads', is_flag=True, help='Also verify payload digests')
@click.option('-q', '--quiet', is_flag=True, help='Do not print progress information')
@click.option('-o', '--output', type=click.File('w'), help='Output file with verification details')
def check(infile, decompress_alg, verify_payloads, quiet, output):
    """
    Verify WARC record consistency.

    Check digests of all records in a WARC file and print a summary. You can verify all block and
    payload digests in the given WARC file and print a summary of all corrupted and (optionally)
    all intact records.

    The command will exit with a non-zero exit code if at least one record fails verification.
    """

    decompress_alg = getattr(CompressionAlg, decompress_alg)

    failed_digests = []
    num_total = 0
    num_no_digest = 0
    try:
        desc_tpl = 'Verifying digests ({} failed)'
        pbar = tqdm(verify_digests(infile, verify_payloads, decompress_alg),
                    desc=desc_tpl.format(0), unit=' record(s)', leave=False, disable=quiet, mininterval=0.2)

        for v in pbar:
            num_total += 1

            status = 'OK'
            if v['block_digest_ok'] is False:
                status = 'FAIL'
            elif v['block_digest_ok'] is None:
                status = 'NO_DIGEST'

            if 'payload_digest_ok' in v:
                if v['payload_digest_ok'] is True:
                    status += ', PAYLOAD_OK'
                elif v['payload_digest_ok'] is False:
                    status += ', PAYLOAD_FAIL'
                elif v['payload_digest_ok'] is None:
                    status += ', PAYLOAD_NO_DIGEST'

            if 'FAIL' in status:
                failed_digests.append(v['record_id'])
                pbar.set_description(desc_tpl.format(len(failed_digests)))
            elif 'NO_DIGEST' in status and 'OK' not in status:
                num_no_digest += 1

            if output:
                output.write(f'{v["record_id"]}: {status}\n')

        if output:
            output.close()

    except KeyboardInterrupt:
        if not quiet:
            click.echo('Verification aborted.')
            sys.exit(1)
    finally:
        if not quiet and num_total > 0:
            if not failed_digests:
                click.echo(f'{num_total - num_no_digest} records were verified successfully.')
                if num_no_digest:
                    click.echo(f'{num_no_digest} records were skipped without digest.')
            else:
                click.echo('Failed records:')
                click.echo('===============')
                for rec in failed_digests:
                    click.echo(rec)
                sys.exit(1)


@main.command()
@click.argument('infile', type=click.Path(dir_okay=False, exists=True))
@click.argument('offset', type=int)
@click.option('-o', '--output', type=click.File('wb'), default=sys.stdout.buffer,
              help='Output file, default is stdout')
@click.option('--payload', is_flag=True,
              help='Output only record payload (transfer and/or content encoding are preserved')
@click.option('--headers', is_flag=True, help='Output only record (and HTTP) headers')
def extract(infile, offset, output, payload, headers):
    """
    Extract WARC record by offset.

    You can extract individual records at a given byte offset with either just
    headers, payload, or both.
    """

    compl_record = not (payload or headers)
    with open(infile, 'rb') as stream:
        stream.seek(offset)
        try:
            record = next(ArchiveIterator(stream))
            if compl_record:
                record.write(output)
            elif headers:
                record.headers.write(PythonIOStreamAdapter(output))
                output.write(b'\r\n')
                if record.is_http:
                    record.http_headers.write(PythonIOStreamAdapter(output))
                    output.write(b'\r\n')
            elif payload:
                buf = record.reader.read(4096)
                while buf:
                    output.write(buf)
                    buf = record.reader.read(4096)
        except StopIteration:
            return
        except (FastWARCError, OSError, ValueError) as e:
            click.echo(f'Failed to extract WARC record at offset {offset}: {str(e)}', err=True)


def _index_record(output, fields, preserve_multi_header, record, next_record_offset, file_name):
    idx = dict()
    for f in fields:
        f = f.strip().lower()

        if f == 'offset':
            idx[f] = str(record.stream_pos)
        elif f == 'length':
            idx[f] = str(next_record_offset - record.stream_pos)
        elif f == 'filename':
            idx[f] = file_name
        elif f == 'http:status' and record.is_http:
            idx[f] = str(record.http_headers.status_code)
        elif f.startswith('http:') and record.is_http:
            if preserve_multi_header:
                l = []
                for k, v in record.http_headers:
                    if k == f[5:]:
                        l.append(v)
                if len(l) == 1:
                    idx[f] = l[0]
                elif len(l) > 1:
                    idx[f] = l
            else:
                idx[f] = record.http_headers.get(f[5:])
        elif f in record.headers:
            idx[f] = record.headers.get(f)

    output.write(json.dumps(idx) + '\n')


@main.command()
@click.argument('infiles', type=click.Path(dir_okay=False, exists=True), nargs=-1)
@click.option('-o', '--output', type=click.File('w'), default=sys.stdout,
              help='Output file, default is stdout')
@click.option('-f', '--fields', type=str, metavar='FIELDS',
              default='offset,warc-type,warc-target-uri', show_default=True,
              help='Comma-separated list of indexed fields, eg. "offset", "length", "filename", '
                   '"http:status", "http:<http-header>", or "<warc-record-header>"')
@click.option('--preserve-multi-header', is_flag=True,
              help='Preserve multiple values of HTTP headers as JSON list')
def index(infiles, output, fields, preserve_multi_header):
    """
    Index WARC records as CDXJ.

    WARC files can be indexed to the `CDXJ <https://github.com/webrecorder/cdxj-indexer>`_
    format with a configurable set of fields.
    """
    fields = fields.split(',')
    for infile in infiles:
        with open(infile, 'rb') as stream:
            prev_record = None
            for record in ArchiveIterator(stream):
                if prev_record is not None:
                    _index_record(output, fields, preserve_multi_header, prev_record, record.stream_pos, infile)
                prev_record = record

            if prev_record is not None:
                _index_record(output, fields, preserve_multi_header, prev_record, prev_record.reader.tell(), infile)


boto3 = None
botocore = None
s3 = None


def _init_s3(endpoint_url, aws_access_key,  aws_secret_key=None):
    global boto3, botocore, s3

    if s3 is not None:
        return

    try:
        boto3 = importlib.import_module('boto3')
        botocore = importlib.import_module('botocore')
    except ModuleNotFoundError:
        raise click.UsageError('Boto3 needs to be installed for S3 benchmarking.')

    if not aws_access_key:
        raise click.UsageError("s3:// URLs require '--aws-access-key' to be set.")
    if not aws_secret_key:
        aws_secret_key = getpass.getpass()

    s3 = boto3.resource('s3', endpoint_url=endpoint_url, aws_access_key_id=aws_access_key,
                        aws_secret_access_key=aws_secret_key)


# noinspection PyProtectedMember, HttpUrlsUsage
def _get_raw_stream_from_url(input_url, use_python_stream=False):
    if input_url.startswith('s3://'):
        assert s3 is not None
        try:
            s3_bucket, s3_object = input_url[5:].split('/', 1)
            stream = s3.Object(s3_bucket, s3_object).get()['Body']._raw_stream
        except botocore.exceptions.ClientError as e:
            click.echo(f'Error connecting to S3. Message: {e}.', err=True)
            sys.exit(1)

    elif input_url.startswith('http://') or input_url.startswith('https://'):
        stream = urllib.request.urlopen(input_url)

    else:
        if input_url.startswith('file://'):
            input_url = input_url[7:]
        if not os.path.isfile(input_url):
            click.echo(f"No such file or directory: '{input_url}", err=True)
            sys.exit(1)

        stream = FileStream(input_url, 'rb') if not use_python_stream else open(input_url, 'rb')

    return stream


def _expand_s3_prefixes(input_urls, endpoint_url, aws_access_key, aws_secret_key):
    urls_expanded = []
    for url in input_urls:
        if url.startswith('s3://'):
            _init_s3(endpoint_url, aws_access_key, aws_secret_key)
            s3_bucket, s3_object_prefix = url[5:].split('/', 1)
            urls_expanded.extend('/'.join(('s3:/', s3_bucket, o.key))
                                 for o in s3.Bucket(s3_bucket).objects.filter(Prefix=s3_object_prefix))
        else:
            urls_expanded.append(url)
    return urls_expanded


# noinspection PyPackageRequirements
@main.command()
@click.argument('input_url', nargs=-1)
@click.option('-d', '--decompress-alg', type=click.Choice(['gzip', 'lz4', 'uncompressed', 'auto']),
              default='auto', show_default=True, help='Decompression algorithm')
@click.option('-e', '--endpoint-url', help='S3 endpoint URL', default='https://s3.amazonaws.com', show_default=True)
@click.option('-a', '--aws-access-key', help='AWS access key for s3:// URLs')
@click.option('-s', '--aws-secret-key', help='AWS secret key for s3:// URLs (leave empty to read from STDIN)')
@click.option('--is-prefix', is_flag=True, help='Treat input URL as prefix (only for S3)')
@click.option('-p', '--use-python-stream', is_flag=True,
              help='Use slower Python I/O instead of native FileStream for local files')
@click.option('-H', '--parse-http', is_flag=True, help='Parse HTTP headers')
@click.option('-v', '--verify-digests', is_flag=True, help='Verify record block digests')
@click.option('-f', '--filter-type', type=click.Choice(['warcinfo', 'response', 'resource', 'request', 'metadata',
                                                        'revisit', 'conversation', 'continuation', 'any_type']),
              default=['any_type'], multiple=True, show_default=True, help='Filter for specific WARC record types')
@click.option('-w', '--bench-warcio', is_flag=True, help='Compare FastWARC performance with WARCIO')
def benchmark(input_url, decompress_alg, endpoint_url, aws_access_key, aws_secret_key, is_prefix, use_python_stream,
              verify_digests, parse_http, filter_type, bench_warcio):
    """
    Benchmark FastWARC read performance.

    The FastWARC CLI comes with a benchmarking tool for measuring WARC record decompression and parsing
    performance on your own machine. The benchmarking results can be compared directly with WARCIO.

    Supported WARC sources are local files, S3 and HTTP(s) URLs. Supported compression algorithms
    are GZip, LZ4, or uncompressed.

    The read benchmarking tool has additional options, such as reading WARCs directly from a remote S3 data source
    using `Boto3 <https://boto3.amazonaws.com/v1/documentation/api/latest/index.html>`_.

    See :ref:`FastWARC Benchmarks <fastwarc-benchmarks>` for more information and example benchmarking results.
    """

    if not input_url:
        raise click.UsageError('Need at least one input path.')

    if is_prefix:
        input_url = _expand_s3_prefixes(input_url, endpoint_url, aws_access_key, aws_secret_key)
    else:
        for u in input_url:
            if u.startswith('s3://'):
                _init_s3(endpoint_url, aws_access_key, aws_secret_key)
                break

    if decompress_alg == 'auto':
        decompress_alg = detect_compression_algorithm(input_url[0])
    else:
        decompress_alg = getattr(CompressionAlg, decompress_alg)

    if bench_warcio:
        if decompress_alg == CompressionAlg.lz4:
            click.echo('WARCIO does not support LZ4.', err=True)
            sys.exit(1)

        try:
            import warcio
        except ModuleNotFoundError:
            click.echo("--bench-warcio requires 'warcio' to be installed.", err=True)
            sys.exit(1)

    click.echo(f'Benchmarking read performance from {len(input_url)} input path(s)...')

    rec_type_filter = WarcRecordType.any_type
    if 'any_type' not in filter_type:
        rec_type_filter = WarcRecordType.no_type
        for t in filter_type:
            rec_type_filter |= getattr(WarcRecordType, t)

    def _bench(urls, stream_loader, lib):
        interrupted = False
        start = time.monotonic()
        num = 0

        try:
            def _load_lazy(u, l):
                yield from l(u)

            for _ in tqdm(chain(*(_load_lazy(u, stream_loader) for u in urls)),
                          desc=f'Benchmarking {lib}', unit=' records', leave=False, mininterval=0.5):
                num += 1
        except StreamError as e:
            click.echo(f'Error reading input: {e}', err=True)
            sys.exit(1)
        except KeyboardInterrupt:
            click.echo(f'Benchmark interrupted.', err=True)
            interrupted = True

        return num, time.monotonic() - start, interrupted

    def _fastwarc_iterator(f):
        s = _get_raw_stream_from_url(f, use_python_stream)
        s = wrap_warc_stream(s, 'rb', decompress_alg)
        return ArchiveIterator(s, rec_type_filter, parse_http=parse_http, verify_digests=verify_digests)

    n, t_fastwarc, interrupted = _bench(input_url, _fastwarc_iterator, 'FastWARC')
    click.echo(f'FastWARC: {n:,} records read in {t_fastwarc:.02f} seconds ({n / t_fastwarc:,.02f} records/s).')

    if interrupted:
        sys.exit(1)

    if bench_warcio:
        def _warcio_iterator(f):
            s = _get_raw_stream_from_url(f, True)
            if rec_type_filter == WarcRecordType.any_type:
                yield from warcio.ArchiveIterator(s, no_record_parse=not parse_http, check_digests=verify_digests)
            else:
                # WARCIO does not support record filtering out of the box
                for rec in warcio.ArchiveIterator(s, not parse_http):
                    if rec_type_filter & getattr(WarcRecordType, rec.rec_type) != 0:
                        yield rec

        n, t_warcio, _ = _bench(input_url, _warcio_iterator, 'WARCIO')
        click.echo(f'WARCIO:   {n:,} records read in {t_warcio:.02f} seconds ({n / t_warcio:,.02f} records/s).')
        click.echo(f'Time difference: {t_fastwarc - t_warcio:.02f} seconds, speedup: {t_warcio / t_fastwarc:,.02f}')


if __name__ == '__main__':
    main()
