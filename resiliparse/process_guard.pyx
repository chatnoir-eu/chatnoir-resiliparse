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

import inspect
import platform
from threading import current_thread, Thread
from typing import Any, Iterable

from cpython cimport PyObject, PyThreadState_SetAsyncExc
from libc.stdio cimport FILE, fclose, feof, fgets, fopen, fflush, fprintf, stderr
from libcpp.string cimport string


cdef extern from "<stdio.h>" nogil:
    FILE* popen(const char* command, const char* type);
    int pclose(FILE* stream);

cdef extern from "<string>" namespace "std" nogil:
    string to_string(int i)

cdef size_t strnpos = -1

cdef extern from "<cstdlib>" namespace "std" nogil:
    long strtol(const char* str, char** endptr, int base)

cdef extern from "<signal.h>" nogil:
    const int SIGHUP
    const int SIGINT
    const int SIGTERM
    const int SIGKILL

cdef extern from "<pthread.h>" nogil:
    cdef struct pthread
    ctypedef pthread* pthread_t

    pthread_t pthread_self()
    int pthread_kill(pthread_t thread, int sig)

cdef extern from "<atomic>" namespace "std" nogil:
    cdef cppclass atomic[T]:
        atomic()
        T load() const
        void store(T desired)
        T fetch_add(T arg)
    ctypedef atomic[bint] atomic_bool
    ctypedef atomic[size_t] atomic_size_t

cdef extern from "<unistd.h>" nogil:
    ctypedef int pid_t
    pid_t getpid()
    int getpagesize()
    int usleep(size_t usec)

cdef extern from "<sys/time.h>" nogil:
    ctypedef long int time_t
    ctypedef long int suseconds_t
    cdef struct timeval:
        long int tv_sec
    cdef struct timezone
    int gettimeofday(timeval* tv, timezone* tz)

cdef struct _GuardContext:
    atomic_size_t epoch_counter
    atomic_bool ended

cpdef enum InterruptType:
    exception,
    signal,
    exception_then_signal


class ResiliparseGuardException(BaseException):
    """Resiliparse guard base exception."""


class ExecutionTimeout(ResiliparseGuardException):
    """Execution timeout exception."""


class MemoryLimitExceeded(ResiliparseGuardException):
    """Memory limit exceeded exception."""


cdef class _ResiliparseGuard:
    cdef _GuardContext gctx

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

        # Retain `self`, but do not bind via `__get__()` or else `func` will belong to this class
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


cdef class TimeGuard(_ResiliparseGuard):
    """
    Execution time context guard.

    If a the guarded context runs longer than the pre-defined timeout, the guard will send
    an interrupt to the running function context. To signal progress to the guard and reset
    the timeout, call :meth:`TimeGuard.progress` or :func:`progress` from the guarded context.

    There are two interrupt mechanisms: throwing an asynchronous exception and sending
    a UNIX signal. The exception mechanism is the most gentle method of the two, but
    may be unreliable if execution is blocking outside the Python program flow (e.g.,
    in a native C extension or in a syscall). The signal method is more reliable
    in this regard, but does not work if the guarded thread is not the interpreter main
    thread, since only the main thread can receive and handle signals.

    Interrupt behaviour can be configured with the `interrupt_type` constructor parameter:

    If `interrupt_type` is `InterruptType.exception`, a :class:`ExecutionTimeout`
    exception will be sent to the running thread after `timeout` seconds. If the thread
    does not react, the exception will be thrown once more after `grace_period` seconds.

    If `interrupt_type` is `InterruptType.signal`, first a `SIGINT` will be sent to the
    current thread (which will trigger a :class:`KeyboardInterrupt` exception, but can
    also be handled with a custom `signal` handler. If the thread does not react, a less
    friendly `SIGTERM` will be sent after `grace_period` seconds. A third and final
    attempt of a `SIGTERM` will be sent after `grace_period`.

    If `interrupt_type` is `InterruptType.exception_then_signal` (the default), the
    first attempt will be an exception and after the grace period, the guard will
    start sending signals.

    With `send_kill` set to `True`, the third and final attempt will be a `SIGKILL` instead
    of a `SIGTERM`. This will kill the entire interpreter (even if the guarded thread is not
    the main thread), so you will need an external facility to restart it.
    """

    cdef size_t check_interval
    cdef size_t timeout
    cdef size_t grace_period
    cdef bint send_kill
    cdef InterruptType interrupt_type

    def __init__(self, size_t timeout, size_t grace_period=15, InterruptType interrupt_type=exception_then_signal,
                 bint send_kill=False, size_t check_interval=500):
        """
        Initialize :class:`TimeGuard` context.

        :param timeout: max execution time in seconds before invoking interrupt
        :param grace_period: grace period in seconds after which to send another (harsher) interrupt
        :param interrupt_type: type of interrupt (default: `InterruptType.exception_then_signal`)
        :param send_kill: if sending signals, send `SIGKILL` as third attempt instead of `SIGTERM`
        :param check_interval: interval in milliseconds between execution time checks
        """

    def __cinit__(self, size_t timeout, size_t grace_period=15, InterruptType interrupt_type=exception_then_signal,
                  bint send_kill=False, size_t check_interval=500):
        self.timeout = timeout
        self.grace_period = grace_period
        self.interrupt_type = interrupt_type
        self.send_kill = send_kill
        self.check_interval = check_interval

    cdef void exec_before(self):
        # Save pthread and Python thread IDs (they should be the same, but don't take chances)
        cdef unsigned long main_thread_ident = current_thread().ident
        cdef pthread_t main_thread_id = pthread_self()

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
                        if self.interrupt_type == exception or self.interrupt_type == exception_then_signal:
                            with gil:
                                PyThreadState_SetAsyncExc(main_thread_ident, <PyObject*>ExecutionTimeout)
                        elif self.interrupt_type == signal:
                            pthread_kill(main_thread_id, SIGINT)

                        if self.timeout == 0:
                            break

                    # Grace period exceeded
                    elif now.tv_sec - start >= (self.timeout + self.grace_period) and signals_sent == 1:
                        signals_sent = 2
                        if self.interrupt_type == signal:
                            pthread_kill(main_thread_id, SIGTERM)
                        elif self.interrupt_type == exception_then_signal:
                            pthread_kill(main_thread_id, SIGINT)
                        elif self.interrupt_type == exception:
                            with gil:
                                PyThreadState_SetAsyncExc(main_thread_ident, <PyObject*>ExecutionTimeout)

                    # If process still hasn't reacted, send SIGTERM/SIGKILL and then exit
                    elif now.tv_sec - start >= (self.timeout + self.grace_period * 2) and signals_sent == 2:
                        signals_sent = 3
                        if self.interrupt_type != exception and self.send_kill:
                            pthread_kill(main_thread_id, SIGKILL)
                        elif self.interrupt_type != exception:
                            pthread_kill(main_thread_id, SIGTERM)
                        fprintf(stderr, <char*>b'ERROR: Guarded thread did not respond to TERM signal. '
                                               b'Terminating guard context.\n')
                        fflush(stderr)
                        break


        cdef guard_thread = Thread(target=_thread_exec)
        guard_thread.setDaemon(True)
        guard_thread.start()

    cpdef void progress(self):
        """
        Increment epoch counter to indicate progress and reset the guard timeout.
        This method is thread-safe.
        """
        self.gctx.epoch_counter.fetch_add(1)


def time_guard(size_t timeout, size_t grace_period=15, InterruptType interrupt_type=exception_then_signal,
               bint send_kill=False, check_interval=500) -> TimeGuard:
    """
    Decorator and context manager for guarding the execution time of a program context.

    See :class:`TimeGuard` for details.

    :param timeout: max execution time in seconds before invoking interrupt
    :param grace_period: grace period in seconds after which to send another (harsher) interrupt
    :param interrupt_type: type of interrupt (default: `InterruptType.exception_then_signal`)
    :param send_kill: if sending signals, send `SIGKILL` as third attempt instead of `SIGTERM`
    :param check_interval: interval in milliseconds between execution time checks
    """
    return TimeGuard.__new__(TimeGuard, timeout, grace_period, interrupt_type, send_kill, check_interval)


cpdef progress(ctx=None):
    """
    Increment :class:`TimeGuard` epoch counter to indicate progress and reset the guard timeout
    for the active guard context surrounding the caller.

    If `ctx` ist `None`, the last valid guard context from the global namespace on
    the call stack will be used. If the guard context does not live in the module's
    global namespace, this auto-detection will fail and the caller has to be supplied
    explicitly.

    If `ctx` ist not a valid guard context, the progress report will fail and a
    :class:`RuntimeError` will be raised.

    :param ctx: active guard context (optional, will use last global context from stack if unset)
    """
    if ctx is None:
        for i in range(len(inspect.stack())):
            frame_info = inspect.stack()[i]
            ctx = frame_info[0].f_globals.get(frame_info[3])
            if isinstance(getattr(ctx, '_guard_self', None), TimeGuard):
                break

    if not isinstance(getattr(ctx, '_guard_self', None), TimeGuard):
        raise RuntimeError('No initialized time guard context.')

    # noinspection PyProtectedMember
    (<TimeGuard>ctx._guard_self).progress()


def progress_loop(it: Iterable[Any], ctx=None) -> Iterable[Any]:
    """
    Wraps an iterator to report progress after each iteration.

    :param it: original iterator
    :param ctx: active guard context (optional, will use last global context from stack if unset)
    :return: wrapped iterator
    """
    for i in it:
        yield i
        progress(ctx)


cdef class MemGuard(_ResiliparseGuard):
    """
    Process memory guard.
    """

    cdef size_t check_interval
    cdef size_t max_memory
    cdef bint absolute
    cdef size_t grace_period
    cdef bint send_kill
    cdef InterruptType interrupt_type
    cdef bint is_linux

    def __init__(self, size_t max_memory, bint absolute=True, size_t grace_period=15,
                 InterruptType interrupt_type=exception_then_signal, bint send_kill=False,
                 size_t check_interval=500):
        """
        Initialize :class:`MemGuard` context.

        :param max_memory: max allowed memory kB since context creation before interrupt will be sent
        :param absolute: whether `max_memory` is an absolute limit for the process or a relative growth limit
        :param grace_period: grace period in seconds before an interrupt will be sent after exceeding `max_memory`
        :param interrupt_type: type of interrupt (default: `InterruptType.exception_then_signal`)
        :param send_kill: if sending signals, send `SIGKILL` as third attempt instead of `SIGTERM`
        :param check_interval: interval in milliseconds between memory consumption checks
        """

    def __cinit__(self, size_t max_memory, bint absolute=True, size_t grace_period=15,
                  InterruptType interrupt_type=exception_then_signal, bint send_kill=False,
                 size_t check_interval=800):
        self.max_memory = max_memory
        self.absolute = absolute
        self.grace_period = grace_period
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
        cdef string cmd = string(<char*>b'ps ').append(to_string(getpid())).append(<char*>b' -o rss=')
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

    cdef void exec_before(self):
        # Save pthread and Python thread IDs (they should be the same, but don't take chances)
        cdef unsigned long main_thread_ident = current_thread().ident
        cdef pthread_t main_thread_id = pthread_self()

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

                        # Exceeded, but within grace period
                        elif now.tv_sec - grace_start > self.grace_period and signals_sent == 0:
                            signals_sent = 1
                            if self.interrupt_type == exception or self.interrupt_type == exception_then_signal:
                                with gil:
                                    PyThreadState_SetAsyncExc(main_thread_ident, <PyObject*>MemoryLimitExceeded)
                            elif self.interrupt_type == signal:
                                pthread_kill(main_thread_id, SIGINT)

                        # Grace period exceeded
                        elif now.tv_sec - grace_start > self.grace_period * 2 and signals_sent == 1:
                            signals_sent = 2
                            if self.interrupt_type == signal:
                                pthread_kill(main_thread_id, SIGTERM)
                            elif self.interrupt_type == exception_then_signal:
                                pthread_kill(main_thread_id, SIGINT)
                            elif self.interrupt_type == exception:
                                with gil:
                                    PyThreadState_SetAsyncExc(main_thread_ident, <PyObject*>MemoryLimitExceeded)

                        # If process still hasn't reacted, send SIGTERM/SIGKILL and then exit
                        elif now.tv_sec - grace_start > self.grace_period * 3 and signals_sent == 2:
                            signals_sent = 3
                            if self.interrupt_type != exception and self.send_kill:
                                pthread_kill(main_thread_id, SIGKILL)
                            elif self.interrupt_type != exception:
                                pthread_kill(main_thread_id, SIGTERM)
                            fprintf(stderr, <char *> b'ERROR: Guarded thread did not respond to TERM signal. '
                                                     b'Terminating guard context.\n')
                            fflush(stderr)
                            break

                    elif rss < max_mem and grace_start != 0:
                        # Memory usage dropped, reset grace period
                        grace_start = 0

                    usleep(self.check_interval * 1000)
                    if self.gctx.ended.load():
                        break

        cdef guard_thread = Thread(target=_thread_exec)
        guard_thread.setDaemon(True)
        guard_thread.start()


def mem_guard(size_t max_memory, bint absolute=True, size_t grace_period=15,
              InterruptType interrupt_type=exception_then_signal, bint send_kill=False,
              size_t check_interval=800) -> MemGuard:
    """
    Decorator and context manager for guarding maximum memory usage of a program context.

    See :class:`MemGuard` for details.

    :param max_memory: max allowed memory in kB since context creation before interrupt will be sent
    :param absolute: whether `max_memory` is an absolute limit for the process or a relative growth limit
    :param grace_period: grace period in seconds before an interrupt will be sent after exceeding `max_memory`
    :param interrupt_type: type of interrupt (default: `InterruptType.exception_then_signal`)
    :param send_kill: if sending signals, send `SIGKILL` as third attempt instead of `SIGTERM`
    :param check_interval: interval in milliseconds between memory consumption checks
    """
    return MemGuard.__new__(MemGuard, max_memory, absolute, grace_period, interrupt_type, send_kill, check_interval)
