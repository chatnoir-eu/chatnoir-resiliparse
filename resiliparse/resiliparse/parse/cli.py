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

import codecs
import json
import time
import urllib.request

import click
from tqdm import tqdm

from fastwarc import ArchiveIterator, WarcRecordType
from resiliparse.parse.encoding import detect_encoding, bytes_to_str
from resiliparse.parse.html import HTMLTree


@click.group(context_settings=dict(help_option_names=['-h', '--help']))
def main():
    """Resiliparse Parsing module CLI."""
    pass


@main.command(short_help='Download WHATWG encoding mapping.')
def download_whatwg_mapping():
    """
    Download the current WHATWG encoding mapping, parse and transform it, and
    then print it as a copyable Python dict.
    """

    encodings = json.load(urllib.request.urlopen('https://encoding.spec.whatwg.org/encodings.json'))

    enc_mapped = {}
    for enc in encodings:
        for enc2 in enc['encodings']:
            n = enc2['name'].lower()
            try:
                if n == 'iso-8859-8-i':
                    n = 'iso-8859-8'
                    enc_mapped[n.lower()] = n
                elif n == 'windows-874':
                    n = 'iso-8859-11'
                n = codecs.lookup(n).name
                enc_mapped[n] = n
                for l in enc2['labels']:
                    enc_mapped[l] = n
            except LookupError:
                click.echo(f'skipped {enc2["name"]}', err=True)
    enc_mapped = dict((k, enc_mapped[k]) for k in sorted(enc_mapped))

    click.echo(enc_mapped)


@main.command(short_help='Benchmark Resiliparse HTML parser.')
@click.argument('warc_file', type=click.Path(dir_okay=False, exists=True))
def benchmark_html(warc_file):
    """
    Benchmark Resiliparse HTML parsing by extracting the titles from all HTML pages in a WARC file.

    You can compare the performance to Selectolax (both the old MyHTML and the new Lexbor engine) and
    BeautifulSoup4 by installing the respective packages:

    pip install selectolax beautifulsoup4
    """

    print('HTML parser benchmark <title> extraction:')
    print('=========================================')

    start = time.monotonic()
    for i, record in enumerate(tqdm(ArchiveIterator(open(warc_file, 'rb'), record_types=WarcRecordType.response),
                                    desc=f'Benchmarking Resiliparse (Lexbor)', unit=' docs',
                                    leave=False, mininterval=0.3)):
        content = record.reader.read()
        # noinspection PyStatementEffect
        HTMLTree.parse_from_bytes(content, detect_encoding(content)).title
    t = time.monotonic() - start
    print(f'Resiliparse (Lexbor):  {i} documents in {t:.2f}s ({i / t:.2f} documents/s)')

    try:
        from selectolax.lexbor import LexborHTMLParser as SelectolaxLexbor
        start = time.monotonic()
        for i, record in enumerate(tqdm(ArchiveIterator(open(warc_file, 'rb'), record_types=WarcRecordType.response),
                                        desc=f'Benchmarking Selectolax (Lexbor)', unit=' docs',
                                        leave=False, mininterval=0.3)):
            content = record.reader.read()
            SelectolaxLexbor(bytes_to_str(content, detect_encoding(content))).css_first('title')
        t = time.monotonic() - start
        print(f'Selectolax (Lexbor):   {i} documents in {time.monotonic() - start:.2f}s ({i / t:.2f} documents/s)')

        from selectolax.parser import HTMLParser as SelectolaxMyHTML
        start = time.monotonic()
        for i, record in enumerate(tqdm(ArchiveIterator(open(warc_file, 'rb'), record_types=WarcRecordType.response),
                                        desc=f'Benchmarking Selectolax (MyHTML)', unit=' docs',
                                        leave=False, mininterval=0.3)):
            content = record.reader.read()
            SelectolaxMyHTML(bytes_to_str(content, detect_encoding(content))).css_first('title')
        t = time.monotonic() - start
        print(f'Selectolax (MyHTML):   {i} documents in {time.monotonic() - start:.2f}s ({i / t:.2f} documents/s)')
    except ImportError:
        print('[Not benchmarking Selectolax, because it is not installed.')

    try:
        from bs4 import BeautifulSoup
        start = time.monotonic()
        for i, record in enumerate(tqdm(ArchiveIterator(open(warc_file, 'rb'), record_types=WarcRecordType.response),
                                        desc=f'Benchmarking BeautifulSoup4 (lxml)', unit=' docs',
                                        leave=False, mininterval=0.3)):
            content = record.reader.read()
            # noinspection PyStatementEffect
            BeautifulSoup(bytes_to_str(content, detect_encoding(content)), 'lxml').title
        t = time.monotonic() - start
        print(f'BeautifulSoup4 (lxml): {i} documents in {time.monotonic() - start:.2f}s ({i / t:.2f} documents/s)')
    except ImportError:
        print('[Not benchmarking BeautifulSoup4, because it is not installed.')


if __name__ == '__main__':
    main()
