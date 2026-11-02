.. _parse-http-manual:

HTTP Tools
==========

Helper functions for parsing raw HTTP payloads.

.. _parse-read-http-chunked:

Read Chunked HTTP Payloads
--------------------------

Resiliparse provides :func:`~.parse.http.read_http_chunk` for reading individual chunks from a chunked HTTP body.
When reading WARC records, :ref:`FastWARC <fastwarc-manual>` also supports decoding transfer encodings directly with
``ArchiveIterator(..., auto_decode='transfer')``. Decoding is disabled by default.

The function accepts a binary file-like object that implements both ``readline()`` and ``read(size)``, such as
:class:`io.BytesIO`, and is supposed to be called iteratively until no further
output is produced. Each call will return a single chunk, which can be concatenated with the previous chunks:

.. code-block:: python

  from io import BytesIO
  from resiliparse.parse.http import read_http_chunk

  chunked = b'''c\r\n\
  Resiliparse \r\n\
  6\r\n\
  is an \r\n\
  8\r\n\
  awesome \r\n\
  5\r\n\
  tool.\r\n\
  0\r\n\
  \r\n'''

  reader = BytesIO(chunked)
  decoded = b''
  while chunk := read_http_chunk(reader):
      decoded += chunk

  # b'Resiliparse is an awesome tool.'
  print(decoded)

For convenience, you can also use :func:`~.parse.http.iterate_http_chunks`, which is a generator that wraps around
:func:`~.parse.http.read_http_chunk` and fully consumes the chunked stream:

.. code-block:: python

  from resiliparse.parse.http import iterate_http_chunks

  reader = BytesIO(chunked)
  # b'Resiliparse is an awesome tool.'
  print(b''.join(iterate_http_chunks(reader)))

These helpers stop at an empty chunk or an invalid chunk header. They do not validate the chunk delimiter or parse
HTTP trailers. Each chunk is read with a single ``read(size + 2)`` call, so use a buffered reader that fills requested
reads when processing a stream that can return short reads. Reader exceptions propagate to the caller.
