import os
import signal
import sys
from time import sleep, monotonic

import pytest

from resiliparse.process_guard import InterruptType, ExecutionTimeout, MemoryLimitExceeded, \
    mem_guard, time_guard, progress
from resiliparse.itertools import progress_loop


skip_long = pytest.mark.skipif(os.environ.get('SKIP_LONG'), reason="Skipping long tests")


class SignalSent(Exception):
    def __init__(self, sig):
        self.signal = sig


def signal_handler(sig, _):
    raise SignalSent(sig)


signal.signal(signal.SIGINT, signal_handler)
signal.signal(signal.SIGTERM, signal_handler)


@time_guard(timeout=0, grace_period=1, check_interval=5, interrupt_type=InterruptType.exception)
def wait_func_exc():
    while True:
        sleep(0.001)


@time_guard(timeout=0, grace_period=1, check_interval=5, interrupt_type=InterruptType.exception_then_signal)
def wait_func_exc_signal():
    while True:
        try:
            while True:
                sleep(0.001)
        except ExecutionTimeout:
            pass


@time_guard(timeout=0, grace_period=0, check_interval=5, interrupt_type=InterruptType.signal)
def wait_func_signal():
    while True:
        sleep(0.001)


@time_guard(timeout=0, grace_period=0, check_interval=5, interrupt_type=InterruptType.signal)
def wait_func_signal_term():
    while True:
        sleep(0.001)


@time_guard(timeout=1, grace_period=0, check_interval=5, interrupt_type=InterruptType.exception)
def wait_func_exc_progress():
    start = monotonic()
    while monotonic() - start < 1.1:
        sleep(0.0001)
        progress(wait_func_exc_progress)


@skip_long
def test_time_guard():
    with pytest.raises(ExecutionTimeout):
        wait_func_exc()

    with pytest.raises(SignalSent) as s:
        wait_func_signal()
        assert s.signal == signal.SIGINT

    with pytest.raises(SignalSent) as s:
        wait_func_exc_signal()
        assert s.signal == signal.SIGINT

    with pytest.raises(SignalSent) as s:
        wait_func_signal_term()
        assert s.signal == signal.SIGTERM

    # Test if same guard can be used twice
    with pytest.raises(ExecutionTimeout):
        wait_func_exc()

    # Test context manager interface
    with pytest.raises(ExecutionTimeout):
        with time_guard(timeout=1, grace_period=0, check_interval=5, interrupt_type=InterruptType.exception):
            wait_func_exc()

    # Test progress()
    wait_func_exc_progress()

    def infinite_gen():
        while True:
            yield 1

    # Progress loop
    with pytest.raises(ExecutionTimeout):
        with time_guard(timeout=0, grace_period=0, check_interval=5, interrupt_type=InterruptType.exception) as guard:
            for _ in progress_loop(infinite_gen(), ctx=guard):
                sleep(0.001)


@mem_guard(max_memory=1, absolute=False, check_interval=5, interrupt_type=InterruptType.exception)
def fill_mem():
    l = bytearray()
    while True:
        l.extend(b'\x01' * 2048)
        sleep(0.001)


@mem_guard(max_memory=1, absolute=False, check_interval=5, grace_period=1,
           interrupt_type=InterruptType.exception_then_signal)
def fill_mem_exc_signal():
    l = bytearray()
    while True:
        try:
            while True:
                l.extend(b'\x01' * 2048)
                sleep(0.0001)
        except MemoryLimitExceeded:
            pass


@mem_guard(max_memory=1, absolute=False, check_interval=5, interrupt_type=InterruptType.signal)
def fill_mem_signal():
    l = bytearray()
    while True:
        l.extend(b'\x01' * 2048)
        sleep(0.001)


@mem_guard(max_memory=1, absolute=False, grace_period=1, check_interval=5, interrupt_type=InterruptType.signal)
def fill_mem_signal_term():
    l = bytearray()
    while True:
        l.extend(b'\x01' * 2048)
        sleep(0.001)


@skip_long
def test_mem_guard():
    if sys.platform not in ['linux', 'darwin']:
        # Memory reporting is unreliable on other platforms
        pytest.skip("Skipping mem_guard test")

    with pytest.raises(MemoryLimitExceeded):
        fill_mem()

    with pytest.raises(SignalSent) as s:
        fill_mem_exc_signal()
        assert s.signal == signal.SIGINT

    with pytest.raises(SignalSent) as s:
        fill_mem_signal()
        assert s.signal == signal.SIGINT

    with pytest.raises(SignalSent) as s:
        fill_mem_signal_term()
        assert s.signal == signal.SIGTERM

    # Test if same guard can be used twice
    with pytest.raises(MemoryLimitExceeded):
        fill_mem()

    # Test context manager interface
    with pytest.raises(MemoryLimitExceeded):
        with mem_guard(max_memory=1, absolute=False, check_interval=5, interrupt_type=InterruptType.exception):
            fill_mem()
