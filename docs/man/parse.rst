.. _parse-manual:

Resiliparse Parsing Tools
=========================

The Parse module is a set of helper functions for processing and transforming textual contents or extracting information.

.. _parse-encoding-detection:

Character Encoding Detection
----------------------------
Resiliparse provides fast and accurate text encoding detection with :class:`~.parse.EncodingDetector`, a wrapper around the `uchardet <https://github.com/freedesktop/uchardet>`_ library, which is based on Mozilla's *Universal Charset Detector*.

.. code-block:: python

  from resiliparse.parse import EncodingDetector

  det = EncodingDetector()
  det.update(b'Hello World.')
  enc = det.encoding()  # ASCII

  det.update(b'\xc0 vaillant coeur rien d`impossible.')
  enc = det.encoding()  # WINDOWS-1252

You can call :meth:`~.parse.EncodingDetector.update` multiple times to feed more input. The more data the detector has to sample from, the more accurate the prediction will be. Calling :meth:`~.parse.EncodingDetector.encoding` will return the predicted encoding name as a string and reset the internal state so that the detector can be reused for a different document.

As a convenience shortcut, Resiliparse also provides :func:`~.parse.detect_encoding` which creates and maintains a single global :class:`~.parse.EncodingDetector` instance:

.. code-block:: python

  from resiliparse.parse import detect_encoding

  enc = detect_encoding(b'Potrzeba jest matk\xb1 wynalazk\xf3w.')  # ISO-8859-2


.. _parse-bytes-to-str:

Convert Byte String to Unicode
------------------------------
Detecting the encoding of a byte string is one thing, but the next step is actually decoding it into a Unicode string. Resiliparse provides :func:`~.parse.bytes_to_str`, which does exactly that.

The function takes the raw byte string desired encoding name and tries to decode it into a Python Unicode string. If the decoding fails (due to undecodable characters), it will try to fall back onto UTF-8 and Windows-1252. If both fallbacks fail as well, the string will be decoded with the originally intended encoding and invalid characters will either be skipped or replaced with a suitable replacement character (controllable via the ``errors`` parameter, which accepts the same values as Python's :meth:`str.decode`).

.. code-block:: python

  from resiliparse.parse import detect_encoding, bytes_to_str

  bytestr = b'\xc3\x9cbung macht den Meister'
  decoded = bytes_to_str(bytestr, detect_encoding(bytestr))  # 'Übung macht den Meister'

Of course simple :meth:`bytestr.decode` would be sufficient for such a trivial example, but sometimes the encoding detection is inaccurate or fails completely or the string turns out to contain mixed or broken encodings. In that case there is no other option than trying multiple encodings and ignoring errors if all of them fail. The default fallback encodings to try in that case can be overridden with the ``fallback_encodings`` parameter.

.. important::

  For these fallback encodings, keep in mind that single-byte encodings without undefined codepoints (such as IANA ISO-8859-1) will never fail, so it does not make sense to have more than one of those in the fallback list. In fact, even very dense encodings such as Windows-1252 are very unlikely to ever fail.

:func:`bytes_to_str` also ensures that the resulting string can be re-encoded as UTF-8 without errors, which is not always the case when doing a simple :meth:`str.encode`:

.. code-block:: python

  from resiliparse.parse import bytes_to_str

  # This will produce the unencodable string 'ઉ\udd7a笞':
  unencodeable = b'+Condensed'.decode('utf-7', errors='ignore')

  # OK, but somewhat broken: b'+Condense-'
  unencodeable.encode('utf-7')

  # Error: UnicodeEncodeError: 'utf-8' codec can't encode character '\udd7a' in position 1: surrogates not allowed
  unencodeable.encode()

With :func:`~.parse.bytes_to_str`, these issues can be avoided:

.. code-block:: python

  # Produces '+Condensed', because UTF-8 fallback can decode the string without errors
  bytes_to_str(b'+Condensed', 'utf-7')

  # But even without fallbacks, we get 'ઉ笞', which can at least be re-encoded as UTF-8
  bytes_to_str(b'+Condensed', 'utf-7', fallback_encodings=[])


.. _parse-read-http-chunked:

Read Chunked HTTP Payloads
--------------------------

Contrary to `WARCIO <https://github.com/webrecorder/warcio>`_, Resiliparse's :ref:`FastWARC <fastwarc-manual>` does not automatically decode chunked HTTP responses. This is simply a design decision in favour of simplicity, since decoding chunked HTTP payloads is actually the crawler's job. In the `Common Crawl <https://commoncrawl.org>`_, for example, all chunked payloads are already decoded and the original ``Transfer-Encoding`` header is preserved as ``X-Crawler-Transfer-Encoding: chunked``. We do, however, acknowledge that in some cases it is still necessary to decode chunked payloads anyway, which is why Resiliparse provides :func:`~.parse.read_http_chunk` as a helper function for this.

The function accepts a buffered reader (either a :class:`fastwarc.stream_io.BufferedReader` or a file-like Python object that implements ``readline()``, such as :class:`io.BytesIO`) and is supposed to be called iteratively until no further output is produced. Each call will return a single chunk, which can be concatenated with the previous chunks:

.. code-block:: python

  from fastwarc.stream_io import BufferedReader, BytesIOStream
  from resiliparse.parse import read_http_chunk

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

For convenience, you can also use :func:`~.parse.iterate_http_chunks`, which is a generator that wraps around :func:`~.parse.read_http_chunk` and fully consumes the chunked stream:

.. code-block:: python

  from resiliparse.parse import iterate_http_chunks

  # b'Resiliparse is an awesome tool.'
  print(b''.join(iterate_http_chunks(reader)))
