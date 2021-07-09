.. _process-guard-manual:


Resiliparse Process Guards
==========================

The Resiliparse Process Guard module is a set of decorators and context managers for guarding running tasks to stay within pre-defined limits on execution time and memory usage. Process guards help to ensure the (partially) successful completion of batch processing jobs in which individual tasks may time out or use abnormal amounts of memory, but in which the success of the whole job is not threatened by (a few) individual failures. A guarded processing context will be interrupted upon exceeding its resource limits so that the task can be skipped or rescheduled.

TimeGuard
---------

:class:`.TimeGuard` guards the execution time of a function or other program context to not exceed a certain time limit. Upon reaching this limit, the execution is interrupted by sending an exception or signal to the executing thread. The guard timeout can be reset at any time by proactively reporting progress to the guard instance (see: :ref:`timeguard-report-progress`).

For guarding a function, the decorator interface can be used:

.. code-block:: python

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

.. note::

  Since decorated functions are not picklable, you cannot use the decorator interface in applications such as PySpark. In that case, use the :ref:`context manager interface <timeguard-as-context-manager>` instead. Everything else in this section still applies.

This will send an asynchronous :exc:`.ExecutionTimeout` exception to the running thread after 10 seconds to end the loop. If the running thread does not react to this interrupt, a follow-up ``SIGINT`` signal will be sent after a certain grace period (default: 15 seconds). This signal can be caught either as a :exc:`KeyboardInterrupt` exception or via a custom ``signal`` handler. If the grace period times out again, a ``SIGTERM`` will be sent as a final attempt, after which the guard context will exit.

.. note::

  If you want to be on the safe side, you should place the ``try/except`` block around the loop, since there is a (very) small chance the exception will fire while the loop condition is being evaluated. In a practical scenario, however, you will more often than not want to simply skip to the next iteration, in which case it is probably more convenient to catch the exception inside the loop and have some sort of external contingency mechanism in place for restarting the whole batch should the exception not be caught properly. If you are working with "heavy-weight" iterators that take significant amounts of processing time or may raise exceptions on their own, you may want to have a look at :ref:`itertools-exception-loops`.

.. _timeguard-interrupt-escalation-behaviour:

Interrupt Escalation Behaviour
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
The above-described interrupt escalation behaviour is configurable. There are two basic interrupt mechanisms: throwing an asynchronous exception or sending a UNIX signal. The exception mechanism is the most gentle method of the two, but it may be unreliable if execution is blocking outside the Python program flow (e.g., in a native C extension or in a syscall). The signal method is a bit more reliable in this regard, but it does not work if the guarded thread is not the interpreter main thread, since in Python, only the main thread can receive and handle signals. Thus, if you are guarding a dedicated worker thread, you have to use exceptions.

The three supported escalation strategies are :attr:`~.InterruptType.exception`, :attr:`~.InterruptType.signal`, and :attr:`~.InterruptType.exception_then_signal` (which is the default):

.. code-block:: python

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

The grace period is configurable with the ``grace_period=<SECONDS>`` parameter. The minimum interval between escalation levels is one second (i.e., the next signal/exception will wait at least another second, even if ``grace_period`` is zero) If UNIX signals are being sent, you can also set ``send_kill=True`` to send a ``SIGKILL`` instead of a ``SIGTERM`` as the last ditch attempt. This signal cannot be caught and will immediately end the Python interpreter (thus you will need an external facility to restart it).

.. _timeguard-report-progress:

Reporting Progress
^^^^^^^^^^^^^^^^^^
The timeout can be reset at any time by calling the context guard's :meth:`~.TimeGuard.progress()` function. This is important in a loop whose total execution time is unknown, but in which each individual iteration should not exceed a certain duration:

.. code-block:: python

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

The :meth:`~.TimeGuard.progress()` function will automatically select the last active guard context from the *global* scope on the stack. In some cases, this does not work, so that you will have to call the function explicitly on the context instance itself:

.. code-block:: python

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


.. _timeguard-as-context-manager:

Using TimeGuard as a Context Manager
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
Instead of the decorator interface, :class:`.TimeGuard` also provides a context manager interface that can be used with Python's ``with`` statement for guarding arbitrary program contexts:

.. code-block:: python

  with time_guard(timeout=10):
      for _ in range(1000):
          try:
              sleep(0.1)
          except ExecutionTimeout:
              break

To report progress and reset the timeout, call the :meth:`~.TimeGuard.progress()` method on the guard instance as you would with decorator API:

.. code-block:: python

  with time_guard(timeout=10) as guard:
      for _ in range(1000):
          try:
              sleep(0.1)
              guard.progress()
          except ExecutionTimeout:
              break


TimeGuard Check Interval
^^^^^^^^^^^^^^^^^^^^^^^^
By default, :class:`.TimeGuard` monitors the execution time in steps of 500 ms. If you need a higher resolution, you can configure a lower check interval with ``check_interval=<MILLISECONDS>``.


MemGuard
--------

:class:`.MemGuard` guards a function or program context to stay within pre-defined memory bounds. If the running Python process ever exceeds these bounds while the guard context is active, an exception or signal will be sent to the executing thread.

.. code-block:: python

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

This will raise an exception immediately upon exceeding the pre-defined process memory limit of 50 MiB. If the thread does not react to this exception, the same escalation procedure will kick in as known from :class:`.TimeGuard`. In order for :class:`.MemGuard` to tolerate short spikes above the memory limit, set ``grace_period`` to a positive non-zero value. If memory usage exceeds the limit, a timer will start that expires after ``grace_period`` seconds and triggers the interrupt procedure. If memory usage falls below the threshold during the grace period, the timer is reset.

:class:`.MemGuard` provides the same parameters as :class:`.TimeGuard` for controlling the interrupt escalation behaviour (see: :ref:`timeguard-interrupt-escalation-behaviour`), but the time interval before triggering the next escalation level is independent of the grace period and defaults to five seconds to give the application sufficient time to react and deallocate excess memory. This secondary grace period can be configured with the ``secondary_grace_period`` parameter and must be at least one second.

Using MemGuard as a Context Manager
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
Similar to :class:`.TimeGuard`, :class:`.MemGuard` can also be used as a context manager:

.. code-block:: python

  with mem_guard(max_memory=1024 * 50, grace_period=2):
      x = []
      try:
          while True:
              x.extend([1] * 1000)
      except MemoryLimitExceeded:
          print('Memory limit exceeded')
          x.clear()

.. Attention::

  Particularly with this notation, remember to actually deallocate your buffers, since they will not automatically go out of scope as they would when returning from a function call!

MemGuard Check Interval
^^^^^^^^^^^^^^^^^^^^^^^
By default, :class:`.MemGuard` checks the current memory usage every 500 ms. If you need a higher resolution, you can configure a lower check interval with ``check_interval=<MILLISECONDS>``. For performance reasons, however, this interval should be chosen as large as possible, since the check involves reading from the ``/proc`` filesystem on Linux or invoking the ``ps`` command on other POSIX platforms, which is a relatively expensive operation.
