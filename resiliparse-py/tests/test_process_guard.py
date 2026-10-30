import gc

import os
import platform
import signal
import subprocess
import sys
import textwrap
import time
from time import sleep, monotonic
import warnings

import pytest

if os.name != 'posix':
    pytest.skip('Process Guards are only available on Unix.', allow_module_level=True)

# GitHub's macOS VM is too slow, so we often get race conditions with short timeouts.
if platform.system() == 'Darwin' and 'CIBUILDWHEEL' in os.environ:
    pytest.skip('macOS CI detected: Skipping unreliable process guard tests due to CI slowness.',
                allow_module_level=True)

from resiliparse.process_guard import InterruptType, ExecutionTimeout, MemoryLimitExceeded, \
    mem_guard, time_guard, progress
from resiliparse.process_guard import progress_loop


class SigIntSent(Exception):
    pass


class SigTermSent(Exception):
    pass


def sigint_handler(_, __):
    raise SigIntSent


def sigterm_handler(_, __):
    raise SigTermSent


@pytest.fixture(autouse=True)
def guard_signal_handlers():
    # Do not replace pytest's handlers during test collection or leave ours
    # installed for unrelated tests.
    previous_int = signal.signal(signal.SIGINT, sigint_handler)
    previous_term = signal.signal(signal.SIGTERM, sigterm_handler)
    try:
        yield
    finally:
        signal.signal(signal.SIGINT, previous_int)
        signal.signal(signal.SIGTERM, previous_term)


@time_guard(timeout_ms=20, grace_period_ms=0, check_interval=5, interrupt_type=InterruptType.exception)
def wait_func_exc():
    while True:
        sleep(0.001)


@time_guard(timeout_ms=40, grace_period_ms=20, check_interval=5, interrupt_type=InterruptType.exception)
def wait_func_exc_escalate():
    ignored = 0
    with warnings.catch_warnings():
        warnings.simplefilter('ignore', RuntimeWarning)
        while True:
            try:
                while True:
                    sleep(0.001)
            except ExecutionTimeout as e:
                if ignored >= 2:
                    raise e
                ignored += 1


@time_guard(timeout_ms=40, grace_period_ms=50, check_interval=5, interrupt_type=InterruptType.exception_then_signal)
def wait_func_exc_signal():
    while True:
        try:
            while True:
                sleep(0.001)
        except ExecutionTimeout:
            pass


@time_guard(timeout_ms=20, grace_period=0, check_interval=5, interrupt_type=InterruptType.signal)
def wait_func_signal():
    while True:
        sleep(0.001)


@time_guard(timeout_ms=40, grace_period=0, check_interval=5, interrupt_type=InterruptType.signal)
def wait_func_signal_term():
    while True:
        try:
            while True:
                sleep(0.001)
        except SigIntSent:
            pass


@time_guard(timeout_ms=40, grace_period_ms=50, check_interval=5, interrupt_type=InterruptType.exception_then_signal)
def wait_func_signal_term_escalate():
    with warnings.catch_warnings():
        warnings.simplefilter('ignore', RuntimeWarning)
        while True:
            try:
                while True:
                    sleep(0.001)
            except (ExecutionTimeout, SigIntSent):
                pass


# Run for longer than the timeout while leaving headroom for stack inspection
# and scheduling delays when the entire suite runs on a busy CI worker.
@time_guard(timeout_ms=500, grace_period_ms=100, check_interval=50, interrupt_type=InterruptType.exception)
def wait_func_exc_progress():
    start = monotonic()
    while monotonic() - start < 1.1:
        time.sleep(0.001)
        progress()


# noinspection PyUnreachableCode
@pytest.mark.slow
def test_time_guard():
    with pytest.raises(ExecutionTimeout):
        wait_func_exc()

    with pytest.raises(ExecutionTimeout):
        wait_func_exc_escalate()

    with pytest.raises(SigIntSent):
        wait_func_signal()

    with pytest.raises(SigIntSent):
        wait_func_exc_signal()

    with pytest.raises(SigTermSent):
        wait_func_signal_term()

    with pytest.raises(SigTermSent):
        wait_func_signal_term_escalate()

    # Test if same guard can be used twice
    with pytest.raises(ExecutionTimeout):
        wait_func_exc()

    # Test context manager interface
    with pytest.raises(ExecutionTimeout):
        with time_guard(timeout_ms=20, grace_period_ms=10, check_interval=10, interrupt_type=InterruptType.exception):
            while True:
                sleep(0.001)

    # Test progress()
    wait_func_exc_progress()

    def infinite_gen():
        while True:
            yield 1

    # Progress loop
    start = monotonic()
    with time_guard(timeout_ms=250, grace_period_ms=100, check_interval=50,
                    interrupt_type=InterruptType.exception) as guard:
        for _ in progress_loop(infinite_gen(), ctx=guard):
            sleep(0.001)
            if monotonic() - start > .6:
                break


# Leave room for watchdog/thread startup; only the guarded allocation loop
# should exceed the RSS limit. A 1 KiB limit can fire before its handlers exist.
MEMORY_GROWTH_KIB = 16 * 1024
ALLOCATION_SIZE = 1024 * 1024


@mem_guard(max_memory=MEMORY_GROWTH_KIB, absolute=False, check_interval=5, interrupt_type=InterruptType.exception)
def fill_mem():
    l = bytearray()
    while True:
        l.extend(b'\x01' * ALLOCATION_SIZE)
        sleep(0.005)


@mem_guard(max_memory=MEMORY_GROWTH_KIB, absolute=False, check_interval=5, grace_period_ms=0, secondary_grace_period_ms=250,
           interrupt_type=InterruptType.exception_then_signal)
def fill_mem_exc_signal():
    l = bytearray()
    while True:
        try:
            while True:
                l.extend(b'\x01' * ALLOCATION_SIZE)
                sleep(0.005)
        except MemoryLimitExceeded:
            pass


@mem_guard(max_memory=MEMORY_GROWTH_KIB, absolute=False, check_interval=5, interrupt_type=InterruptType.signal)
def fill_mem_signal():
    l = bytearray()
    while True:
        l.extend(b'\x01' * ALLOCATION_SIZE)
        sleep(0.005)


@mem_guard(max_memory=MEMORY_GROWTH_KIB, absolute=False, grace_period_ms=0, secondary_grace_period_ms=250, check_interval=5,
           interrupt_type=InterruptType.signal)
def fill_mem_signal_term():
    l = bytearray()
    while True:
        try:
            while True:
                l.extend(b'\x01' * ALLOCATION_SIZE)
                sleep(0.005)
        except SigIntSent:
            pass


@pytest.mark.slow
def test_mem_guard():
    if platform.system() != 'Linux':
        pytest.skip("Skipping mem_guard test due to unsupported platform")

    with pytest.raises(MemoryLimitExceeded):
        fill_mem()

    time.sleep(.5)
    gc.collect()
    time.sleep(.5)

    with pytest.raises(SigIntSent):
        fill_mem_exc_signal()

    time.sleep(.5)
    gc.collect()
    time.sleep(.5)

    with pytest.raises(SigIntSent):
        fill_mem_signal()

    time.sleep(.5)
    gc.collect()
    time.sleep(.5)

    with pytest.raises(SigTermSent):
        fill_mem_signal_term()

    time.sleep(.5)
    gc.collect()
    time.sleep(.5)

    # Test if same guard can be used twice
    with pytest.raises(MemoryLimitExceeded):
        fill_mem()

    time.sleep(.5)
    gc.collect()
    time.sleep(.5)

    # Test context manager interface
    with pytest.raises(MemoryLimitExceeded):
        with mem_guard(max_memory=MEMORY_GROWTH_KIB, absolute=False, check_interval=5, interrupt_type=InterruptType.exception):
            l = bytearray()
            while True:
                l.extend(b'\x01' * ALLOCATION_SIZE)
                sleep(0.005)


@pytest.mark.parametrize('guard_type', ['time', 'memory'])
@pytest.mark.parametrize('interface', ['decorator', 'context'])
@pytest.mark.parametrize('start_thread', [False, True])
def test_guard_startup_failure_cleanup(guard_type, interface, start_thread):
    if guard_type == 'memory' and platform.system() != 'Linux':
        pytest.skip('Memory guards require Linux')

    # Isolate watchdog regressions: a broken guard must not inject exceptions
    # into pytest itself or poison the registry used by subsequent tests.
    result = subprocess.run([sys.executable, '-c', textwrap.dedent('''
        import sys
        import threading
        from resiliparse.process_guard import time_guard, mem_guard, InterruptType

        kind, interface, start_thread = sys.argv[1:]
        class StartupFailure(BaseException):
            pass

        def new_guard():
            options = dict(check_interval=5, interrupt_type=InterruptType.exception)
            if kind == 'time':
                return time_guard(timeout=60, **options)
            return mem_guard(max_memory=2**40, **options)

        threads = []
        original_start = threading.Thread.start
        def failing_start(thread):
            threads.append(thread)
            if start_thread == 'True':
                original_start(thread)
            raise StartupFailure('startup interrupted')

        guard = new_guard()
        threading.Thread.start = failing_start
        try:
            try:
                if interface == 'decorator':
                    guard(lambda: None)()
                else:
                    with guard:
                        raise AssertionError('body must not run')
            except StartupFailure:
                pass
            else:
                raise AssertionError('startup failure was swallowed')
        finally:
            threading.Thread.start = original_start

        assert all(not thread.is_alive() for thread in threads), 'watchdog leaked'
        # Both the same instance and another instance must be usable afterward.
        with guard:
            pass
        with new_guard():
            pass
    '''), guard_type, interface, str(start_thread)], capture_output=True, text=True, timeout=15)
    assert result.returncode == 0, result.stdout + result.stderr


@pytest.mark.parametrize('guard_type', ['time', 'memory'])
@pytest.mark.parametrize('interface', ['decorator', 'context'])
def test_guard_interrupted_startup_stops_escalation(guard_type, interface):
    if guard_type == 'memory' and platform.system() != 'Linux':
        pytest.skip('Memory guards require Linux')

    result = subprocess.run([sys.executable, '-c', textwrap.dedent('''
        import signal
        import sys
        import threading
        import time
        from resiliparse.process_guard import time_guard, mem_guard, InterruptType

        kind, interface = sys.argv[1:]
        class Interrupted(BaseException):
            pass

        signals = []
        def interrupt(sig, frame):
            signals.append(sig)
            if sig == signal.SIGINT:
                raise Interrupted()
        signal.signal(signal.SIGINT, interrupt)
        signal.signal(signal.SIGTERM, interrupt)

        options = dict(check_interval=5, interrupt_type=InterruptType.signal)
        if kind == 'time':
            guard = time_guard(timeout_ms=0, grace_period_ms=200, **options)
        else:
            guard = mem_guard(max_memory=1, secondary_grace_period_ms=200, **options)

        original_start = threading.Thread.start
        def slow_start(thread):
            original_start(thread)
            # Ensure the watchdog interrupts startup, not the guarded body.
            time.sleep(10)
        threading.Thread.start = slow_start
        try:
            try:
                if interface == 'decorator':
                    guard(lambda: None)()
                else:
                    with guard:
                        raise AssertionError('body must not run')
            except Interrupted:
                pass
            else:
                raise AssertionError('watchdog did not interrupt startup')
        finally:
            threading.Thread.start = original_start

        time.sleep(.6)  # Past both escalation deadlines.
        assert signals == [signal.SIGINT], signals
        assert len(threading.enumerate()) == 1, 'watchdog leaked'
    '''), guard_type, interface], capture_output=True, text=True, timeout=15)
    assert result.returncode == 0, result.stdout + result.stderr
