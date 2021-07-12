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

import typing as t

from resiliparse.process_guard cimport progress


def progress_loop(it, ctx=None) -> t.Iterable[t.Any]:
    """
    progress_loop(it, ctx=None)

    Wraps an iterator into a pass-through iterator that reports progress
    to an active :class:`resiliparse.process_guard.TimeGuard` context guard after each iteration.

    :param it: original iterator
    :type it: t.Iterable[t.Any]
    :param ctx: active guard context (will use last global context from stack if unset)
    :return: wrapped iterator
    :rtype: t.Iterable[t.Any]
    """
    for i in it:
        yield i
        progress(ctx)


def exc_loop(it):
    """
    exc_loop(it)

    Wraps an iterator into another iterator that catches and returns any exceptions raised
    while evaluating the input iterator.

    This is primarily useful for unreliable generators that may throw unpredictably at any time
    for unknown reasons (e.g., generators reading from a network data source). If you do not want to
    wrap the entire loop in a ``try/except`` clause, you can use an :func:`exc_loop` to catch
    any such exceptions and return them.
    Remember that a generator will end after throwing an exception, so if the input iterator is
    a generator, you will have to create a new instance in order to retry or continue.

    :param it: original iterator
    :type it: t.Iterable[t.Any]
    :return: iterator of items and ``None`` or ``None`` and exception instance
    :rtype: t.Iterable[(t.Any | None, BaseException | None)]
    """
    i = iter(it)
    while True:
        try:
            yield next(i), None
        except StopIteration as e:
            raise e
        except BaseException as e:
            yield None, e


def warc_retry(archive_iterator, stream_factory, retry_count=3, seek=True):
    """
    warc_retry(archive_iterator, stream_factory, retry_count=3, seek=True)

    Wrap a :class:`fastwarc.warc.ArchiveIterator` to try to continue reading after a stream failure.

    Use if the underlying stream is unreliable, such as when reading from a network data source.
    If an exception other than :exc:`StopIteration` is raised while consuming the iterator, the WARC
    reading process will be retried up to `retry_count` times. When a stream failure occurs,
    ``archive_iterator`` will be reinitialised with a new stream object by calling ``stream_factory``.

    The new stream object returned by ``stream_factory()`` must be seekable. If the stream does not
    support seeking, you can set ``seek=False``. In this case, the stream position in bytes of the last
    successfully read record will be passed as a parameter to ``stream_factory()``. The factory is then
    expected to return a stream that already starts at this exact position (or else reading would
    restart from the beginning resulting in duplicate records). This is primarily useful for streams
    that are not inherently seekable, but have an external facility for starting them at the correct
    position (such as S3 HTTPS streams created from range requests).

    As another option, ``seek`` can also be ``None``, which instructs :func:`warc_retry` to consume the
    stream up to the continuation position. The stream returned by ``stream_factory()`` must start at
    the beginning and will be read normally, but all bytes before the last record will be skipped over
    before continuing to parse the contents. This is the most expensive method of "seeking" on a stream
    and should only be used if the stream is not seekable and there is no other option for starting it
    at the correct offset.

    Exceptions raised inside ``stream_factory()`` will be caught and count towards ``retry_count``.

    Requires FastWARC to be installed.

    :param archive_iterator: input WARC iterator
    :type archive_iterator: fastwarc.warc.ArchiveIterator
    :param stream_factory: callable returning a new stream instance to continue iteration in case of failure
    :type stream_factory: t.Callable[[], t.Any] | t.Callable[[int], t.Any]
    :param retry_count: maximum number of retries before giving up (set to ``None`` or zero for no limit)
    :type retry_count: int
    :param seek: whether to seek to previous position on new stream object (or ``None`` for "stream consumption")
    :type seek: bool | None
    :return: wrapped :class:`~fastwarc.warc.ArchiveIterator`
    :rtype: t.Iterable[fastwarc.warc.WarcRecord]
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

            while True:
                try:
                    if seek is True:
                        stream = stream_factory()
                        stream.seek(last_pos)
                        break
                    elif seek is False:
                        stream = stream_factory(last_pos)
                        break
                    elif seek is None:
                        consumed = 0
                        stream = stream_factory()
                        while consumed < last_pos:
                            n = len(stream.read(min(16384, last_pos - consumed)))
                            if n == 0:
                                return  # Unexpected EOF
                            consumed += n
                        break

                except BaseException as e:
                    retries += 1
                    if retry_count and retries > retry_count:
                        raise e

            # noinspection PyProtectedMember
            archive_iterator._set_stream(stream)
            it = archive_iterator.__iter__()
            skip_next = True
