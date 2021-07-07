.. _process-guard-api:

Resiliparse Process Guards
==========================

Resiliparse Process Guard API documentation.

.. automodule:: resiliparse.process_guard
   :members:
   :undoc-members:

   .. autoclass:: InterruptType

      Resiliparse context guard interrupt type.

      .. autoattribute:: exception

         Send only exceptions

      .. autoattribute:: signal

         Send only signals

      .. autoattribute:: exception_then_signal

         Send an exception first and follow up with signals
