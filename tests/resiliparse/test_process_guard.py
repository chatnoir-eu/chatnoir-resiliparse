import signal
import sys
from time import sleep, monotonic

import pytest

from resiliparse.process_guard import InterruptType, ExecutionTimeout, MemoryLimitExceeded, \
    mem_guard, time_guard, progress
from resiliparse.itertools import progress_loop


signal_sent = None


def signal_handler(sig, _):
    global signal_sent
    signal_sent = sig


signal.signal(signal.SIGINT, signal_handler)
signal.signal(signal.SIGTERM, signal_handler)


@time_guard(timeout=1, grace_period=1, check_interval=10, interrupt_type=InterruptType.exception)
def wait_func_exc():
    while True:
        sleep(0.001)


@time_guard(timeout=1, grace_period=1, check_interval=10, interrupt_type=InterruptType.exception_then_signal)
def wait_func_exc_signal():
    while not signal_sent:
        try:
            while not signal_sent:
                sleep(0.001)
        except ExecutionTimeout:
            pass


@time_guard(timeout=1, grace_period=0, check_interval=10, interrupt_type=InterruptType.signal)
def wait_func_signal():
    while not signal_sent:
        sleep(0.001)


@time_guard(timeout=1, grace_period=0, check_interval=10, interrupt_type=InterruptType.signal)
def wait_func_signal_term():
    while signal_sent != signal.SIGTERM:
        sleep(0.001)


@time_guard(timeout=1, grace_period=0, check_interval=10, interrupt_type=InterruptType.exception)
def wait_func_exc_progress():
    start = monotonic()
    while monotonic() - start < 1.5:
        progress(wait_func_exc_progress)
        sleep(0.001)


def test_time_guard():
    with pytest.raises(ExecutionTimeout):
        wait_func_exc()

    global signal_sent
    signal_sent = None
    wait_func_signal()
    assert signal_sent == signal.SIGINT

    signal_sent = None
    wait_func_exc_signal()
    assert signal_sent == signal.SIGINT

    signal_sent = None
    wait_func_signal_term()
    assert signal_sent == signal.SIGTERM

    # Test if same guard can be used twice
    with pytest.raises(ExecutionTimeout):
        wait_func_exc()

    # Test context manager interface
    with pytest.raises(ExecutionTimeout):
        with time_guard(timeout=1, grace_period=0, check_interval=10, interrupt_type=InterruptType.exception):
            wait_func_exc()

    # progress()
    wait_func_exc_progress()

    def infinite_gen():
        while True:
            yield 1

    # Progress loop
    start = monotonic()
    with time_guard(timeout=1, grace_period=0, check_interval=10, interrupt_type=InterruptType.exception) as guard:
        for _ in progress_loop(infinite_gen(), ctx=guard):
            if monotonic() - start >= 1.5:
                break
            sleep(0.001)


mem_limit = 1024


@mem_guard(max_memory=mem_limit, absolute=False, check_interval=10, interrupt_type=InterruptType.exception)
def fill_mem():
    l = []
    while True:
        l.extend([1] * 50)


@mem_guard(max_memory=mem_limit, absolute=False, check_interval=10, grace_period=1,
           interrupt_type=InterruptType.exception)
def fill_mem_grace():
    l = []
    while True:
        l.extend([1] * 50)


@mem_guard(max_memory=mem_limit, absolute=False, check_interval=10, grace_period=1,
           interrupt_type=InterruptType.exception_then_signal)
def fill_mem_exc_signal():
    l = []
    try:
        while True:
            l.extend([1] * 50)
    except MemoryLimitExceeded:
        while not signal_sent:
            l.extend([1] * 50)


@mem_guard(max_memory=mem_limit, absolute=False, check_interval=10, interrupt_type=InterruptType.signal)
def fill_mem_signal():
    l = []
    while not signal_sent:
        l.extend([1] * 50)


@mem_guard(max_memory=mem_limit, absolute=False, grace_period=1, check_interval=10, interrupt_type=InterruptType.signal)
def fill_mem_signal_term():
    l = []
    while signal_sent != signal.SIGTERM:
        l.extend([1] * 50)


def test_mem_guard():
    if sys.platform != 'linux':
        # Memory reporting is unreliable on other platforms
        pytest.skip("Skipping mem_guard test")

    with pytest.raises(MemoryLimitExceeded):
        fill_mem()

    with pytest.raises(MemoryLimitExceeded):
        fill_mem_grace()

    global signal_sent
    signal_sent = None
    fill_mem_exc_signal()
    assert signal_sent == signal.SIGINT

    signal_sent = None
    fill_mem_signal()
    assert signal_sent == signal.SIGINT

    signal_sent = None
    fill_mem_signal_term()
    assert signal_sent == signal.SIGTERM

    # Test if same guard can be used twice
    with pytest.raises(MemoryLimitExceeded):
        fill_mem()

    # Test context manager interface
    with pytest.raises(MemoryLimitExceeded):
        with mem_guard(max_memory=mem_limit, absolute=False, interrupt_type=InterruptType.exception):
            fill_mem()
