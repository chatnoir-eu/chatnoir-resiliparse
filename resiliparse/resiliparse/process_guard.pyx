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
from libc.signal cimport SIGINT, SIGTERM, SIGKILL

import inspect
import platform
import threading

from resiliparse_inc.cstdlib cimport strtol
from resiliparse_inc.pthread cimport pthread_kill, pthread_t, pthread_self
from resiliparse_inc.string cimport string, to_string
from resiliparse_inc.stdio cimport FILE, fclose, feof, fgets, fopen, fflush, fprintf, popen, pclose, stderr
from resiliparse_inc.sys.time cimport timeval, gettimeofday
from resiliparse_inc.unistd cimport getpagesize, getpid, usleep


class ResiliparseGuardException(BaseException):
    """Resiliparse guard base exception."""


class ExecutionTimeout(ResiliparseGuardException):
    """Execution timeout exception."""


class MemoryLimitExceeded(ResiliparseGuardException):
    """Memory limit exceeded exception."""


@cython.auto_pickle(False)
cdef class _ResiliparseGuard:
    """Resiliparse context guard base class."""

    def __cinit__(self, *args, **kwargs):
        self.gctx.epoch_counter.store(0)
        self.gctx.ended.store(False)

    def __dealloc__(self):
        self.finish()

    cdef void finish(self):
        if not self.gctx.ended.load():
            self.gctx.ended.store(True)

    def __call__(self, func):
        def guard_wrapper(*args, **kwargs):
            self.exec_before()
            ret = func(*args, **kwargs)
            self.exec_after()
            self.finish()
            return ret

        # Retain self, but do not bind via __get__() or else func will belong to this class
        guard_wrapper._guard_self = self

        # Decorate with public methods of guard instance for convenience
        for attr in dir(self):
            if not attr.startswith('_'):
                setattr(guard_wrapper, attr, getattr(self, attr))

        return guard_wrapper

    def __enter__(self):
        self.exec_before()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.exec_after()
        self.finish()

    cdef void exec_before(self):
        pass

    cdef void exec_after(self):
        pass

    cdef type get_exception_type(self):
        """Interrupt exception type to send"""
        pass

    cdef void send_interrupt(self, unsigned char escalation_level, pthread_t target_thread) nogil:
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
            fprintf(stderr, <char*>b'ERROR: Guarded thread did not respond to TERM signal.\n')
            fflush(stderr)


@cython.auto_pickle(False)
cdef class TimeGuard(_ResiliparseGuard):
    """
    Decorator and context manager for guarding the execution time of a running task.

    Use the :func:`time_guard` factory function for instantiation.
    """

    # noinspection PyMethodOverriding
    def __cinit__(self, size_t timeout, size_t grace_period=15, InterruptType interrupt_type=exception_then_signal,
                  bint send_kill=False, size_t check_interval=500):
        self.timeout = timeout
        self.grace_period = max(1u, grace_period)
        self.interrupt_type = interrupt_type
        self.send_kill = send_kill
        self.check_interval = check_interval

    cdef type get_exception_type(self):
        return ExecutionTimeout

    cdef void exec_before(self):
        cdef pthread_t main_thread = pthread_self()

        def _thread_exec():
            cdef size_t last_epoch = 0
            cdef timeval now
            gettimeofday(&now, NULL)
            cdef size_t start = now.tv_sec
            cdef unsigned char signals_sent = 0

            with nogil:
                while True:
                    usleep(self.check_interval * 1000)

                    if self.gctx.ended.load():
                        break

                    gettimeofday(&now, NULL)

                    if self.gctx.epoch_counter.load() > last_epoch:
                        start = now.tv_sec
                        last_epoch = self.gctx.epoch_counter.load()
                        signals_sent = 0

                    # Exceeded, but within grace period
                    if self.timeout == 0 or (now.tv_sec - start >= self.timeout and signals_sent == 0):
                        signals_sent = 1
                        self.send_interrupt(0, main_thread)
                        if self.timeout == 0:
                            break

                    # Grace period exceeded
                    elif now.tv_sec - start >= (self.timeout + self.grace_period) and signals_sent == 1:
                        signals_sent = 2
                        self.send_interrupt(1, main_thread)

                    # If process still hasn't reacted, send SIGTERM/SIGKILL and then exit
                    elif now.tv_sec - start >= (self.timeout + self.grace_period * 2) and signals_sent == 2:
                        signals_sent = 3
                        self.send_interrupt(2, main_thread)
                        fprintf(stderr, <char*>b'Terminating guard context.\n')
                        fflush(stderr)
                        break


        cdef guard_thread = threading.Thread(target=_thread_exec)
        guard_thread.setDaemon(True)
        guard_thread.start()

    cpdef void progress(self):
        """
        progress(self)
        
        Increment epoch counter to indicate progress and reset the guard timeout.
        This method is thread-safe.
        """
        cdef timeval now
        gettimeofday(&now, NULL)
        self.gctx.epoch_counter.store(now.tv_sec)


def time_guard(size_t timeout, size_t grace_period=15, InterruptType interrupt_type=exception_then_signal,
               bint send_kill=False, size_t check_interval=500) -> TimeGuard:
    """
    time_guard(timeout, grace_period=15, interrupt_type=exception_then_signal, \
               send_kill=False, check_interval=500)

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
    :param grace_period: grace period in seconds after which to send another interrupt
    :type grace_period: int
    :param interrupt_type: type of interrupt
    :type interrupt_type: InterruptType
    :param send_kill: send ``SIGKILL`` as third attempt instead of ``SIGTERM`` (ignored if
                      ``interrupt_type`` is :attr:`~InterruptType.exception`)
    :type send_kill: bool
    :param check_interval: interval in milliseconds between execution time checks
    :type check_interval: int
    :rtype: TimeGuard
    """
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


@cython.auto_pickle(False)
cdef class MemGuard(_ResiliparseGuard):
    """
    Decorator and context manager for enforcing memory limits on a running task.

    Use the :func:`mem_guard` factory function for instantiation.
    """

    # noinspection PyMethodOverriding
    def __cinit__(self, size_t max_memory, bint absolute=True, size_t grace_period=0, size_t secondary_grace_period=5,
                  InterruptType interrupt_type=exception_then_signal, bint send_kill=False, size_t check_interval=500):
        self.max_memory = max_memory
        self.absolute = absolute
        self.grace_period = grace_period
        self.secondary_grace_period = max(1u, secondary_grace_period)
        self.interrupt_type = interrupt_type
        self.send_kill = send_kill
        self.check_interval = check_interval
        self.is_linux = (platform.system() == 'Linux')

    cdef size_t _get_rss_linux(self) nogil:
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

    cdef size_t _get_rss_posix(self) nogil:
        cdef string cmd = string(<char*>b'ps -p ').append(to_string(getpid())).append(<char*>b' -o rss=')
        cdef string buffer = string(64, <char>0)
        cdef string out
        cdef FILE* fp = popen(cmd.c_str(), <char*>b'r')
        if fp == NULL:
            return 0
        while not feof(fp):
            if fgets(buffer.data(), 64, fp) != NULL:
                out.append(buffer.data())
        pclose(fp)
        return strtol(out.c_str(), NULL, 10)

    cdef inline size_t _get_rss(self) nogil:
        if self.is_linux:
            return self._get_rss_linux()
        else:
            return self._get_rss_posix()

    cdef type get_exception_type(self):
        return MemoryLimitExceeded

    cdef void exec_before(self):
        cdef pthread_t main_thread = pthread_self()

        cdef size_t max_mem = self.max_memory
        if not self.absolute:
            max_mem += self._get_rss()

        def _thread_exec():
            cdef size_t grace_start = 0
            cdef unsigned char signals_sent = 0
            cdef size_t rss = 0
            cdef timeval now

            with nogil:
                while True:
                    rss = self._get_rss()

                    if rss > max_mem:
                        # Memory usage above limit, start grace period
                        gettimeofday(&now, NULL)
                        if grace_start == 0:
                            grace_start = now.tv_sec
                            signals_sent = 0

                        # Grace period exceeded
                        if self.grace_period == 0 or (now.tv_sec - grace_start > self.grace_period
                                                      and signals_sent == 0):
                            signals_sent = 1
                            self.send_interrupt(0, main_thread)

                        # Secondary grace period exceeded
                        elif now.tv_sec - grace_start > self.grace_period + self.secondary_grace_period \
                                and signals_sent == 1:
                            signals_sent = 2
                            self.send_interrupt(1, main_thread)

                        # If process still hasn't reacted, send SIGTERM/SIGKILL and then exit
                        elif now.tv_sec - grace_start > self.grace_period + self.secondary_grace_period * 2 \
                                and signals_sent == 2:
                            signals_sent = 3
                            self.send_interrupt(2, main_thread)
                            fprintf(stderr, <char*>b'Terminating guard context.\n')
                            fflush(stderr)
                            break

                    elif rss < max_mem and grace_start != 0:
                        # Memory usage dropped, reset grace period
                        grace_start = 0
                        signals_sent = 0

                    usleep(self.check_interval * 1000)
                    if self.gctx.ended.load():
                        break

        cdef guard_thread = threading.Thread(target=_thread_exec)
        guard_thread.setDaemon(True)
        guard_thread.start()


def mem_guard(size_t max_memory, bint absolute=True, size_t grace_period=0, size_t secondary_grace_period=5,
              InterruptType interrupt_type=exception_then_signal, bint send_kill=False,
              size_t check_interval=500) -> MemGuard:
    """
    mem_guard(max_memory, absolute=True, grace_period=0, secondary_grace_period=5, \
              interrupt_type=exception_then_signal, send_kill=False, check_interval=500)

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

    :param max_memory: max allowed memory in KiB since context creation before interrupt will be sent
    :type max_memory: int
    :param absolute: whether ``max_memory`` is an absolute limit for the process or a relative growth limit
    :type absolute: bool
    :param grace_period: grace period in seconds before sending an interrupt after exceeding ``max_memory``
    :type grace_period: int
    :param secondary_grace_period: time to wait after ``grace_period`` before triggering next escalation level
    :type secondary_grace_period: int
    :param interrupt_type: type of interrupt
    :type interrupt_type: InterruptType
    :param send_kill: send ``SIGKILL`` as third attempt instead of ``SIGTERM`` (ignored if
                     ``interrupt_type`` is :attr:`~InterruptType.exception`)
    :type send_kill: bool
    :param check_interval: interval in milliseconds between memory consumption checks
    :type check_interval: int
    :rtype: MemGuard
    """
    return MemGuard.__new__(MemGuard, max_memory, absolute, grace_period, secondary_grace_period,
                            interrupt_type, send_kill, check_interval)
