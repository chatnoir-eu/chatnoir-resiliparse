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
from joblib import Parallel, delayed
import hashlib
import os
import re
import time
import tempfile
import unicodedata
import urllib.request
from urllib.error import HTTPError

import click
from tqdm import tqdm

from fastwarc import ArchiveIterator, WarcRecordType
from resiliparse.parse.encoding import detect_encoding, bytes_to_str
from resiliparse.parse.html import HTMLTree
import resiliparse.parse.lang as rlang


@click.group(context_settings=dict(help_option_names=['-h', '--help']))
def main():
    """Resiliparse Parsing module CLI."""
    pass


@main.group()
def encoding():
    """Encoding tools CLI"""
    pass


@encoding.command(short_help='Download WHATWG encoding mapping.')
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


@main.group()
def html():
    """HTML tools CLI"""
    pass


@html.command(short_help='Benchmark Resiliparse HTML parser.')
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


@main.group()
def lang():
    """Language tools CLI"""
    pass


DEFAULT_WIKI_LANGS = ('en,ru,zh,de,vi,fr,sv,uk,pt,fa,sr,it,pl,ja,nl,es,ca,sh,ko,tr,fi,cs,no,he,id,hu,el,ro,th,ar,bg,hy,'
                      'da,bn,cy,eo,hr,ka,eu,gl,hi,be,sk,ur,ms,ml,mk,et,te,lt,ce,az,tt,kk,sl,ta,lv,uz,mr,tl,my,oc,vo,tg,'
                      'bs,ne,is,ku,mg,pa,nn,sw,ga,fy,la,li,af,io,qu,os,sa,gd,yo,wa,sq,zu,ve,yi,wo,lo,kw,ln,gu,ti,ha,mn,'
                      'ky,su,lg,ba,kn,ee,ie,se,sc,or,am,fo,kv,ps,sm,za,lb,dv,rm,br,ch,cu,iu,sn,an,jv,mt,mi,om,ki,km,ia,'
                      'ss,si,rw,bo,st,so,kl,sg,na,ht,cr,as,av,rn,sd,bh,tw,tk,ig,ug,ff,ny,gn,to,cv,ks,xh,bi,dz,pi,ay,co,'
                      'ty,fj,ik,nv,ak,ab,gv,tn,kg,bm,ts')


@lang.command(short_help='Download Wikipedia dumps for language detection.')
@click.argument('dumpdate')
@click.option('-l', '--langs', help='Comma-separated list of languages to download', default=DEFAULT_WIKI_LANGS,
              show_default=True)
@click.option('-o', '--outdir', help='Output directory', default='wikidumps', type=click.Path(file_okay=False))
@click.option('-j', '--jobs', help='Parallel download jobs (3 is the Wikimedia rate limit)', default=3, type=int)
def download_wiki_dumps(dumpdate, langs, outdir, jobs):
    """
    Download the first Wikipedia article multistream part for each of the specified languages.

    The downloaded dumps can then be extracted with `Wikiextractor <https://github.com/attardi/wikiextractor>`_.
    """
    if not os.path.isdir(outdir):
        os.makedirs(outdir)

    def dl(l):
        try:
            with urllib.request.urlopen(f'https://dumps.wikimedia.org/{l}wiki/{dumpdate}/dumpstatus.json') as metaf:
                metadata = json.load(metaf)

            if 'articlesmultistreamdump' not in metadata['jobs']:
                return
            metadata = next(iter(metadata['jobs']['articlesmultistreamdump']['files'].values()))
            url = metadata['url']
            md5sum = metadata['md5']
            total_bytes = metadata['size']
            md5hash = hashlib.md5()
            outfilepath = os.path.join(outdir, f'{l}wiki.{url.split(".")[-1]}')
            with open(outfilepath, 'wb') as outf:
                    with urllib.request.urlopen(f'https://dumps.wikimedia.org{url}') as dumpf:
                        blocksize = 4096
                        with tqdm(desc=f'Downloading {l}wiki', unit='b', unit_scale=True, total=total_bytes,
                                  leave=False) as dlprog:
                            while buf := dumpf.read(blocksize):
                                md5hash.update(buf)
                                outf.write(buf)
                                dlprog.update(len(buf))
            if md5hash.hexdigest() != md5sum:
                click.echo(f'Output "{os.path.basename(outfilepath)}" corrupted, deleting it.', err=True)
                os.unlink(outfilepath)
        except HTTPError as e:
            click.echo(f'Error downloading {l}wiki: {e}', err=True)

    Parallel(n_jobs=jobs, verbose=0, prefer='threads')(delayed(dl)(l.strip()) for l in langs.split(','))


@lang.command(short_help='Create a language detection dataset.')
@click.argument('indir')
@click.argument('outdir')
@click.option('--val-size', help='Portion of the data to use for validation', default=5, type=int)
@click.option('--test-size', help='Portion of the data to use for testing', default=5, type=int)
@click.option('--min-examples', help='Minimum number of examples per language', default=50000,
              show_default=True, type=int)
@click.option('-j', '--jobs', help='Parallel jobs', default=4, type=int)
def create_dataset(indir, outdir, val_size, test_size, min_examples, jobs):
    """
    Create a language detection dataset from a set of extracted Wikipedia article dumps.

    Expected is a directory containing one subdirectory per language (with the language name,
    e.g. "en" or "enwiki") with any number of subdirectories and ``wiki_*`` plaintext files. Use
    `Wikiextractor <https://github.com/attardi/wikiextractor>`_ for creating the plaintext
    directories for each language.
    
    Empty lines and ``<doc>`` tags will be stripped from the plaintext, otherwise the texts
    are expected to be clean already.

    The created dataset will consist of one directory for each language, each containing three files
    for train, validation, and test with one example per line. The order of the lines is randomized.
    """
    if not os.path.isdir(outdir):
        os.makedirs(outdir)

    langdirs = [os.path.join(indir, d) for d in os.listdir(indir) if os.path.isdir(os.path.join(indir, d))]
    Parallel(n_jobs=jobs, verbose=10, backend='multiprocessing')(
        delayed(_process_raw_lang_dir)(l, outdir, val_size, test_size, min_examples) for l in langdirs)


def _process_raw_lang_dir(indir, outdir_base, val_size, test_size, min_examples):
    lang_name = os.path.basename(indir).replace('wiki', '')
    outdir = os.path.join(outdir_base, lang_name)

    if not os.path.isdir(outdir):
        os.makedirs(outdir)

    def dir_walk():
        for d, f in ((d[0], d[2]) for d in os.walk(indir)):
            for fi in f:
                yield os.path.join(d, fi)

    def hash_fname(base_dir, hash_hex):
        d = os.path.join(base_dir, hash_hex[:2], hash_hex[2:4])
        os.makedirs(d, exist_ok=True)
        return os.path.join(d, hash_hex)

    wiki_markup_re = re.compile(r'(\[\[|\]\])')
    try:
        with tempfile.TemporaryDirectory(dir=outdir) as tempdir:
            line_hashes = set()
            click.echo(f'Parsing input files for {lang_name}...', err=True)
            for infile_name in dir_walk():
                if not os.path.basename(infile_name).startswith('wiki_'):
                    continue

                with open(infile_name, 'r') as infile:
                    for line in infile:
                        line = wiki_markup_re.sub('', unicodedata.normalize('NFKC', line))
                        if len(line) < 200 or line.startswith('<doc id=') or line.startswith('</doc>'):
                            continue

                        h = hashlib.sha1()
                        h.update(line.encode())
                        with open(hash_fname(tempdir, h.hexdigest()), 'w') as outfile:
                            outfile.write(line)
                        line_hashes.add(h.digest())

            if len(line_hashes) < min_examples:
                click.echo(f'Not enough examples for creating a dataset for {lang_name} ({len(line_hashes)}).',
                           err=True)
                return

            train_name = os.path.join(outdir, 'train.txt')
            val_name = os.path.join(outdir, 'val.txt')
            test_name = os.path.join(outdir, 'test.txt')
            test_end = int(len(line_hashes) * test_size * 0.01)
            val_end = test_end + int(len(line_hashes) * val_size * 0.01)

            with open(test_name, 'w') as testf, open(val_name, 'w') as valf, open(train_name, 'w') as trainf:
                click.echo(f'Writing examples for {lang_name}...', err=True)
                for i, h in enumerate(line_hashes):
                    hname = hash_fname(tempdir, h.hex())
                    if i < test_end:
                        testf.write(open(hname).read())
                    elif test_end < i < val_end:
                        valf.write(open(hname).read())
                    else:
                        trainf.write(open(hname).read())
                    os.unlink(hname)
            click.echo(f'Examples for {lang_name} finished.', err=True)
    finally:
        if not os.listdir(outdir):
            os.rmdir(outdir)


@lang.command(short_help='Train fast language detection model vectors')
@click.argument('indir')
@click.option('-s', '--in-split', help='Which input split to use', default='train',
              type=click.Choice(['train', 'test', 'val']), show_default=True)
@click.option('-f', '--out-format', help='Output format (raw vectors or C code)', default='raw',
              type=click.Choice(['raw', 'c']), show_default=True)
@click.option('-l', '--vector-size', help='Output vector size', default=200, type=int, show_default=True)
def train_vectors(indir, in_split, out_format, vector_size):
    """
    Train and print vectors for fast language detection.

    Expects the following directory structure:

    ..
        indir/
            <LANGCODE 1>/
                test.txt
                train.txt
                val.txt
            <LANGCODE 2>/
                test.txt
                train.txt
                val.txt
            ...
    """

    langs = sorted(os.listdir(indir))
    if out_format == 'c':
        click.echo(f'''/* Resiliparse fast language detection profiles. */

#ifndef RESILIPARSE_LANG_PROFILES_H
#define RESILIPARSE_LANG_PROFILES_H

#define LANG_VEC_SIZE {vector_size}
typedef const uint8_t lang_vec_t[LANG_VEC_SIZE];

typedef struct lang {{
    const char* lang;
    const lang_vec_t vec;
}} lang_t;

static const lang_t LANGS[] = {{''')

    for i, l in enumerate(langs):
        if out_format == 'c':
            if i > 0:
                click.echo(',')
            click.echo('    ', nl=False)

        vec = rlang.train_language_examples(open(os.path.join(indir, l, in_split + '.txt'), 'r'), vector_size)
        if out_format == 'c':
            click.echo(f'{{"{l}", {{{", ".join(str(i) for i in vec)}}}}}', nl=False)
        else:
            click.echo(vec)

    if out_format == 'c':
        click.echo('\n};\n')
        click.echo('#define N_LANGS sizeof(LANGS) / sizeof(lang_t)\n')
        click.echo('#endif  // RESILIPARSE_LANG_PROFILES_H')


if __name__ == '__main__':
    main()
