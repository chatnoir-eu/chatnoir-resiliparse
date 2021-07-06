# ChatNoir Resiliparse

A collection of robust and fast processing tools for parsing and analyzing (not only) web archive data.

Resiliparse is a part of the [ChatNoir](https://github.com/chatnoir-eu/) web data processing pipeline.

## Installing Resiliparse
Pre-built Resiliparse binaries can be installed from PyPi:
```bash
pip install resiliparse
```

## Building Resiliparse
To build Resiliparse from sources, you can either compile it from the PyPi source package or directly from this repository. To build Resiliparse from PyPi, run:
```bash
pip install --no-binary resiliparse resiliparse
```
If you prefer to build directly from this repository instead, run:
```bash
# Create venv (recommended, but not required)
python3 -m venv venv && source venv/bin/activate

# Install build dependencies
pip install cython setuptools

# Build and install
BUILD_PACKAGES=resiliparse python setup.py install
```

## Process Guards
The Resiliparse Process Guard module is a set of decorators and context managers for guarding a processing context to stay within pre-defined limits on execution time and memory usage. Process guards help to ensure the (partially) successful completion of batch processing jobs in which individual tasks may time out or use abnormal amounts of memory, but in which the success of the whole job is not threatened by (a few) individual failures. A guarded processing context will be interrupted upon exceeding its resource limits so that the task can be skipped or rescheduled.

### TimeGuard
TimeGuard guards a function or a specific execution context to not exceed a set execution time limit. Upon reaching this limit, an exception or a signal will be sent to interrupt execution. The guard timeout can be reset at any time by proactively reporting progress to the guard instance.

For guarding a function, the decorator interface can be used:
```python
from time import sleep
from resiliparse.process_guard import time_guard, ExecutionTimeout

@time_guard(timeout=10)
def foo():
    for _ in range(1000):
        try:
            sleep(0.1)
        except ExecutionTimeout:
            print('Time out!')
            break

foo()
```
This will send an asynchronous `ExecutionTimeout` exception to the running thread after 10 seconds to end the loop. If the running thread does not react, a `SIGINT` UNIX signal will be sent after a certain grace period (default: 15 seconds). This signal can be caught either as a `KeyboardInterrupt` exception or via a custom `signal` handler. If the grace period times out again, a `SIGTERM` will be sent as a final attempt, after which the guard context will exit.

**Hint:** If you want to be on the safe side, you should place the `try/except` block around the loop, since there is a (very) small chance the exception will fire while the loop condition is being evaluated. In a practical scenario, however, you will more often than not want to simply skip to the next iteration, in which case it is probably more convenient to catch the exception inside the loop and have some sort of external contingency mechanism in place for restarting the whole batch should the exception not be caught properly. If you are working with "heavy-weight" iterators that take significant amounts of processing time or may raise exceptions on their own, you may want to have a look at [Exception Loops](#Exception-Loops).

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
    for _ in range(1000):
        try:
            sleep(0.1)
            progress()
        except ExecutionTimeout:
            print('Time out!')
            break

foo()
```
The `progress()` function will automatically select the last active guard context from the *global* scope on the stack. In some cases, this does not work, so that you will have to call the function explicitly on the context instance itself:
```python
def foo():
    @time_guard(timeout=10)
    def bar():
        for _ in range(1000):
            try:
                sleep(0.1)
                # Function bar() is not in the global scope,
                # so we have to reference the guard context explicitly.
                bar.progress()
            except ExecutionTimeout:
                print('Time out!')
                break
    bar()
foo()
```

#### Using TimeGuard as a Context Manager
Instead of the decorator interface, TimeGuard also provides a context manager interface that can be used with Python's `with` statement:
```python
with time_guard(timeout=10):
    for _ in range(1000):
        try:
            sleep(0.1)
        except ExecutionTimeout:
            break
```
To report progress and reset the timeout, call the `progress()` method on the guard instance as you would with decorator API:

```python
with time_guard(timeout=10) as guard:
    for _ in range(1000):
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
        print('Memory limit exceeded')
        x.clear()

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
        print('Memory limit exceeded')
        x.clear()
```
Particularly with this notation, remember to actually deallocate your buffers, since they will not automatically go out of scope as they would when returning from a function call! 

#### MemGuard Check Interval
By default, MemGuard will check the current memory usage every 500ms. If you need a higher resolution, you can configure a lower check interval with `check_interval=<MILLISECONDS>`. For performance reasons, however, this interval should be chosen as large as possible, since the check involves reading from the `/proc` filesystem on Linux or invoking the `ps` command on other POSIX platforms, which is a relatively expensive operation.


## Itertools
Resiliparse Itertools are a collection of convenient and robust helper functions for iterating over data from unreliable sources using other tools from the Resiliparse toolkit.

### Progress Loops
Progress loops are a convenience tool for iterating data with an active [TimeGuard](#TimeGuard) context. Since running a `for` loop in a TimeGuard with progress being reported after each iteration is a very common pattern, you can use the `progress_loop` pass-through generator as a shortcut:
```python
from time import sleep
from resiliparse.itertools import progress_loop
from resiliparse.process_guard import time_guard, ExecutionTimeout

@time_guard(timeout=10)
def foo():
    for _ in progress_loop(range(1000)):
        try:
            sleep(0.1)
        except ExecutionTimeout:
            break

foo()
```
In cases where context auto-detection doesn't work, the active guard context can be passed to the generator via the `ctx` parameter:
```python
with time_guard(timeout=10) as guard:
    for _ in progress_loop(range(1000), ctx=guard):
        try:
            sleep(0.1)
        except ExecutionTimeout:
            break
```

### Exception Loops
Exception loops wrap an iterator to catch and return any exceptions raised while evaluating the input iterator.

This is primarily useful for unreliable generators that may throw unpredictably at any time  for unknown reasons (e.g., generators reading from a network data source). If you do not want to  wrap the entire loop in a `try/except` clause, you can use an :func:`exc_loop` to catch  any such exceptions and return them. 

Remember that a generator will end after throwing an exception, so if the input iterator is  a generator, you will have to create a new instance in order to retry or continue.
```python
from resiliparse.itertools import exc_loop

def throw_gen():
    from random import random
    for i in range(100):
        if random() <= 0.1:
            raise Exception('Random exception')
        yield i

for val, exc in exc_loop(throw_gen()):
    if exc is not None:
        print('Exception raised:', exc)
        break

    print(val)
```

### WARC Retry Loops
The WARC retry loop helper wraps a [FastWARC](../fastwarc/README.md) `ArchiveIterator` instance to retry in case of read failures.

Use a WARC retry loop if the underlying stream is unreliable, such as when reading from a network data source. If an exception other than `StopIteration` is raised while consuming the iterator, the WARC reading process will be retried up to `retry_count` times. When a stream failure occurs,  the `ArchiveIterator` will be reinitialised with a new stream object by calling `stream_factory`. The new stream object returned by `stream_factory()` must be seekable.
```python
from fastwarc.warc import ArchiveIterator
from resiliparse.itertools import warc_retry

def stream_factory():
    return open('warcfile.warc.gz', 'rb')

for record in warc_retry(ArchiveIterator(stream_factory()), stream_factory, retry_count=3):
    pass
```
