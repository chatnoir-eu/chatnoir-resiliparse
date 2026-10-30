// Copyright 2026 Janek Bevendorff
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use pyo3::create_exception;
use pyo3::exceptions::{PyBaseException, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyModule;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

create_exception!(
    resiliparse.process_guard,
    ResiliparseGuardException,
    PyBaseException,
    "Base exception raised by a Resiliparse process guard."
);
create_exception!(
    resiliparse.process_guard,
    ExecutionTimeout,
    ResiliparseGuardException,
    "Execution time limit exceeded."
);
create_exception!(resiliparse.process_guard, MemoryLimitExceeded, ResiliparseGuardException, "Memory limit exceeded.");

/// Method used to interrupt a task after a guard limit is exceeded.
#[pyclass(
    eq,
    eq_int,
    from_py_object,
    rename_all = "snake_case",
    module = "resiliparse.process_guard"
)]
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InterruptType {
    Exception = 0,
    Signal = 1,
    ExceptionThenSignal = 2,
}

struct GuardContext {
    epoch_counter: AtomicU64,
    ended: AtomicBool,
}

impl GuardContext {
    fn new() -> Self {
        Self {
            epoch_counter: AtomicU64::new(0),
            ended: AtomicBool::new(false),
        }
    }
}

static ACTIVE_GUARDS: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
static MONOTONIC_START: OnceLock<Instant> = OnceLock::new();

fn time_millis() -> u64 {
    MONOTONIC_START
        .get_or_init(Instant::now)
        .elapsed()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

fn active_guards() -> &'static Mutex<HashSet<&'static str>> {
    ACTIVE_GUARDS.get_or_init(|| Mutex::new(HashSet::new()))
}

struct GuardLifecycle {
    context: Arc<GuardContext>,
    active_type: Option<&'static str>,
}

impl GuardLifecycle {
    fn new() -> Self {
        Self {
            context: Arc::new(GuardContext::new()),
            active_type: None,
        }
    }

    fn setup(&mut self, guard_type: &'static str) -> PyResult<()> {
        let mut active = active_guards().lock().unwrap_or_else(|error| error.into_inner());
        if !active.insert(guard_type) {
            return Err(PyRuntimeError::new_err("Guard contexts of the same type cannot be nested."));
        }
        self.active_type = Some(guard_type);
        self.context.epoch_counter.store(0, Ordering::Release);
        self.context.ended.store(false, Ordering::Release);
        Ok(())
    }

    fn finish(&mut self) {
        self.context.ended.store(true, Ordering::Release);
        if let Some(guard_type) = self.active_type.take() {
            active_guards()
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .remove(guard_type);
        }
    }
}

impl Drop for GuardLifecycle {
    fn drop(&mut self) {
        self.context.ended.store(true, Ordering::Release);
        if let Some(guard_type) = self.active_type.take() {
            active_guards()
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .remove(guard_type);
        }
    }
}

#[derive(Clone, Copy)]
struct TargetThread {
    python_id: libc::c_long,
    #[cfg(unix)]
    pthread: libc::pthread_t,
}

impl TargetThread {
    fn current() -> Self {
        Self {
            // SAFETY: this is called by a Python method while its thread holds
            // the GIL; CPython returns that thread's stable identifier.
            python_id: unsafe { pyo3::ffi::PyThread_get_thread_ident() as libc::c_long },
            #[cfg(unix)]
            // SAFETY: pthread_self is valid for the current POSIX thread.
            pthread: unsafe { libc::pthread_self() },
        }
    }
}

#[cfg(unix)]
fn send_signal(target: TargetThread, signal: libc::c_int) {
    // SAFETY: target was captured from a live Python thread in this process.
    unsafe {
        libc::pthread_kill(target.pthread, signal);
    }
}

fn send_exception(context: &GuardContext, target: TargetThread, exception_name: &'static str) {
    if context.ended.load(Ordering::Acquire) {
        return;
    }
    Python::attach(|py| {
        if context.ended.load(Ordering::Acquire) {
            return;
        }
        let Ok(module) = PyModule::import(py, "resiliparse.process_guard") else {
            return;
        };
        let Ok(exception_type) = module.getattr(exception_name) else {
            return;
        };
        // SAFETY: CPython documents this API for scheduling an exception in a
        // target thread. The exception type stays strongly referenced here.
        let result = unsafe {
            pyo3::ffi::PyThreadState_SetAsyncExc(target.python_id, exception_type.as_ptr() as *mut pyo3::ffi::PyObject)
        };
        if result > 1 {
            // SAFETY: roll back an ambiguous thread match as required by the
            // PyThreadState_SetAsyncExc API contract.
            unsafe {
                pyo3::ffi::PyThreadState_SetAsyncExc(target.python_id, std::ptr::null_mut());
            }
        }
    });
}

fn warn_unresponsive_thread() {
    Python::attach(|py| {
        if let Ok(warnings) = PyModule::import(py, "warnings") {
            let _ = warnings.call_method1("warn", ("ERROR: Guarded thread did not respond to TERM signal.",));
        }
    });
}

fn interrupt(
    context: &GuardContext,
    target: TargetThread,
    interrupt_type: InterruptType,
    send_kill: bool,
    escalation_level: u8,
    exception_name: &'static str,
) {
    if context.ended.load(Ordering::Acquire) {
        return;
    }
    match (escalation_level, interrupt_type) {
        (0, InterruptType::Exception | InterruptType::ExceptionThenSignal)
        | (1, InterruptType::Exception)
        | (2, InterruptType::Exception) => send_exception(context, target, exception_name),
        (0, InterruptType::Signal) => send_signal(target, libc::SIGINT),
        (1, InterruptType::Signal) => send_signal(target, libc::SIGTERM),
        (1, InterruptType::ExceptionThenSignal) => send_signal(target, libc::SIGINT),
        (2, InterruptType::Signal | InterruptType::ExceptionThenSignal) => {
            send_signal(target, if send_kill { libc::SIGKILL } else { libc::SIGTERM });
        }
        _ => {}
    }
    if escalation_level == 2 && send_kill && interrupt_type != InterruptType::Exception {
        warn_unresponsive_thread();
    }
}

fn time_watchdog(
    context: Arc<GuardContext>,
    target: TargetThread,
    timeout: u64,
    grace_period: u64,
    interrupt_type: InterruptType,
    send_kill: bool,
    check_interval: u64,
    exception_name: &'static str,
) {
    let mut last_epoch = 0;
    let mut start = time_millis();
    let mut signals_sent = 0u8;
    loop {
        thread::sleep(Duration::from_millis(check_interval));
        if context.ended.load(Ordering::Acquire) {
            break;
        }
        let now = time_millis();
        let epoch = context.epoch_counter.load(Ordering::Acquire);
        if epoch > last_epoch {
            start = now;
            last_epoch = epoch;
            signals_sent = 0;
        }
        let elapsed = now.saturating_sub(start);
        if (timeout == 0 || elapsed >= timeout) && signals_sent == 0 {
            signals_sent = 1;
            interrupt(&context, target, interrupt_type, send_kill, 0, exception_name);
        } else if elapsed >= timeout.saturating_add(grace_period) && signals_sent == 1 {
            signals_sent = 2;
            interrupt(&context, target, interrupt_type, send_kill, 1, exception_name);
        } else if elapsed >= timeout.saturating_add(grace_period.saturating_mul(2)) && signals_sent == 2 {
            interrupt(&context, target, interrupt_type, send_kill, 2, exception_name);
            break;
        }
    }
}

#[derive(Clone, Copy)]
enum WatchdogTask {
    Time {
        timeout: u64,
        grace_period: u64,
        interrupt_type: InterruptType,
        send_kill: bool,
        check_interval: u64,
    },
    Memory {
        max_memory: u64,
        grace_period: u64,
        secondary_grace_period: u64,
        interrupt_type: InterruptType,
        send_kill: bool,
        check_interval: u64,
    },
}

#[pyclass(module = "resiliparse.process_guard", name = "_GuardWatchdog")]
struct GuardWatchdog {
    context: Arc<GuardContext>,
    target: TargetThread,
    task: WatchdogTask,
}

#[pymethods]
impl GuardWatchdog {
    fn run(&self, py: Python<'_>) {
        let context = Arc::clone(&self.context);
        let target = self.target;
        let task = self.task;
        py.detach(move || match task {
            WatchdogTask::Time {
                timeout,
                grace_period,
                interrupt_type,
                send_kill,
                check_interval,
            } => time_watchdog(
                context,
                target,
                timeout,
                grace_period,
                interrupt_type,
                send_kill,
                check_interval,
                "ExecutionTimeout",
            ),
            WatchdogTask::Memory {
                max_memory,
                grace_period,
                secondary_grace_period,
                interrupt_type,
                send_kill,
                check_interval,
            } => memory_watchdog(
                context,
                target,
                max_memory,
                grace_period,
                secondary_grace_period,
                interrupt_type,
                send_kill,
                check_interval,
            ),
        });
    }
}

/// Guard execution while allowing periodic progress reports to reset its timer.
#[pyclass(module = "resiliparse.process_guard", name = "_TimeGuard")]
struct TimeGuard {
    lifecycle: GuardLifecycle,
    target_thread: Option<TargetThread>,
    timeout: u64,
    grace_period: u64,
    interrupt_type: InterruptType,
    send_kill: bool,
    check_interval: u64,
}

#[pymethods]
impl TimeGuard {
    /// Report progress and restart the execution-time countdown.
    fn progress(&self) {
        self.lifecycle
            .context
            .epoch_counter
            .store(time_millis(), Ordering::Release);
    }

    fn _setup(&mut self) -> PyResult<()> {
        self.lifecycle.setup("TimeGuard")
    }

    fn _exec_before(&mut self) {
        self.target_thread = Some(TargetThread::current());
    }

    fn _make_watchdog(&self, py: Python<'_>) -> PyResult<Py<GuardWatchdog>> {
        let target = self
            .target_thread
            .ok_or_else(|| PyRuntimeError::new_err("Guard thread was not initialized."))?;
        Py::new(
            py,
            GuardWatchdog {
                context: Arc::clone(&self.lifecycle.context),
                target,
                task: WatchdogTask::Time {
                    timeout: self.timeout,
                    grace_period: self.grace_period,
                    interrupt_type: self.interrupt_type,
                    send_kill: self.send_kill,
                    check_interval: self.check_interval,
                },
            },
        )
    }

    fn _exec_after(&self) {}

    fn _stop(&self) {
        self.lifecycle.context.ended.store(true, Ordering::Release);
    }

    fn _finish(&mut self) {
        self.lifecycle.finish();
    }

    fn __del__(&mut self) {
        self.lifecycle.finish();
    }
}

/// Create a native execution-time guard.
///
/// The guard can be used as a decorator or context manager. After the timeout,
/// it interrupts the guarded thread. Calls to :meth:`TimeGuard.progress` reset
/// the timeout counter. ``timeout_ms`` and ``grace_period_ms`` override their
/// seconds-based counterparts.
///
/// Interrupts can raise ``ExecutionTimeout``, send ``SIGINT`` followed by
/// ``SIGTERM``, or first raise an exception and then escalate to signals.
/// ``send_kill=True`` uses ``SIGKILL`` for the final signal escalation.
///
/// :param timeout: Timeout in seconds, default 60; use ``timeout_ms`` for finer resolution
/// :param timeout_ms: Timeout in milliseconds, overriding ``timeout``
/// :param grace_period: Delay in seconds between escalation attempts, default 15
/// :param grace_period_ms: Grace period in milliseconds, overriding ``grace_period``
/// :param interrupt_type: Exception, signal, or exception-then-signal behavior
/// :param send_kill: Use ``SIGKILL`` for the final signal escalation
/// :param check_interval: Watchdog polling interval in milliseconds, default 500
/// :return: A time guard that can decorate a function or guard a context
/// :raise ValueError: If both timeout and timeout_ms are None
/// :raise OverflowError: If a duration is negative or out of range
/// :raise RuntimeError: When entering a nested time guard
/// :raise ExecutionTimeout: In the guarded task if the time limit is exceeded and exceptions are enabled
#[pyfunction]
#[pyo3(signature = (timeout=60, timeout_ms=None, grace_period=15, grace_period_ms=None, interrupt_type=None, send_kill=false, check_interval=500))]
fn time_guard(
    timeout: Option<u64>,
    timeout_ms: Option<u64>,
    grace_period: Option<u64>,
    grace_period_ms: Option<u64>,
    interrupt_type: Option<InterruptType>,
    send_kill: bool,
    check_interval: u64,
    py: Python<'_>,
) -> PyResult<Py<TimeGuard>> {
    let timeout = timeout_ms
        .or_else(|| timeout.map(|seconds| seconds.saturating_mul(1000)))
        .ok_or_else(|| PyValueError::new_err("No timeout set"))?;
    let grace_period = grace_period_ms
        .or_else(|| grace_period.map(|seconds| seconds.saturating_mul(1000)))
        .unwrap_or(15_000)
        .max(10);
    Ok(Py::new(
        py,
        TimeGuard {
            lifecycle: GuardLifecycle::new(),
            target_thread: None,
            timeout,
            grace_period,
            interrupt_type: interrupt_type.unwrap_or(InterruptType::ExceptionThenSignal),
            send_kill,
            check_interval,
        },
    )?)
}

#[cfg(target_os = "linux")]
fn current_rss_kib() -> u64 {
    let Ok(statm) = std::fs::read_to_string(format!("/proc/{}/statm", std::process::id())) else {
        return 0;
    };
    let Some(pages) = statm
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u64>().ok())
    else {
        return 0;
    };
    // SAFETY: sysconf has no pointer arguments and _SC_PAGESIZE is supported on Linux.
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if page_size <= 0 {
        return 0;
    }
    pages.saturating_mul(page_size as u64) / 1024
}

#[cfg(not(target_os = "linux"))]
fn current_rss_kib() -> u64 {
    0
}

fn memory_watchdog(
    context: Arc<GuardContext>,
    target: TargetThread,
    max_memory: u64,
    grace_period: u64,
    secondary_grace_period: u64,
    interrupt_type: InterruptType,
    send_kill: bool,
    check_interval: u64,
) {
    let mut grace_start = 0;
    let mut signals_sent = 0u8;
    loop {
        let rss = current_rss_kib();
        if rss > max_memory {
            let now = time_millis();
            if grace_start == 0 {
                grace_start = now;
                signals_sent = 0;
            }
            let elapsed = now.saturating_sub(grace_start);
            if (grace_period == 0 || elapsed >= grace_period) && signals_sent == 0 {
                signals_sent = 1;
                interrupt(&context, target, interrupt_type, send_kill, 0, "MemoryLimitExceeded");
            } else if elapsed >= grace_period.saturating_add(secondary_grace_period) && signals_sent == 1 {
                signals_sent = 2;
                interrupt(&context, target, interrupt_type, send_kill, 1, "MemoryLimitExceeded");
            } else if elapsed >= grace_period.saturating_add(secondary_grace_period.saturating_mul(2))
                && signals_sent == 2
            {
                interrupt(&context, target, interrupt_type, send_kill, 2, "MemoryLimitExceeded");
                break;
            }
        } else if rss < max_memory && grace_start != 0 {
            grace_start = 0;
            signals_sent = 0;
        }

        thread::sleep(Duration::from_millis(check_interval));
        if context.ended.load(Ordering::Acquire) {
            break;
        }
    }
}

/// Guard process memory usage on Linux.
#[pyclass(module = "resiliparse.process_guard", name = "_MemGuard")]
struct MemGuard {
    lifecycle: GuardLifecycle,
    target_thread: Option<TargetThread>,
    max_memory: u64,
    active_max_memory: u64,
    absolute: bool,
    grace_period: u64,
    secondary_grace_period: u64,
    interrupt_type: InterruptType,
    send_kill: bool,
    check_interval: u64,
}

#[pymethods]
impl MemGuard {
    fn _setup(&mut self) -> PyResult<()> {
        self.lifecycle.setup("MemGuard")
    }

    fn _exec_before(&mut self) -> PyResult<()> {
        if !cfg!(target_os = "linux") {
            return Err(PyRuntimeError::new_err(format!("Unsupported platform: {}", std::env::consts::OS)));
        }
        self.target_thread = Some(TargetThread::current());
        self.active_max_memory = if self.absolute {
            self.max_memory
        } else {
            self.max_memory.saturating_add(current_rss_kib())
        };
        Ok(())
    }

    fn _make_watchdog(&self, py: Python<'_>) -> PyResult<Py<GuardWatchdog>> {
        let target = self
            .target_thread
            .ok_or_else(|| PyRuntimeError::new_err("Guard thread was not initialized."))?;
        Py::new(
            py,
            GuardWatchdog {
                context: Arc::clone(&self.lifecycle.context),
                target,
                task: WatchdogTask::Memory {
                    max_memory: self.active_max_memory,
                    grace_period: self.grace_period,
                    secondary_grace_period: self.secondary_grace_period,
                    interrupt_type: self.interrupt_type,
                    send_kill: self.send_kill,
                    check_interval: self.check_interval,
                },
            },
        )
    }

    fn _exec_after(&self) {}

    fn _stop(&self) {
        self.lifecycle.context.ended.store(true, Ordering::Release);
    }

    fn _finish(&mut self) {
        self.lifecycle.finish();
    }

    fn __del__(&mut self) {
        self.lifecycle.finish();
    }
}

/// Create a native Linux process-memory guard.
///
/// The guard can be used as a decorator or context manager. If process memory
/// exceeds ``max_memory``, a grace timer starts. Returning below the limit
/// during this interval resets the timer. If memory remains above the limit,
/// the guard interrupts the guarded thread and follows the selected escalation
/// procedure.
///
/// ``max_memory`` is measured in KiB. With ``absolute=True``, it is the total
/// process RSS limit. Otherwise it limits RSS growth above the value measured
/// when the guard starts. ``grace_period_ms`` and
/// ``secondary_grace_period_ms`` override their seconds-based values.
///
/// :param max_memory: Maximum process RSS or growth in KiB
/// :param absolute: Treat ``max_memory`` as an absolute limit, default true
/// :param grace_period: Delay in seconds before the first interrupt, default 0
/// :param grace_period_ms: Grace period in milliseconds, overriding ``grace_period``
/// :param secondary_grace_period: Delay in seconds between later escalation attempts, default 5
/// :param secondary_grace_period_ms: Secondary grace period in milliseconds
/// :param interrupt_type: Exception, signal, or exception-then-signal behavior
/// :param send_kill: Use ``SIGKILL`` for the final signal escalation
/// :param check_interval: Watchdog polling interval in milliseconds, default 500
/// :return: A memory guard that can decorate a function or guard a context
/// :raise OverflowError: If a memory limit or duration is negative or out of range
/// :raise RuntimeError: When entering the guard outside Linux or inside another memory guard
/// :raise MemoryLimitExceeded: In the guarded task if the memory limit is exceeded and exceptions are enabled
#[pyfunction]
#[pyo3(signature = (max_memory, absolute=true, grace_period=0, grace_period_ms=None, secondary_grace_period=5, secondary_grace_period_ms=None, interrupt_type=None, send_kill=false, check_interval=500))]
fn mem_guard(
    max_memory: u64,
    absolute: bool,
    grace_period: u64,
    grace_period_ms: Option<u64>,
    secondary_grace_period: u64,
    secondary_grace_period_ms: Option<u64>,
    interrupt_type: Option<InterruptType>,
    send_kill: bool,
    check_interval: u64,
    py: Python<'_>,
) -> PyResult<Py<MemGuard>> {
    let grace_period = grace_period_ms.unwrap_or_else(|| grace_period.saturating_mul(1000));
    let secondary_grace_period = secondary_grace_period_ms
        .unwrap_or_else(|| secondary_grace_period.saturating_mul(1000))
        .max(10);
    Ok(Py::new(
        py,
        MemGuard {
            lifecycle: GuardLifecycle::new(),
            target_thread: None,
            max_memory,
            active_max_memory: max_memory,
            absolute,
            grace_period,
            secondary_grace_period,
            interrupt_type: interrupt_type.unwrap_or(InterruptType::ExceptionThenSignal),
            send_kill,
            check_interval,
        },
    )?)
}

/// Native watchdogs used by :mod:`resiliparse.process_guard`.
#[pymodule(name = "process_guard", module = "resiliparse")]
pub mod _process_guard {
    use super::*;

    #[pymodule_init]
    pub fn __init__(m: &Bound<'_, PyModule>) -> PyResult<()> {
        let py = m.py();
        m.add_class::<InterruptType>()?;
        m.add_class::<GuardWatchdog>()?;
        m.add_class::<TimeGuard>()?;
        m.add_class::<MemGuard>()?;
        m.add_function(wrap_pyfunction!(time_guard, m)?)?;
        m.add_function(wrap_pyfunction!(mem_guard, m)?)?;
        m.add("ResiliparseGuardException", py.get_type::<ResiliparseGuardException>())?;
        m.add("ExecutionTimeout", py.get_type::<ExecutionTimeout>())?;
        m.add("MemoryLimitExceeded", py.get_type::<MemoryLimitExceeded>())?;
        m.setattr("__all__", Vec::<String>::new())?;
        Ok(())
    }
}
