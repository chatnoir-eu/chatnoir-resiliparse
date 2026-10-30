.. _fastwarc-warc-api:

FastWARC
========

Resiliparse FastWARC API documentation (Python bindings).

WARC
----

.. automodule:: fastwarc.warc
   :members:
   :exclude-members: WarcRecordType, WarcHeader
   :special-members: __iter__, __next__
   :show-inheritance:

   .. autoclass:: WarcRecordType
      :members:
      :undoc-members:
      :exclude-members: unknown, any_type, no_type
      :member-order: bysource

      .. autoattribute:: unknown

         Special type: unknown record type (filter only)

      .. autoattribute:: any_type

         Special type: any record type (filter only)

      .. autoattribute:: no_type

         Special type: no record type (filter only)

   .. autoclass:: WarcHeader
      :members:
      :undoc-members:
      :member-order: bysource


.. _fastwarc-stream-io-api:

StreamIO
--------

.. automodule:: fastwarc.stream_io
   :members:
   :show-inheritance:
