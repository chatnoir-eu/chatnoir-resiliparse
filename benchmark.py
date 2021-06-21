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

import getpass
import os
import sys
import time
import urllib.request

import botocore.exceptions
import click
from tqdm import tqdm

from fastwarc.stream_io import FileStream
from fastwarc.warc import ArchiveIterator, WarcRecordType
from fastwarc.tools import CompressionAlg, IllegalCompressionAlgorithmError, \
    detect_compression_algorithm, wrap_warc_stream


@click.group(context_settings=dict(help_option_names=['-h', '--help']))
def main():
    return 0


def _get_raw_stream_from_url(input_url, endpoint_url=None, aws_access_key=None,  aws_secret_key=None,
                             use_python_stream=False):

    # noinspection HttpUrlsUsage
    if input_url.startswith('s3://'):
        try:
            import boto3
            import botocore
        except ModuleNotFoundError:
            raise click.UsageError('Boto3 needs to be installed for S3 benchmarking.')

        if not aws_access_key:
            raise click.UsageError("s3:// URLs require '--aws-access-key' to be set.")
        if not aws_secret_key:
            aws_secret_key = getpass.getpass()

        try:
            s3 = boto3.resource('s3', endpoint_url=endpoint_url, aws_access_key_id=aws_access_key,
                                aws_secret_access_key=aws_secret_key)
            s3_bucket, s3_object = input_url[5:].split('/', 2)
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


# noinspection PyPackageRequirements
@main.command()
@click.argument('input_url')
@click.option('-d', '--decompress-alg', type=click.Choice(['gzip', 'lz4', 'uncompressed', 'auto']),
              default='auto', show_default=True, help='Decompression algorithm')
@click.option('-e', '--endpoint-url', help='S3 endpoint URL', default='https://s3.amazonaws.com', show_default=True)
@click.option('-a', '--aws-access-key', help='AWS access key for s3:// URLs')
@click.option('-s', '--aws-secret-key', help='AWS secret key for s3:// URLs (leave empty to read from STDIN)')
@click.option('-p', '--use-python-stream', is_flag=True,
              help='Use slower Python I/O instead of native FileStream for local files')
@click.option('-H', '--parse-http', is_flag=True, help='Parse HTTP headers')
@click.option('-f', '--filter-type', type=click.Choice(['warcinfo', 'response', 'resource', 'request', 'metadata',
                                                        'revisit', 'conversation', 'continuation', 'any_type']),
              default=['any_type'], multiple=True, show_default=True, help='Filter for specific WARC record types')
@click.option('-w', '--bench-warcio', is_flag=True, help='Compare FastWARC performance with WARCIO')
def read(input_url, decompress_alg, endpoint_url, aws_access_key, aws_secret_key, use_python_stream,
         parse_http, filter_type, bench_warcio):
    """
    Benchmark WARC read performance.

    Supported WARC sources are local files, S3 and HTTP(s) URLs.
    Supported compression algorithms are GZip, LZ4, or uncompressed.
    """

    try:
        if decompress_alg == 'auto':
            decompress_alg = detect_compression_algorithm(input_url)
        else:
            decompress_alg = getattr(CompressionAlg, decompress_alg)
    except IllegalCompressionAlgorithmError:
        click.echo('Could not auto-detect compression algorithm.', err=True)
        sys.exit(1)

    if bench_warcio:
        if decompress_alg == CompressionAlg.lz4:
            click.echo('WARCIO does not support LZ4.', err=True)
            sys.exit(1)

        try:
            import warcio
        except ModuleNotFoundError:
            click.echo("--use-warcio requires 'warcio' to be installed.", err=True)
            sys.exit(1)

    stream = _get_raw_stream_from_url(input_url, endpoint_url, aws_access_key, aws_secret_key, use_python_stream)
    stream = wrap_warc_stream(stream, 'rb', decompress_alg)

    rec_type_filter = WarcRecordType.any_type
    if 'any_type' not in filter_type:
        rec_type_filter = WarcRecordType.no_type
        for t in filter_type:
            rec_type_filter |= getattr(WarcRecordType, t)

    def _bench(it, lib):
        start = time.monotonic()
        num = 0
        for _ in tqdm(it, desc=f'Benchmarking {lib}', unit=' records', leave=False):
            num += 1
        return num, time.monotonic() - start

    archive_it_fastwarc = ArchiveIterator(stream, parse_http, rec_type_filter)
    n, t_fastwarc = _bench(archive_it_fastwarc, 'FastWARC')
    click.echo(f'FastWARC: {n} records read in {t_fastwarc:.02f} seconds ({n / t_fastwarc:.02f} records/s).')

    if bench_warcio:
        stream = _get_raw_stream_from_url(input_url, endpoint_url, aws_access_key, aws_secret_key, True)
        archive_it_warcio = warcio.ArchiveIterator(stream, not parse_http)
        n, t_warcio = _bench(archive_it_warcio, 'WARCIO')
        click.echo(f'WARCIO:   {n} records read in {t_warcio:.02f} seconds ({n / t_warcio:.02f} records/s).')
        click.echo(f'Time difference: {t_fastwarc - t_warcio:.02f} seconds, speedup: {t_warcio / t_fastwarc:.02f}')


if __name__ == '__main__':
    sys.exit(main())
