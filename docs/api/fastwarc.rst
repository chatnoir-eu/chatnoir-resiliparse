.. _fastwarc-warc-api:

FastWARC
========

Resiliparse FastWARC API documentation.

WARC
----

.. automodule:: fastwarc.warc
   :members:
   :special-members: __init__
   :undoc-members:
   :exclude-members: CaseInsensitiveStr, CaseInsensitiveStrDict

   .. autoclass:: WarcRecordType
      :members:
      :undoc-members:
      :exclude-members: unknown, any_type, no_type
      :member-order: bysource

      Enum indicating a WARC record's type as given by its ``WARC-Type`` header.
      Multiple types can be combined with boolean operators for filtering records.

      .. autoattribute:: unknown

         Special type: unknown record type (filter only)

      .. autoattribute:: any_type

         Special type: any record type (filter only)

      .. autoattribute:: no_type

         Special type: no record type (filter only)


.. _fastwarc-stream-io-api:

StreamIO
--------

.. automodule:: fastwarc.stream_io
   :members:
   :undoc-members:
   :special-members: __init__
