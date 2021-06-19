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

import sys
import time

import click
from tqdm import tqdm

from fastwarc.tools import CompressionAlg, recompress_warc_interactive


@click.group(context_settings=dict(help_option_names=['-h', '--help']))
def main():
    pass


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
@click.option('--in-compress-alg', type=click.Choice(['gzip', 'lz4', 'uncompressed', 'auto']),
              default='auto', show_default=True,
              help='Compression algorithm for decoding input file (auto tries to detect based on file extension)')
@click.option('-l', '--compress-level', type=int, default=None, help='Compression level (defaults to max)')
@click.option('-q', '--quiet', is_flag=True, help='Do not print progress information')
def recompress(infile, outfile, compress_alg, in_compress_alg, compress_level, quiet):
    compress_alg = getattr(CompressionAlg, compress_alg)
    compress_alg_in = getattr(CompressionAlg, in_compress_alg)

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
        for _, b in tqdm(recompress_warc_interactive(infile, outfile, compress_alg_in, compress_alg, **comp_args),
                         desc='Recompressing WARC file', unit=' record(s)', leave=False, disable=quiet, mininterval=0.2):
            num += 1
            bytes_written += b

        if not quiet:
            click.echo('Recompression completed.')
    except KeyboardInterrupt:
        if not quiet:
            click.echo('Recompression aborted.')
    finally:
        if not quiet:
            click.echo(f'  - Records recompressed: {num}')
            click.echo(f'  - Bytes written: {_human_readable_bytes(bytes_written)}')
            click.echo(f'  - Completed in: {time.time() - start:.02f} seconds')
