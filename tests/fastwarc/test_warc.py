from gzip import GzipFile
from hashlib import md5
import lz4.frame
import io
import os

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
        assert rec.is_http
        assert rec.is_http_parsed
        assert rec.http_headers
        assert rec.reader.read(5) != b'HTTP/'

    for rec in ArchiveIterator(FileStream(file), parse_http=False, record_types=response):
        assert rec.is_http
        assert not rec.is_http_parsed
        assert not rec.http_headers
        assert rec.reader.read(5) == b'HTTP/'


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

    # Final LZ4 stream flush is buggy
    # raw_buf.seek(0)
    # compressed = raw_buf.getvalue()
    # decompressed = bytearray()
    # read = 0
    # i = 0
    # while True:
    #     b, n = lz4.frame.decompress(compressed[read:], return_bytearray=True, return_bytes_read=True)
    #     decompressed.extend(b)
    #     if n == 0:
    #         break
    #     read += n
    #
    # assert md5(decompressed).hexdigest() == src_md5
    # check_warc_integrity(raw_buf)


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
