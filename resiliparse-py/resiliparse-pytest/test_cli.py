import os
import shutil
import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace

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
