.. _parse-http-manual:

HTTP Tools
==========

Helper functions for parsing raw HTTP payloads.

.. _parse-read-http-chunked:

Read Chunked HTTP Payloads
--------------------------

Contrary to `WARCIO <https://github.com/webrecorder/warcio>`_, Resiliparse's :ref:`FastWARC <fastwarc-manual>` does not automatically decode chunked HTTP responses. This is simply a design decision in favour of simplicity, since decoding chunked HTTP payloads is actually the crawler's job. In the `Common Crawl <https://commoncrawl.org>`_, for example, all chunked payloads are already decoded and the original ``Transfer-Encoding`` header is preserved as ``X-Crawler-Transfer-Encoding: chunked``. We do, however, acknowledge that in some cases it is still necessary to decode chunked payloads anyway, which is why Resiliparse provides :func:`~.parse.http.read_http_chunk` as a helper function for this.

The function accepts a buffered reader (either a :class:`fastwarc.stream_io.BufferedReader` or a file-like Python object that implements ``readline()``, such as :class:`io.BytesIO`) and is supposed to be called iteratively until no further output is produced. Each call will return a single chunk, which can be concatenated with the previous chunks:

.. code-block:: python

  from fastwarc.stream_io import BufferedReader, BytesIOStream
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

  reader = BufferedReader(BytesIOStream(chunked))
  decoded = b''
  while chunk := read_http_chunk(reader):
      decoded += chunk

  # b'Resiliparse is an awesome tool.'
  print(decoded)

For convenience, you can also use :func:`~.parse.http.iterate_http_chunks`, which is a generator that wraps around :func:`~.parse.http.read_http_chunk` and fully consumes the chunked stream:

.. code-block:: python

  from resiliparse.parse.http import iterate_http_chunks

  # b'Resiliparse is an awesome tool.'
  print(b''.join(iterate_http_chunks(reader)))
