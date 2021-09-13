from io import BytesIO
import os
import pytest

from fastwarc.warc import *
from fastwarc.stream_io import *
from resiliparse.itertools import *


DATA_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', 'data'))


def throw_gen(max_val, exc_val):
    for i in range(max_val):
        if i == exc_val:
            raise Exception('Random exception')
        yield i


def test_exc_loop():
    exc = None
    last_val = None
    for val, exc in exc_loop(throw_gen(100, 58)):
        if exc is not None:
            break
        last_val = val

    assert isinstance(exc, Exception)
    assert last_val == 57


class ThrowStream(BytesIO):
    def __init__(self, in_bytes):
        super().__init__(in_bytes)
        self.read_count = 0

    def read(self, size):
        self.read_count += 1
        if self.read_count <= 3:
            raise Exception('Random exception')
        return super().read(size)


def test_warc_retry():
    in_file = os.path.join(DATA_DIR, 'warcfile.warc')
    stream = ThrowStream(open(in_file, 'rb').read())

    def stream_factory(offset=0, reset=False):
        if reset:
            stream.read_count = 0
        stream.seek(offset)
        return stream

    rec_ids = [rec.record_id for rec in ArchiveIterator(FileStream(in_file))]
    assert len(rec_ids) == 50

    prev_stream_pos = -1
    for i, rec in enumerate(warc_retry(ArchiveIterator(stream_factory(reset=True)), stream_factory, retry_count=3)):
        assert rec.stream_pos > prev_stream_pos
        prev_stream_pos = rec.stream_pos
        assert rec_ids[i] == rec.record_id

    prev_stream_pos = -1
    for i, rec in enumerate(warc_retry(ArchiveIterator(stream_factory(0, reset=True)), stream_factory,
                                       retry_count=3, seek=False)):
        assert rec.stream_pos > prev_stream_pos
        prev_stream_pos = rec.stream_pos
        assert rec_ids[i] == rec.record_id

    prev_stream_pos = -1
    for i, rec in enumerate(warc_retry(ArchiveIterator(stream_factory(reset=True)), stream_factory,
                                       retry_count=3, seek=None)):
        assert rec.stream_pos > prev_stream_pos
        prev_stream_pos = rec.stream_pos
        assert rec_ids[i] == rec.record_id

    with pytest.raises(Exception):
        for _ in warc_retry(ArchiveIterator(stream_factory(reset=True)), stream_factory, retry_count=2):
            pass
