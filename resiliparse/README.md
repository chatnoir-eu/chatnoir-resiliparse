# ChatNoir Resiliparse

A collection of robust and fast processing tools for parsing and analyzing (not only) web archive data.

Resiliparse is a part of the [ChatNoir](https://github.com/chatnoir-eu/) web data processing pipeline.


## ProcessGuard
The Resiliparse ProcessGuard module is a set of decorators and context managers for guarding a processing context to stay within pre-defined limits for execution time and memory usage. ProcessGuard helps to ensure the (partially) successful completion of batch processing jobs in which individual tasks may time out or use abnormal amounts of memory, but in which the success of the whole job is not threatened by (a few) individual failures. A guarded processing context will be interrupted upon exceeding its resource limits so that the task can be skipped or rescheduled.

### TimeGuard

Guard a function or a specific execution context to not exceed a set execution time limit. Upon reaching this limit, an exception or a signal will be sent to interrupt execution. The guard timeout can be reset at any time by proactively reporting progress to the guard instance.

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
This will send an asynchronous `ExecutionTimeout` exception to the running thread after 10 seconds to end the loop. If the running thread does not react, a `SIGINT` UNIX signal will be sent after a certain grace period (default: 15 seconds). This signal can be caught either as a `KeyboardInterrupt` exception or via a custom `signal` handler. If the grace period times out again, a `SIGTERM` will be sent as a final attempt, and the guard context will exit.

#### Signalling behaviour
The signalling behaviour can be configured as either `exception`, `signal`, or `exception_then_signal` (the default):
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
The grace period is configurable with the `grace_period=<SECONDS>` parameter. If UNIX signals are being sent, you can also set `send_kill=True` to send a `SIGKILL` instead of a `SIGTERM` as the last ditch attempt. This signal cannot be caught and will immediately end the Python interpreter.

#### Reporting progress
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

#### Using TimeGuard as context manager
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
