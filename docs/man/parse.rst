.. _parse-manual:

Resiliparse Parsing Tools
=========================

The Parse module is a set of helper functions for processing and transforming textual contents or extracting information, particularly from web pages.

.. _parse-encoding-detection:

Character Encoding Detection
----------------------------
Resiliparse provides fast and accurate text encoding detection with :class:`~.parse.EncodingDetector`, a wrapper around the `uchardet <https://github.com/freedesktop/uchardet>`_ library, which is based on Mozilla's *Universal Charset Detector*.

.. code-block:: python

  from resiliparse.parse import EncodingDetector

  det = EncodingDetector()
  det.update(b'\xff\xfeH\x00e\x00l\x00l\x00o\x00 \x00W\x00o\x00r\x00l\x00d\x00')
  enc = det.encoding()  # utf-16-le

  det.update(b'Autres temps, autres m\x9curs.')
  enc = det.encoding()  # cp1252 (Windows-1252)

You can call :meth:`~.parse.EncodingDetector.update` multiple times to feed more input. The more data the detector has to sample from, the more accurate the prediction will be. Calling :meth:`~.parse.EncodingDetector.encoding` will return the predicted encoding name as a string and reset the internal state so that the detector can be reused for a different document.

By default, the detected encoding name is remapped according to the WHATWG encoding specification (see: :ref:`parse-map-encodings` for a detailed description). If you do not want this remapping to take place, set ``html5_compatible=False`` on the call to :meth:`~.parse.EncodingDetector.encoding`. In this case, the returned value can be ``None`` if the encoding could not be determined. With WHATWG remapping enabled, unknown encodings are mapped to UTF-8. This is convenient most of the time, but it also means that the source text is not necessarily decodable without errors, so care should be taken here. You can use :func:`~.parse.bytes_to_str` to avoid decoding errors (see :ref:`parse-bytes-to-str` for details).

As a convenience shortcut, Resiliparse also provides :func:`~.parse.detect_encoding` which creates and maintains a single global :class:`~.parse.EncodingDetector` instance:

.. code-block:: python

  from resiliparse.parse import detect_encoding

  enc = detect_encoding(b'Potrzeba jest matk\xb1 wynalazk\xf3w.')  # iso8859-2


.. _parse-map-encodings:

Map Encodings to WHATWG Specification
-------------------------------------
Before decoding the contents of a web page with an encoding that was extracted from either the HTTP ``Content-Type`` header or the HTML body itself, it often makes sense to remap the encoding according to `WHATWG encoding specification
<https://encoding.spec.whatwg.org/#names-and-labels>`_. The WHATWG mapping is designed to boil down the many possible encoding names to a smaller subset of canonical names while taking into account common encoding mislabelling practices. The mapping is primarily designed for author-supplied encoding names, but it also makes sense to apply it to auto-detected encoding names, since it remaps some encodings based on observed practices on the web, such as the mapping from ISO-8859-1 to Windows-1252, which is more likely to be correct, even if both are possible. You can remap a given encoding name as follows:

.. code-block:: python

  from resiliparse.parse import map_encoding_to_html5

  print(map_encoding_to_html5('iso-8859-1'))    # cp1252
  print(map_encoding_to_html5('csisolatin9'))   # iso8859-15
  print(map_encoding_to_html5('oops'))          # utf-8

You see that the given input name does not necessarily have to be a valid Python encoding name, but the returned output will be. Unknown or invalid encodings are mapped to UTF-8. Set ``fallback_utf8=False`` if you prefer to get ``None`` back instead.

If you use :class:`~.parse.EncodingDetector` for encoding auto-detection (see: :ref:`parse-encoding-detection`), encoding names are already remapped by default.


.. _parse-bytes-to-str:

Convert Byte String to Unicode
------------------------------
Detecting the encoding of a byte string is one thing, but the next step is to actually decode it into a Unicode string. Resiliparse provides :func:`~.parse.bytes_to_str`, which does exactly that.

The function takes the raw byte string and a desired encoding name and tries to decode it into a Python Unicode string. If the decoding fails (due to undecodable characters), it will try to fall back to UTF-8 and Windows-1252. If both fallbacks fail as well, the string will be decoded with the originally intended encoding and invalid characters will either be skipped or replaced with a suitable replacement character (controllable via the ``errors`` parameter, which accepts the same values as Python's ``str.decode()``).

.. code-block:: python

  from resiliparse.parse import detect_encoding, bytes_to_str

  bytestr = b'\xc3\x9cbung macht den Meister'
  decoded = bytes_to_str(bytestr, detect_encoding(bytestr))  # 'Übung macht den Meister'

Of course a simple ``bytestr.decode()`` would be sufficient for such a trivial example, but sometimes, the supplied encoding is inaccurate or the string turns out to contain mixed or broken encodings. In that case there is no other option than to try multiple encodings and to ignore any errors if all of them fail. The default fallback encodings for this situation (UTF-8 and Windows-1252) can be overridden with the ``fallback_encodings`` parameter.

.. warning::

  When setting custom fallback encodings, keep in mind that single-byte encodings without undefined codepoints (such as IANA ISO-8859-1) will never fail, so it does not make sense to have more than one of those in the fallback list. In fact, even very dense encodings such as Windows-1252 are very unlikely to ever fail.

:func:`~.parse.bytes_to_str` also ensures that the resulting string can be re-encoded as UTF-8 without errors, which is not always the case when doing a simple ``str.encode()``:

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
