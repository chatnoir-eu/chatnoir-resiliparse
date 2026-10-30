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
import os
import pytest

from fastwarc.warc import ArchiveIterator
from resiliparse.itertools import *

DATA_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', 'test-data'))


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

    with open(in_file, 'rb') as warc_file:
        rec_ids = [rec.record_id for rec in ArchiveIterator(warc_file)]
    assert len(rec_ids) == 50

    # Errors in stream
    prev_stream_pos = -1
    for i, rec in enumerate(warc_retry(ArchiveIterator(stream_factory(reset=True)), stream_factory,
                                       retry_count=5)):
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
        for rec in warc_retry(ArchiveIterator(stream_factory(reset=True)), err_factory,
                               retry_count=5, seek=False):
            assert rec.stream_pos > prev_stream_pos
            prev_stream_pos = rec.stream_pos


@pytest.mark.parametrize('wrapper', [exc_loop, lambda source: warc_retry(source, lambda: None)])
def test_iterator_initialization_error(wrapper):
    wrapped = wrapper(42)
    assert iter(wrapped) is wrapped
    with pytest.raises(TypeError):
        next(wrapped)
    # A failed Python __iter__ call must not be attempted again.
    assert next(wrapped, None) is None


def test_exception_loop_preserves_exception_identity():
    error = RandomException('source failed')

    def source():
        yield object()
        raise error

    wrapped = exc_loop(source())
    item, exc = next(wrapped)
    assert item is not None and exc is None
    assert next(wrapped) == (None, error)
    assert next(wrapped, None) is None
    assert next(wrapped, None) is None


def test_exception_loop_resumes_python_iterator():
    error = RandomException('temporary failure')

    class RecoveringIterator:
        def __init__(self):
            self.calls = 0

        def __iter__(self):
            return self

        def __next__(self):
            self.calls += 1
            if self.calls == 1:
                raise error
            if self.calls == 2:
                return 'recovered'
            raise StopIteration

    assert list(exc_loop(RecoveringIterator())) == [(None, error), ('recovered', None)]


def test_progress_loop_reports_after_yield(monkeypatch):
    from resiliparse import process_guard

    context = object()
    calls = []
    monkeypatch.setattr(process_guard, 'progress', calls.append)
    source = [object(), object()]
    wrapped = process_guard.progress_loop(source, ctx=context)
    assert iter(wrapped) is wrapped
    assert next(wrapped) is source[0]
    assert calls == []
    assert next(wrapped) is source[1]
    assert calls == [context]
    assert next(wrapped, None) is None
    assert calls == [context, context]
    assert next(wrapped, None) is None
    assert calls == [context, context]


@pytest.mark.parametrize('failure', ['initialize', 'source', 'progress'])
def test_progress_loop_propagates_python_exception(monkeypatch, failure):
    from resiliparse import process_guard

    error = RandomException('iteration failed')

    def fail(*args):
        raise error

    class BrokenIterable:
        __iter__ = fail

    def broken_source():
        raise error
        yield

    monkeypatch.setattr(process_guard, 'progress', fail)
    source = {'initialize': BrokenIterable(), 'source': broken_source(), 'progress': [1]}[failure]
    wrapped = process_guard.progress_loop(source)
    if failure == 'progress':
        assert next(wrapped) == 1
    with pytest.raises(RandomException) as caught:
        next(wrapped)
    assert caught.value is error
    assert next(wrapped, None) is None
