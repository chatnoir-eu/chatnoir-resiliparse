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

import io
import json
import subprocess
import sys

import pytest
from click.testing import CliRunner

from fastwarc import cli
from fastwarc.warc import ArchiveIterator, WarcRecord, WarcRecordType


@pytest.fixture
def warc_file(tmp_path):
    output = io.BytesIO()
    response = WarcRecord()
    response.init_headers(WarcRecordType.response, b'uuid:cli-response')
    response.is_http = True
    response.headers['WARC-Target-URI'] = 'https://example.org/'
    response.set_bytes_content(b'HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n'
                               b'Set-Cookie: a=1\r\nSet-Cookie: b=2\r\n\r\nHello CLI')
    response.parse_http()
    response.write(output, checksum_data=True)
    resource = WarcRecord()
    resource.init_headers(WarcRecordType.resource, b'uuid:cli-resource')
    resource.set_bytes_content(b'Resource body')
    resource.write(output, checksum_data=True)
    path = tmp_path / 'source.warc'
    path.write_bytes(output.getvalue())
    return path


def invoke(*arguments):
    result = CliRunner().invoke(cli.main, [str(argument) for argument in arguments])
    assert result.exit_code == 0, (result.output, result.exception)
    return result


def snapshot(source):
    return [(record.record_id, record.reader.read()) for record in ArchiveIterator(source, parse_http=False)]


@pytest.mark.parametrize('algorithm,suffix', [('gzip', '.gz'), ('lz4', '.lz4'), ('zstd', '.zst'),
                                              ('uncompressed', '')])
def test_recompress_index_extract_and_check(warc_file, algorithm, suffix):
    output = warc_file.parent / ('recompressed.warc' + suffix)
    result = invoke('recompress', warc_file, output, '--quiet')
    assert not result.output
    assert snapshot(str(output)) == snapshot(str(warc_file))
    invoke('check', output, '--verify-payloads', '--quiet')
    invoke('check', output, '--verify-payloads', '--quiet', '--decompress-alg', algorithm)
    entries = [json.loads(line) for line in invoke('index', output, '--fields',
                'offset,length,warc-type,http:status,http:set-cookie', '--preserve-multi-header').stdout.splitlines()]
    assert len(entries) == 2
    assert entries[0]['http:status'] == '200'
    assert entries[0]['http:set-cookie'] == ['a=1', 'b=2']
    assert sum(int(entry['length']) for entry in entries) == output.stat().st_size
    for entry in entries:
        offset = entry['offset']
        extracted = invoke('extract', output, offset)
        records = list(ArchiveIterator(io.BytesIO(extracted.stdout_bytes)))
        assert len(records) == 1
        assert str(records[0].record_type) == entry['warc-type']
    assert invoke('extract', output, entries[0]['offset'], '--payload').stdout_bytes == b'Hello CLI'
    headers = invoke('extract', output, entries[0]['offset'], '--headers').stdout_bytes
    assert b'WARC/1.1' in headers and b'HTTP/1.1 200 OK' in headers
    assert headers.count(b'\r\n\r\n') == 2
    assert b'Hello CLI' not in headers
    invoke('benchmark', output, '--parse-http')


@pytest.mark.parametrize('algorithm,suffix', [('gzip', '.gz'), ('lz4', '.lz4'), ('zstd', '.zst')])
def test_explicit_compression_and_extensionless_input(warc_file, algorithm, suffix):
    compressed = warc_file.parent / 'no-extension'
    invoke('recompress', warc_file, compressed, '-c', algorithm, '--quiet')
    restored = warc_file.parent / 'restored.warc'
    invoke('recompress', compressed, restored, '-d', algorithm, '--quiet')
    assert snapshot(str(restored)) == snapshot(str(warc_file))
    invoke('check', compressed, '--quiet')  # Auto-detect magic, not just suffixes.


@pytest.mark.parametrize('algorithm,level', [('gzip', 1), ('zstd', 1)])
def test_explicit_compression_level(warc_file, algorithm, level, monkeypatch):
    calls = []
    writer = cli._WRITERS[algorithm]
    def recording_writer(*args, **kwargs):
        calls.append(kwargs)
        return writer(*args, **kwargs)
    monkeypatch.setitem(cli._WRITERS, algorithm, recording_writer)
    output = warc_file.parent / 'compressed'
    invoke('recompress', warc_file, output, '-c', algorithm, '-l', level, '--quiet')
    assert calls == [{'compression_level': level}]
    assert snapshot(str(output)) == snapshot(str(warc_file))


def test_unsupported_level_and_same_input_are_rejected(warc_file):
    output = warc_file.parent / 'output.warc.lz4'
    result = CliRunner().invoke(cli.main, ['recompress', str(warc_file), str(output), '-l', '3'])
    assert result.exit_code != 0 and 'only for gzip and zstd' in result.output
    assert not output.exists()
    original = warc_file.read_bytes()
    result = CliRunner().invoke(cli.main, ['recompress', str(warc_file), str(warc_file), '--quiet'])
    assert result.exit_code != 0
    assert warc_file.read_bytes() == original


@pytest.mark.parametrize('quiet', [False, True])
def test_check_invalid_digest_returns_failure(warc_file, quiet):
    warc_file.write_bytes(warc_file.read_bytes().replace(b'Hello CLI', b'Jello CLI'))
    details = warc_file.parent / 'digests.txt'
    arguments = ['check', str(warc_file), '--verify-payloads', '-o', str(details)]
    if quiet:
        arguments.append('--quiet')
    result = CliRunner().invoke(cli.main, arguments)
    assert result.exit_code == 1
    assert 'FAIL, PAYLOAD_FAIL' in details.read_text()
    if quiet:
        assert not result.output


def test_check_reports_missing_digests(tmp_path):
    record = WarcRecord()
    record.init_headers(WarcRecordType.resource, b'uuid:no-digest')
    record.set_bytes_content(b'body')
    source = tmp_path / 'no-digest.warc'
    with source.open('wb') as stream:
        record.write(stream)
    result = invoke('check', source, '--verify-payloads')
    assert '1 records were skipped without digest.' in result.output


@pytest.mark.parametrize('python_stream', [False, True])
def test_benchmark_uses_requested_paths_and_filters(warc_file, python_stream):
    arguments = ['benchmark', warc_file, warc_file, '--filter-type', 'response', '--parse-http']
    if python_stream:
        arguments.append('--use-python-stream')
    result = invoke(*arguments)
    assert 'FastWARC: 2 records read' in result.output
    assert 'FastWARC: 0 records read' in invoke('benchmark', warc_file, '-f', 'conversion').output


@pytest.mark.parametrize('python_stream', [False, True])
def test_benchmark_reads_distinct_inputs_with_mixed_compression(warc_file, python_stream):
    compressed = warc_file.parent / 'response-and-resource.warc.gz'
    invoke('recompress', warc_file, compressed, '--quiet')
    resource_only = warc_file.parent / 'resource-only.warc'
    record = WarcRecord()
    record.init_headers(WarcRecordType.resource, b'uuid:distinct-input')
    record.set_bytes_content(b'Distinct input body')
    with resource_only.open('wb') as output:
        record.write(output)

    options = ['--use-python-stream'] if python_stream else []
    assert 'FastWARC: 1 records read' in invoke('benchmark', resource_only, *options).output
    assert 'FastWARC: 2 records read' in invoke('benchmark', compressed, *options).output
    for inputs in [(resource_only, compressed), (compressed, resource_only)]:
        assert 'FastWARC: 3 records read' in invoke('benchmark', *inputs, *options).output
        result = invoke('benchmark', *inputs, *options, '--filter-type', 'response')
        assert 'FastWARC: 1 records read' in result.output


def test_benchmark_http_stream_is_closed(warc_file, monkeypatch):
    source = io.BytesIO(warc_file.read_bytes())
    monkeypatch.setattr(cli.urllib.request, 'urlopen', lambda url: source)
    assert 'FastWARC: 2 records read' in invoke('benchmark', 'https://example.org/warc').output
    assert source.closed


def test_warcio_comparison_honors_filter(warc_file):
    pytest.importorskip('warcio')
    result = invoke('benchmark', warc_file, '-w', '-H', '-f', 'response')
    assert 'FastWARC: 1 records read' in result.output
    assert 'WARCIO:   1 records read' in result.output


def test_native_io_errors_are_click_errors(warc_file):
    broken = warc_file.parent / 'broken.gz'
    broken.write_bytes(b'not gzip')
    result = CliRunner().invoke(cli.main, ['check', str(broken), '-d', 'gzip', '--quiet'])
    assert result.exit_code == 1
    assert 'Error:' in result.output
    assert not isinstance(result.exception, AttributeError)


def test_cli_import_does_not_load_cython_extensions():
    subprocess.run([sys.executable, '-c', '''
import importlib.abc
import sys
class NoLegacy(importlib.abc.MetaPathFinder):
    def find_spec(self, fullname, path=None, target=None):
        if fullname in ('fastwarc.legacy.stream_io', 'fastwarc.legacy.warc', 'fastwarc.legacy.tools'):
            raise ImportError('Cython extension requested: ' + fullname)
sys.meta_path.insert(0, NoLegacy())
from fastwarc.cli import main
assert main.name == 'main'
'''], check=True, capture_output=True, text=True)


@pytest.mark.parametrize('command', ['check', 'recompress'])
def test_quiet_interrupt_is_not_success(warc_file, command, monkeypatch):
    def interrupted(*args, **kwargs):
        raise KeyboardInterrupt
        yield
    monkeypatch.setattr(cli, '_verify_digests' if command == 'check' else '_recompress_records', interrupted)
    arguments = [command, str(warc_file)]
    if command == 'recompress':
        arguments.append(str(warc_file.parent / 'out.gz'))
    result = CliRunner().invoke(cli.main, [*arguments, '--quiet'])
    assert result.exit_code == 1


@pytest.mark.parametrize('suffix', ['.lz4', '.zst'])
def test_warcio_rejects_unsupported_compression(warc_file, suffix):
    compressed = warc_file.parent / ('out.warc' + suffix)
    invoke('recompress', warc_file, compressed, '--quiet')
    result = CliRunner().invoke(cli.main, ['benchmark', str(compressed), '--bench-warcio'])
    assert result.exit_code != 0
    assert 'WARCIO does not support' in result.output


@pytest.mark.parametrize('size,expected', [(1, '1 bytes'), (2048, '2 KiB'), (1536, '1.50 KiB'),
                                          (1024 ** 5, '1024 TiB')])
def test_human_readable_bytes(size, expected):
    assert cli._human_readable_bytes(size) == expected


def test_recompress_summary(warc_file):
    result = invoke('recompress', warc_file, warc_file.parent / 'out.gz')
    assert 'Recompression completed.' in result.output
    assert 'Records recompressed: 2' in result.output
    assert 'Bytes written:' in result.output
    assert 'Completed in:' in result.output


def test_extract_empty_input_and_read_error(tmp_path, monkeypatch):
    empty = tmp_path / 'empty.warc'
    empty.touch()
    assert invoke('extract', empty, 0).stdout_bytes == b''

    def broken_iterator(*args, **kwargs):
        raise OSError('read failed')
    monkeypatch.setattr(cli, 'ArchiveIterator', broken_iterator)
    result = CliRunner().invoke(cli.main, ['extract', str(empty), '5'])
    assert result.exit_code == 1
    assert 'Failed to extract WARC record at offset 5: read failed' in result.output


def test_index_filename_and_single_headers(warc_file):
    fields = 'filename,http:content-type,http:missing,warc-type'
    entries = [json.loads(line) for line in invoke('index', warc_file, '-f', fields).stdout.splitlines()]
    assert entries[0] == {'filename': str(warc_file), 'http:content-type': 'text/plain',
                          'http:missing': None, 'warc-type': 'response'}
    assert entries[1] == {'filename': str(warc_file), 'warc-type': 'resource'}
    entries = [json.loads(line) for line in invoke('index', warc_file, '-f', fields,
                                                 '--preserve-multi-header').stdout.splitlines()]
    assert entries[0]['http:content-type'] == 'text/plain'
    assert 'http:missing' not in entries[0]


def test_benchmark_file_url_and_missing_input(warc_file):
    assert '2 records read' in invoke('benchmark', warc_file.as_uri()).output
    result = CliRunner().invoke(cli.main, ['benchmark'])
    assert result.exit_code != 0 and 'Need at least one input path' in result.output
    result = CliRunner().invoke(cli.main, ['benchmark', str(warc_file.parent / 'missing')])
    assert result.exit_code == 1 and 'No such file or directory' in result.output


def test_benchmark_native_error(warc_file):
    warc_file.write_bytes(b'invalid gzip')
    result = CliRunner().invoke(cli.main, ['benchmark', str(warc_file), '-d', 'gzip'])
    assert result.exit_code == 1 and 'Error reading input:' in result.output


@pytest.mark.parametrize('library', ['fastwarc', 'warcio'])
def test_benchmark_interrupt(warc_file, monkeypatch, library):
    def interrupted(*args, **kwargs):
        raise KeyboardInterrupt
        yield
    arguments = ['benchmark', str(warc_file)]
    if library == 'fastwarc':
        monkeypatch.setattr(cli, '_warc_records', interrupted)
    else:
        warcio = pytest.importorskip('warcio')
        monkeypatch.setattr(warcio, 'ArchiveIterator', interrupted)
        arguments.append('--bench-warcio')
    result = CliRunner().invoke(cli.main, arguments)
    assert result.exit_code == 1 and 'Benchmark interrupted.' in result.output


def test_benchmark_without_warcio(warc_file, monkeypatch):
    import builtins
    original_import = builtins.__import__
    def without_warcio(name, *args, **kwargs):
        if name == 'warcio':
            raise ModuleNotFoundError(name)
        return original_import(name, *args, **kwargs)
    monkeypatch.setattr(builtins, '__import__', without_warcio)
    result = CliRunner().invoke(cli.main, ['benchmark', str(warc_file), '-w'])
    assert result.exit_code == 1 and "requires 'warcio' to be installed" in result.output


@pytest.fixture
def mock_s3(warc_file, monkeypatch):
    from types import SimpleNamespace
    from unittest.mock import Mock

    body = io.BytesIO(warc_file.read_bytes())
    resource = Mock()
    resource.Object.return_value.get.return_value = {'Body': SimpleNamespace(_raw_stream=body)}
    resource.Bucket.return_value.objects.filter.return_value = [SimpleNamespace(key='prefix/page.warc')]
    boto3 = SimpleNamespace(resource=Mock(return_value=resource))
    class ClientError(Exception):
        pass
    botocore = SimpleNamespace(exceptions=SimpleNamespace(ClientError=ClientError))
    modules = {'boto3': boto3, 'botocore': botocore}
    original_import = cli.importlib.import_module
    monkeypatch.setattr(cli.importlib, 'import_module',
                        lambda name: modules[name] if name in modules else original_import(name))
    for name in ('boto3', 'botocore', 's3'):
        monkeypatch.setattr(cli, name, None)
    return SimpleNamespace(boto3=boto3, botocore=botocore, resource=resource, body=body)


def test_s3_initialization_and_reuse(mock_s3, monkeypatch):
    monkeypatch.setattr(cli.getpass, 'getpass', lambda: 'prompted-secret')
    cli._init_s3('https://s3.example', 'access')
    mock_s3.boto3.resource.assert_called_once_with('s3', endpoint_url='https://s3.example',
                                                 aws_access_key_id='access', aws_secret_access_key='prompted-secret')
    cli._init_s3('https://other.example', 'other', 'other')
    assert mock_s3.boto3.resource.call_count == 1


def test_s3_missing_dependency_and_credentials(mock_s3, monkeypatch):
    with pytest.raises(cli.click.UsageError, match='require.*aws-access-key'):
        cli._init_s3('https://s3.example', None)
    def missing_module(name):
        raise ModuleNotFoundError(name)
    monkeypatch.setattr(cli.importlib, 'import_module', missing_module)
    with pytest.raises(cli.click.UsageError, match='Boto3 needs to be installed'):
        cli._init_s3('https://s3.example', 'access', 'secret')


@pytest.mark.parametrize('prefix', [False, True])
def test_benchmark_s3(mock_s3, warc_file, prefix):
    arguments = ['benchmark', 's3://bucket/prefix', '-a', 'access', '-s', 'secret']
    if prefix:
        arguments.extend(['--is-prefix', str(warc_file)])
    result = invoke(*arguments)
    assert f'FastWARC: {4 if prefix else 2} records read' in result.output
    mock_s3.resource.Object.assert_called_once_with('bucket', 'prefix/page.warc' if prefix else 'prefix')
    if prefix:
        mock_s3.resource.Bucket.return_value.objects.filter.assert_called_once_with(Prefix='prefix')
    assert mock_s3.body.closed


def test_benchmark_s3_client_error(mock_s3):
    mock_s3.resource.Object.return_value.get.side_effect = mock_s3.botocore.exceptions.ClientError('access denied')
    result = CliRunner().invoke(cli.main, ['benchmark', 's3://bucket/key', '-a', 'access', '-s', 'secret'])
    assert result.exit_code == 1
    assert 'Error connecting to S3. Message: access denied.' in result.output


def test_module_entry_point(monkeypatch, capsys):
    import runpy
    # Avoid runpy's warning about executing an already imported module.
    monkeypatch.delitem(sys.modules, 'fastwarc.cli')
    monkeypatch.setattr(sys, 'argv', ['fastwarc.cli', '--help'])
    with pytest.raises(SystemExit) as error:
        runpy.run_module('fastwarc.cli', run_name='__main__')
    assert error.value.code == 0
    assert 'FastWARC Command Line Interface.' in capsys.readouterr().out
