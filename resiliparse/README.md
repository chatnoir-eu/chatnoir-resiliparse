# ChatNoir Resiliparse

A collection of robust and fast processing tools for parsing and analyzing (not only) web archive data.

Resiliparse is a part of the [ChatNoir](https://github.com/chatnoir-eu/) web data processing pipeline.

## Building Resiliparse

You can compile Resiliparse either from the PyPi source package or directly from this repository. To build FastWARC from PyPi, run
```bash
pip install --no-binary resiliparse resiliparse
```
If you prefer to build directly from this repository instead, run:
```bash
# Create venv (recommended, but not required):
python3 -m venv venv && source venv/bin/activate

# Build and install:
pip install cython setuptools
BUILD_PACKAGES=resiliparse python setup.py install
```

## Process Guards
The Resiliparse Process Guard module is a set of decorators and context managers for guarding a processing context to stay within pre-defined limits on execution time and memory usage. ProcessGuard helps to ensure the (partially) successful completion of batch processing jobs in which individual tasks may time out or use abnormal amounts of memory, but in which the success of the whole job is not threatened by (a few) individual failures. A guarded processing context will be interrupted upon exceeding its resource limits so that the task can be skipped or rescheduled.

### TimeGuard

TimeGuard guards a function or a specific execution context to not exceed a set execution time limit. Upon reaching this limit, an exception or a signal will be sent to interrupt execution. The guard timeout can be reset at any time by proactively reporting progress to the guard instance.

For guarding a function, the decorator interface can be used:
```python
from time import sleep
from resiliparse.process_guard import time_guard, ExecutionTimeout

@time_guard(timeout=10)
def foo():
    try:
        while True:
            sleep(0.1)
            
    except ExecutionTimeout:
        print('Time out!')

foo()
```
This will send an asynchronous `ExecutionTimeout` exception to the running thread after 10 seconds to end the loop. If the running thread does not react, a `SIGINT` UNIX signal will be sent after a certain grace period (default: 15 seconds). This signal can be caught either as a `KeyboardInterrupt` exception or via a custom `signal` handler. If the grace period times out again, a `SIGTERM` will be sent as a final attempt, after which the guard context will exit.

#### Interrupt Escalation Behaviour
The above-described interrupt escalation behaviour is configurable. There are two basic interrupt mechanisms: throwing an asynchronous exception or sending a UNIX signal. The exception mechanism is the most gentle method of the two, but it may be unreliable if execution is blocking outside the Python program flow (e.g., in a native C extension or in a syscall). The signal method is a bit more reliable in this regard, but it does not work if the guarded thread is not the interpreter main thread, since in Python, only the main thread can receive and handle signals. Thus, if you are guarding a dedicated worker thread, you have to use exceptions.

The three supported escalation strategies are `exception`, `signal`, or `exception_then_signal` (the default):
```python
from resiliparse.process_guard import time_guard, InterruptType

# Send an `ExecutionTimeout` exception and repeat twice after the grace period.
@time_guard(timeout=10, interrupt_type=InterruptType.exception)
def foo():
    pass

# Send a `SIGINT` and follow up with up to two `SIGTERM`s after the grace period.
@time_guard(timeout=10, interrupt_type=InterruptType.signal)
def foo():
    pass

# Send an `ExecutionTimeout` exception and follow up with a `SIGINT` and a
# `SIGTERM` after the grace period. This is the default behaviour.
@time_guard(timeout=10, interrupt_type=InterruptType.exception_then_signal)
def foo():
    pass
```
The grace period is configurable with the `grace_period=<SECONDS>` parameter. The minimum interval between escalation levels is one second (i.e., the next signal/exception will wait at least another second, even if `grace_period` is zero) If UNIX signals are being sent, you can also set `send_kill=True` to send a `SIGKILL` instead of a `SIGTERM` as the last ditch attempt. This signal cannot be caught and will immediately end the Python interpreter.

#### Reporting Progress
The timeout can be reset at any time by calling the context guard's `progress()` function. This is important in a loop whose total execution time is unknown, but in which each individual iteration should not exceed a certain duration:
```python
from time import sleep
from resiliparse.process_guard import progress, time_guard, ExecutionTimeout

@time_guard(timeout=10)
def foo():
    try:
        while True:
            sleep(0.1)
            progress()
            
    except ExecutionTimeout:
        print('Time out!')  # This will never happen

foo()
```
The `progress()` function will automatically select the last active guard context from the *global* scope on the stack. In some cases, this does not work, so that you will have to call the function explicitly on the context instance itself:
```python
def foo():
    @time_guard(timeout=10)
    def bar():
        try:
            # This loop runs forever
            while True:
                sleep(0.1)
                # Function bar() is not in the global scope,
                # so we have to reference the guard context explicitly.
                bar.progress()
                
        except ExecutionTimeout:
            print('Time out!')  # This will never happen
    bar()
foo()
```

#### Using TimeGuard as a Context Manager
Instead of the decorator interface, TimeGuard also provides a context manager interface that can be used with Pythons `with` statement:
```python
with time_guard(timeout=10):
    while True:
        try:
            sleep(0.1)
        except ExecutionTimeout:
            break
```
To report progress and reset the timeout, call the `progress()` method on the guard instance as you would with decorator API:

```python
with time_guard(timeout=10) as guard:
    while True:
        try:
            sleep(0.1)
            guard.progress()
        except ExecutionTimeout:
            break
```

### MemGuard

Similar to TimeGuard, MemGuard guards a processing context to stay within pre-defined memory bounds. Upon exceeding these bounds, an exception or signal will be sent to the executing thread.
```python
from resiliparse.process_guard import mem_guard, MemoryLimitExceeded

@mem_guard(max_memory=1024 * 50)
def foo():
    x = []
    try:
        while True:
            x.extend([1] * 1000)
            
    except MemoryLimitExceeded:
        x.clear()
        print('Memory limit exceeded')

foo()
```
This will raise an exception immediately upon exceeding the pre-defined process memory limit of 50 MiB. If the thread does not react to this exception, the same escalation procedure will kick in as known from TimeGuard. In order for MemGuard to tolerate short spikes above the memory limit, set `grace_period` to a positive non-zero value. If memory usage exceeds the limit, a timer will start that expires after `grace_period` seconds and triggers the interrupt procedure. If memory usage falls below the threshold during the grace period, the timer is reset.

MemGuard provides the same parameters as TimeGuard for controlling the interrupt escalation behaviour (see: [TimeGuard interrupt escalation behaviour](#Interrupt-Escalation-Behaviour)), but the time interval before triggering the next escalation level is independent of the grace period and defaults to five seconds to give the application sufficient time to react and deallocate excess memory. This secondary grace period can be configured with the `secondary_grace_period` parameter and must be at least one second.

#### Using MemGuard as a Context Manager

Similar to TimeGuard, MemGuard can also be used as a context manager:
```python
with mem_guard(max_memory=1024 * 50, grace_period=2):
    x = []
    try:
        while True:
            x.extend([1] * 1000)
            
    except MemoryLimitExceeded:
        x.clear()
        print('Memory limit exceeded')
```
Particularly with this notation, remember to actually deallocate your buffers, since they will not automatically go out of scope as they would when returning from a function call! 

#### MemGuard Check Interval
By default, MemGuard will check the current memory usage every 500ms. If you need a higher resolution, you can configure a lower check interval with `check_interval=<MILLISECONDS>`. For performance reasons, however, this interval should be chosen as large as possible, since the check involves reading from the `/proc` filesystem on Linux or invoking the `ps` command on other POSIX platforms, which is a relatively expensive operation.
