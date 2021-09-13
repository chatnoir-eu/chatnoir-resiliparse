import io
import os
import pytest

from fastwarc.warc import *
from fastwarc.stream_io import *
from resiliparse.itertools import *


DATA_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', 'data'))


class RandomException(Exception):
    pass


def throw_gen(max_val, exc_val):
    for i in range(max_val):
        if i == exc_val:
            raise RandomException()
        yield i


def test_exc_loop():
    # Fail loop
    exc = None
    last_val = None
    for val, exc in exc_loop(throw_gen(100, 58)):
        if exc is not None:
            break
        last_val = val

    assert isinstance(exc, RandomException)
    assert last_val == 57

    # No-fail loop
    exc = None
    last_val = None
    for val, exc in exc_loop(range(100)):
        if exc is not None:
            break
        last_val = val

    assert exc is None
    assert last_val == 99


class ThrowStream(io.BytesIO):
    def __init__(self, in_bytes, throw_idx):
        super().__init__(in_bytes)
        self.read_count = 0
        self.throw_idx = throw_idx

    def read(self, size):
        self.read_count += 1
        if self.read_count - 1 in self.throw_idx:
            raise RandomException()
        return super().read(size)


def test_warc_retry():
    in_file = os.path.join(DATA_DIR, 'warcfile.warc')
    stream = ThrowStream(open(in_file, 'rb').read(), [0, 3, 4, 8, 9])

    def stream_factory(offset=0, reset=False):
        if reset:
            stream.read_count = 0
        stream.seek(offset)
        return stream

    rec_ids = [rec.record_id for rec in ArchiveIterator(FileStream(in_file))]
    assert len(rec_ids) == 50

    # Errors in stream
    prev_stream_pos = -1
    for i, rec in enumerate(warc_retry(ArchiveIterator(stream_factory(reset=True)), stream_factory, retry_count=5)):
        assert rec.stream_pos > prev_stream_pos
        prev_stream_pos = rec.stream_pos
        assert rec_ids[i] == rec.record_id

    prev_stream_pos = -1
    for i, rec in enumerate(warc_retry(ArchiveIterator(stream_factory(0, reset=True)), stream_factory,
                                       retry_count=5, seek=False)):
        assert rec.stream_pos > prev_stream_pos
        prev_stream_pos = rec.stream_pos
        assert rec_ids[i] == rec.record_id

    prev_stream_pos = -1
    for i, rec in enumerate(warc_retry(ArchiveIterator(stream_factory(reset=True)), stream_factory,
                                       retry_count=5, seek=None)):
        assert rec.stream_pos > prev_stream_pos
        prev_stream_pos = rec.stream_pos
        assert rec_ids[i] == rec.record_id

    with pytest.raises(RandomException):
        for _ in warc_retry(ArchiveIterator(stream_factory(reset=True)), stream_factory, retry_count=4):
            pass

    # Error in stream_factory
    factory_has_raised = False

    def err_factory(offset=0):
        nonlocal factory_has_raised
        if offset > 0 and not factory_has_raised:
            # Raise exactly once during stream creation
            factory_has_raised = True
            raise RandomException()
        return stream_factory(offset, False)

    prev_stream_pos = -1
    for i, rec in enumerate(warc_retry(ArchiveIterator(stream_factory(reset=True)), err_factory,
                                       retry_count=6, seek=False)):
        assert rec.stream_pos > prev_stream_pos
        prev_stream_pos = rec.stream_pos
        assert rec_ids[i] == rec.record_id

    factory_has_raised = False
    prev_stream_pos = -1
    with pytest.raises(RandomException):
        for rec in warc_retry(ArchiveIterator(stream_factory(reset=True)), err_factory, retry_count=5, seek=False):
            assert rec.stream_pos > prev_stream_pos
            prev_stream_pos = rec.stream_pos
