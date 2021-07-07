# Copyright 2021 Janek Bevendorff
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

# distutils: language = c++

from typing import Any, Callable, Iterable, Optional, Union, Tuple

from resiliparse.process_guard cimport progress


def progress_loop(it: Iterable[Any], ctx=None) -> Iterable[Any]:
    """
    Wraps an iterator into a pass-through iterator that reports progress
    to an active :class:`resiliparse.process_guard.TimeGuard` context guard after each iteration.

    :param it: original iterator
    :type it: Iterable
    :param ctx: active guard context (will use last global context from stack if unset)
    :type ctx: optional
    :return: wrapped iterator
    """
    for i in it:
        yield i
        progress(ctx)


def exc_loop(it: Iterable[Any]) -> Iterable[Tuple[Optional[Any], Optional[Union[Exception, BaseException]]]]:
    """
    Wraps an iterator into another iterator that catches and returns any exceptions raised
    while evaluating the input iterator.

    This is primarily useful for unreliable generators that may throw unpredictably at any time
    for unknown reasons (e.g., generators reading from a network data source). If you do not want to
    wrap the entire loop in a ``try/except`` clause, you can use an :func:`exc_loop` to catch
    any such exceptions and return them.
    Remember that a generator will end after throwing an exception, so if the input iterator is
    a generator, you will have to create a new instance in order to retry or continue.

    :param it: original iterator
    :type it: Iterable
    :return: iterator of items and ``None`` or ``None`` and exception instance
    """
    it = iter(it)
    while True:
        try:
            yield next(it), None
        except StopIteration as e:
            raise e
        except BaseException as e:
            yield None, e


def warc_retry(archive_iterator, stream_factory: Callable, retry_count: int = 3):
    """
    Wrap a :class:`fastwarc.warc.ArchiveIterator` instance to retry in case of read failures.

    Use if the underlying stream is unreliable, such as when reading from a network data source.
    If an exception other than :exc:`StopIteration` is raised while consuming the iterator, the WARC
    reading process will be retried up to `retry_count` times. When a stream failure occurs,
    ``archive_iterator`` will be reinitialised with a new stream object by calling ``stream_factory``.
    The new stream object returned by ``stream_factory()`` must be seekable.

    Requires FastWARC to be installed.

    :param archive_iterator: input WARC iterator
    :type archive_iterator: ~fastwarc.warc.ArchiveIterator
    :param stream_factory: callable returning a new stream instance to continue iteration in case of failure
    :type stream_factory: Callable
    :param retry_count: maximum number of retries before giving up (set to ``None`` or zero for no limit)
    :type retry_count: int, optional, default: 3
    :return: wrapped :class:`~fastwarc.warc.ArchiveIterator`
    """

    cdef size_t retries = 0
    cdef size_t last_pos = 0
    cdef bint skip_next = False
    it = archive_iterator.__iter__()

    while True:
        try:
            if skip_next:
                next(it)
                skip_next = False
            next_rec = next(it)
            last_pos = next_rec.stream_pos
            yield next_rec
        except StopIteration as e:
            raise e
        except BaseException as e:
            retries += 1
            if retry_count and retries > retry_count:
                raise e
            stream = stream_factory()
            stream.seek(max(0, <long>last_pos))
            # noinspection PyProtectedMember
            archive_iterator._set_stream(stream)
            it = archive_iterator.__iter__()
            skip_next = True
