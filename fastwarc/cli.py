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

from fastwarc import FastWARCError
from fastwarc.benchmark import benchmark
from fastwarc.tools import CompressionAlg
import fastwarc.tools as tools


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
    """Recompress a WARC file with different settings."""

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
        for _, b in tqdm(tools.recompress_warc_interactive(infile, outfile, decompress_alg, compress_alg, **comp_args),
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
    """Verify WARC consistency by checking all digests."""

    decompress_alg = getattr(CompressionAlg, decompress_alg)

    failed_digests = []
    num_total = 0
    num_no_digest = 0
    try:
        desc_tpl = 'Verifying digests ({} failed)'
        pbar = tqdm(tools.verify_digests(infile, verify_payloads, decompress_alg),
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


main.add_command(benchmark)
