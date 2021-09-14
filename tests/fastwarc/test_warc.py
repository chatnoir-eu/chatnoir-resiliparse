import codecs
from gzip import GzipFile
from hashlib import md5
import lz4.frame
import io
import os

import pytest

from fastwarc.stream_io import *
from fastwarc.warc import *


DATA_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', 'data'))
NUM_RECORDS = 50
NUM_RECORDS_OF_TYPE = 16


def iterate_warc(stream):
    rec_ids = set()
    for rec in ArchiveIterator(stream, parse_http=False):
        assert rec.record_id.startswith('<urn:')
        assert rec.record_id not in rec_ids
        assert rec.record_type in [warcinfo, response, request, metadata]
        rec_ids.add(rec.record_id)
    assert len(rec_ids) == NUM_RECORDS


def test_archive_iterator():
    iterate_warc(FileStream(os.path.join(DATA_DIR, 'warcfile.warc')))
    iterate_warc(GZipStream(FileStream(os.path.join(DATA_DIR, 'warcfile.warc.gz'))))
    iterate_warc(LZ4Stream(FileStream(os.path.join(DATA_DIR, 'warcfile.warc.lz4'))))
    iterate_warc(open(os.path.join(DATA_DIR, 'warcfile.warc'), 'rb'))
    iterate_warc(io.BytesIO(open(os.path.join(DATA_DIR, 'warcfile.warc'), 'rb').read()))


def test_stream_type_auto_detection():
    iterate_warc(FileStream(os.path.join(DATA_DIR, 'warcfile.warc')))
    iterate_warc(FileStream(os.path.join(DATA_DIR, 'warcfile.warc.gz')))
    iterate_warc(FileStream(os.path.join(DATA_DIR, 'warcfile.warc.lz4')))

    iterate_warc(open(os.path.join(DATA_DIR, 'warcfile.warc'), 'rb'))
    iterate_warc(open(os.path.join(DATA_DIR, 'warcfile.warc.gz'), 'rb'))
    iterate_warc(open(os.path.join(DATA_DIR, 'warcfile.warc.lz4'), 'rb'))

    iterate_warc(io.BytesIO(open(os.path.join(DATA_DIR, 'warcfile.warc'), 'rb').read()))
    iterate_warc(io.BytesIO(open(os.path.join(DATA_DIR, 'warcfile.warc.gz'), 'rb').read()))
    iterate_warc(io.BytesIO(open(os.path.join(DATA_DIR, 'warcfile.warc.lz4'), 'rb').read()))


def iterate_with_offsets(stream):
    offsets = []
    rec_ids = []
    for i, rec in enumerate(ArchiveIterator(stream, parse_http=False)):
        if i == 0:
            assert rec.stream_pos == 0
        else:
            assert rec.stream_pos > offsets[-1]

        offsets.append(rec.stream_pos)
        assert rec.record_id
        rec_ids.append(rec.record_id)

    assert len(offsets) == NUM_RECORDS

    for i, offset in enumerate(offsets):
        stream.seek(offset)
        expected_records = NUM_RECORDS - i
        count = 0
        for j, rec in enumerate(ArchiveIterator(stream, parse_http=False)):
            if j == 0:
                assert rec.record_id == rec_ids[i]
                if rec.record_type == response:
                    assert rec.verify_block_digest()

            count += 1
        assert count == expected_records


def test_record_offsets():
    iterate_with_offsets(open(os.path.join(DATA_DIR, 'warcfile.warc'), 'rb'))
    iterate_with_offsets(open(os.path.join(DATA_DIR, 'warcfile.warc.gz'), 'rb'))
    iterate_with_offsets(open(os.path.join(DATA_DIR, 'warcfile.warc.lz4'), 'rb'))


def test_record_types():
    file = os.path.join(DATA_DIR, 'warcfile.warc')

    for i, rec in enumerate(ArchiveIterator(FileStream(file), parse_http=True)):
        if i == 0:
            assert rec.record_type == warcinfo
            assert not rec.http_headers
        elif (i - 1) % 3 == 0:
            assert rec.record_type == request
            assert rec.http_headers
        elif (i - 1) % 3 == 1:
            assert rec.record_type == response
            assert rec.http_headers
        elif (i - 1) % 3 == 2:
            assert rec.record_type == metadata
            assert not rec.http_headers

    for rec in ArchiveIterator(FileStream(file), parse_http=False, record_types=warcinfo):
        assert rec.record_type == warcinfo

    for rec in ArchiveIterator(FileStream(file), parse_http=False, record_types=response):
        assert rec.record_type == response

    for rec in ArchiveIterator(FileStream(file), parse_http=False, record_types=request):
        assert rec.record_type == request

    for i, rec in enumerate(ArchiveIterator(FileStream(file), parse_http=False, record_types=request | response)):
        if i % 2 == 0:
            assert rec.record_type == request
        else:
            assert rec.record_type == response


def test_record_func_filters():
    file = os.path.join(DATA_DIR, 'warcfile.warc')

    count = 0
    for _ in ArchiveIterator(FileStream(file), parse_http=False, func_filter=is_warc_10):
        count += 1
    assert count == NUM_RECORDS

    count = 0
    for _ in ArchiveIterator(FileStream(file), parse_http=False, func_filter=is_warc_11):
        count += 1
    assert count == 0

    count = 0
    for rec in ArchiveIterator(FileStream(file), parse_http=False, func_filter=has_block_digest):
        assert rec.verify_block_digest()
        count += 1
    assert count == NUM_RECORDS_OF_TYPE

    count = 0
    for rec in ArchiveIterator(FileStream(file), parse_http=True, func_filter=has_payload_digest):
        assert rec.verify_payload_digest()
        count += 1
    assert count == NUM_RECORDS_OF_TYPE

    count = 0
    for rec in ArchiveIterator(FileStream(file), parse_http=False, func_filter=is_http):
        assert rec.is_http
        assert rec.record_type in [request, response]
        count += 1
    assert count == NUM_RECORDS_OF_TYPE * 2 + 1

    count = 0
    for rec in ArchiveIterator(FileStream(file), parse_http=False, func_filter=is_concurrent):
        assert rec.record_type in [response, metadata]
        count += 1
    assert count == NUM_RECORDS_OF_TYPE * 2


def test_verify_digests():
    file = os.path.join(DATA_DIR, 'warcfile.warc')

    count = 0
    for rec in ArchiveIterator(FileStream(file), parse_http=False, verify_digests=True):
        count += 1
        assert rec.verify_block_digest()
        assert not rec.verify_payload_digest()
        if rec.record_type == response:
            rec.parse_http()
            assert rec.verify_payload_digest(consume=True)
            assert not rec.verify_block_digest()

    assert count == NUM_RECORDS_OF_TYPE

    for rec in ArchiveIterator(FileStream(file), parse_http=True, record_types=response):
        assert rec.verify_payload_digest()
        assert not rec.verify_block_digest()


def test_record_http_parsing():
    file = os.path.join(DATA_DIR, 'warcfile.warc')

    for rec in ArchiveIterator(FileStream(file), parse_http=True, record_types=response):
        # General
        assert rec.is_http
        assert rec.is_http_parsed
        assert rec.http_headers

        # Headers
        assert not rec.headers.status_code
        assert rec.http_headers.status_code
        assert str(rec.http_headers.status_code) in rec.http_headers.status_line
        assert len(rec.http_headers.asdict()) <= len(rec.http_headers.astuples())
        assert rec.http_content_type.startswith('text/')

        assert 'Content-Type' in rec.http_headers
        if 'charset=' in rec.http_headers.get('Content-Type'):
            charset = rec.http_headers['Content-Type'].split('charset=')[1].lower()
            try:
                codecs.lookup(charset)
                assert rec.http_charset == charset
            except LookupError:
                assert rec.http_charset is None

        # Content
        assert rec.reader.read(5) != b'HTTP/'

    for rec in ArchiveIterator(FileStream(file), parse_http=False, record_types=response):
        assert rec.is_http
        assert not rec.is_http_parsed
        assert not rec.http_headers
        assert rec.http_content_type is None
        assert rec.http_charset is None
        assert rec.reader.read(5) == b'HTTP/'
        assert not rec.headers.status_code


def test_record_content_reader():
    file = os.path.join(DATA_DIR, 'warcfile.warc')

    count = 0
    for rec in ArchiveIterator(FileStream(file), parse_http=False, record_types=response):
        count += 1
        assert rec.content_length == len(rec.reader.read())
    assert count == NUM_RECORDS_OF_TYPE

    count = 0
    for rec in ArchiveIterator(FileStream(file), parse_http=True, record_types=response):
        count += 1
        assert rec.content_length == len(rec.reader.read())
    assert count == NUM_RECORDS_OF_TYPE


def test_record_content_consume():
    count = 0
    for rec in ArchiveIterator(FileStream(os.path.join(DATA_DIR, 'warcfile.warc')), parse_http=True):
        count += 1
        rec.reader.consume()
    assert count == NUM_RECORDS


def check_warc_integrity(stream):
    count = 0
    count_response = 0
    for rec in ArchiveIterator(stream, parse_http=False):
        if rec.record_type == response:
            assert rec.verify_block_digest()
            rec.parse_http()
            assert rec.verify_payload_digest()
            count_response += 1
        count += 1
    assert count == NUM_RECORDS
    assert count_response == NUM_RECORDS_OF_TYPE


def test_record_writer():
    file = os.path.join(DATA_DIR, 'warcfile.warc')
    buf = io.BytesIO()

    # Without HTTP
    for rec in ArchiveIterator(FileStream(file), parse_http=False):
        rec.write(buf)
    buf.seek(0)
    check_warc_integrity(buf)

    # With HTTP and re-checksumming
    buf = io.BytesIO()
    for rec in ArchiveIterator(FileStream(file), parse_http=True):
        rec.write(buf)
    buf.seek(0)
    check_warc_integrity(buf)

    # With HTTP and re-checksumming
    buf = io.BytesIO()
    for rec in ArchiveIterator(FileStream(file), parse_http=True):
        rec.write(buf, checksum_data=True)
    buf.seek(0)
    check_warc_integrity(buf)

    # Check raw stream data for identity
    source_bytes = open(os.path.join(DATA_DIR, 'warcfile.warc'), 'rb').read()
    buf = io.BytesIO()
    for rec in ArchiveIterator(io.BytesIO(source_bytes), parse_http=False):
        rec.write(buf)
    buf.seek(0)
    assert md5(source_bytes).hexdigest() == md5(buf.getvalue()).hexdigest()


def test_warc_writer_compression():
    source_bytes = open(os.path.join(DATA_DIR, 'warcfile.warc'), 'rb').read()
    src_md5 = md5(source_bytes).hexdigest()

    # GZip
    raw_buf = io.BytesIO()
    com_buf = GZipStream(raw_buf)
    for rec in ArchiveIterator(io.BytesIO(source_bytes), parse_http=False):
        rec.write(com_buf)

    raw_buf.seek(0)
    assert md5(GzipFile(fileobj=raw_buf).read()).hexdigest() == src_md5
    raw_buf.seek(0)
    check_warc_integrity(raw_buf)

    # LZ4
    raw_buf = io.BytesIO()
    com_buf = LZ4Stream(raw_buf)
    for rec in ArchiveIterator(io.BytesIO(source_bytes), parse_http=False):
        rec.write(com_buf)

    raw_buf.seek(0)
    compressed = raw_buf.getvalue()
    decompressed = bytearray()
    read = 0
    while True:
        b, n = lz4.frame.decompress(compressed[read:], return_bytearray=True, return_bytes_read=True)
        decompressed.extend(b)
        read += n
        if read >= len(compressed):
            break

    assert md5(decompressed).hexdigest() == src_md5
    check_warc_integrity(raw_buf)


def test_clipped_warc_gz():
    file = os.path.join(DATA_DIR, 'clipped.warc.gz')

    rec_count = 0
    for rec in ArchiveIterator(FileStream(file), parse_http=False):
        content = rec.reader.read()
        assert content[:5] == b'HTTP/'
        assert len(content) < rec.content_length
        assert not rec.verify_block_digest()
        rec_count += 1
    assert rec_count > 0

    rec_count = 0
    for rec in ArchiveIterator(FileStream(file), parse_http=True):
        content = rec.reader.read()
        assert rec.http_headers
        assert len(content) < rec.content_length
        assert not rec.verify_payload_digest()
        rec_count += 1
    assert rec_count > 0


def test_warc_headers():
    new_record = WarcRecord()
    headers = new_record.headers
    with pytest.raises(KeyError):
        # noinspection PyStatementEffect
        new_record.record_id

    # Set various headers
    headers['WARC-Record-ID'] = 'abc'
    assert new_record.record_id == 'abc'
    headers.status_line = 'WARC/1.0'
    assert headers.status_line == 'WARC/1.0'
    headers['WARC-Target-URI'] = 'https://examle.com'
    assert 'WARC-Target-URI' in headers
    assert 'WARC-IP-Address' not in headers

    # Direct dict manipulation
    d = headers.asdict()
    d['X-Abc'] = '123'
    d.update({'X-Xyz': '456'})
    assert headers['X-Abc'] == '123'
    assert headers['X-Xyz'] == '456'
    headers['X-Abc'] = '789'
    assert headers['X-Abc'] == '789'
    assert headers['X-Xyz'] == '456'
    assert d['X-Abc'] == '789'
    assert d['X-Xyz'] == '456'
    assert 'X-Abc' in headers.asdict()
    assert 'X-Xyz' in headers.asdict()

    # Case-insensitive matching
    assert 'warc-record-id' in headers
    assert 'Warc-Record-Id' in headers
    headers['X-FooBaR'] = 'abc'
    assert 'x-foobar' in headers
    assert 'X-FOOBAR' in headers
    assert 'x-foobar' in headers.asdict()
    assert 'X-FOOBAR' in headers.asdict()
    assert headers.get('x-foobar') == 'abc'

    # Duplicate headers
    dict_len = len(headers.asdict())
    tuple_len = len(headers.astuples())
    headers.append('X-Custom-Header', 'Foobar')
    assert headers['X-Custom-Header'] == 'Foobar'
    headers.append('X-Custom-Header', 'Foobarbaz')
    assert len(headers.asdict()) == dict_len + 1
    assert len(headers.astuples()) == tuple_len + 2
    assert headers['X-Custom-Header'] == 'Foobarbaz'
    assert ('X-Custom-Header', 'Foobar') in headers.astuples()
    assert ('X-Custom-Header', 'Foobarbaz') in headers.astuples()

    # Case-insensitive set vs. append
    tuple_len = len(headers.astuples())
    headers['X-FOOBAR'] = 'xyz'
    assert headers['x-foobar'] == 'xyz'
    assert len(headers.astuples()) == tuple_len
    headers.append('X-FOOBAR', 'aaa')
    assert len(headers.astuples()) == tuple_len + 1

    # Iterate headers
    header_copy1 = WarcHeaderMap()
    header_copy2 = WarcHeaderMap()
    for k, v in headers:
        header_copy1.append(k, v)
        header_copy2[k] = v
    assert header_copy1 == headers
    assert header_copy2 != headers
    assert header_copy2.asdict() == headers.asdict()

    header_copy = WarcHeaderMap()
    for k, v in zip(headers.keys(), headers.values()):
        header_copy[k] = v
    assert header_copy.asdict() == headers.asdict()

    assert tuple(headers) == headers.astuples()

    # Test record types
    type_mapping = dict(
        warcinfo=warcinfo,
        response=response,
        resource=resource,
        request=request,
        metadata=metadata,
        revisit=revisit,
        conversion=conversion,
        continuation=continuation,
        unknown=unknown
    )
    for str_type, enum_type in type_mapping.items():
        new_record.record_type = enum_type
        assert new_record.record_type == enum_type
        assert headers['WARC-Type'] == str_type

        headers['WARC-Type'] = str_type
        assert new_record.record_type == enum_type
        assert headers['WARC-Type'] == str_type

    # Multiline headers
    headers['X-Bar1'] = 'abc\ndef'
    assert headers['X-Bar1'] == 'abc def'
    headers['X-Bar2'] = 'abc\r\ndef'
    assert headers['X-Bar2'] == 'abc def'

    # Clear headers
    headers.clear()
    assert len(headers) == 0
    assert len(headers.astuples()) == 0
    assert len(headers.asdict()) == 0
    assert headers.status_line == ''

    headers['X-Abc'] = 'Foo'
    headers.status_line = 'WARC/1.0'
    assert len(headers) == 1
    assert len(headers.astuples()) == 1
    assert len(headers.asdict()) == 1
    assert headers.status_line == 'WARC/1.0'

    # Clear headers dict
    headers.asdict().clear()
    assert len(headers) == 0
    assert len(headers.astuples()) == 0
    assert len(headers.asdict()) == 0
    assert headers.status_line == ''


new_record_bytes_content = b"""HTTP/1.1 200 OK\r\n\
Content-Type: text/html; charset=utf-8\r\n\
Content-Length: 69\r\n\
X-Multiline-Header: Hello\r\n\
  World\r\n\r\n\
<!doctype html>\n\
<meta charset="utf-8">\n\
<title>Test</title>\n\n\
Barbaz\n"""


def test_create_new_warc_record():
    # Init basic record
    new_record = WarcRecord()
    new_record.init_headers(len(new_record_bytes_content), response)
    assert new_record.headers.status_line == 'WARC/1.1'
    assert new_record.record_id.startswith('<urn:')
    assert new_record.record_type == response
    assert new_record.content_length == 0
    assert 'WARC-Type' in new_record.headers
    assert 'WARC-Date' in new_record.headers
    assert 'WARC-Record-ID' in new_record.headers
    assert new_record.headers['WARC-Record-ID'] == new_record.record_id
    assert 'Content-Length' in new_record.headers
    assert new_record.headers['Content-Length'] == str(len(new_record_bytes_content))
    new_record.headers['Content-Type'] = 'application/http; msgtype=response'
    new_record.headers['X-Multiline-Header'] = 'Hello\r\nWorld'
    assert new_record.headers['X-Multiline-Header'] == 'Hello World'

    # Set content
    new_record.set_bytes_content(new_record_bytes_content)

    # Write and read back
    stream = io.BytesIO()
    new_record.write(stream)
    stream.seek(0)

    count = 0
    for rec in ArchiveIterator(stream, parse_http=True):
        assert rec.headers.status_line == new_record.headers.status_line
        assert rec.headers['X-Multiline-Header'] == 'Hello World'
        assert rec.record_id == new_record.record_id
        assert rec.record_type == new_record.record_type
        assert rec.is_http
        assert rec.http_headers.status_code == 200
        assert rec.http_content_type == 'text/html'
        assert rec.http_charset == 'utf-8'
        assert rec.http_headers['X-Multiline-Header'] == 'Hello World'
        count += 1
    assert count == 1
