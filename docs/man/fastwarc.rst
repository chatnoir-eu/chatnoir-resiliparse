.. _fastwarc-manual:

FastWARC
========

FastWARC is a high-performance WARC parsing library written in Rust with bindings available for Python. Its design goals
are high speed, a low and fixed memory footprint, ease of use, and byte-level content preservation. FastWARC supports
compressed and uncompressed WARC/1.0 and WARC/1.1 streams. Supported compression algorithms are Gzip, Zstd, and LZ4.

FastWARC belongs to the :ref:`ChatNoir Resiliparse toolkit <resiliparse-docs-index>` for fast and robust web data
processing.

Why FastWARC and not WARCIO?
----------------------------

`WARCIO <https://github.com/webrecorder/warcio>`__ is a fantastic tool for reading and writing WARCs, but it is
implemented entirely in Python and thus rather inefficient for large web crawls at the tera- or petabyte scale where a
few seconds of additional processing time add up quickly. FastWARC, on the other hand, is written in efficient,
low-level Rust. WARCIO is also known to `mangle <https://github.com/webrecorder/warcio/issues/128>`__ `headers
<https://github.com/webrecorder/warcio/issues/129>`__ in certain situations, whereas FastWARC always tries to preserve
the exact byte contents of a record. FastWARC also supports more compression options with Zstandard (Zstd), which has
been `proposed as a new standard <https://iipc.github.io/warc-specifications/specifications/warc-zstd/>`__, and LZ4, a
much faster compression algorithm than Gzip.

FastWARC's API is inspired by WARCIO, but the goal was never to be a drop-in replacement. The API has also diverged
further over the years as new features were added. One notable difference is that we decided against adding support for
the legacy ARC format. If you need that kind of backwards compatibility, use WARCIO instead.


.. _fastwarc-installation:

Installing FastWARC
-------------------
Pre-built FastWARC binaries for Python can be installed from `PyPi <https://pypi.org/project/FastWARC/>`_:

.. code-block:: bash

  pip install fastwarc

Building FastWARC From Source
-----------------------------
.. note::

    If you just want to use FastWARC in Python, building the bindings from source is usually necessary, unless there are
    no pre-built binaries for your platform.

To build FastWARC from the `source repository <https://github.com/chatnoir-eu/chatnoir-resiliparse>`__, you need Rust
and Cargo installed. If that is not the case, do that first:

.. code:: bash

   # Install Rust toolchain (if not already installed)
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

Afterwards, follow these steps to build FastWARC from the GitHub repository:

.. code:: bash

   # Clone repository
   git clone https://github.com/chatnoir-eu/chatnoir-resiliparse.git
   cd chatnoir-resiliparse

   # Create a fresh venv first (recommended)
   python3 -m venv venv && source venv/bin/activate

   # Option 1: Build and install in editable mode (best for development)
   python3 -m pip install -e ./fastwarc-py

   # Option 2: Build and install wheels in separate steps (best for redistribution)
   python3 -m pip wheel -w build ./fastwarc-py
   ls ./build/*.whl | xargs python3 -m pip install

Iterating WARC Files
--------------------

.. important::

  Version 2.0 removes the legacy bindings and compatibility shims. Update scripts to use the current API names, such as
  ``GzipReader`` and ``GzipWriter`` in place of ``GZipStream``.


The central class for stream-processing WARC files is :class:`.ArchiveIterator`:

.. code-block:: python

   from fastwarc import ArchiveIterator

   # Explicit file-like object
   with open('warcfile.warc.gz', 'rb') as f:
       for record in ArchiveIterator(f):
           print(record.record_id)

   # File path (more efficient for local files)
   for record in ArchiveIterator('warcfile.warc.gz'):
       print(record.record_id)

This will iterate over all records in the file and print out their IDs. You can pass any file-like Python object or a
file path as a string to :class:`.ArchiveIterator`. The stream or file can be either uncompressed or a Gzip-, Zstd-, or
LZ4-compressed WARC. FastWARC will try to auto-detect the stream format (unless you pass ``stream_detect=False``). But
if you already know the compression algorithm beforehand, you can optimize the process a little by explicitly passing a
:class:`.GzipReader`, :class:`.ZstdReader`, or :class:`.LZ4Reader` object instead:

.. code-block:: python

   from fastwarc.stream_io import *

   # Gzip:
   stream = GzipReader('warcfile.warc.gz')

   # Zstandard:
   stream = ZstdReader('warcfile.warc.zst')

   # LZ4:
   stream = LZ4Reader('warcfile.warc.lz4')

Instead of a filename, you can also use any kind of file-like object:

.. code-block:: python

  stream = GzipReader(open('warcfile.warc.gz', 'rb'))

If `fsspec <https://filesystem-spec.readthedocs.io/>`__ is installed (which is a dependency if you installed FastWARC as
``fastwarc[fsspec]``), you can also use a (remote) URL instead of a local file path:

.. code-block:: python

   from fastwarc import ArchiveIterator

   # Read remote S3 object (with optional credentials)
   creds = {'key': '...',
            'secret': '...',
            'endpoint_url': '...'}
   for record in ArchiveIterator('s3://mybucket/warcfile.warc.gz', fsspec_args=creds):
       print(record.record_id)

   # Or without stream auto-detection:
   for record in ArchiveIterator(GzipReader('s3://mybucket/warcfile.warc.gz', fsspec_args=creds)):
       print(record.record_id)

Create your own :class:`fsspec.core.OpenFile` object if you need more control:

.. code-block:: python

   from fsspec import open as fsspec_open

   with fsspec_open('s3://mybucket/warcfile.warc.gz', 'rb', **creds) as f:
       for record in ArchiveIterator(f):
           print(record.record_id)


Filtering Records
-----------------
FastWARC provides several ways in which you can filter and efficiently skip records you are not interested in. These
filters are checked very early in the parsing process, right after the WARC header block has been read. Multiple types
of filters can be combined.

Record Type Filter
^^^^^^^^^^^^^^^^^^
If you want only records of a certain type, you can skip all other records efficiently by specifying a bitmask of the
desired record types:

.. code-block:: python

   from fastwarc import ArchiveIterator, WarcRecordType

   for record in ArchiveIterator(stream, record_types=WarcRecordType.request | WarcRecordType.response):
       pass

This will skip all records with a ``WARC-Type`` other than ``request`` or ``response``.

Content-Length Filter
^^^^^^^^^^^^^^^^^^^^^
You can automatically skip any records whose ``Content-Length`` exceeds or is lower than a certain value:

.. code-block:: python

   from fastwarc import ArchiveIterator

   # Skip all records that are larger than 500 KiB
   for record in ArchiveIterator(stream, max_content_length=512 << 10):
       pass

   # Skip all records that are smaller than 128 bytes
   for record in ArchiveIterator(stream, min_content_length=128):
       pass


Function Filters
^^^^^^^^^^^^^^^^
If the above-mentioned filter mechanisms are not sufficient, you can pass a function object that accepts as its only
parameter a :class:`.WarcRecord` and returns a ``bool`` value as a filter predicate. This filter type is slower than the
previous filters, but still more efficient than checking the same thing later on in the loop (especially when you use
the native predicate functions provided by FastWARC, see below).

FastWARC comes with a handful of existing filters that you can use:

.. code-block:: python

   from fastwarc.warc import *

   # Skip any non-HTTP records.
   for record in ArchiveIterator(stream, func_filter=is_http):
       pass

   # Skip records without a block digest (without verifying it).
   for record in ArchiveIterator(stream, func_filter=has_block_digest):
       pass

   # Skip records that are not WARC/1.1.
   for record in ArchiveIterator(stream, func_filter=is_warc_11):
      pass

   # Skip records that are neither requests nor responses, same as record_types=(request | response).
   for record in ArchiveIterator(stream, func_filter=has_record_type(request | response)):
       pass

Other Function Filters
^^^^^^^^^^^^^^^^^^^^^^
The full list of pre-defined function filters is:

- :func:`is_warc_10 <.is_warc_10>`
- :func:`is_warc_11 <.is_warc_11>`
- :func:`has_block_digest <.has_block_digest>`
- :func:`has_valid_block_digest <.has_valid_block_digest>`
- :func:`has_payload_digest <.has_payload_digest>`
- :func:`has_valid_payload_digest <.has_valid_payload_digest>`
- :func:`is_http <.is_http>`
- :func:`is_concurrent <.is_concurrent>`
- :func:`has_record_type(...) <.has_record_type>`
- :func:`has_content_length_lte(...) <.has_content_length_lte>`
- :func:`has_content_length_gte(...) <.has_content_length_gte>`

:func:`.has_record_type`, :func:`.has_content_length_lte`, and :func:`.has_content_length_gte` are parameterised
predicates. They are equivalent to using ``record_types``, ``min_content_length``, and ``max_content_length`` in the
:class:`.ArchiveIterator` constructor.

Besides these, you can pass any Python callable that accepts a :class:`.WarcRecord` and returns a ``bool``:

.. code-block:: python

   from fastwarc import WarcHeader

   # Skip records which haven't been identified as HTML pages
   for record in ArchiveIterator(stream,
         func_filter=lambda r: r.headers.get(WarcHeader.WARC_IDENTIFIED_PAYLOAD_TYPE) == 'text/html'):
       pass

Pure Python functions are less efficient than FastWARC's provided function filters, so try to keep them minimal.

Digest Filters
^^^^^^^^^^^^^^
You can skip all records with an invalid ``WARC-Block-Digest`` or ``WARC-Payload-Digest`` header. Unlike other filter
predicates, :func:`has_valid_block_digest` and :func:`has_valid_payload_digest` are executed only *after* the content is
available, which is needed for the digest calculation. These filters will skip records without or with an invalid block
or payload digest. You can also set ``verify_digests=True`` as a shorthand for specifying explicit digest filters for
both types of digests.

.. code-block:: python

   for record in ArchiveIterator(stream, verify_digests=True):
       pass

.. warning::

  These are the most expensive filters of all, as they will create an in-memory copy of the whole record. See
  :ref:`verifying-record-digests` for more information on how digest verification works.

Combining Filters
^^^^^^^^^^^^^^^^^
Multiple filters can be combined with a custom function or lambda:

.. code-block:: python

   # Skip records without any sort of digest header
   for record in ArchiveIterator(stream,
         func_filter=lambda r: has_block_digest(r) and has_payload_digest(r)):
       pass


Record Properties
-----------------
The :class:`.ArchiveIterator` returns objects of type :class:`.WarcRecord`, which have various properties:

.. code-block:: python

   for record in ArchiveIterator(stream):
       record.headers              # Dict-like object containing the WARC headers
       record.record_id            # Shorthand for record.headers[WarcHeader.WARC_RECORD_ID]
       record.record_type          # Shorthand for record.headers[WarcHeader.WARC_TYPE]
       record.record_date          # Parsed record.headers[WarcHeader.WARC_DATE]
       record.content_length       # Effective record payload length
       record.stream_pos           # Record start offset in the (uncompressed) stream
       record.is_http              # Boolean indicating whether record is an HTTP record
       record.is_http_parsed       # Boolean indicating whether the record has been parsed as HTTP
       record.http_headers         # Dict-like object containing the parsed HTTP headers
       record.http_content_type    # Plain HTTP Content-Type without charset
       record.http_charset         # HTTP charset from the Content-Type header (if any)
       record.http_date            # Parsed HTTP Date header
       record.http_last_modified   # Parsed HTTP Last-Modified header
       record.reader               # A WarcRecordPayloadReader for the record content

       # Read up to 1024 bytes from the record stream
       body = record.reader.read(1024)

       # Read the remaining record bytes
       body += record.reader.read()

       # Or: consume rest of stream without allocating a buffer for it (i.e., skip over)
       record.reader.consume()

HTTP request and response records are parsed automatically for convenience. If not needed (or wanted), you can disable
this behaviour by passing ``parse_http=False`` to the :class:`.ArchiveIterator` constructor to avoid unnecessary
processing. :attr:`record.reader <.WarcRecord.reader>` will then start at the beginning of the HTTP header block instead
of the HTTP body. You can parse HTTP headers later on a per-record basis by calling :meth:`record.parse_http()
<.WarcRecord.parse_http>` as long as the :class:`.WarcRecordPayloadReader` hasn't been consumed at that point.

.. _freezing-records:

Freezing Records
----------------

By default, :attr:`.WarcRecord.reader` returns a live reader on the WARC input stream. This means that when the iterator
returns the :class:`.WarcRecord` instance, no content bytes have been consumed yet and no buffer has been allocated for
them. This is very efficient, and it also prevents large records inadvertently consuming too much memory. However, that
also means that once the content bytes have been read or once the iterator moves to the next record, there is no way to
read them again. Also, if the input stream is compressed, you cannot seek backwards in the stream, even if you still
hold a reference to the :class:`.WarcRecord` instance.

To work around this, you can freeze a record by calling :meth:`.WarcRecord.freeze`. This will consume the record payload
reader, store the bytes in an internal buffer, and detach the record from the input stream. The reader returned by
:attr:`.WarcRecord.reader` then no longer points to the input stream, but to the frozen byte buffer. Frozen records can
be used indefinitely, and seeking on their payload reader is supported both in the forward and in the backward
direction.

.. _verifying-record-digests:

Verifying Record Digests
------------------------
If a record has digest headers, you can verify the consistency of the record contents and/or its HTTP payload:

.. code-block:: python

   from fastwarc import WarcHeader

   for record in ArchiveIterator(stream, parse_http=False):
       if WarcHeader.WARC_BLOCK_DIGEST in record.headers:
           print('Block digest OK:', record.verify_block_digest(consume=False))

       if WarcHeader.WARC_PAYLOAD_DIGEST in record.headers:
           # It's safe to call this even if the record has no HTTP payload
           record.parse_http()
           print('Payload digest OK:', record.verify_payload_digest(consume=False))

Note that :meth:`~.WarcRecord.verify_block_digest` and :meth:`~.WarcRecord.verify_payload_digest` will simply return
``False`` and not raise an error if the headers do not exist, so check that first. Keep in mind that the block
verification will fail if the reader has been (partially) consumed, so automatic HTTP parsing has to be turned off for
this to work.

.. warning::

  Calling either of these two methods will create an in-memory copy of the remaining record stream to preserve its
  contents for further processing (if ``consume=False``). That's why verifying the HTTP payload digest after verifying
  the block digest worked in the first place. If your records are very large, you need to ensure that they fit into
  memory entirely (e.g. by checking :attr:`record.content_length <.WarcRecord.content_length>`).

If you do not need to preserve the stream contents, you can set ``consume=True``. This will avoid the creation of a
stream copy altogether and fully consume the rest of the record instead. However, that also means that the payload is
lost after verifying the digests.


.. _fastwarc-writing-warcs:

Writing WARC Files
------------------

FastWARC supports creating new WARC records from scratch or from existing byte buffers. The created records can then be
written out using any file-like writer object. For writing compressed WARCs, you should use the writers provided in
:mod:`.stream_io`.

Create and Serialise Records
^^^^^^^^^^^^^^^^^^^^^^^^^^^^
Here's how you can create a new record and populate it with headers and a payload:

.. code:: python

   from fastwarc import WarcRecord, WarcRecordType

   record = WarcRecord()

   # Initialize mandatory headers.
   record.init_headers(WarcRecordType.response, b'uuid:record-uuid')

   # Alternative: initialize mandatory headers with an auto-generated random ID.
   record.init_headers(WarcRecordType.response, None)

   # Set the target ID header.
   record.headers.append_bytes(WarcHeader.WARC_TARGET_URI, b'https://example.com/index.html')

   # Set the payload bytes (automatically adjusts the Content-Length header).
   payload = (b'HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n' +
              b'<!DOCTYPE html><html><body>Hello, world!</body></html>')
   record.set_bytes_payload(payload)

   # Mark the record as an HTTP record by setting the correct Content-Type.
   record.is_http = True

   # Write out the record.
   with open('out.warc', 'wb') as out_file:
       record.write(out_file)

Output file:

.. code::

   WARC/1.1
   WARC-Type: response
   WARC-Date: 2026-06-11T11:07:04.191283Z
   WARC-Record-ID: <urn:uuid:94719fe2-63ee-4060-b322-fc4ddbdc0834>
   Content-Length: 98
   WARC-Target-URI: http://example.com/index.html
   Content-Type: application/http; msgtype=response

   HTTP/1.1 200 OK
   Content-Type: text/html

   <!DOCTYPE html><html><body>Hello, world!</body></html>

Instead of constructing the record manually, you can also parse an existing byte string:

.. code:: python

   record = WarcRecord.from_bytes(in_buf)

   # Write out the record.
   import io
   out_buf = io.BytesIO()
   record.write(out_buf)

   # Unless the record has been mutated, the output is guaranteed to be byte-identical.
   assert in_buf == out_buf.getvalue()

You can also use :meth:`.WarcRecord.write` with ``checksum_data=True`` to automatically calculate and write SHA-1
checksums of the record contents and HTTP payload (if available).

Write Compressed WARCs
^^^^^^^^^^^^^^^^^^^^^^
Compressed WARC files consist of a series frames (or members in Gzip lingo) that can individually be decompressed, one
per record. To correctly write such a WARC file, use the compressing writers from FastWARC's :mod:`stream_io` module.

.. code:: python

   from fastwarc import WarcRecord, WarcRecordType
   from fastwarc.stream_io import GzipWriter

   # Create a request record.
   request = WarcRecord()
   request.init_headers(WarcRecordType.request, None);
   request.set_bytes_payload(b'GET / HTTP/1.1\r\nHost: example.com\r\n\r\n')
   request.is_http = True

   # Create a response record.
   response = WarcRecord()
   response.init_headers(WarcRecordType.response, None);
   response.set_bytes_payload(b'HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n' +
                              b'<!DOCTYPE html><html><body>Hello, world!</body></html>')
   response.is_http = True

   # Create output file, write records, and end each compression member with finish().
   with GzipWriter('out.warc.gz') as gzip_writer:
       request.write(gzip_writer)
       gzip_writer.finish()
       response.write(gzip_writer)
       gzip_writer.finish()

Write Custom Zstandard Dictionaries
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
Zstandard-compressed WARCs support custom compression dictionaries to achieve better compression ratios. The dictionary
is stored in a special dictionary frame before the first record in the output file. Such a dictionary can, e.g., be
trained on the first few records of a WARC file. FastWARC supports reading and writing WARCs with custom dictionaries.
If FastWARC encounters a dictionary frame at the start of a Zstandard WARC, it will automatically use it to decompress
the remainder of the stream.

Here's how you can create a ``.warc.zst`` file with a dictionary frame in it:

.. code:: python

   from fastwarc import WarcRecord
   from fastwarc.stream_io import ZstdWriter, zstd_train_dictionary_from_samples

   # record_bytes = [b'WARC/1.1\r\n...', ...];

   # Train dict on the first few records (raises OSError if size is too small).
   MAX_DICT_SIZE = 128 << 10  # 128 KiB
   zstd_dict = zstd_train_dictionary_from_samples(record_bytes[:10], MAX_DICT_SIZE)

   # Write a Zstandard WARC with a dictionary frame.
   with ZstdWriter('out.warc.zst', dictionary=zstd_dict) as zstd_writer:
       for b in record_bytes:
           r = WarcRecord.from_bytes(b)
           r.write(zstd_writer)
           zstd_writer.finish()


.. _fastwarc-clueweb:

ClueWeb Notes
-------------
FastWARC is a standards-compliant WARC parser. Unfortunately, the `ClueWeb <https://lemurproject.org/>`__ authors were
somewhat creative with the standard. If you work with these datasets, you will inevitably notice certain defects in the
files that result in premature stream aborts. This applies to both the old ClueWeb09, as well as the new ClueWeb22.
Following is a list of known ClueWeb WARC defects and how to work around them:

ClueWeb09
^^^^^^^^^
The `ClueWeb09 <https://lemurproject.org/clueweb09.php/>`__ uses a non-standard WARC version and the WARC headers are
separated by ``LF`` instead of ``CRLF`` line breaks. In addition, some header values contain line breaks without proper
continuation indents and non-ASCII characters. To prevent FastWARC from choking on this creative WARC flavour, pass
``quirks_mode=True`` to your :class:`.ArchiveIterator`.

ClueWeb22
^^^^^^^^^
`ClueWeb22 <https://lemurproject.org/clueweb22.php/>`__ WARCs are a bit more predictable than ClueWeb09 WARCs, but have
non-trivial defects nonetheless.

*First*, the initial ``warcinfo`` records are missing the required ``Content-Length`` header, so we have to rely on
heuristics to determine where the record ends. If quirks mode is on, FastWARC will read and buffer a ``warcinfo`` record
until a blank line is found (up to ``max_header_len`` bytes) and then scan forward to the next valid WARC version line
in the next iteration. Without quirks mode, FastWARC will emit a ``warcinfo`` record with an empty payload and then exit
with an ``"Invalid WARC header"`` error.

*Second*, all records are of type ``response`` with ``Content-Type: application/http; msgtype=response``, yet they
contain only the HTML body and not the full HTTP response (the correct record type would be ``resource`` with
``Content-Type: text/html``). This incorrect type description will trigger FastWARC's automatic HTTP parsing, which will
result in empty or incomplete record bodies. To avoid this, explicitly set ``parse_http=False``.

*Third*, all ``response`` records have a number of ``VDOM-*`` headers, which can get extremely large (up to 1MiB).  When
parsing ClueWeb22 WARCs, set the iterator's ``max_header_len`` option accordingly.

.. _fastwarc-benchmarks:

Benchmarks
----------
Depending on your CPU, memory architecture, storage speed, and the WARC compression algorithm, you can typically expect
speedups between 1.5x and 5x and in extreme cases even up to 13x over WARCIO.

Read directly from DDR4-DRAM, FastWARC can achieve throughput for uncompressed WARC files of more than 6.4 GiB/s on a
single core. In more realistic scenarios, the throughput is usually in the order of 1.5--2.5 GiB/s. Gzip-compressed
WARCs cap out at around 850-900 MiB/s. Zstd WARCS come at around 1 GiB/s (with compression level 3) and LZ4 at around
1.6 GiB/s.

The FastWARC GitHub repository contains a `comprehensive suite of benchmarks
<https://github.com/chatnoir-eu/chatnoir-resiliparse/tree/develop/benchmarks/warc#readme>`__ of FastWARC (both Rust
native and Python), as well as several other popular open source WARC reading libraries.

The following times were benchmarked on a 2020 M1 MacBook Pro:

.. code-block::

   # Uncompressed WARC:
   FastWARC: 1.9s, 59283 records/s, 2748.7 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
   WARCIO: 5.0s, 22642 records/s, 1049.8 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)

   # Gzip WARC:
   FastWARC: 6.5s, 17600 records/s, 816.0 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
   WARCIO: 9.8s, 11706 records/s, 542.7 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)

The following times were benchmarked on a Threadripper 2920X 12-Core CPU with a Samsung 980PRO NVMe SSD (cold read
without page cache):

.. code-block::

   # Uncompressed WARC:
   FastWARC: 3.5s, 32395 records/s, 1502.0 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
   WARCIO: 15.7s, 7294 records/s, 338.2 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)

   # Gzip WARC:
   FastWARC: 6.9s, 16470 records/s, 763.7 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
   WARCIO: 25.9s, 4419 records/s, 204.9 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)

For more detailed information and benchmarking results, checkout the `benchmarks folder
<https://github.com/chatnoir-eu/chatnoir-resiliparse/tree/develop/benchmarks/warc>`__ in the GitHub repository.
