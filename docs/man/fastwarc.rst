.. _fastwarc-manual:

FastWARC
========

FastWARC is a high-performance WARC parsing library for Python written in C++/Cython. The API is inspired in large parts by `WARCIO <https://github.com/webrecorder/warcio>`_, but does not aim at being a drop-in replacement.  FastWARC supports compressed and uncompressed WARC/1.0 and WARC/1.1 streams. Supported compression algorithms are GZip and LZ4.

FastWARC belongs to the :ref:`ChatNoir Resiliparse toolkit <resiliparse-index>` for fast and robust web data processing.

Why FastWARC and not WARCIO?
----------------------------
WARCIO is a fantastic tool for reading and writing WARCs, but it is implemented entirely in Python and thus becomes rather inefficient for large web crawls at the tera- or petabyte scale where a few seconds of additional processing time add up quickly. FastWARC solves these performance issues by being written in efficient, low-level C++. We also took the opportunity to add support for LZ4, a much, much (!) faster compression algorithm than GZip, which unfortunately is the only compression algorithm mentioned in the `WARC specification <https://iipc.github.io/warc-specifications/>`_ (and thus also the only one supported by WARCIO, although it wouldn't be a big deal to add that).

FastWARC's design goals are high speed, a low and fixed memory footprint, and simplicity. For the latter reason, we decided against adding support for the legacy ARC format. If you need that kind of backwards compatibility, use WARCIO instead.


.. _fastwarc-installation:

Installing FastWARC
-------------------
Pre-built FastWARC binaries for most Linux platforms can be installed from PyPi:

.. code-block:: bash

  pip install fastwarc

.. warning::

  These binaries are provided **purely for your convenience**. Since they are built on the very old *manylinux* base system for better compatibility, their performance isn't optimal (though still better than WARCIO).

  *For best performance, see the next section on how to build FastWARC yourself.*

Building FastWARC
-----------------
You can compile FastWARC either from the PyPi source package or directly from this repository, though in any case, you need to install all build-time dependencies first. For Debian / Ubuntu, this is done with:

.. code-block:: bash

  sudo apt install build-essential python3-dev zlib1g-dev liblz4-dev

Then to build FastWARC from PyPi, run

.. code-block:: bash

  pip install --no-binary fastwarc fastwarc

That's it. If you prefer to build directly from this repository instead, run:

.. code-block:: bash

  # Create venv (recommended, but not required)
  python3 -m venv venv && source venv/bin/activate

  # Install additional build dependencies
  pip install cython setuptools

  # Build and install:
  BUILD_PACKAGES=fastwarc python setup.py install

Iterating WARC Files
--------------------
The central class for stream-processing WARC files is :class:`fastwarc.warc.ArchiveIterator`:

.. code-block:: python

  from fastwarc.warc import ArchiveIterator

  for record in ArchiveIterator(open('warcfile.warc.gz', 'rb')):
      print(record.record_id)

This will iterate over all records in the file and print out their IDs. You can pass any file-like Python object to :class:`.ArchiveIterator`, for either an uncompressed or a GZip- or LZ4-compressed WARC. FastWARC will try to auto-detect the stream format, but if you know the compression algorithm beforehand, you can speed up the process a little by explicitly passing a :class:`.GZipStream` or :class:`.LZ4Stream` object instead:

.. code-block:: python

  from fastwarc.stream_io import *

  # GZip:
  stream = GZipStream(open('warcfile.warc.gz', 'rb'))

  # LZ4:
  stream = LZ4Stream(open('warcfile.warc.lz4', 'rb'))

As a further optimization for local files, it is recommended that you use a :class:`.FileStream` instead of a Python file object. :class:`.FileStream` is a native file reader that circumvents the entire Python I/O stack for better performance:

.. code-block:: python

  from fastwarc.stream_io import *
  stream = GZipStream(FileStream('warcfile.warc.gz', 'rb'))

Filtering Records
-----------------
FastWARC provides several ways in which you can filter and efficiently skip records you are not interested in. These filters are checked very early in the parsing process, right after the WARC header block has been read. Multiple types of filters can be combined.

Record Type Filter
^^^^^^^^^^^^^^^^^^
If you want only records of a certain type, you can skip all other records efficiently by specifying a bitmask of the desired record types:

.. code-block:: python

  from fastwarc.warc import ArchiveIterator, WarcRecordType

  for record in ArchiveIterator(stream, record_types=WarcRecordType.request | WarcRecordType.response):
      pass

This will skip all records with a ``WARC-Type`` other than ``request`` or ``response``.

Content-Length Filter
^^^^^^^^^^^^^^^^^^^^^
You can automatically skip any records whose ``Content-Length`` exceeds or is lower than a certain value:

.. code-block:: python

  from fastwarc.warc import ArchiveIterator

  # Skip all records that are larger than 500 KiB
  for record in ArchiveIterator(stream, max_content_length=512000):
      pass

  # Skip all records that are smaller than 128 bytes
  for record in ArchiveIterator(stream, min_content_length=128):
      pass


Function Filter
^^^^^^^^^^^^^^^
If the above-mentioned filter mechanisms are not sufficient, you can pass a function object that accepts as its only parameter a :class:`.WarcRecord` and returns a ``bool`` value as a filter predicate. This filter type is much slower than the previous filters, but probably still more efficient than checking the same thing later on in the loop. Be aware that since the record body hasn't been seen yet, you cannot access any information beyond what is in the record headers.

FastWARC comes with a handful of existing filters that you can use:

.. code-block:: python

  from fastwarc.warc import *

  # Skip any non-HTTP records
  for record in ArchiveIterator(stream, func_filter=is_http):
      pass

  # Skip records without a block digest
  for record in ArchiveIterator(stream, func_filter=has_block_digest):
      pass

  # Skip records that are not WARC/1.1
  for record in ArchiveIterator(stream, func_filter=is_warc_11):
      pass

The full list of pre-defined function filters is: :func:`.is_warc_10`, :func:`.is_warc_11`, :func:`.has_block_digest`, :func:`.has_payload_digest`, :func:`.is_http`, :func:`.is_concurrent`. Besides these, you can pass any Python callable that accepts a :class:`.WarcRecord` and returns a ``bool``:

.. code-block:: python

  # Skip records which haven't been identified as HTML pages
  for record in ArchiveIterator(stream, func_filter=lambda r: r.headers.get('WARC-Identified-Payload-Type') == 'text/html'):
      pass

  # Skip records without any sort of digest header
  for record in ArchiveIterator(stream, func_filter=lambda r: has_block_digest(r) and has_payload_digest(r)):
      pass

Digest Filter
^^^^^^^^^^^^^
This is the only filter that is executed after the content is available and will skip any records without or with an invalid block digest:

.. code-block:: python

  for record in ArchiveIterator(stream, verify_digests=True):
      pass

.. note::

  This is the most expensive filter of all and it will create an in-memory copy of the whole record. See :ref:`verifying-record-digests` for more information on how digest verification works.

Record Properties
-----------------
The :class:`.ArchiveIterator` returns objects of type :class:`.WarcRecord`, which have various properties:

.. code-block:: python

  for record in ArchiveIterator(stream):
      record.headers          # Dict-like object containing the WARC headers
      record.record_id        # Shorthand for record.headers['WARC-Record-ID']
      record.record_type      # Shorthand for record.headers['WARC-Type']
      record.content_length   # Effective record payload length
      record.stream_pos       # Record start offset in the (uncompressed) stream
      record.is_http          # Boolean indicating whether record is an HTTP record
      record.http_headers     # Dict-like object containing the parsed HTTP headers
      record.http_charset     # HTTP charset/encoding as reported by the server (if any)
      record.reader           # A BufferedReader for the record content

      # Read and return up to 1024 bytes from the record stream
      body = record.reader.read(1024)

      # Consume and return the remaining record bytes
      body += record.reader.read()

      # Or: Consume rest of stream without allocating a buffer for it (i.e., skip over)
      record.reader.consume()

As you can see, HTTP request and response records are parsed automatically for convenience. If not needed, you can disable this behaviour by passing ``parse_http=False`` to the :class:`.ArchiveIterator` constructor to avoid unnecessary processing. :attr:`record.reader <.WarcRecord.reader>` will then start at the beginning of the HTTP header block instead of the HTTP body. You can parse HTTP headers later on a per-record basis by calling :meth:`record.parse_http() <.WarcRecord.parse_http>` as long as the :class:`.BufferedReader` hasn't been consumed at that point.


.. _verifying-record-digests:

Verifying Record Digests
------------------------
If a record has digest headers, you can verify the consistency of the record contents and/or its HTTP payload:

.. code-block:: python

  for record in ArchiveIterator(stream, parse_http=False):
      if 'WARC-Block-Digest' in record.headers:
          print('Block digest OK:', record.verify_block_digest())

      if 'WARC-Payload-Digest' in record.headers:
          record.parse_http()    # It's safe to call this even if the record has no HTTP payload
          print('Payload digest OK:', record.verify_payload_digest())

Note that the ``verify_*`` methods will simply return ``False`` if the headers do not exist, so check that first. Also keep in mind that the block verification will fail if the reader has been (partially) consumed, so automatic HTTP parsing has to be turned off for this to work.

A word of warning: Calling either of these two methods will create an in-memory copy of the remaining record stream to preserve its contents for further processing (that's why verifying the HTTP payload digest after verifying the block digest worked in the first place). If your records are very large, you need to ensure that they fit into memory entirely (e.g. by checking :attr:`record.content_length <.WarcRecord.content_length>`). If you do not want to preserve the stream contents, you can set ``consume=True`` as a parameter. This will avoid the creation of a stream copy altogether and fully consume the rest of the record instead.
