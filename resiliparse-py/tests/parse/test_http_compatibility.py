"""Synthetic HTTP chunk streams with fixed legacy output and reader positions."""
import inspect
from io import BytesIO

import pytest
from resiliparse.parse.http import iterate_http_chunks, read_http_chunk

CASES = [
    pytest.param(b'', [], 0, id='empty'),
    pytest.param(b'0\r\n\r\n', [], 5, id='zero-chunk'),
    pytest.param(b'3\r\nabc\r\n0\r\n\r\nTRAILER', [b'abc'], 13, id='trailing-data'),
    pytest.param(b'3;extension=yes\r\nabc\r\n2\r\nde\r\n0\r\n\r\n', [b'abc', b'de'], 34, id='chunk-extension'),
    pytest.param(b'0A\r\n0123456789\r\n0\r\n\r\n', [b'0123456789'], 21, id='uppercase-size'),
    pytest.param(b'  +3\r\nabc\r\n0\r\n\r\n', [b'abc'], 16, id='signed-size-with-whitespace'),
    pytest.param(b'0x3\r\nabc\r\n0\r\n\r\n', [b'abc'], 15, id='hex-prefix'),
    pytest.param(b'3junk\r\nabc\r\n0\r\n\r\n', [b'abc'], 17, id='size-with-junk-suffix'),
    pytest.param(b'nothex\r\nbody', [], 10, id='invalid-size'),
    pytest.param(b'xyz\r\nbody', [], 7, id='invalid-hex'),
    pytest.param(b'\r\nbody', [], 4, id='empty-header'),
    pytest.param(b'3\r\nab', [b'ab'], 5, id='truncated-payload'),
    pytest.param(b'3\r\nabc', [b'abc'], 6, id='missing-delimiter'),
    pytest.param(b'3\r\nabcX', [b'abc'], 7, id='partial-delimiter'),
    pytest.param(b'3\r\nabcXY0\r\n\r\n', [b'abc'], 13, id='unvalidated-delimiter'),
    pytest.param(b'3\nabc\n0\n', [], 4, id='lf-only-header'),
    pytest.param(b'3', [], 1, id='unterminated-size'),
    pytest.param(b'f', [], 1, id='unterminated-hex-size'),
    pytest.param(b'3\r\n\x00\xff\x80\r\n0\r\n\r\n', [b'\x00\xff\x80'], 13, id='binary-payload'),
    pytest.param(b'1\r\na\r\n0\r\nX-Trailer: value\r\n\r\n', [b'a'], 11, id='trailer-position'),
    pytest.param(b'-1\r\nx', [], 5, id='negative-one'),
    pytest.param(b'-2\r\nx', [], 4, id='negative-two'),
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
        def readline(self):
            raise OSError('synthetic reader failure')

    chunks = iterate_http_chunks(BrokenReader())
    with pytest.raises(OSError, match='synthetic reader failure'):
        next(chunks)
    assert list(chunks) == []


def test_native_http_module():
    assert inspect.isbuiltin(read_http_chunk)
    assert inspect.isbuiltin(iterate_http_chunks)
    assert read_http_chunk.__module__ == 'resiliparse.parse.http'


def test_http_chunk_preserves_single_read_behavior():
    class ShortReader(BytesIO):
        def read(self, size):
            self.requested_size = size
            return super().read(min(size, 2))

    reader = ShortReader(b'3\r\nabc\r\n0\r\n\r\n')
    assert read_http_chunk(reader) == b'ab'
    assert reader.requested_size == 5
    assert reader.tell() == 5
