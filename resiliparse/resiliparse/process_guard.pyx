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

from cpython cimport PyObject, PyThreadState_SetAsyncExc
cimport cython
from libc.stdint cimport uint64_t
from libc.signal cimport SIGINT, SIGTERM, SIGKILL
from libcpp.string cimport string, to_string

import inspect
import platform
import threading
import warnings

from resiliparse_inc.cstdlib cimport strtol
from resiliparse_inc.pthread cimport pthread_kill, pthread_t, pthread_self
from resiliparse_inc.stdio cimport FILE, fclose, feof, fgets, fopen
from resiliparse_inc.time cimport timespec, clock_gettime, CLOCK_MONOTONIC
from resiliparse_inc.unistd cimport getpagesize, getpid, usleep


class ResiliparseGuardException(BaseException):
    """Resiliparse guard base exception."""


class ExecutionTimeout(ResiliparseGuardException):
    """Execution timeout exception."""


class MemoryLimitExceeded(ResiliparseGuardException):
    """Memory limit exceeded exception."""


cdef size_t MAX_SIZE_T = <size_t>-1


cdef inline uint64_t time_millis() noexcept nogil:
    cdef timespec t
    clock_gettime(CLOCK_MONOTONIC, &t)
    return t.tv_sec * 1000u + t.tv_nsec / <long>1e6


__GUARD_CTX_ACTIVE = set()


# noinspection PyAttributeOutsideInit
@cython.auto_pickle(False)
cdef class _ResiliparseGuard:
    """Resiliparse context guard base class."""

    def __cinit__(self, *args, **kwargs):
        self.gctx.epoch_counter.store(0)
        self.gctx.ended.store(False)
        self.guard_thread = None

    def __dealloc__(self):
        self.finish()

    cdef inline bint setup(self) except 0:
        global __GUARD_CTX_ACTIVE
        if self.__class__.__name__ in __GUARD_CTX_ACTIVE:
            raise RuntimeError('Guard contexts of the same type cannot be nested.')
        __GUARD_CTX_ACTIVE.add(self.__class__.__name__)
        self.gctx.epoch_counter.store(0)
        self.gctx.ended.store(False)
        return True

    cdef inline bint start_guard_thread(self, func, args=()) except 0:
        self.guard_thread = threading.Thread(target=func, args=args, daemon=True)
        self.guard_thread.start()
        return True

    cdef inline bint finish(self) except 0:
        self.gctx.ended.store(True)
        if self.guard_thread is not None:
            self.guard_thread.join()
            self.guard_thread = None

        global __GUARD_CTX_ACTIVE
        if __GUARD_CTX_ACTIVE is not None and self.__class__.__name__ in __GUARD_CTX_ACTIVE:
            __GUARD_CTX_ACTIVE.remove(self.__class__.__name__)

        return True

    def __call__(self, func):
        def guard_wrapper(*args, **kwargs):
            self.setup()
            self.exec_before()
            try:
                return func(*args, **kwargs)
            finally:
                self.exec_after()
                self.finish()

        # Retain self, but do not bind via __get__() or else func will belong to this class
        guard_wrapper._guard_self = self

        # Decorate with public methods of guard instance for convenience
        for attr in dir(self):
            if not attr.startswith('_'):
                setattr(guard_wrapper, attr, getattr(self, attr))

        return guard_wrapper

    def __enter__(self):
        self.setup()
        self.exec_before()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.exec_after()
        self.finish()

    cdef void exec_before(self) except *:
        pass

    cdef void exec_after(self) except *:
        pass

    cdef type get_exception_type(self):
        """Interrupt exception type to send"""
        pass

    cdef void send_interrupt(self, unsigned char escalation_level, pthread_t target_thread) noexcept nogil:
        if self.gctx.ended.load():
            return

        if escalation_level == 0:
            if self.interrupt_type == exception or self.interrupt_type == exception_then_signal:
                with gil:
                    self.exc_type = self.get_exception_type()
                    PyThreadState_SetAsyncExc(<unsigned long>target_thread, <PyObject*>self.exc_type)
            elif self.interrupt_type == signal:
                pthread_kill(target_thread, SIGINT)

        elif escalation_level == 1:
            if self.interrupt_type == signal:
                pthread_kill(target_thread, SIGTERM)
            elif self.interrupt_type == exception_then_signal:
                pthread_kill(target_thread, SIGINT)
            elif self.interrupt_type == exception:
                with gil:
                    self.exc_type = self.get_exception_type()
                    PyThreadState_SetAsyncExc(<unsigned long>target_thread, <PyObject*>self.exc_type)

        elif escalation_level == 2:
            if self.interrupt_type != exception and self.send_kill:
                pthread_kill(target_thread, SIGKILL)
            elif self.interrupt_type != exception:
                pthread_kill(target_thread, SIGTERM)
            elif self.interrupt_type == exception:
                with gil:
                    self.exc_type = self.get_exception_type()
                    PyThreadState_SetAsyncExc(<unsigned long>target_thread, <PyObject*>self.exc_type)
            with gil:
                warnings.warn('ERROR: Guarded thread did not respond to TERM signal.', RuntimeWarning)


@cython.auto_pickle(False)
cdef class TimeGuard(_ResiliparseGuard):
    """
    Decorator and context manager for guarding the execution time of a running task.

    Use the :func:`time_guard` factory function for instantiation.
    """

    # noinspection PyMethodOverriding
    def __cinit__(self, size_t timeout, size_t grace_period, InterruptType interrupt_type, bint send_kill,
                  size_t check_interval):
        self.timeout = timeout
        self.grace_period = max(10u, grace_period)
        self.interrupt_type = interrupt_type
        self.send_kill = send_kill
        self.check_interval = check_interval

    cdef type get_exception_type(self):
        return ExecutionTimeout

    cdef void exec_before(self) except *:
        cdef pthread_t main_thread = pthread_self()

        def _thread_exec():
            cdef uint64_t last_epoch = 0
            cdef uint64_t now = time_millis()
            cdef uint64_t start = now
            cdef unsigned char signals_sent = 0

            with nogil:
                while True:
                    usleep(self.check_interval * 1000)

                    if self.gctx.ended.load():
                        break

                    now = time_millis()

                    if self.gctx.epoch_counter.load() > last_epoch:
                        start = now
                        last_epoch = self.gctx.epoch_counter.load()
                        signals_sent = 0

                    # Exceeded, but within grace period
                    if (self.timeout == 0 or now - start >= self.timeout) and signals_sent == 0:
                        signals_sent = 1
                        self.send_interrupt(0, main_thread)

                    # Grace period exceeded
                    elif now - start >= (self.timeout + self.grace_period) and signals_sent == 1:
                        signals_sent = 2
                        self.send_interrupt(1, main_thread)

                    # If process still hasn't reacted, send SIGTERM/SIGKILL and then exit
                    elif now - start >= (self.timeout + self.grace_period * 2) and signals_sent == 2:
                        signals_sent = 3
                        self.send_interrupt(2, main_thread)
                        break

        self.start_guard_thread(_thread_exec)

    cpdef void progress(self):
        """
        progress(self)
        
        Increment epoch counter to indicate progress and reset the guard timeout.
        This method is thread-safe.
        """
        self.gctx.epoch_counter.store(time_millis())


def time_guard(timeout=60, timeout_ms=None, grace_period=15, grace_period_ms=None,
               InterruptType interrupt_type=exception_then_signal, bint send_kill=False,
               size_t check_interval=500) -> TimeGuard:
    """
    time_guard(timeout=60, timeout_ms=None, grace_period=15, grace_period_ms=None, \
               interrupt_type=exception_then_signal, send_kill=False, check_interval=500)

    Create a :class:`TimeGuard` instance that can be used as a decorator or context manager
    for guarding the execution time of a running task.

    If a the guarded context runs longer than the pre-defined timeout, the guard will send
    an interrupt to the running thread. The timeout can be reset at any time by proactively
    reporting progress to the guard instance. This can be done by calling
    :meth:`TimeGuard.progress` on the guard instance or the convenience function
    :func:`progress` (if the context is in the global scope).

    There are two interrupt mechanisms: throwing an asynchronous exception and sending
    a UNIX signal. The exception mechanism is the most gentle method of the two, but
    may be unreliable if execution is blocking outside the Python program flow (e.g.,
    in a native C extension or in a syscall). The signal method is more reliable
    in this regard, but does not work if the guarded thread is not the interpreter main
    thread, since only the main thread can receive and handle signals.

    The Interrupt behaviour can be configured with the ``interrupt_type`` parameter, which
    accepts an enum value of type :class:`InterruptType`:

    If ``interrupt_type`` is :attr:`~InterruptType.exception`, an :exc:`ExecutionTimeout`
    exception will be sent to the running thread after `timeout` seconds. If the thread
    does not react, the exception will be thrown once more after `grace_period` seconds.

    If ``interrupt_type`` is :attr:`~InterruptType.signal`, first a ``SIGINT`` will be sent to the
    current thread (which will trigger a :exc:`KeyboardInterrupt` exception, but can
    also be handled with a custom ``signal`` handler). If the thread does not react, a less
    friendly ``SIGTERM`` will be sent after ``grace_period`` seconds. A third and final
    attempt of a ``SIGTERM`` will be sent after another ``grace_period`` seconds.

    If ``interrupt_type`` is :attr:`~InterruptType.exception_then_signal` (the default), the
    first attempt will be an exception and after the grace period, the guard will start
    sending signals.

    With ``send_kill`` set to ``True``, the third and final attempt will be a ``SIGKILL`` instead
    of a ``SIGTERM``. This will kill the entire interpreter (even if the guarded thread is not
    the main thread), so you will need an external facility to restart it.

    :param timeout: max execution time in seconds before invoking interrupt
    :type timeout: int
    :param timeout_ms: max execution time in milliseconds (use instead of ``timeout`` if higher resolution needed)
    :type timeout_ms: int
    :param grace_period: grace period in seconds after which to send another interrupt
    :type grace_period: int
    :param grace_period_ms: grace period in milliseconds (use instead of ``grace_period`` if higher resolution needed)
    :type grace_period_ms: int
    :param interrupt_type: type of interrupt
    :type interrupt_type: InterruptType
    :param send_kill: send ``SIGKILL`` as third attempt instead of ``SIGTERM`` (ignored if
                      ``interrupt_type`` is :attr:`~InterruptType.exception`)
    :type send_kill: bool
    :param check_interval: interval in milliseconds between execution time checks
    :type check_interval: int
    :rtype: TimeGuard
    """
    if timeout is timeout_ms is None:
        raise ValueError('No timeout set')

    timeout = timeout * 1000 if timeout_ms is None else timeout_ms
    grace_period = grace_period * 1000 if grace_period_ms is None else grace_period_ms

    return TimeGuard.__new__(TimeGuard, timeout, grace_period, interrupt_type, send_kill, check_interval)


cpdef progress(ctx=None):
    """
    progress(ctx=None)
    
    Increment :class:`TimeGuard` epoch counter to indicate progress and reset the guard timeout
    for the active guard context surrounding the caller.

    If ``ctx`` ist ``None``, the last valid guard context from the global namespace on
    the call stack will be used. If the guard context does not live in the module's
    global namespace, this auto-detection will fail and the caller has to be supplied
    explicitly.

    If no valid guard context can be determined, the progress report will fail and a
    :class:`RuntimeError` will be raised.

    :param ctx: active guard context (will use last global context from stack if unset)
    :raise RuntimeError: if no valid :class:`TimeGuard` found
    """
    if ctx is None:
        for i in range(len(inspect.stack())):
            frame_info = inspect.stack()[i]
            ctx = frame_info[0].f_globals.get(frame_info[3])
            if isinstance(ctx, TimeGuard) or isinstance(getattr(ctx, '_guard_self', None), TimeGuard):
                break

    if isinstance(ctx, TimeGuard):
        (<TimeGuard>ctx).progress()
        return

    if not isinstance(getattr(ctx, '_guard_self', None), TimeGuard):
        raise RuntimeError('No initialized time guard context.')

    # noinspection PyProtectedMember, PyUnresolvedReferences
    (<TimeGuard>ctx._guard_self).progress()


def progress_loop(it, ctx=None):
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


@cython.auto_pickle(False)
cdef class MemGuard(_ResiliparseGuard):
    """
    Decorator and context manager for enforcing memory limits on a running task.

    Use the :func:`mem_guard` factory function for instantiation.
    """

    # noinspection PyMethodOverriding
    def __cinit__(self, size_t max_memory, bint absolute, size_t grace_period, size_t secondary_grace_period,
                  InterruptType interrupt_type, bint send_kill, size_t check_interval):
        self.max_memory = max_memory
        self.absolute = absolute
        self.grace_period = grace_period
        self.secondary_grace_period = max(10u, secondary_grace_period)
        self.interrupt_type = interrupt_type
        self.send_kill = send_kill
        self.check_interval = check_interval

    # cdef size_t _get_rss_posix(self) noexcept nogil:
    #     cdef string cmd = string(<char*>b'ps -p ').append(to_string(getpid())).append(<char*>b' -o rss=')
    #     cdef string buffer = string(64, <char>0)
    #     cdef string out
    #     cdef FILE* fp = popen(cmd.c_str(), <char*>b'r')
    #     if fp == NULL:
    #         return 0
    #     while not feof(fp):
    #         if fgets(buffer.data(), 64, fp) != NULL:
    #             out.append(buffer.data())
    #     pclose(fp)
    #     return strtol(out.c_str(), NULL, 10)

    cdef inline size_t _get_rss(self) noexcept nogil:
        cdef string proc_file = string(<char*>b'/proc/').append(to_string(getpid())).append(<char*>b'/statm')
        cdef string buffer = string(64, <char>0)
        cdef string statm
        cdef FILE* fp = fopen(proc_file.c_str(), <char*>b'r')
        if fp == NULL:
            return 0
        while not feof(fp):
            if fgets(buffer.data(), 64, fp) != NULL:
                statm.append(buffer.data())
        fclose(fp)

        statm = statm.substr(statm.find(<char*>b' ') + 1)   # VmSize (skip)
        statm = statm.substr(0, statm.find(<char*>b' '))    # VmRSS
        return strtol(statm.c_str(), NULL, 10) * getpagesize() // 1024u

    cdef type get_exception_type(self):
        return MemoryLimitExceeded

    cdef void exec_before(self) except *:
        if platform.system() != 'Linux':
            raise RuntimeError(f'Unsupported platform: {platform.system()}')

        cdef pthread_t main_thread = pthread_self()

        cdef size_t max_mem = self.max_memory
        if not self.absolute:
            max_mem += self._get_rss()

        def _thread_exec():
            cdef uint64_t grace_start = 0
            cdef uint64_t now = 0
            cdef unsigned char signals_sent = 0
            cdef size_t rss = 0

            with nogil:
                while True:
                    rss = self._get_rss()

                    if rss > max_mem:
                        # Memory usage above limit, start grace period
                        now = time_millis()

                        if grace_start == 0:
                            grace_start = now
                            signals_sent = 0

                        # Grace period exceeded
                        if (self.grace_period == 0 or now - grace_start >= self.grace_period) and signals_sent == 0:
                            signals_sent = 1
                            self.send_interrupt(0, main_thread)

                        # Secondary grace period exceeded
                        elif now - grace_start >= self.grace_period + self.secondary_grace_period \
                                and signals_sent == 1:
                            signals_sent = 2
                            self.send_interrupt(1, main_thread)

                        # If process still hasn't reacted, send SIGTERM/SIGKILL and then exit
                        elif now - grace_start >= self.grace_period + self.secondary_grace_period * 2 \
                                and signals_sent == 2:
                            signals_sent = 3
                            self.send_interrupt(2, main_thread)
                            break

                    elif rss < max_mem and grace_start != 0:
                        # Memory usage dropped, reset grace period
                        grace_start = 0
                        signals_sent = 0

                    usleep(self.check_interval * 1000)
                    if self.gctx.ended.load():
                        break

        self.start_guard_thread(_thread_exec)


def mem_guard(size_t max_memory, bint absolute=True, grace_period=0, grace_period_ms=None,
              secondary_grace_period=5, secondary_grace_period_ms=None,
              InterruptType interrupt_type=exception_then_signal, bint send_kill=False,
              size_t check_interval=500) -> MemGuard:
    """
    mem_guard(max_memory, absolute=True, grace_period=0, grace_period_ms=0, secondary_grace_period=5, \
              secondary_grace_period_ms=None, interrupt_type=exception_then_signal, send_kill=False, \
              check_interval=500)

    Create a :class:`MemGuard` instance that can be used as a decorator or context manager
    for enforcing memory limits on a running task.

    :class:`MemGuard` guards a function or other context to stay within pre-defined memory
    bounds. If the running Python process exceeds these bounds while the guard context
    is active, an exception or signal will be sent to the executing thread.

    If the thread does not react to this exception, the same escalation procedure will
    kick in as known from :class:`TimeGuard`. In order for :class:`MemGuard` to tolerate
    short spikes above the memory limit, set the ``grace_period`` parameter to a
    positive non-zero value. If memory usage exceeds the limit, a timer will start
    that expires after ``grace_period`` seconds and triggers the interrupt procedure.
    If memory usage falls below the threshold during the grace period, the timer is reset.

    :class:`MemGuard` provides the same parameters as :class:`TimeGuard` for controlling
    the interrupt escalation behaviour, but the time interval before triggering the
    next escalation level is independent of the grace period and defaults to five
    seconds to give the application sufficient time to react and deallocate excess memory.
    This secondary grace period can be configured with the ``secondary_grace_period``
    parameter and must be at least one second.

    .. warning::

        :class:`MemGuard` is supported only on Linux. You can decorate functions with it
        on other platforms, but calling them will raise an error.

    :param max_memory: max allowed memory in KiB since context creation before interrupt will be sent
    :type max_memory: int
    :param absolute: whether ``max_memory`` is an absolute limit for the process or a relative growth limit
    :type absolute: bool
    :param grace_period: grace period in seconds before sending an interrupt after exceeding ``max_memory``
    :type grace_period: int
    :param grace_period_ms: grace period in milliseconds (use instead of ``grace_period`` if higher resolution needed)
    :type grace_period_ms: int
    :param secondary_grace_period: time to wait after ``grace_period`` before triggering next escalation level
    :type secondary_grace_period: int
    :param secondary_grace_period_ms: secondary grace period in milliseconds (use instead of ``secondary_grace_period``
                                      if higher resolution needed)
    :type secondary_grace_period_ms: int
    :param interrupt_type: type of interrupt
    :type interrupt_type: InterruptType
    :param send_kill: send ``SIGKILL`` as third attempt instead of ``SIGTERM`` (ignored if
                     ``interrupt_type`` is :attr:`~InterruptType.exception`)
    :type send_kill: bool
    :param check_interval: interval in milliseconds between memory consumption checks
    :type check_interval: int
    :rtype: MemGuard
    """
    grace_period = grace_period * 1000 if grace_period_ms is None else grace_period_ms
    secondary_grace_period = secondary_grace_period * 1000 \
        if secondary_grace_period_ms is None else secondary_grace_period_ms

    return MemGuard.__new__(MemGuard, max_memory, absolute, grace_period, secondary_grace_period,
                            interrupt_type, send_kill, check_interval)
