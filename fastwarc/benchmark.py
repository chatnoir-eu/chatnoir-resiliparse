#!/usr/bin/env python3
#
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

from itertools import chain
import getpass
import importlib
import os
import sys
import time
import urllib.request

import botocore.exceptions
import click
from tqdm import tqdm

from fastwarc.stream_io import FileStream, StreamError
from fastwarc.warc import ArchiveIterator, WarcRecordType
from fastwarc.tools import CompressionAlg, detect_compression_algorithm, wrap_warc_stream


@click.group()
def benchmark():
    """Benchmark FastWARC performance."""
    return 0


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
@benchmark.command()
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
def read(input_url, decompress_alg, endpoint_url, aws_access_key, aws_secret_key, is_prefix, use_python_stream,
         verify_digests, parse_http, filter_type, bench_warcio):
    """
    Benchmark WARC read performance.

    Supported WARC sources are local files, S3 and HTTP(s) URLs.
    Supported compression algorithms are GZip, LZ4, or uncompressed.
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
                          desc=f'Benchmarking {lib}', unit=' records', leave=False, mininterval=0.3):
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
    click.echo(f'FastWARC: {n} records read in {t_fastwarc:.02f} seconds ({n / t_fastwarc:.02f} records/s).')

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
        click.echo(f'WARCIO:   {n} records read in {t_warcio:.02f} seconds ({n / t_warcio:.02f} records/s).')
        click.echo(f'Time difference: {t_fastwarc - t_warcio:.02f} seconds, speedup: {t_warcio / t_fastwarc:.02f}')
