# Copyright 2026 Janek Bevendorff
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
from collections import defaultdict
import json
import hashlib
import io
import os
from math import fsum
import re
import time
import tempfile
import unicodedata
import urllib.request
from urllib.error import HTTPError
import sys

from fastwarc import ArchiveIterator, WarcRecordType
from resiliparse.parse.encoding import detect_encoding, bytes_to_str
from resiliparse.parse.html import HTMLTree
import resiliparse.parse.lang as rlang

# Optional 'cli' dependencies
MISSING_DEPS = False
try:
    import click
except ModuleNotFoundError:
    print('Missing dependency: click', file=sys.stderr)
    print('Install Resiliparse with the "cli" flag: pip install "resiliparse[cli]"', file=sys.stderr)
    sys.exit(1)

try:
    from tqdm import tqdm
    import joblib
except ModuleNotFoundError:
    MISSING_DEPS = True


def _fasttext_native_predict(model, text):
    """Return FastText's raw native predictions for a single line of text."""
    return model.f.predict(text.replace('\n', ' ') + '\n', 1, 0.0, 'strict')


@click.group(context_settings=dict(help_option_names=['-h', '--help']))
def main():
    """Resiliparse Command Line Interface."""

    if MISSING_DEPS:
        click.echo('Missing CLI dependencies!', err=True)
        click.echo('Install Resiliparse with the "cli" flag: pip install "resiliparse[cli]"', err=True)
        sys.exit(1)


@main.group()
def encoding():
    """Encoding module tools."""
    pass


@encoding.command()
def download_whatwg_mapping():
    """
    Download WHATWG encoding mapping.

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
    """HTML module tools."""
    pass


@html.command()
@click.argument('warc_file', type=click.Path(dir_okay=False, exists=True))
def benchmark(warc_file):
    """
    Benchmark Resiliparse HTML parser.

    Benchmark Resiliparse HTML parsing by extracting the titles from all HTML pages in a WARC file.

    You can compare the performance to Selectolax's Lexbor engine and BeautifulSoup4 by installing
    the PyPi packages ``selectolax`` and ``beautifulsoup4``. Install
    Resiliparse with the ``cli-benchmark`` flag to install all optional third-party dependencies automatically.

    See :ref:`Resiliparse HTML Parser Benchmarks <parse-html-benchmark>` for more details
    and example benchmarking results.
    """

    print('HTML parser benchmark <title> extraction:')
    print('=========================================')

    start = time.monotonic()
    i = 0
    for i, record in enumerate(tqdm(ArchiveIterator(warc_file, record_types=WarcRecordType.response),
                                    desc=f'Benchmarking Resiliparse (Lexbor)', unit=' docs',
                                    leave=False, mininterval=0.3), start=1):
        content = record.reader.read()
        # noinspection PyStatementEffect
        HTMLTree.parse_from_bytes(content, detect_encoding(content) or 'utf-8').title
    t = time.monotonic() - start
    print(f'Resiliparse (Lexbor):  {i} documents in {t:.2f}s ({i / t:.2f} documents/s)')

    try:
        from selectolax.lexbor import LexborHTMLParser as SelectolaxLexbor
        start = time.monotonic()
        i = 0
        for i, record in enumerate(tqdm(ArchiveIterator(warc_file, record_types=WarcRecordType.response),
                                        desc=f'Benchmarking Selectolax (Lexbor)', unit=' docs',
                                        leave=False, mininterval=0.3), start=1):
            content = record.reader.read()
            SelectolaxLexbor(bytes_to_str(content, detect_encoding(content) or 'utf-8')).css_first('title')
        t = time.monotonic() - start
        print(f'Selectolax (Lexbor):   {i} documents in {time.monotonic() - start:.2f}s ({i / t:.2f} documents/s)')

    except ModuleNotFoundError:
        print('[Not benchmarking Selectolax, because it is not installed.')

    try:
        from bs4 import BeautifulSoup
        start = time.monotonic()
        i = 0
        for i, record in enumerate(tqdm(ArchiveIterator(warc_file, record_types=WarcRecordType.response),
                                        desc=f'Benchmarking BeautifulSoup4 (lxml)', unit=' docs',
                                        leave=False, mininterval=0.3), start=1):
            content = record.reader.read()
            # noinspection PyStatementEffect
            BeautifulSoup(bytes_to_str(content, detect_encoding(content) or 'utf-8'), 'lxml').title
        t = time.monotonic() - start
        print(f'BeautifulSoup4 (lxml): {i} documents in {time.monotonic() - start:.2f}s ({i / t:.2f} documents/s)')
    except ModuleNotFoundError:
        print('[Not benchmarking BeautifulSoup4, because it is not installed.')


@main.group()
def lang():
    """Language module tools."""
    pass


DEFAULT_WIKI_LANGS = ('en,ru,zh,de,vi,fr,sv,uk,pt,fa,sr,it,pl,ja,nl,es,ca,sh,ko,tr,fi,cs,no,he,id,hu,el,ro,th,ar,bg,hy,'
                      'da,bn,cy,eo,hr,ka,eu,gl,hi,be,sk,ur,ms,ml,mk,et,te,lt,ce,az,tt,kk,sl,ta,lv,uz,mr,tl,my,oc,vo,tg,'
                      'bs,ne,is,ku,mg,pa,nn,sw,ga,fy,la,li,af,io,qu,os,sa,gd,yo,wa,sq,zu,ve,yi,wo,lo,kw,ln,gu,ti,ha,mn,'
                      'ky,su,lg,ba,kn,ee,ie,se,sc,or,am,fo,kv,ps,sm,za,lb,dv,rm,br,ch,cu,iu,sn,an,jv,mt,mi,om,ki,km,ia,'
                      'ss,si,rw,bo,st,so,kl,sg,na,ht,cr,as,av,rn,sd,bh,tw,tk,ig,ug,ff,ny,gn,to,cv,ks,xh,bi,dz,pi,ay,co,'
                      'ty,fj,ik,nv,ak,ab,gv,tn,kg,bm,ts')


@lang.command()
@click.argument('dumpdate')
@click.option('-l', '--langs', help='Comma-separated list of languages to download', default=DEFAULT_WIKI_LANGS)
@click.option('-o', '--outdir', help='Output directory', default='wikidumps', type=click.Path(file_okay=False))
@click.option('-j', '--jobs', help='Parallel download jobs (3 is the Wikimedia rate limit)', default=3, type=int)
def download_wiki_dumps(dumpdate, langs, outdir, jobs):
    """
    Download Wikipedia dumps for language detection.

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

    joblib.Parallel(n_jobs=jobs, verbose=0, prefer='threads')(joblib.delayed(dl)(l.strip()) for l in langs.split(','))


@lang.command()
@click.argument('indir')
@click.argument('outdir')
@click.option('--val-size', help='Portion of the data to use for validation', default=5, type=int)
@click.option('--test-size', help='Portion of the data to use for testing', default=5, type=int)
@click.option('--min-examples', help='Minimum number of examples per language', default=10000,
              show_default=True, type=int)
@click.option('-j', '--jobs', help='Parallel jobs', default=4, type=int)
def create_dataset(indir, outdir, val_size, test_size, min_examples, jobs):
    """
    Create a language detection dataset.

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
    joblib.Parallel(n_jobs=jobs, verbose=10, backend='multiprocessing')(
        joblib.delayed(_process_raw_lang_dir)(l, outdir, val_size, test_size, min_examples) for l in langdirs)


def _process_raw_lang_dir(indir, outdir_base, val_size, test_size, min_examples):
    lang_name = os.path.basename(indir).replace('wiki', '')
    outdir = os.path.join(outdir_base, lang_name)

    train_name = os.path.join(outdir, 'train.txt')
    val_name = os.path.join(outdir, 'val.txt')
    test_name = os.path.join(outdir, 'test.txt')

    if not os.path.isdir(outdir):
        os.makedirs(outdir)
    elif os.path.isfile(train_name) and os.path.getsize(train_name) > 0:
        click.echo(f'Skipping {lang_name}, since dataset for language already present.', err=True)
        return

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

            test_end = int(len(line_hashes) * test_size * 0.01)
            val_end = test_end + int(len(line_hashes) * val_size * 0.01)

            with open(test_name, 'w') as testf, open(val_name, 'w') as valf, open(train_name, 'w') as trainf:
                click.echo(f'Writing examples for {lang_name}...', err=True)
                for i, h in enumerate(line_hashes):
                    hname = hash_fname(tempdir, h.hex())
                    if i < test_end:
                        testf.write(open(hname).read())
                    elif i < val_end:
                        valf.write(open(hname).read())
                    else:
                        trainf.write(open(hname).read())
                    os.unlink(hname)
            click.echo(f'Examples for {lang_name} finished.', err=True)
    finally:
        if not os.listdir(outdir):
            os.rmdir(outdir)


# Wikipedia languages ordered by number of users
# Data taken from https://en.wikipedia.org/wiki/List_of_Wikipedias
_WIKI_BIAS = ['en', 'es', 'fr', 'de', 'zh', 'ru', 'pt', 'it', 'ar', 'ja', 'tr', 'id', 'nl', 'simple', 'pl', 'fa', 'he',
              'vi', 'sv', 'ko', 'hi', 'uk', 'ro', 'cs', 'no', 'fi', 'hu', 'da', 'th', 'ca', 'bn', 'el', 'bg', 'sr',
              'ms', 'hr', 'az', 'zh-yue', 'sk', 'sl', 'ta', 'eo', 'sh', 'arz', 'lt', 'et', 'ml', 'la', 'af', 'mr', 'bs',
              'sq', 'ur', 'ka', 'eu', 'gl', 'tl', 'nn', 'hy', 'ang', 'kk', 'be', 'te', 'lv', 'mk', 'my', 'ast',
              'zh-classical', 'sco', 'als', 'ceb', 'is', 'wuu', 'mn', 'be-tarask', 'kn', 'cy', 'br', 'uz', 'gu', 'an',
              'bar', 'ne', 'si', 'lb', 'jv', 'zh-min-nan', 'war', 'sw', 'ga', 'ku', 'ckb', 'oc', 'nds', 'yi', 'ia',
              'fy', 'tt', 'scn', 'pa', 'gan', 'am', 'lmo', 'km', 'tg', 'sa', 'ba', 'azb', 'io', 'as', 'vo', 'ky', 'pnb',
              'vec', 'so', 'cv', 'or', 'hak', 'pdc', 'hif', 'ce', 'bh', 'mg', 'su', 'mzn', 'ht', 'nap', 'qu', 'ps',
              'fo', 'li', 'se', 'bo', 'gd', 'pms', 'nds-nl', 'new', 'bat-smg', 'vls', 'yo', 'rue', 'diq', 'ace', 'tk',
              'bpy', 'dv', 'hsb', 'eml', 'cu', 'os', 'wa', 'sah', 'ksh', 'sc', 'chr', 'szl', 'nah', 'mt', 'lad', 'co',
              'pam', 'ug', 'bcl', 'cdo', 'arc', 'rm', 'gv', 'got', 'frr', 'dsb', 'ab', 'crh', 'xmf', 'zu', 'iu', 'rmy',
              'cr', 'ie', 'ilo', 'gn', 'ext', 'mi', 'ha', 'csb', 'ay', 'pcd', 'sd', 'map-bms', 'min', 'lo', 'jbo', 'nv',
              'sn', 'haw', 'frp', 'vep', 'ch', 'glk', 'lij', 'wo', 'udm', 'cbk-zam', 'kw', 'bxr', 'pap', 'ee', 'fur',
              'av', 'kv', 'roa-rup', 'fiu-vro', 'mhr', 'ig', 'stq', 'bjn', 'nrm', 'mwl', 'bug', 'kl', 'gag', 'tpi',
              'bi', 'zea', 'kab', 'ak', 'ln', 'myv', 'tw', 'xh', 'na', 'mai', 'roa-tara', 'nov', 'rw', 'pfl', 'chy',
              'pih', 'kaa', 'mrj', 'kg', 'bm', 'krc', 'za', 'sm', 'lez', 'pnt', 'xal', 'st', 'om', 'kbd', 'to', 'dz',
              'tn', 'ks', 'tet', 'ts', 'rn', 'ny', 'mdf', 'gom', 'ti', 'fj', 'lfn', 'koi', 'lbe', 'ik', 'tyv', 'ki',
              'ff', 'pag', 'ss', 'tum', 'srn', 'lg', 'ty', 've', 'jam', 'ltg', 'pi', 'hyw', 'sg', 'kr', 'olo', 'nso',
              'ady', 'din', 'lrc', 'dty', 'tcy', 'sat', 'aa', 'hz', 'ary', 'ban', 'kbp', 'atj', 'gor', 'shn', 'inh',
              'ng', 'mus', 'mh', 'nqo', 'ii', 'mnw', 'avk', 'szy', 'cho', 'gcr', 'ho', 'kj', 'smn', 'awa', 'lld', 'mad',
              'alt', 'mni', 'dag', 'skr', 'nia', 'trv', 'tay', 'shi']


@lang.command()
@click.argument('indir', type=click.Path(exists=True, file_okay=False))
@click.option('-s', '--split', help='Which input split to use', default='train',
              type=click.Choice(['train', 'test', 'val']), show_default=True)
@click.option('-f', '--out-format', help='Output format (raw vectors or Rust code)', default='raw',
              type=click.Choice(['raw', 'rust']), show_default=True)
@click.option('-l', '--vector-size', help='Output vector size', default=256, type=click.IntRange(min=1),
              show_default=True)
@click.option('-o', '--output', type=click.Path(dir_okay=False), help='Write profiles to this file instead of stdout')
def train_vectors(indir, split, out_format, vector_size, output):
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

    # Sort descending by amount of data for biasing towards common languages
    bias = {language: index for index, language in enumerate(_WIKI_BIAS)}
    langs = sorted((name for name in os.listdir(indir) if os.path.isdir(os.path.join(indir, name))),
                   key=lambda language: (bias.get(language, len(bias)), language))

    if not langs:
        raise click.ClickException('No language directories found')
    if out_format == 'rust' and vector_size != 256:
        raise click.BadParameter('Rust language profiles require 256 bins', param_hint='--vector-size')
    if out_format == 'rust' and any(re.fullmatch(r'[A-Za-z][A-Za-z0-9_-]*', language) is None
                                    for language in langs):
        raise click.ClickException(
            'Rust profile language codes must contain only letters, digits, hyphens, or underscores')
    rendered = io.StringIO()

    def echo(text='', nl=True):
        click.echo(text, file=rendered, nl=nl)

    if out_format == 'rust':
        echo(
            f'// Generated automatically, do not modify.\n\n#[rustfmt::skip]\npub(super) const LANGS: &[(&str, [u8; {vector_size}])] = &[')
    else:
        echo('# (lang, vec)')

    for l in langs:
        path = os.path.join(indir, l, split + '.txt')
        try:
            with open(path, encoding='utf-8') as examples:
                vec = rlang.train_language_examples(examples, vector_size)
        except (OSError, ValueError, UnicodeError) as error:
            raise click.ClickException(f'{path}: {error}') from error
        if out_format == 'rust':
            echo(f'    ("{l}", {vec!r}),')
        else:
            echo((l, vec))

    if out_format == 'rust':
        echo('];')

    if output is None:
        click.echo(rendered.getvalue(), nl=False)
    else:
        with click.open_file(output, 'w', encoding='utf-8', atomic=True) as target:
            target.write(rendered.getvalue())


@lang.command()
@click.argument('indir', type=click.Path(exists=True, file_okay=False))
@click.option('-s', '--split', help='Which input split to use', type=click.Choice(['val', 'test']),
              default='val', show_default=True)
@click.option('-l', '--langs', help='Restrict languages to this comma-separated list')
@click.option('-c', '--cutoff', type=int, help='Prediction cutoff', default=1200, show_default=True)
@click.option('-t', '--truncate', type=int, help='Truncate examples to this length')
@click.option('-f', '--fasttext-model', help='Use the specified FastText model for samples above cutoff',
              type=click.Path(exists=True, dir_okay=False))
@click.option('--sort-lang', is_flag=True, help='Sort by language instead of F1')
@click.option('--print-cm', is_flag=True, help='Print confusion matrix (may be very big)')
def evaluate(indir, split, langs, truncate, cutoff, sort_lang, print_cm, fasttext_model):
    """
    Evaluate language prediction performance.
    """
    if langs is not None:
        langs = {l.strip() for l in langs.split(',')}
    in_langs = sorted([l for l in os.listdir(indir) if langs is None or l in langs])

    if fasttext_model:
        try:
            import fasttext
        except ModuleNotFoundError:
            click.echo('FastText needs to be installed if --fasttext-model is set.', err=True)
            click.echo('Run "pip install fasttext" to install it.', err=True)
            return

        # FastText prints useless warnings after loading a model, so silence its print method to make it shut up
        # See: https://github.com/facebookresearch/fastText/issues/909
        fasttext.FastText.eprint = lambda x: None
        fasttext_model = fasttext.load_model(fasttext_model)

    recall_matrix = defaultdict(list)
    precision_matrix = defaultdict(list)
    confusion_matrix = defaultdict(lambda: defaultdict(int))

    for lang in tqdm(in_langs, desc='Evaluating languages', unit='language', leave=False):
        for line in tqdm(open(os.path.join(indir, lang, split + '.txt'), 'r'),
                         desc=f'Predicting examples for {lang}', unit=' examples', leave=False):

            if truncate:
                line = line[:truncate]

            plang = rlang.detect_fast(line, cutoff=cutoff, langs=langs)[0]
            if plang == 'unknown':
                if fasttext_model:
                    ft_pred = _fasttext_native_predict(fasttext_model, line)
                    plang = ft_pred[0][1].replace('__label__', '') if ft_pred else '-'
                else:
                    plang = '-'
            recall_matrix[lang].append(plang == lang)
            precision_matrix[plang].append(plang == lang)
            confusion_matrix[lang][plang] += 1

    acc = []
    results = []
    sum_examples = 0
    for lang in in_langs:
        precision = sum(precision_matrix[lang]) / max(1, len(precision_matrix[lang]))
        recall = sum(recall_matrix[lang]) / max(1, len(recall_matrix[lang]))
        if precision + recall == 0:
            f1 = 0.0
        else:
            f1 = 2.0 * (precision * recall) / (precision + recall)
        n_ex = len(recall_matrix[lang])
        results.append((lang, precision, recall, f1, n_ex))
        acc.append(recall * n_ex)
        sum_examples += n_ex

    click.echo('Lang, Precision, Recall, F1, Top Confusions, Num Examples')
    if not sort_lang:
        results.sort(key=lambda x: x[3], reverse=True)
    for r in results:
        click.echo(f'{r[0]}, {r[1]:.2f}, {r[2]:.2f}, {r[3]:.2f}, {r[4]}')

    acc = fsum(acc) / sum_examples
    click.echo(f'\nAccuracy: {acc:.2f}')

    if print_cm:
        label_width = max(len(l) for l in in_langs)
        col_width = max(max(max(len(str(l2)) for l2 in l1.values()) for l1 in confusion_matrix.values()),
                        label_width) + 2

        click.echo(f'\nConfusion matrix:\n' + (' ' * label_width), nl=False)
        for l in in_langs:
            click.echo(f'{l:>{col_width}}', nl=False)

        click.echo()
        for l1 in in_langs:
            click.echo(l1, nl=False)
            for l2 in in_langs:
                click.echo(f'{confusion_matrix[l1][l2]:>{col_width}}', nl=False)
            click.echo()


@lang.command()
@click.argument('infile', type=click.Path(exists=True, dir_okay=False))
@click.option('-r', '--rounds', help='Number of rounds to benchmark', type=int, default=10000, show_default=True)
@click.option('-f', '--fasttext-model', help='FastText model to benchmark',
              type=click.Path(exists=True, dir_okay=False))
def benchmark(infile, rounds, fasttext_model):
    """
    Benchmark Resiliparse against FastText and Langid.

    Either package must be installed for this comparison. Install Resiliparse with the
    ``cli-benchmark`` flag to install all optional third-party dependencies automatically.
    """

    if fasttext_model:
        try:
            import fasttext
        except ModuleNotFoundError:
            click.echo('FastText needs to be installed if --fasttext-model is set.', err=True)
            click.echo('Run "pip install fasttext" to install it.', err=True)
            return

        # FastText prints useless warnings after loading a model, so silence its print method to make it shut up
        # See: https://github.com/facebookresearch/fastText/issues/909
        fasttext.FastText.eprint = lambda x: None
        fasttext_model = fasttext.load_model(fasttext_model)
    else:
        click.echo('Skipping FastText benchmark, since no model has been specified.', err=True)

    bench_langid = True
    try:
        import langid
    except ModuleNotFoundError:
        click.echo('Skipping langid benchmark, since it is not installed.', err=True)
        click.echo('Run "pip install langid" to install it.', err=True)
        bench_langid = False

    with open(infile, 'rb') as source:
        data = source.read()
    in_data = bytes_to_str(re.sub(b'\r?\n', b' ', data))
    # Keep lazy model initialization outside the measured prediction loops.
    rlang.detect_fast(in_data)
    if fasttext_model:
        fasttext_predict = fasttext_model.f.predict
        fasttext_input = in_data + '\n'
        fasttext_predict(fasttext_input, 1, 0.0, 'strict')
    if bench_langid:
        langid.classify(in_data)
    click.echo(f'Benchmarking language detectors ({rounds:,} rounds):')

    start = time.monotonic()
    for _ in tqdm(range(rounds), desc='Benchmarking Resiliparse', unit=' rounds', leave=False, mininterval=0.3):
        rlang.detect_fast(in_data)
    resiliparse_time = time.monotonic() - start
    print(f'Resiliparse: {resiliparse_time:.1f}s')

    if fasttext_model:
        start = time.monotonic()
        for _ in tqdm(range(rounds), desc='Benchmarking FastText', unit=' rounds', leave=False, mininterval=0.3):
            fasttext_predict(fasttext_input, 1, 0.0, 'strict')
        elapsed = time.monotonic() - start
        speedup = elapsed / resiliparse_time if resiliparse_time > 0 else float('inf')
        print(f'FastText:    {elapsed:.1f}s (Resiliparse speedup: {speedup:.2f}x)')

    if bench_langid:
        start = time.monotonic()
        for _ in tqdm(range(rounds), desc='Benchmarking Langid', unit=' rounds', leave=False, mininterval=0.3):
            langid.classify(in_data)
        elapsed = time.monotonic() - start
        speedup = elapsed / resiliparse_time if resiliparse_time > 0 else float('inf')
        print(f'Langid:      {elapsed:.1f}s (Resiliparse speedup: {speedup:.2f}x)')


if __name__ == '__main__':
    main()
