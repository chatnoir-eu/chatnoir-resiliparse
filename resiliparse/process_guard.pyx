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
from threading import current_thread, Thread
from typing import Any, Iterable

from cpython cimport PyObject, PyThreadState_SetAsyncExc

cdef extern from "<signal.h>" nogil:
    const int SIGHUP
    const int SIGINT
    const int SIGTERM
    const int SIGKILL

cdef extern from "<pthread.h>" nogil:
    ctypedef struct pthread
    ctypedef pthread* pthread_t

    pthread_t pthread_self()
    int pthread_kill(pthread_t thread, int sig)

    # ctypedef struct pthread_mutex_t:
    #     long sig
    #     char* opaque
    # ctypedef struct pthread_mutexattr_t:
    #     long sig
    #     char * opaque
    # pthread_mutex_t PTHREAD_MUTEX_INITIALIZER
    # int pthread_mutex_init(pthread_mutex_t* mutex, const pthread_mutexattr_t* attr)
    # int pthread_mutex_destroy(pthread_mutex_t* mutex)
    # int pthread_mutex_lock(pthread_mutex_t* mutex)
    # int pthread_mutex_unlock(pthread_mutex_t* mutex)


cdef extern from "<atomic>" namespace "std" nogil:
    cdef cppclass atomic[T]:
        atomic()
        T load() const
        void store(T desired)
        T fetch_add(T arg)
    ctypedef atomic[bint] atomic_bool
    ctypedef atomic[size_t] atomic_size_t


cdef extern from "<unistd.h>" nogil:
    int usleep(size_t usec)


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
    the timeout, call :func:`refresh()` from the guarded context.

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

    cdef size_t timeout
    cdef size_t grace_period
    cdef bint send_kill
    cdef InterruptType interrupt_type

    def __init__(self, size_t timeout, size_t grace_period=15,
                 InterruptType interrupt_type=exception_then_signal, bint send_kill=False):
        """
        Initialize :class:`TimeGuard` context.

        :param timeout: max execution time in seconds before invoking interrupt
        :param grace_period: grace period in seconds after which to send another (harsher) interrupt
        :param interrupt_type: type of interrupt (default: `InterruptType.exception_then_signal`)
        :param send_kill: if sending signals, send `SIGKILL` as third attempt instead of `SIGTERM`
        """

    def __cinit__(self, size_t timeout, size_t grace_period=15,
                  InterruptType interrupt_type=exception_then_signal, bint send_kill=False):
        self.timeout = timeout
        self.grace_period = grace_period
        self.interrupt_type = interrupt_type
        self.send_kill = send_kill

    cdef void exec_before(self):
        # Save pthread and Python thread IDs (they should be the same, but don't take chances)
        cdef unsigned long main_thread_ident = current_thread().ident
        cdef pthread_t main_thread_id = pthread_self()

        def _thread_exec():
            cdef size_t sec_ctr = 0
            cdef size_t last_epoch = 0

            with nogil:
                while True:
                    if self.gctx.ended.load():
                        break

                    usleep(500 * 1000)

                    if self.gctx.epoch_counter.load() > last_epoch:
                        sec_ctr = 0
                        last_epoch = self.gctx.epoch_counter.load()
                    else:
                        sec_ctr += 1

                    # Exceeded, but within grace period
                    if self.timeout == 0 or sec_ctr == self.timeout * 2:
                        if self.interrupt_type == exception or self.interrupt_type == exception_then_signal:
                            with gil:
                                PyThreadState_SetAsyncExc(main_thread_ident, <PyObject*>ExecutionTimeout)
                        elif self.interrupt_type == signal:
                            pthread_kill(main_thread_id, SIGINT)

                        if self.timeout == 0:
                            break

                    # Grace period exceeded
                    elif sec_ctr == (self.timeout + self.grace_period) * 2:
                        if self.interrupt_type == signal:
                            pthread_kill(main_thread_id, SIGTERM)
                        elif self.interrupt_type == exception_then_signal:
                            pthread_kill(main_thread_id, SIGINT)
                        elif self.interrupt_type == exception:
                            with gil:
                                PyThreadState_SetAsyncExc(main_thread_ident, <PyObject*>ExecutionTimeout)

                    # If process still hasn't reacted, send SIGTERM/SIGKILL and then exit
                    elif sec_ctr >= (self.timeout + self.grace_period * 2) * 2:
                        if self.interrupt_type != exception and self.send_kill:
                            pthread_kill(main_thread_id, SIGKILL)
                        elif self.interrupt_type != exception:
                            pthread_kill(main_thread_id, SIGTERM)
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


def time_guard(size_t timeout, size_t grace_period=15,
               InterruptType interrupt_type=exception_then_signal, bint send_kill=False) -> TimeGuard:
    """
    Decorator for guarding the execution time of a function.

    See :class:`TimeGuard` for details.

    :param timeout: max execution time in seconds before invoking interrupt
    :param grace_period: grace period in seconds after which to send another (harsher) interrupt
    :param interrupt_type: type of interrupt (default: `InterruptType.exception_then_signal`)
    :param send_kill: if sending signals, send `SIGKILL` as third attempt instead of `SIGTERM`
    """
    return TimeGuard.__new__(TimeGuard, timeout, grace_period, interrupt_type, send_kill)


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
        raise RuntimeError('No initialized guard context.')

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
