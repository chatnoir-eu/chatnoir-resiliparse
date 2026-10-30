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

import inspect
import threading
from typing import Any, Callable, Iterator, Optional, TypeVar

from . import _native

try:
    _process_guard = _native._load_process_guard()
except AttributeError:
    raise ImportError('resiliparse.process_guard is available only on Unix') from None

from . import itertools as _itertools

__all__ = [
    'mem_guard',
    'progress',
    'progress_loop',
    'time_guard',
    'ExecutionTimeout',
    'ResiliparseGuardException',
    'MemGuard',
    'MemoryLimitExceeded',
    'TimeGuard',
    'InterruptType',
]

InterruptType = _process_guard.InterruptType
ResiliparseGuardException = _process_guard.ResiliparseGuardException
ExecutionTimeout = _process_guard.ExecutionTimeout
MemoryLimitExceeded = _process_guard.MemoryLimitExceeded

_F = TypeVar('_F', bound=Callable[..., Any])


class _Guard:
    """Python context-manager and decorator adapter for a native guard."""

    def __init__(self, native_guard: Any):
        self._native = native_guard
        self._guard_thread = None

    def __call__(self, func: _F) -> _F:
        return _make_guarded_callable(self, func)  # type: ignore[return-value]

    def __enter__(self):
        self._native._setup()
        try:
            self._exec_before()
        except BaseException:
            self._finish()
            raise
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        try:
            self._native._exec_after()
        finally:
            self._finish()
        return False

    def _exec_before(self):
        self._native._exec_before()
        watchdog = self._native._make_watchdog()
        self._guard_thread = threading.Thread(target=watchdog.run, daemon=True)
        self._guard_thread.start()

    def _finish(self):
        self._native._stop()
        try:
            if self._guard_thread is not None and self._guard_thread.ident is not None:
                self._guard_thread.join()
        finally:
            self._native._finish()
            self._guard_thread = None

    def __del__(self):
        try:
            self._finish()
        except BaseException:
            pass


def _make_guarded_callable(guard: _Guard, func: _F) -> _F:
    def guard_wrapper(*args, **kwargs):
        guard._native._setup()
        try:
            guard._exec_before()
            return func(*args, **kwargs)
        finally:
            try:
                guard._native._exec_after()
            finally:
                guard._finish()

    # Keep the guard available to progress() and expose its public helpers on
    # the decorated function, matching the context-guard convenience API.
    guard_wrapper._guard_self = guard
    for attr in dir(guard):
        if not attr.startswith('_'):
            setattr(guard_wrapper, attr, getattr(guard, attr))
    return guard_wrapper  # type: ignore[return-value]


class TimeGuard(_Guard):
    """Decorator and context manager for guarding the execution time of a task.

    Instances are created by :func:`time_guard`. The timeout can be reset by
    calling :meth:`progress` from the guarded code.
    """

    def progress(self) -> None:
        """Report progress and restart the guard timeout countdown."""
        self._native.progress()


class MemGuard(_Guard):
    """Decorator and context manager for limiting Linux process memory usage.

    Instances are created by :func:`mem_guard`.
    """


def time_guard(timeout=60, timeout_ms=None, grace_period=15, grace_period_ms=None,
               interrupt_type=InterruptType.exception_then_signal, send_kill=False,
               check_interval=500) -> TimeGuard:
    """
    Create a :class:`TimeGuard` for guarding the execution time of a running task.

    The guard can be used as a decorator or context manager. If the guarded task
    exceeds ``timeout``, the configured interrupt is sent. Reporting progress
    with :meth:`TimeGuard.progress` or :func:`progress` resets the timer.

    There are two interrupt mechanisms: throwing an asynchronous exception and
    sending a UNIX signal. Exceptions are gentle but may not interrupt blocking
    native calls or system calls. Signals are more reliable, but only the
    interpreter's main thread can receive and handle them.

    With ``interrupt_type=InterruptType.exception``, an :exc:`ExecutionTimeout`
    is raised after the timeout and again after the grace period if necessary.
    With ``InterruptType.signal``, the guard sends ``SIGINT`` followed by
    ``SIGTERM``. The default, ``InterruptType.exception_then_signal``, first
    raises an exception and then escalates to signals. ``send_kill=True`` uses
    ``SIGKILL`` for the final signal attempt; this terminates the whole process.

    :param timeout: Maximum execution time in seconds, default 60
    :param timeout_ms: Maximum execution time in milliseconds, overriding ``timeout``
    :param grace_period: Delay in seconds between interrupt attempts, default 15
    :param grace_period_ms: Grace period in milliseconds, overriding ``grace_period``
    :param interrupt_type: Exception, signal, or exception-then-signal behavior
    :param send_kill: Use ``SIGKILL`` instead of ``SIGTERM`` for the final attempt
    :param check_interval: Watchdog polling interval in milliseconds, default 500
    :return: A decorator and context manager for execution-time limits
    :raise ValueError: If both timeout and timeout_ms are None
    :raise OverflowError: If a duration is negative or out of range
    :raise RuntimeError: When entering a nested time guard
    :raise ExecutionTimeout: In the guarded task if its timeout is exceeded and exceptions are enabled
    """
    native = _process_guard.time_guard(
        timeout, timeout_ms, grace_period, grace_period_ms,
        interrupt_type, send_kill, check_interval,
    )
    return TimeGuard(native)


def mem_guard(max_memory: int, absolute: bool = True, grace_period: int = 0, grace_period_ms=None,
              secondary_grace_period: int = 5, secondary_grace_period_ms=None,
              interrupt_type=InterruptType.exception_then_signal, send_kill: bool = False,
              check_interval: int = 500) -> MemGuard:
    """
    Create a :class:`MemGuard` for enforcing a process-memory limit on Linux.

    If process RSS exceeds ``max_memory``, a grace timer starts. The timer resets
    if memory usage falls below the limit. If the limit remains exceeded, the
    guard interrupts the task and follows the selected escalation procedure.

    With ``absolute=True``, ``max_memory`` is the total RSS limit in KiB.
    Otherwise it is the allowed RSS growth above the value measured when the
    guard starts. The first grace period defaults to zero; later escalation
    attempts are separated by the secondary grace period, which defaults to
    five seconds.

    .. warning::

        Memory guards are supported only on Linux. Using the guard on another
        platform raises :class:`RuntimeError` when it is entered or called.

    :param max_memory: Maximum allowed RSS or RSS growth in KiB
    :param absolute: Interpret ``max_memory`` as an absolute process limit
    :param grace_period: Delay in seconds before the first interrupt
    :param grace_period_ms: First grace period in milliseconds, overriding ``grace_period``
    :param secondary_grace_period: Delay in seconds between later escalation attempts
    :param secondary_grace_period_ms: Secondary grace period in milliseconds
    :param interrupt_type: Exception, signal, or exception-then-signal behavior
    :param send_kill: Use ``SIGKILL`` instead of ``SIGTERM`` for the final attempt
    :param check_interval: Watchdog polling interval in milliseconds, default 500
    :return: A decorator and context manager for memory limits
    :raise OverflowError: If a memory limit or duration is negative or out of range
    :raise RuntimeError: When entering the guard outside Linux or inside another memory guard
    :raise MemoryLimitExceeded: In the guarded task if the memory limit is exceeded and exceptions are enabled
    """
    native = _process_guard.mem_guard(
        max_memory, absolute, grace_period, grace_period_ms,
        secondary_grace_period, secondary_grace_period_ms,
        interrupt_type, send_kill, check_interval,
    )
    return MemGuard(native)


def progress(ctx=None) -> None:
    """
    Report progress to the active :class:`TimeGuard` and reset its timeout.

    If ``ctx`` is ``None``, search the Python call stack for a guard context in
    the global namespace or a decorated function carrying ``_guard_self``. If
    the context is local or cannot be found, pass it explicitly.

    :param ctx: Active :class:`TimeGuard` context, or ``None`` for stack lookup
    :raise RuntimeError: If no active time guard can be found
    """
    if ctx is None:
        for frame_info in inspect.stack():
            ctx = frame_info.frame.f_globals.get(frame_info.function)
            if isinstance(ctx, TimeGuard) or isinstance(getattr(ctx, '_guard_self', None), TimeGuard):
                break
            for local in frame_info.frame.f_locals.values():
                if isinstance(local, TimeGuard):
                    ctx = local
                    break
                guard = getattr(local, '_guard_self', None)
                if isinstance(guard, TimeGuard):
                    ctx = guard
                    break
            else:
                continue
            break

    if isinstance(ctx, TimeGuard):
        ctx.progress()
        return

    guard = getattr(ctx, '_guard_self', None)
    if not isinstance(guard, TimeGuard):
        raise RuntimeError('No initialized time guard context.')
    guard.progress()


def progress_loop(it, ctx=None) -> Iterator[Any]:
    """
    Wrap an iterable and report progress to an active time guard after each item.

    This pass-through helper is useful when every iteration should reset the
    timeout. The guard is advanced when iteration resumes after yielding an
    item. If ``ctx`` is ``None``, the guard is found using :func:`progress`'s
    stack lookup.

    :param it: Iterable to pass through
    :param ctx: Active time-guard context, or ``None`` for automatic lookup
    :return: Iterator yielding the original items
    :raise TypeError: During iteration if the input is not iterable
    :raise RuntimeError: During iteration if no active time guard can be found
    """
    return _itertools._process_guard_progress_loop(it, ctx)
