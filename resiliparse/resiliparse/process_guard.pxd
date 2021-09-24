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

from resiliparse_inc.atomic cimport atomic_uint64_t, atomic_bool
from resiliparse_inc.pthread cimport pthread_t


cdef struct GuardContext:
    atomic_uint64_t epoch_counter
    atomic_bool ended


cpdef enum InterruptType:
    exception,
    signal,
    exception_then_signal


cdef class _ResiliparseGuard:
    cdef GuardContext gctx
    cdef object guard_thread
    cdef size_t check_interval
    cdef bint send_kill
    cdef InterruptType interrupt_type
    cdef type exc_type

    cdef inline bint setup(self) except 0
    cdef inline bint finish(self) except 0
    cdef inline bint start_guard_thread(self, func, args=*) except 0
    cdef void exec_before(self) except *
    cdef void exec_after(self) except *
    cdef type get_exception_type(self)
    cdef void send_interrupt(self, unsigned char escalation_level, pthread_t target_thread) nogil


cdef class TimeGuard(_ResiliparseGuard):
    cdef size_t timeout
    cdef size_t grace_period

    cdef type get_exception_type(self)
    cdef void exec_before(self) except *
    cpdef void progress(self)


cpdef progress(ctx=*)


cdef class MemGuard(_ResiliparseGuard):
    cdef size_t max_memory
    cdef bint absolute
    cdef size_t grace_period
    cdef size_t secondary_grace_period

    cdef inline size_t _get_rss(self) nogil
    cdef type get_exception_type(self)
    cdef void exec_before(self) except *
