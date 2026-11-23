"""Synthetic HTTP chunk streams with fixed output and reader positions."""
import inspect
from io import BytesIO

import pytest
from resiliparse.parse.http import iterate_http_chunks, read_http_chunk

CASES = [
    pytest.param(b'0\r\n\r\n', [], 5, id='zero-chunk'),
    pytest.param(b'3\r\nabc\r\n0\r\n\r\nTRAILER', [b'abc'], 13, id='trailing-data'),
    pytest.param(b'3;extension=yes\r\nabc\r\n2\r\nde\r\n0\r\n\r\n', [b'abc', b'de'], 34, id='chunk-extension'),
    pytest.param(b'0A\r\n0123456789\r\n0\r\n\r\n', [b'0123456789'], 21, id='uppercase-size'),
    pytest.param(b'  +3\r\nabc\r\n0\r\n\r\n', [b'abc'], 16, id='signed-size-with-whitespace'),
    pytest.param(b'3\r\n\x00\xff\x80\r\n0\r\n\r\n', [b'\x00\xff\x80'], 13, id='binary-payload'),
    pytest.param(b'1\r\na\r\n0\r\nX-Trailer: value\r\n\r\n', [b'a'], 29, id='trailer-position'),
]


@pytest.mark.parametrize(('data', 'expected_chunks', 'position'), CASES)
@pytest.mark.parametrize('use_iterator', [False, True], ids=['read', 'iterate'])
def test_synthetic_http_compatibility(data, expected_chunks, position, use_iterator):
    reader = BytesIO(data)
    if use_iterator:
        chunks = list(iterate_http_chunks(reader))
    else:
        chunks = []
        while chunk := read_http_chunk(reader):
            chunks.append(chunk)
    assert chunks == expected_chunks
    assert reader.tell() == position


def test_http_iterator_is_lazy_and_stays_exhausted():
    reader = BytesIO(b'1\r\na\r\n0\r\n\r\n1\r\nb\r\n0\r\n\r\n')
    chunks = iterate_http_chunks(reader)
    assert reader.tell() == 0
    assert iter(chunks) is chunks
    assert next(chunks) == b'a'
    assert reader.tell() == 6
    assert list(chunks) == []
    position = reader.tell()
    assert list(chunks) == []
    assert reader.tell() == position
    assert read_http_chunk(reader) == b'b'


def test_http_iterator_propagates_reader_errors_once():
    class BrokenReader:
        def read(self, size):
            raise OSError('synthetic reader failure')

    chunks = iterate_http_chunks(BrokenReader())
    with pytest.raises(OSError, match='synthetic reader failure'):
        next(chunks)
    assert list(chunks) == []


def test_native_http_module():
    assert inspect.isbuiltin(read_http_chunk)
    assert inspect.isbuiltin(iterate_http_chunks)
    assert read_http_chunk.__module__ == 'resiliparse.parse.http'


def test_http_chunk_retries_short_reads():
    class ShortReader(BytesIO):
        def read(self, size):
            return super().read(min(size, 2))

    reader = ShortReader(b'3\r\nabc\r\n0\r\n\r\n')
    assert read_http_chunk(reader) == b'abc'
    assert read_http_chunk(reader) == b''


@pytest.mark.parametrize(('data', 'message', 'position'), [
    pytest.param(b'', 'Incomplete HTTP chunk framing', 0, id='empty'),
    pytest.param(b'0x3\r\nabc\r\n0\r\n\r\n', 'Invalid HTTP chunk size', 5, id='hex-prefix'),
    pytest.param(b'3junk\r\nabc\r\n0\r\n\r\n', 'Invalid HTTP chunk size', 7, id='size-with-junk-suffix'),
    pytest.param(b'nothex\r\nbody', 'Invalid HTTP chunk size', 8, id='invalid-size'),
    pytest.param(b'xyz\r\nbody', 'Invalid HTTP chunk size', 5, id='invalid-hex'),
    pytest.param(b'\r\nbody', 'Invalid HTTP chunk size', 2, id='empty-header'),
    pytest.param(b'3\r\nab', 'failed to fill whole buffer', 5, id='truncated-payload'),
    pytest.param(b'3\r\nabc', 'failed to fill whole buffer', 6, id='missing-delimiter'),
    pytest.param(b'3\r\nabcX', 'failed to fill whole buffer', 7, id='partial-delimiter'),
    pytest.param(b'3\r\nabcXY0\r\n\r\n', 'Invalid HTTP chunk terminator', 8, id='invalid-delimiter'),
    pytest.param(b'3\nabc\n0\n', 'Incomplete HTTP chunk framing', 2, id='lf-only-header'),
    pytest.param(b'3', 'Incomplete HTTP chunk framing', 1, id='unterminated-size'),
    pytest.param(b'f', 'Incomplete HTTP chunk framing', 1, id='unterminated-hex-size'),
    pytest.param(b'-1\r\nx', 'Invalid HTTP chunk size', 4, id='negative-one'),
    pytest.param(b'-2\r\nx', 'Invalid HTTP chunk size', 4, id='negative-two'),
    pytest.param(b'0\r\nX-Test: value\r\n', 'Incomplete HTTP chunk framing', 18, id='truncated-trailer'),
])
@pytest.mark.parametrize('use_iterator', [False, True], ids=['read', 'iterate'])
def test_invalid_chunk_framing(data, message, position, use_iterator):
    reader = BytesIO(data)
    if use_iterator:
        chunks = iterate_http_chunks(reader)
        with pytest.raises(OSError, match=message):
            list(chunks)
        assert list(chunks) == []
    else:
        with pytest.raises(OSError, match=message):
            read_http_chunk(reader)
    assert reader.tell() == position
