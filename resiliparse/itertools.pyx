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

from typing import Any, Iterable, Optional, Union, Tuple

from resiliparse.process_guard cimport progress


def progress_loop(it: Iterable[Any], ctx=None) -> Iterable[Any]:
    """
    Wraps an iterator into a pass-through iterator that reports progress
    to an active :class:`TimeGuard` context guard after each iteration.

    :param it: original iterator
    :param ctx: active guard context (optional, will use last global context from stack if unset)
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
    wrap the entire loop in a `try/except` clause, you can use an :func:`exc_loop` to catch
    any such exceptions and return them.
    Remember that a generator will end after throwing an exception, so if the input iterator is
    a generator, you will have to create a new instance in order to retry or continue.

    :param it: original iterator
    :return: iterator of items and `None` or `None` and exception instance
    """
    it = iter(it)
    while True:
        try:
            yield next(it), None
        except StopIteration as e:
            raise e
        except (Exception, BaseException) as e:
            yield None, e
