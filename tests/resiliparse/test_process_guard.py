import signal
import sys
from time import sleep, monotonic
import warnings

import pytest

from resiliparse.process_guard import InterruptType, ExecutionTimeout, MemoryLimitExceeded, \
    mem_guard, time_guard, progress
from resiliparse.itertools import progress_loop

warnings.filterwarnings('ignore')


class SigIntSent(Exception):
    pass


class SigTermSent(Exception):
    pass


def sigint_handler(_, __):
    raise SigIntSent


def sigterm_handler(_, __):
    raise SigTermSent


signal.signal(signal.SIGINT, sigint_handler)
signal.signal(signal.SIGTERM, sigterm_handler)


@time_guard(timeout=0, grace_period_ms=10, check_interval=5, interrupt_type=InterruptType.exception)
def wait_func_exc():
    while True:
        sleep(0.001)


@time_guard(timeout_ms=10, grace_period_ms=10, check_interval=5, interrupt_type=InterruptType.exception)
def wait_func_exc_escalate():
    ignored = 0
    while True:
        try:
            while True:
                sleep(0.001)
        except ExecutionTimeout as e:
            if ignored >= 2:
                raise e
            ignored += 1


@time_guard(timeout_ms=10, grace_period_ms=10, check_interval=5, interrupt_type=InterruptType.exception_then_signal)
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


@time_guard(timeout_ms=10, grace_period=0, check_interval=5, interrupt_type=InterruptType.signal)
def wait_func_signal_term():
    while True:
        try:
            while True:
                sleep(0.001)
        except SigIntSent:
            pass


@time_guard(timeout_ms=10, grace_period_ms=10, check_interval=5, interrupt_type=InterruptType.exception_then_signal)
def wait_func_signal_term_escalate():
    while True:
        try:
            while True:
                sleep(0.001)
        except (ExecutionTimeout, SigIntSent):
            pass


@time_guard(timeout_ms=50, grace_period=0, check_interval=5, interrupt_type=InterruptType.exception)
def wait_func_exc_progress():
    start = monotonic()
    while monotonic() - start < .2:
        sleep(0.001)
        progress()


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
        with time_guard(timeout=0, grace_period_ms=10, check_interval=5, interrupt_type=InterruptType.exception):
            wait_func_exc()

    # Test progress()
    wait_func_exc_progress()

    def infinite_gen():
        while True:
            yield 1

    # Progress loop
    start = monotonic()
    with time_guard(timeout_ms=50, grace_period=10, check_interval=5, interrupt_type=InterruptType.exception) as guard:
        for _ in progress_loop(infinite_gen(), ctx=guard):
            sleep(0.001)
            if monotonic() - start > .2:
                break


@mem_guard(max_memory=1, absolute=False, check_interval=5, interrupt_type=InterruptType.exception)
def fill_mem():
    l = bytearray()
    while True:
        l.extend(b'\x01' * 2048)
        sleep(0.001)


@mem_guard(max_memory=1, absolute=False, check_interval=5, grace_period_ms=0, secondary_grace_period_ms=10,
           interrupt_type=InterruptType.exception_then_signal)
def fill_mem_exc_signal():
    l = bytearray()
    while True:
        try:
            while True:
                l.extend(b'\x01' * 2048)
                sleep(0.001)
        except MemoryLimitExceeded:
            pass


@mem_guard(max_memory=1, absolute=False, check_interval=5, interrupt_type=InterruptType.signal)
def fill_mem_signal():
    l = bytearray()
    while True:
        l.extend(b'\x01' * 2048)
        sleep(0.001)


@mem_guard(max_memory=1, absolute=False, grace_period_ms=0, secondary_grace_period_ms=10, check_interval=5,
           interrupt_type=InterruptType.signal)
def fill_mem_signal_term():
    l = bytearray()
    while True:
        try:
            while True:
                l.extend(b'\x01' * 2048)
                sleep(0.001)
        except SigIntSent:
            pass


def test_mem_guard():
    if sys.platform not in ['linux', 'darwin']:
        # Memory reporting is unreliable on other platforms
        pytest.skip("Skipping mem_guard test")

    with pytest.raises(MemoryLimitExceeded):
        fill_mem()

    with pytest.raises(SigIntSent):
        fill_mem_exc_signal()

    with pytest.raises(SigIntSent):
        fill_mem_signal()

    with pytest.raises(SigTermSent):
        fill_mem_signal_term()

    # Test if same guard can be used twice
    with pytest.raises(MemoryLimitExceeded):
        fill_mem()

    # Test context manager interface
    with pytest.raises(MemoryLimitExceeded):
        with mem_guard(max_memory=1, absolute=False, check_interval=5, interrupt_type=InterruptType.exception):
            fill_mem()
