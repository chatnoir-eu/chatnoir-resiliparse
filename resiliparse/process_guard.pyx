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
from typing import Callable

from cpython cimport PyObject, PyThreadState_SetAsyncExc

cdef extern from "<signal.h>" nogil:
    const int SIGHUP
    const int SIGINT
    const int SIGTERM

cdef extern from "<pthread.h>" nogil:
    ctypedef struct pthread
    ctypedef pthread* pthread_t

    pthread_t pthread_self()
    int pthread_kill(pthread_t thread, int sig)

cdef extern from "<unistd.h>" nogil:
    int usleep(size_t usec)


cdef struct _GuardStatus:
    size_t epoch_counter
    bint ended
    void* lock


cpdef enum InterruptType:
    exception,
    signal,
    exception_then_signal


class ResiliparseException(BaseException):
    """Resiliparse base exception."""


class ExecutionTimeout(ResiliparseException):
    """Execution timeout exception."""


class MemoryLimitExceeded(ResiliparseException):
    """Memory limit exceeded exception."""


def timer_guard(size_t timeout, size_t grace_period=15, InterruptType interrupt_type=exception_then_signal):
    """
    Decorator for guarding execution time of a function.

    If a function runs longer than the pre-defined timeout, the guard will send an
    interrupt to the running function context.

    There are two interrupt mechanisms: throwing an asynchronous exception and sending
    a UNIX signal. The exception mechanism is the most gentle method of the two, but
    may be unreliable if execution is blocking outside the Python program context (e.g.,
    in a native C extension or in a `sleep()` routine).

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

    :param timeout: max execution time in seconds before invoking interrupt
    :param grace_period: grace period in seconds after which to send another (harsher) interrupt
    :param interrupt_type: type of interrupt (default: `InterruptType.exception_then_signal`)

    """
    def decorator(func: Callable):
        cdef _GuardStatus gs
        gs.epoch_counter = 0
        gs.ended = False
        # pthread_mutex_init(&gs.lock, NULL)

        def wrapper(*args, **kwargs):
            _init_timer_guard(
                func=func,
                guard_status=&gs,
                timeout=timeout,
                grace_period=grace_period,
                interrupt_type=interrupt_type)
            print(func, 'f')
            func(*args, **kwargs)
            gs.ended = True

        wrapper._rp_gs = <unsigned long>&gs
        return wrapper

    return decorator


def progress(caller):
    cdef _GuardStatus* gs = <_GuardStatus*>caller._rp_gs
    # pthread_mutex_lock(&gs.lock)
    gs.epoch_counter += 1
    # pthread_mutex_unlock(&gs.lock)


cdef void _init_timer_guard(func, _GuardStatus* guard_status,
                            size_t timeout, size_t grace_period, InterruptType interrupt_type):

    # Save pthread and Python thread IDs (they should be the same, but don't take chances)
    cdef unsigned long main_thead_ident = current_thread().ident
    cdef pthread_t main_thread_id = pthread_self()

    class TimerGuardThread(Thread):
        def run(self):
            cdef size_t i = 0
            cdef size_t last_epoch = 0

            with nogil:
                while True:
                    if guard_status.ended:
                        break

                    usleep(500 * 1000)
                    with gil:
                        print(guard_status.epoch_counter)

                    # pthread_mutex_lock(&guard_status.lock)
                    if guard_status.epoch_counter > last_epoch:
                        i = 0
                        last_epoch = guard_status.epoch_counter
                    else:
                        i += 1
                    # pthread_mutex_unlock(&guard_status.lock)

                    # Exceeded, but within grace period
                    if i == timeout * 2:
                        if interrupt_type == exception or interrupt_type == exception_then_signal:
                            with gil:
                                PyThreadState_SetAsyncExc(main_thead_ident, <PyObject*>ExecutionTimeout)
                        elif interrupt_type == signal:
                            pthread_kill(main_thread_id, SIGINT)

                    # Grace period exceeded
                    elif i == (timeout + grace_period) * 2:
                        if interrupt_type == signal:
                            pthread_kill(main_thread_id, SIGTERM)
                        elif interrupt_type == exception_then_signal:
                            pthread_kill(main_thread_id, SIGINT)
                        elif interrupt_type == exception:
                            with gil:
                                PyThreadState_SetAsyncExc(main_thead_ident, <PyObject*>ExecutionTimeout)

                    # If process still hasn't reacted, send SIGTERM and then exit
                    elif i >= (timeout + grace_period * 3) * 2:
                        if interrupt_type != exception:
                            pthread_kill(main_thread_id, SIGTERM)
                        break

    cdef guard_thread = TimerGuardThread()
    guard_thread.setDaemon(True)
    guard_thread.start()
