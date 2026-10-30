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

import ast
import builtins
import hashlib
import io
import json
import os
import runpy
import shutil
import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace
from urllib.error import HTTPError

import pytest

from click.testing import CliRunner

from resiliparse import cli


def test_cli_uses_native_bindings():
    assert cli.ArchiveIterator.__module__ == 'fastwarc.warc'
    assert cli.HTMLTree.__module__.startswith('resiliparse.parse.html')
    assert cli.detect_encoding.__module__ == 'resiliparse.parse.encoding'
    assert cli.rlang.detect_fast.__module__ == 'resiliparse.parse.lang'
    assert CliRunner().invoke(cli.main, ['--help']).exit_code == 0


def test_cli_train_rust_profiles(tmp_path):
    for language in ['zz', 'en']:
        directory = tmp_path / language
        directory.mkdir()
        (directory / 'train.txt').write_text('abc\n', encoding='utf-8')
    # Files alongside language directories are not datasets.
    (tmp_path / 'ignore.txt').write_text('ignored', encoding='utf-8')
    result = CliRunner().invoke(cli.main, ['lang', 'train-vectors', str(tmp_path), '-f', 'rust'])
    assert result.exit_code == 0, result.output
    assert '#[rustfmt::skip]' in result.output
    assert '[u8; 256]' in result.output
    rows = [line for line in result.output.splitlines() if line.startswith('    (')]
    expected = cli.rlang.train_language_examples(['abc\n'])
    assert rows == [f'    ("en", {expected!r}),', f'    ("zz", {expected!r}),']
    assert CliRunner().invoke(cli.main, ['lang', 'train-vectors', str(tmp_path), '-l', '0']).exit_code == 2
    assert CliRunner().invoke(cli.main,
                              ['lang', 'train-vectors', str(tmp_path), '-f', 'rust', '-l', '8']).exit_code == 2


def test_cli_html_benchmark_counts_responses():
    warc = Path(__file__).resolve().parents[2] / 'test-data' / 'warcfile.warc'
    expected = sum(1 for _ in cli.ArchiveIterator(str(warc), record_types=cli.WarcRecordType.response))
    result = CliRunner().invoke(cli.main, ['html', 'benchmark', str(warc)])
    assert result.exit_code == 0, result.output
    assert f'{expected} documents in' in result.output


def test_cli_language_benchmark_calls_each_backend(tmp_path, monkeypatch):
    calls = {'resiliparse': [], 'fasttext': [], 'langid': []}
    native_detect = cli.rlang.detect_fast

    def detect(text):
        calls['resiliparse'].append(text)
        return native_detect(text)

    def predict(text, k, threshold, on_unicode_error):
        calls['fasttext'].append(text)
        return [(1.0, '__label__en')]

    def classify(text):
        calls['langid'].append(text)
        return 'en', 1.0

    monkeypatch.setattr(cli.rlang, 'detect_fast', detect)
    ticks = iter([0.0, 2.0, 2.0, 10.0, 10.0, 30.0])
    monkeypatch.setattr(cli, 'time', SimpleNamespace(monotonic=lambda: next(ticks)))
    monkeypatch.setitem(sys.modules, 'fasttext', SimpleNamespace(
        FastText=SimpleNamespace(eprint=None),
        load_model=lambda _: SimpleNamespace(f=SimpleNamespace(predict=predict))))
    monkeypatch.setitem(sys.modules, 'langid', SimpleNamespace(classify=classify))
    infile = tmp_path / 'sample.txt'
    infile.write_text('First line\nSecond line', encoding='utf-8')
    model = tmp_path / 'model.bin'
    model.touch()
    result = CliRunner().invoke(cli.main, ['lang', 'benchmark', str(infile), '-r', '3', '-f', str(model)])
    assert result.exit_code == 0, result.output
    assert 'Benchmarking language detectors (3 rounds)' in result.output
    for name, inputs in calls.items():
        assert len(inputs) == 4, name  # One warm-up, then three measured predictions.
        expected = {'First line Second line\n'} if name == 'fasttext' else {'First line Second line'}
        assert set(inputs) == expected
    assert all(label in result.output for label in ['Resiliparse:', 'FastText:', 'Langid:'])
    assert 'FastText:    8.0s (Resiliparse speedup: 4.00x)' in result.output
    assert 'Langid:      20.0s (Resiliparse speedup: 10.00x)' in result.output


def test_cli_dataset_and_evaluation(tmp_path):
    raw = tmp_path / 'raw' / 'enwiki'
    raw.mkdir(parents=True)
    sentence = 'This is an English article about language detection and preparing training examples. '
    (raw / 'wiki_00').write_text(''.join(f'{sentence * 4}{i}\n' for i in range(20)), encoding='utf-8')
    dataset = tmp_path / 'dataset'
    runner = CliRunner()
    result = runner.invoke(cli.main, ['lang', 'create-dataset', str(raw.parent), str(dataset),
                                      '--min-examples', '10', '--val-size', '20', '--test-size', '20', '-j', '1'])
    assert result.exit_code == 0, result.output
    for split, count in [('train', 12), ('val', 4), ('test', 4)]:
        assert len((dataset / 'en' / f'{split}.txt').read_text().splitlines()) == count
    result = runner.invoke(cli.main, ['lang', 'evaluate', str(dataset), '--langs', 'en'])
    assert result.exit_code == 0, result.output
    assert 'Accuracy: 1.00' in result.output


@pytest.mark.skipif(shutil.which('rustc') is None, reason='Rust compiler required')
def test_cli_generated_rust_profiles_compile(tmp_path):
    dataset = tmp_path / 'dataset'
    dataset.mkdir()
    expected = []
    for language in cli.rlang.supported_langs():
        directory = dataset / language
        directory.mkdir()
        examples = [f'A synthetic training article for {language}.\n', 'Another training example.\n']
        (directory / 'train.txt').write_text(''.join(examples), encoding='utf-8')
        expected.append((language, cli.rlang.train_language_examples(examples)))

    target = tmp_path / 'lang_profiles.rs'
    result = CliRunner().invoke(cli.main, ['lang', 'train-vectors', str(dataset),
                                           '--out-format', 'rust', '--output', str(target)])
    assert result.exit_code == 0, result.output
    assert result.output == ''
    source = target.read_text(encoding='utf-8')
    assert '#[rustfmt::skip]' in source
    assert len([line for line in source.splitlines() if line.startswith('    (')]) == len(expected)

    rows = ',\n'.join(f'("{language}", {vector!r})' for language, vector in expected)
    check = tmp_path / 'check.rs'
    check.write_text(
        'mod profiles { include!("lang_profiles.rs"); }\n'
        'fn main() {\n'
        f'    let expected: &[(&str, [u8; 256])] = &[{rows}];\n'
        '    assert_eq!(profiles::LANGS.len(), expected.len());\n'
        '    for (language, vector) in expected {\n'
        '        let actual = profiles::LANGS.iter().find(|(code, _)| code == language).unwrap();\n'
        '        assert_eq!(&actual.1, vector);\n'
        '    }\n'
        '}\n', encoding='utf-8')
    binary = tmp_path / 'check.exe'
    rust_env = os.environ.copy()
    rust_env.pop('LD_PRELOAD', None)
    compiled = subprocess.run(['rustc', '--edition=2024', str(check), '-o', str(binary)],
                              capture_output=True, text=True, env=rust_env, timeout=60)
    assert compiled.returncode == 0, compiled.stderr
    subprocess.run([str(binary)], check=True, env=rust_env, timeout=30)


def test_cli_training_failure_preserves_output(tmp_path):
    dataset = tmp_path / 'dataset'
    for language, text in [('en', 'Valid example\n'), ('zz', '')]:
        directory = dataset / language
        directory.mkdir(parents=True)
        (directory / 'train.txt').write_text(text, encoding='utf-8')
    target = tmp_path / 'lang_profiles.rs'
    target.write_text('existing model', encoding='utf-8')
    result = CliRunner().invoke(cli.main, ['lang', 'train-vectors', str(dataset), '-f', 'rust', '-o', str(target)])
    assert result.exit_code == 1
    assert 'pub(super)' not in result.output
    assert target.read_text(encoding='utf-8') == 'existing model'


@pytest.mark.parametrize('dependency', ['click', 'tqdm', 'joblib'])
def test_cli_missing_dependencies(monkeypatch, dependency, capsys):
    native_import = builtins.__import__

    def import_module(name, *args, **kwargs):
        if name == dependency:
            raise ModuleNotFoundError(name)
        return native_import(name, *args, **kwargs)

    monkeypatch.setattr(builtins, '__import__', import_module)
    if dependency == 'click':
        with pytest.raises(SystemExit, match='1'):
            runpy.run_path(cli.__file__)
        assert 'Missing dependency: click' in capsys.readouterr().err
    else:
        module = runpy.run_path(cli.__file__)
        result = CliRunner().invoke(module['main'], ['lang'])
        assert result.exit_code == 1
        assert 'Missing CLI dependencies!' in result.output
        assert 'resiliparse[cli]' in result.output


def test_cli_module_entry_point(monkeypatch, capsys):
    monkeypatch.setattr(sys, 'argv', ['resiliparse', '--help'])
    with pytest.raises(SystemExit, match='0'):
        runpy.run_path(cli.__file__, run_name='__main__')
    assert 'Resiliparse Command Line Interface' in capsys.readouterr().out


def test_cli_encoding_mapping(monkeypatch):
    encodings = [{'encodings': [
        {'name': 'UTF-8', 'labels': ['unicode-1-1-utf-8', 'utf8']},
        {'name': 'iso-8859-8-i', 'labels': ['logical']},
        {'name': 'windows-874', 'labels': ['dos-874']},
        {'name': 'not-a-real-codec', 'labels': ['invalid']},
    ]}]
    urls = []

    def urlopen(url):
        urls.append(url)
        return io.BytesIO(json.dumps(encodings).encode())

    monkeypatch.setattr(cli.urllib.request, 'urlopen', urlopen)
    result = CliRunner().invoke(cli.main, ['encoding', 'download-whatwg-mapping'])
    assert result.exit_code == 0, result.output
    mapping = ast.literal_eval(result.stdout)
    assert mapping['utf8'] == mapping['unicode-1-1-utf-8'] == 'utf-8'
    assert mapping['logical'] == 'iso8859-8'
    assert mapping['dos-874'] == 'iso8859-11'
    assert 'invalid' not in mapping
    assert list(mapping) == sorted(mapping)
    assert 'skipped not-a-real-codec' in result.stderr
    assert urls == ['https://encoding.spec.whatwg.org/encodings.json']


@pytest.mark.parametrize('outcome', ['success', 'corrupt', 'missing', 'http-error'])
def test_cli_download_dumps(tmp_path, monkeypatch, outcome):
    payload = b'compressed dump contents' * 400  # Exercise multiple download blocks.
    requested = []

    def urlopen(url):
        requested.append(url)
        if url.endswith('dumpstatus.json'):
            if outcome == 'http-error':
                raise HTTPError(url, 404, 'Not found', None, None)
            jobs = {} if outcome == 'missing' else {'articlesmultistreamdump': {'files': {'part': {
                'url': '/enwiki/dump.xml.bz2',
                'md5': hashlib.md5(payload if outcome == 'success' else b'wrong').hexdigest(),
                'size': len(payload),
            }}}}
            return io.BytesIO(json.dumps({'jobs': jobs}).encode())
        assert url == 'https://dumps.wikimedia.org/enwiki/dump.xml.bz2'
        return io.BytesIO(payload)

    monkeypatch.setattr(cli.urllib.request, 'urlopen', urlopen)
    outdir = tmp_path / 'dumps'
    result = CliRunner().invoke(cli.main, ['lang', 'download-wiki-dumps', '20260101',
                                          '--langs', ' en ', '-o', str(outdir), '-j', '1'])
    assert result.exit_code == 0, result.output
    assert requested[0] == 'https://dumps.wikimedia.org/enwiki/20260101/dumpstatus.json'
    if outcome == 'success':
        assert (outdir / 'enwiki.bz2').read_bytes() == payload
    else:
        assert list(outdir.iterdir()) == []
    if outcome == 'corrupt':
        assert 'corrupted, deleting it' in result.output
    if outcome == 'http-error':
        assert 'Error downloading enwiki: HTTP Error 404' in result.output


@pytest.mark.parametrize('installed', [False, True])
@pytest.mark.parametrize('elapsed', [0.0, 0.25])
def test_cli_optional_html_benchmarks(monkeypatch, installed, elapsed):
    calls = {'selectolax': [], 'bs4': []}
    # A short benchmark can finish within one clock tick on Windows.
    timestamps = iter([0.0, elapsed] * 3)
    monkeypatch.setattr(cli, 'time', SimpleNamespace(perf_counter=lambda: next(timestamps)))

    def selectolax(text):
        calls['selectolax'].append(text)
        return SimpleNamespace(css_first=lambda selector: 'title' if selector == 'title' else None)

    def beautiful_soup(text, parser):
        assert parser == 'lxml'
        calls['bs4'].append(text)
        return SimpleNamespace(title='title')

    monkeypatch.setitem(sys.modules, 'selectolax', SimpleNamespace())
    monkeypatch.setitem(sys.modules, 'selectolax.lexbor',
                        SimpleNamespace(LexborHTMLParser=selectolax) if installed else None)
    monkeypatch.setitem(sys.modules, 'bs4', SimpleNamespace(BeautifulSoup=beautiful_soup) if installed else None)
    warc = Path(__file__).resolve().parents[2] / 'test-data' / 'warcfile.warc'
    result = CliRunner().invoke(cli.main, ['html', 'benchmark', str(warc)])
    assert result.exit_code == 0, result.output
    if elapsed == 0:
        assert 'inf documents/s' in result.output
    if installed:
        expected = sum(1 for _ in cli.ArchiveIterator(str(warc), record_types=cli.WarcRecordType.response))
        assert len(calls['selectolax']) == len(calls['bs4']) == expected
        assert calls['selectolax'] == calls['bs4']
        assert all(isinstance(text, str) for text in calls['bs4'])
        assert f'{expected} documents in' in result.output
    else:
        assert 'Not benchmarking Selectolax' in result.output
        assert 'Not benchmarking BeautifulSoup4' in result.output
        assert calls == {'selectolax': [], 'bs4': []}


def test_cli_dataset_filtering_and_existing_output(tmp_path):
    raw = tmp_path / 'raw' / 'enwiki'
    raw.mkdir(parents=True)
    article = 'An English article with enough characters to serve as a language training example. ' * 4
    (raw / 'wiki_00').write_text(f'<doc id="1">\nshort\n\n[[{article}]]\n[[{article}]]\n</doc>\n')
    (raw / 'notes').write_text(article + 'ignored\n')
    (raw.parent / 'ignored.txt').write_text(article)
    dataset = tmp_path / 'dataset'
    dataset.mkdir()
    runner = CliRunner()
    args = ['lang', 'create-dataset', str(raw.parent), str(dataset), '--min-examples', '2', '-j', '1']
    result = runner.invoke(cli.main, args)
    assert result.exit_code == 0, result.output
    assert 'Not enough examples' in result.output
    assert list(dataset.iterdir()) == []  # Temporary files and the empty language directory are removed.
    args[args.index('2')] = '1'
    (dataset / 'en').mkdir()  # Existing empty language directories are reusable.
    result = runner.invoke(cli.main, args)
    assert result.exit_code == 0, result.output
    target = dataset / 'en' / 'train.txt'
    assert target.read_text() == article + '\n'
    result = runner.invoke(cli.main, args)
    assert result.exit_code == 0, result.output
    assert 'dataset for language already present' in result.output
    assert target.read_text() == article + '\n'


@pytest.mark.parametrize('problem', ['no-languages', 'invalid-language', 'missing-split', 'invalid-utf8'])
def test_cli_training_errors(tmp_path, problem):
    if problem != 'no-languages':
        directory = tmp_path / ('bad.code' if problem == 'invalid-language' else 'en')
        directory.mkdir()
        if problem != 'missing-split':
            (directory / 'train.txt').write_bytes(b'\xff' if problem == 'invalid-utf8' else b'example\n')
    result = CliRunner().invoke(cli.main, ['lang', 'train-vectors', str(tmp_path), '-f', 'rust'])
    assert result.exit_code == 1
    expected = {'no-languages': 'No language directories', 'invalid-language': 'Rust profile language codes',
                'missing-split': 'train.txt', 'invalid-utf8': 'train.txt'}
    assert expected[problem] in result.output
    assert 'pub(super)' not in result.output


def test_cli_raw_training_uses_requested_split(tmp_path):
    directory = tmp_path / 'en'
    directory.mkdir()
    (directory / 'test.txt').write_text('abc\n')
    result = CliRunner().invoke(cli.main, ['lang', 'train-vectors', str(tmp_path), '-s', 'test', '-l', '8'])
    assert result.exit_code == 0, result.output
    assert result.output.splitlines() == ['# (lang, vec)', repr(('en', cli.rlang.train_language_examples(['abc\n'], 8)))]


@pytest.mark.parametrize('prediction', [None, [], [(1.0, '__label__en')]])
def test_cli_evaluation_unknown_and_fasttext(tmp_path, monkeypatch, prediction):
    directory = tmp_path / 'en'
    directory.mkdir()
    (directory / 'test.txt').write_text('First example\nSecond example\n')
    args = ['lang', 'evaluate', str(tmp_path), '-s', 'test', '-c', '0', '-t', '5', '--sort-lang', '--print-cm']
    calls = []
    if prediction is not None:
        def predict(text, k, threshold, on_unicode_error):
            calls.append((text, k, threshold, on_unicode_error))
            return prediction

        monkeypatch.setitem(sys.modules, 'fasttext', SimpleNamespace(
            FastText=SimpleNamespace(eprint=None),
            load_model=lambda _: SimpleNamespace(f=SimpleNamespace(predict=predict))))
        model = tmp_path / 'model.bin'
        model.touch()
        args.extend(['-f', str(model)])
    result = CliRunner().invoke(cli.main, args)
    assert result.exit_code == 0, result.output
    assert f'Accuracy: {"1.00" if prediction else "0.00"}' in result.output
    assert 'Confusion matrix:' in result.output
    if prediction is not None:
        assert calls == [('First\n', 1, 0.0, 'strict'), ('Secon\n', 1, 0.0, 'strict')]


@pytest.mark.parametrize('command', ['evaluate', 'benchmark'])
def test_cli_fasttext_dependency_missing(tmp_path, monkeypatch, command):
    monkeypatch.setitem(sys.modules, 'fasttext', None)
    model = tmp_path / 'model.bin'
    model.touch()
    source = tmp_path if command == 'evaluate' else model
    result = CliRunner().invoke(cli.main, ['lang', command, str(source), '-f', str(model)])
    assert result.exit_code == 0, result.output
    assert 'FastText needs to be installed' in result.output
    assert 'pip install fasttext' in result.output


def test_cli_language_benchmark_without_optional_backends(tmp_path, monkeypatch):
    monkeypatch.setitem(sys.modules, 'langid', None)
    infile = tmp_path / 'sample.txt'
    infile.write_text('A short English sentence.\n')
    result = CliRunner().invoke(cli.main, ['lang', 'benchmark', str(infile), '-r', '1'])
    assert result.exit_code == 0, result.output
    assert 'Skipping FastText benchmark' in result.output
    assert 'Skipping langid benchmark' in result.output
    assert 'Resiliparse:' in result.output


@pytest.mark.parametrize('langs', [None, 'en,README.txt'])
def test_cli_evaluation_ignores_files(tmp_path, langs):
    directory = tmp_path / 'en'
    directory.mkdir()
    (directory / 'val.txt').write_text(
        'This is an English article about language detection and preparing training examples.\n', encoding='utf-8')
    (tmp_path / 'README.txt').write_text('Dataset description', encoding='utf-8')
    (tmp_path / 'model.bin').write_bytes(b'not a language directory')
    args = ['lang', 'evaluate', str(tmp_path)]
    if langs is not None:
        args.extend(['--langs', langs])
    result = CliRunner().invoke(cli.main, args)
    assert result.exit_code == 0, result.output
    assert 'en, 1.00, 1.00, 1.00, 1' in result.output
    assert 'Accuracy: 1.00' in result.output
    assert 'README.txt' not in result.output
    assert 'model.bin' not in result.output
