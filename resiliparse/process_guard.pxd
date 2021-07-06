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

from resiliparse_inc.atomic cimport atomic_size_t, atomic_bool
from resiliparse_inc.pthread cimport pthread_t


cdef struct _GuardContext:
    atomic_size_t epoch_counter
    atomic_bool ended


cpdef enum InterruptType:
    exception,
    signal,
    exception_then_signal


cdef class _ResiliparseGuard:
    cdef _GuardContext gctx
    cdef size_t check_interval
    cdef bint send_kill
    cdef InterruptType interrupt_type
    cdef type exc_type

    cdef void finish(self)
    cdef void exec_before(self)
    cdef void exec_after(self)
    cdef type get_exception_type(self)
    cdef void send_interrupt(self, unsigned char escalation_level, pthread_t target_thread) nogil


cdef class TimeGuard(_ResiliparseGuard):
    cdef size_t timeout
    cdef size_t grace_period

    cdef type get_exception_type(self)
    cdef void exec_before(self)
    cpdef void progress(self)


cpdef progress(ctx=*)


cdef class MemGuard(_ResiliparseGuard):
    cdef size_t max_memory
    cdef bint absolute
    cdef size_t grace_period
    cdef size_t secondary_grace_period
    cdef bint is_linux

    cdef size_t _get_rss_linux(self) nogil
    cdef size_t _get_rss_posix(self) nogil
    cdef inline size_t _get_rss(self) nogil
    cdef type get_exception_type(self)
    cdef void exec_before(self)
