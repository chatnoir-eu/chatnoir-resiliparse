import time

import click
from tqdm import tqdm

from fastwarc import ArchiveIterator, WarcRecordType
from resiliparse.parse.encoding import detect_encoding, bytes_to_str
from resiliparse.parse.html import HTMLTree


@click.command()
@click.argument('warc_file', type=click.Path(dir_okay=False, exists=True))
def benchmark(warc_file):
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
    benchmark()
