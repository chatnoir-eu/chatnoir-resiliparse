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

import datetime as dt
import gzip
import hashlib
import io
import pickle
from pathlib import Path

import pytest

import fastwarc
from fastwarc.stream_io import *
from fastwarc.warc import *


def get_fixtures_path():
    return Path(__file__).resolve().parents[2] / "test-data"


HTTP_BODY = b"Hello"


def _http_payload(body: bytes = HTTP_BODY) -> bytes:
    return (
            b"HTTP/1.1 200 OK\r\n"
            b"Date: Tue, 15 Nov 1994 08:12:31 GMT\r\n"
            b"Last-Modified: Tue, 15 Nov 1994 12:45:26 GMT\r\n"
            b"Content-Type: text/plain; charset=utf-8\r\n"
            + f"Content-Length: {len(body)}\r\n".encode()
            + b"\r\n"
            + body
    )


def _make_http_record(record_type=response, *, record_urn: bytes = b"uuid:test-record",
                      body: bytes = HTTP_BODY) -> WarcRecord:
    record = WarcRecord()
    record.init_headers(unknown, record_urn)
    record.record_type = record_type
    record.is_http = True
    record.set_bytes_content(_http_payload(body))
    return record


def _serialize_record(record: WarcRecord) -> bytes:
    stream = io.BytesIO()
    record.write(stream)
    return stream.getvalue()


def _serialize_records(*records: WarcRecord) -> io.BytesIO:
    stream = io.BytesIO()
    for record in records:
        record.write(stream)
    stream.seek(0)
    return stream


def test_package_reexports():
    assert WarcRecord is WarcRecord
    assert HeaderMap is HeaderMap
    assert WarcHeaderMap is HeaderMap
    assert ArchiveIterator is fastwarc.ArchiveIterator
    assert "ArchiveIterator" in fastwarc.__all__
    assert "GzipReader" in fastwarc.stream_io.__all__


def test_warc_record_type_binary_operators():
    assert int(response) == 4
    assert response | request == 20
    assert 1 | response == 5
    assert response & request == 0
    assert 7 & response == 4
    assert response ^ request == 20
    assert 1 ^ response == 5
    assert ~response == (~4 & 0xFFFF)


@pytest.mark.parametrize(
    ("name", "value"),
    [
        ("WARC_TYPE", "WARC-Type"),
        ("WARC_RECORD_ID", "WARC-Record-ID"),
        ("WARC_DATE", "WARC-Date"),
        ("CONTENT_LENGTH", "Content-Length"),
        ("CONTENT_TYPE", "Content-Type"),
        ("WARC_CONCURRENT_TO", "WARC-Concurrent-To"),
        ("WARC_BLOCK_DIGEST", "WARC-Block-Digest"),
        ("WARC_PAYLOAD_DIGEST", "WARC-Payload-Digest"),
        ("WARC_IP_ADDRESS", "WARC-IP-Address"),
        ("WARC_REFERS_TO", "WARC-Refers-To"),
        ("WARC_REFERS_TO_TARGET_URI", "WARC-Refers-To-Target-URI"),
        ("WARC_REFERS_TO_DATE", "WARC-Refers-To-Date"),
        ("WARC_TARGET_URI", "WARC-Target-URI"),
        ("WARC_TRUNCATED", "WARC-Truncated"),
        ("WARC_WARCINFO_ID", "WARC-Warcinfo-ID"),
        ("WARC_FILENAME", "WARC-Filename"),
        ("WARC_PROFILE", "WARC-Profile"),
        ("WARC_IDENTIFIED_PAYLOAD_TYPE", "WARC-Identified-Payload-Type"),
        ("WARC_SEGMENT_ORIGIN_ID", "WARC-Segment-Origin-ID"),
        ("WARC_SEGMENT_NUMBER", "WARC-Segment-Number"),
        ("WARC_SEGMENT_TOTAL_LENGTH", "WARC-Segment-Total-Length"),
    ],
)
def test_warc_header_conversions(name: str, value: str):
    header = getattr(WarcHeader, name)

    assert getattr(fastwarc.warc, "WarcHeader") is WarcHeader
    assert getattr(WarcHeader, name) == header


@pytest.mark.parametrize(
    ("name", "value"),
    [
        ("WARC_TYPE", "WARC-Type"),
        ("WARC_RECORD_ID", "WARC-Record-ID"),
        ("WARC_DATE", "WARC-Date"),
        ("CONTENT_LENGTH", "Content-Length"),
        ("CONTENT_TYPE", "Content-Type"),
        ("WARC_CONCURRENT_TO", "WARC-Concurrent-To"),
        ("WARC_BLOCK_DIGEST", "WARC-Block-Digest"),
        ("WARC_PAYLOAD_DIGEST", "WARC-Payload-Digest"),
        ("WARC_IP_ADDRESS", "WARC-IP-Address"),
        ("WARC_REFERS_TO", "WARC-Refers-To"),
        ("WARC_REFERS_TO_TARGET_URI", "WARC-Refers-To-Target-URI"),
        ("WARC_REFERS_TO_DATE", "WARC-Refers-To-Date"),
        ("WARC_TARGET_URI", "WARC-Target-URI"),
        ("WARC_TRUNCATED", "WARC-Truncated"),
        ("WARC_WARCINFO_ID", "WARC-Warcinfo-ID"),
        ("WARC_FILENAME", "WARC-Filename"),
        ("WARC_PROFILE", "WARC-Profile"),
        ("WARC_IDENTIFIED_PAYLOAD_TYPE", "WARC-Identified-Payload-Type"),
        ("WARC_SEGMENT_ORIGIN_ID", "WARC-Segment-Origin-ID"),
        ("WARC_SEGMENT_NUMBER", "WARC-Segment-Number"),
        ("WARC_SEGMENT_TOTAL_LENGTH", "WARC-Segment-Total-Length"),
    ],
)
def test_warc_header_str_and_repr(name: str, value: str):
    header = getattr(WarcHeader, name)

    assert str(header) == value
    assert repr(header) == f"WarcHeader.{name}"


@pytest.mark.parametrize(
    "record_type",
    [
        warcinfo,
        response,
        resource,
        request,
        metadata,
        revisit,
        conversion,
        continuation,
        unknown,
        any_type,
        no_type,
    ],
)
def test_warc_record_type_conversion_rust_internal(record_type: WarcRecordType):
    # Covers conversion between Python enum and Rust-internal enum types
    record = WarcRecord()
    record.record_type = record_type
    assert record.record_type == record_type


def test_header_map_binding_surface():
    headers = HeaderMap("latin1")
    assert headers.is_empty()
    assert len(headers) == 0
    assert headers.to_tuples() == ()
    assert headers.astuples() == ()
    assert headers.asdict() == {}
    assert headers.keys() == ()
    assert headers.keys_bytes() == ()
    assert headers.values() == ()
    assert headers.values_bytes() == ()
    assert headers.items() == ()
    assert headers != HeaderMap("utf-8")

    assert "<no status line>" in repr(headers)
    headers.status_line_bytes = b"HTTP/1.1 204 No Content"
    assert headers.status_line == "HTTP/1.1 204 No Content"
    assert headers.status_line_bytes == b"HTTP/1.1 204 No Content"
    headers.status_line = "HTTP/1.1 200 OK"
    assert headers.status_line == "HTTP/1.1 200 OK"
    assert headers.status_line_bytes == b"HTTP/1.1 200 OK"

    headers.set("X-Test", "abc")
    headers.append("Set-Cookie", "a=1")
    headers.append_bytes(b"Set-Cookie", b"b=2")
    headers.set_bytes(b"X-Bytes", b"xyz")

    assert not headers.is_empty()
    assert len(headers) == 4
    assert headers.to_tuples() == headers.astuples()
    assert headers.asdict()["X-Test"] == "abc"
    assert headers.keys() == ("X-Test", "Set-Cookie", "Set-Cookie", "X-Bytes")
    assert headers.keys_bytes() == (b"X-Test", b"Set-Cookie", b"Set-Cookie", b"X-Bytes")
    assert headers.values() == ("abc", "a=1", "b=2", "xyz")
    assert headers.values_bytes() == (b"abc", b"a=1", b"b=2", b"xyz")
    assert headers.items() == (
        ("X-Test", "abc"),
        ("Set-Cookie", "a=1"),
        ("Set-Cookie", "b=2"),
        ("X-Bytes", "xyz"),
    )
    assert headers != HeaderMap("latin1")
    assert "HTTP/1.1 200 OK" in repr(headers)

    assert "X-Test" in headers
    assert "X-Bytes" in headers
    assert b"X-Test" in headers.keys_bytes()
    assert b"X-Bytes" in headers.keys_bytes()
    headers.remove("X-Test")
    headers.remove_bytes(b"X-Bytes")
    assert "X-Test" not in headers
    assert "X-Bytes" not in headers
    assert b"X-Test" not in headers.keys_bytes()
    assert b"X-Bytes" not in headers.keys_bytes()

    headers.clear()
    assert headers.is_empty()
    assert len(headers) == 0
    assert headers.status_line is None

    headers.set(WarcHeader.CONTENT_TYPE, "text/plain")
    headers.set_bytes(WarcHeader.CONTENT_LENGTH, b"12")
    assert headers.get(WarcHeader.CONTENT_TYPE) == "text/plain"
    assert headers.get_bytes(WarcHeader.CONTENT_LENGTH) == b"12"
    assert headers.contains_key(WarcHeader.CONTENT_TYPE)
    assert headers.contains_key_bytes(WarcHeader.CONTENT_LENGTH)
    headers[WarcHeader.WARC_RECORD_ID] = "<urn:uuid:test>"
    assert headers[WarcHeader.WARC_RECORD_ID] == "<urn:uuid:test>"
    headers.remove(WarcHeader.CONTENT_TYPE)
    headers.remove_bytes(WarcHeader.CONTENT_LENGTH)
    assert not headers.contains_key(WarcHeader.CONTENT_TYPE)


def test_http_headers_absent_on_non_http_record():
    record = WarcRecord()
    record.init_headers(unknown, b"urn:uuid:no-http")
    record.set_bytes_content(b"payload")
    assert record.http_headers is None
    record.parse_http(strict_mode=False)
    assert record.http_headers is None


def test_header_write_parse_roundtrip():
    headers = HeaderMap("latin1")
    headers.status_line_bytes = b"HTTP/1.1 204 No Content"
    headers.append("Set-Cookie", "a=1")
    headers.append_bytes(b"Set-Cookie", b"b=2")
    headers.set_bytes(b"X-Test", b"abc")

    assert headers.encoding() == "latin1"
    assert headers.status_code == 204
    assert headers.reason_phrase == "No Content"
    assert headers.get("missing", "fallback") == "fallback"
    assert headers.get_bytes(b"missing", b"fallback") == b"fallback"
    assert headers.get_multiple("Set-Cookie") == ("a=1", "b=2")
    assert headers.get_bytes_multiple(b"Set-Cookie") == (b"a=1", b"b=2")
    assert headers.contains_key("set-cookie")
    assert headers.contains_key_bytes(b"x-test")
    assert tuple(iter(headers)) == headers.items()
    assert headers["X-Test"] == "abc"
    assert "X-Test" in headers

    serialized = io.BytesIO()
    assert headers.write(serialized) > 0
    serialized.seek(0)

    parsed = HeaderMap("latin1")
    assert parsed.parse(serialized, has_status_line=True) > 0
    assert parsed == headers
    assert parsed.to_dict()["Set-Cookie"] == "a=1,b=2"
    assert parsed.items_bytes()[0][0] == b"Set-Cookie"


def test_warc_record_write():
    record = _make_http_record()
    record.record_date = dt.datetime(2025, 5, 1, 12, 30, tzinfo=dt.timezone(dt.timedelta(hours=2)))
    record.headers[WarcHeader.WARC_CONCURRENT_TO] = "<urn:uuid:other>"

    payload_digest = hashlib.sha1(HTTP_BODY).digest()
    stream = io.BytesIO()
    bytes_written = record.write(stream, checksum_data=True, payload_digest=payload_digest)

    assert bytes_written == len(stream.getvalue())
    assert record.record_date == dt.datetime(2025, 5, 1, 12, 30, tzinfo=dt.timezone(dt.timedelta(hours=2)))

    iterator = ArchiveIterator(io.BytesIO(stream.getvalue()), parse_http=False)
    rec = next(iterator)

    assert rec.record_type == response
    assert rec.is_http is True
    assert rec.stream_pos == 0
    assert has_block_digest(rec) is True
    assert has_payload_digest(rec) is True
    assert has_valid_block_digest(rec) is True
    assert has_valid_payload_digest(rec) is False
    assert rec.verify_block_digest() is True
    assert rec.verify_payload_digest() is False
    assert is_http(rec) is True

    rec.parse_http(strict_mode=False)
    assert rec.http_content_type == "text/plain"
    assert rec.http_charset == "utf-8"
    assert rec.http_headers is not None
    assert rec.http_date == dt.datetime(1994, 11, 15, 8, 12, 31, tzinfo=dt.timezone.utc)
    assert rec.http_last_modified == dt.datetime(1994, 11, 15, 12, 45, 26, tzinfo=dt.timezone.utc)
    assert rec.verify_payload_digest() is True
    assert has_valid_payload_digest(rec) is True

    reader = rec.reader
    assert reader.readline() == HTTP_BODY
    assert reader.tell() == len(HTTP_BODY)
    assert reader.seek(0) == 0
    assert reader.read(2) == HTTP_BODY[:2]
    assert reader.consume() == len(HTTP_BODY) - 2
    assert rec.consume() == 0

    restored = pickle.loads(pickle.dumps(rec))
    assert restored.record_id == rec.record_id
    assert restored.record_type == rec.record_type
    assert restored.is_http_parsed is True
    assert restored.http_content_type == "text/plain"


def test_record_and_header_pickle():
    record = WarcRecord()
    record.init_headers(unknown, b"urn:uuid:test-record")
    record.record_type = response
    record.is_http = True
    record.set_bytes_content(_http_payload())
    assert record.is_frozen

    # Pickle headers
    assert pickle.loads(pickle.dumps(record.headers)) == record.headers

    # Pickle entire record
    record_roundtrip = pickle.loads(pickle.dumps(record))
    assert record_roundtrip == record
    assert record_roundtrip is not record
    assert record_roundtrip.is_frozen
    assert record_roundtrip.record_id == record.record_id
    assert record_roundtrip.content_length == record.content_length
    assert record_roundtrip.headers is not record.headers
    assert record_roundtrip.headers == record.headers
    assert record_roundtrip.reader.read() == record.reader.read()

    # Pickle keeps reader position on original record (if frozen)
    assert record.reader.seek(13, 0) == 13
    record_roundtrip = pickle.loads(pickle.dumps(record))
    assert record_roundtrip == record
    assert record.reader.tell() == 13
    assert record_roundtrip.reader.tell() == 0

    fixture_record = next(ArchiveIterator(get_fixtures_path() / 'warcfile.warc',
                                          parse_http=False, record_types=WarcRecordType.response))
    assert fixture_record.reader.read(13)
    fixture_record.freeze()
    frozen_remaining = fixture_record.reader.read()
    assert fixture_record.reader.seek(0) == 0

    pickled_fixture_record = pickle.loads(pickle.dumps(fixture_record))
    assert pickled_fixture_record.reader.tell() == 0
    assert pickled_fixture_record.reader.read() == frozen_remaining


@pytest.mark.parametrize(
    ("loader", "record_urn", "body", "frozen"),
    [
        (lambda payload: WarcRecord.from_bytes(payload), b"uuid:from-bytes", b"From bytes", True),
        (lambda payload: WarcRecord.from_reader(io.BytesIO(payload)), b"uuid:from-reader", b"From reader",
         False),
    ],
)
def test_warc_record_from_bytes_or_reader(loader, record_urn: bytes, body: bytes, frozen: bool):
    record = _make_http_record(record_urn=record_urn, body=body)
    serialized = _serialize_record(record)

    parsed = loader(serialized)

    assert parsed.is_frozen is frozen
    assert parsed.record_type == response
    assert parsed.record_id == f"<urn:{record_urn.decode()}>"
    assert parsed.is_http is True
    assert record.is_http_parsed is False
    assert parsed.reader.read() == _http_payload(body)


def test_warc_record_set_record_id():
    record = _make_http_record(record_urn=b"uuid:before")
    assert record.record_id == "<urn:uuid:before>"
    record.record_id = "<urn:uuid:after>"
    assert record.record_id == "<urn:uuid:after>"

    # Roundtrip serialize and read
    buf = io.BytesIO()
    record.write(buf)
    new_rec = WarcRecord()
    new_rec.set_bytes_payload(buf.getvalue())
    new_rec.parse_warc_headers()
    assert new_rec.record_id == "<urn:uuid:after>"


def test_warc_record_parse_warc_headers():
    record_bytes = (get_fixtures_path() / "warcfile.warc").read_bytes()
    record = WarcRecord()
    record.set_bytes_payload(record_bytes)

    bytes_read = record.parse_warc_headers()
    assert bytes_read > 0
    assert record.headers.status_line == "WARC/1.0"
    assert record.record_type == warcinfo
    assert record.record_id is not None
    assert record.content_length > 0
    assert record.reader.tell() == 0

    # Parsing without reader errors out
    with pytest.raises(OSError, match="No reader set"):
        WarcRecord().parse_warc_headers()


def test_warc_record_auto_decode_content_and_transfer():
    payload = b"decoded payload"

    content_record = WarcRecord()
    content_record.init_headers(unknown, b"urn:uuid:content-decode")
    content_record.record_type = response
    content_record.is_http = True
    gz_payload = gzip.compress(payload)
    content_record.set_bytes_content(
        b"HTTP/1.1 200 OK\r\n"
        b"Content-Type: text/plain\r\n"
        b"Content-Encoding: gzip\r\n"
        + f"Content-Length: {len(gz_payload)}\r\n".encode()
        + b"\r\n"
        + gz_payload
    )
    content_record.parse_http(auto_decode="content", strict_mode=False)
    assert content_record.reader.read() == payload

    transfer_record = WarcRecord()
    transfer_record.init_headers(unknown, b"urn:uuid:transfer-decode")
    transfer_record.record_type = response
    transfer_record.is_http = True
    chunked_payload = b"%X\r\n%s\r\n0\r\n\r\n" % (len(payload), payload)
    transfer_record.set_bytes_content(
        b"HTTP/1.1 200 OK\r\n"
        b"Content-Type: text/plain\r\n"
        b"Transfer-Encoding: chunked\r\n"
        + f"Content-Length: {len(chunked_payload)}\r\n".encode()
        + b"\r\n"
        + chunked_payload
    )
    transfer_record.parse_http(auto_decode="transfer", strict_mode=False)
    assert transfer_record.reader.read() == payload


def test_warc_record_equality():
    rec1 = next(ArchiveIterator(get_fixtures_path() / 'warcfile.warc',
                                parse_http=False, record_types=WarcRecordType.response))
    rec1.freeze()
    rec2 = next(ArchiveIterator(get_fixtures_path() / 'warcfile.warc',
                                parse_http=False, record_types=WarcRecordType.response))
    rec2.freeze()
    assert rec1 is rec1
    assert rec1 is not rec2
    assert rec1 == rec2
    assert rec2 == rec1

    pickled1 = pickle.loads(pickle.dumps(rec1))
    pickled2 = pickle.loads(pickle.dumps(rec2))

    assert pickled1 == pickled2
    assert rec1 is not pickled1
    assert rec1 == pickled1
    assert rec1 is not pickled2
    assert rec1 == pickled2
    assert rec2 is not pickled1
    assert rec2 == pickled1
    assert rec2 is not pickled2
    assert rec2 == pickled2

    # Mutating records changes equality checks
    rec1.parse_http()
    assert rec1 != rec2
    rec2.parse_http()
    assert rec1 == rec2

    unfrozen = next(ArchiveIterator(get_fixtures_path() / 'warcfile.warc',
                                    parse_http=False, record_types=WarcRecordType.response))
    # Unfrozen records are not reflexive
    assert unfrozen is unfrozen
    assert unfrozen != unfrozen
    assert unfrozen != rec1
    assert unfrozen != rec2


def test_archive_iterator_accepts_pathlike_and_filters(tmp_path: Path):
    first = _make_http_record(response, record_urn=b"uuid:first")
    second = _make_http_record(request, record_urn=b"uuid:second", body=b"World")
    warc_path = tmp_path / "sample.warc"
    warc_path.write_bytes(_serialize_record(first) + _serialize_record(second))

    seen_ids = []
    iterator = ArchiveIterator(
        warc_path,
        record_types=response | request,
        parse_http=False,
        min_content_length=1,
        func_filter=lambda rec: rec.record_type == response,
        fsspec_args=False,
    )

    assert iter(iterator) is iterator
    record = next(iterator)
    seen_ids.append(record.record_id)
    with pytest.raises(StopIteration):
        next(iterator)

    assert seen_ids == ["<urn:uuid:first>"]


def test_archive_iterator_accepts_fsspec_url():
    pytest.importorskip("fsspec")

    fixture = Path(__file__).resolve().parents[2] / "test-data" / "warcfile.warc.zst"
    iterator = ArchiveIterator(
        fixture.resolve().as_uri(),
        parse_http=False,
        record_types=response,
    )

    record = next(iterator)
    assert record.record_type == response
    assert record.record_id is not None
    assert record.verify_block_digest()


def test_archive_iterator_accepts_fsspec_url_with_args_dict():
    pytest.importorskip("fsspec")

    fixture = Path(__file__).resolve().parents[2] / "test-data" / "warcfile.warc.zst"
    iterator = ArchiveIterator(
        fixture.resolve().as_uri(),
        parse_http=False,
        record_types=response,
        fsspec_args={},
    )

    record = next(iterator)
    assert record.record_type == response
    assert record.record_id is not None
    assert record.verify_block_digest()


def test_archive_iterator_inplace():
    first = _make_http_record(response, record_urn=b"uuid:first", body=b"Hello")
    second = _make_http_record(request, record_urn=b"uuid:second", body=b"World")
    stream = io.BytesIO(_serialize_record(first) + _serialize_record(second))

    iterator = ArchiveIterator(stream, parse_http=False, inplace=True)

    rec1 = next(iterator)
    assert rec1.record_id == "<urn:uuid:first>"
    assert rec1.record_type == response
    assert rec1.reader.read() == _http_payload(b"Hello")

    rec2 = next(iterator)
    assert rec1 is rec2
    assert rec2.record_id == "<urn:uuid:second>"
    assert rec2.record_type == request
    assert rec2.reader.read() == _http_payload(b"World")

    with pytest.raises(StopIteration):
        next(iterator)


def test_archive_iterator_restart_preserves_python_options():
    request_record = _make_http_record(request, record_urn=b"uuid:request")
    response_record = _make_http_record(response, record_urn=b"uuid:response")
    archive = _serialize_record(request_record) + _serialize_record(response_record)

    iterator = ArchiveIterator(
        io.BytesIO(archive), record_types=response, parse_http=False, fsspec_args=False,
    )
    first = next(iterator)
    restarted = iterator.restart(io.BytesIO(archive))

    assert isinstance(restarted, ArchiveIterator)
    assert next(restarted).record_id == first.record_id == "<urn:uuid:response>"
    with pytest.raises(StopIteration):
        next(restarted)


def test_warc_record_string_representations():
    record = _make_http_record(response, record_urn=b"uuid:repr")
    assert repr(record) == "<WarcRecord type='response', id='<urn:uuid:repr>'>"
    assert isinstance(str(record), str)


@pytest.mark.parametrize(
    ("func_filter", "parse_http", "expected_count", "validator"),
    [
        pytest.param(is_warc_10, False, 50, None, id="is-warc-10"),
        pytest.param(is_warc_11, False, 0, None, id="is-warc-11"),
        pytest.param(has_block_digest, False, 16, lambda rec: rec.verify_block_digest(), id="has-block-digest"),
        pytest.param(has_payload_digest, True, 16, lambda rec: rec.verify_payload_digest(), id="has-payload-digest"),
        pytest.param(
            is_http,
            False,
            16 * 2 + 1,
            lambda rec: rec.is_http and rec.record_type in [request, response],
            id="is-http",
        ),
        pytest.param(
            is_concurrent,
            False,
            16 * 2,
            lambda rec: WarcHeader.WARC_CONCURRENT_TO in rec.headers,
            id="is-concurrent",
        ),
        pytest.param(
            has_record_type(response | request),
            False,
            16 * 2 + 1,
            lambda rec: rec.is_http and rec.record_type in [response, request],
            id="has-record-type"
        ),
        pytest.param(
            has_content_length_lte(400),
            False,
            33,
            lambda rec: rec.content_length <= 400,
            id="has-content-length-lte"
        ),
        pytest.param(
            has_content_length_gte(600),
            False,
            16,
            lambda rec: rec.content_length >= 600,
            id="has-content-length-gte"
        ),
        pytest.param(
            lambda rec: rec.record_type in [request, response],
            False,
            16 * 2 + 1,
            lambda rec: rec.record_type in [request, response],
            id="lambda",
        ),
    ],
)
def test_record_func_filters(func_filter, parse_http, expected_count, validator):
    file = get_fixtures_path() / 'warcfile.warc'

    count = 0
    for rec in ArchiveIterator(file, parse_http=parse_http, func_filter=func_filter):
        if validator is not None:
            assert validator(rec)
        count += 1
    assert count == expected_count
