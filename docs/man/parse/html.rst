.. _parse-html-manual:

HTML Parsing
============

Resiliparse comes with a light-weight and fast HTML parsing and DOM processing library based on the `Lexbor web browser engine <https://www.lexbor.com/>`_ for processing HTML web pages.


.. _parse-html-to-dom:

Parsing HTML to DOM
-------------------

To parse a Python Unicode string into a DOM tree, construct a new :class:`~.parse.html.HTMLTree` object by calling the static factory method :meth:`~.parse.html.HTMLTree.parse` on your string:

.. code-block:: python

  from resiliparse.parse.html import HTMLTree

  html = """<!doctype html>
  <html>
    <head>
      <meta charset="utf-8">
      <title>Example page</title>
    </head>
    <body>
      <div>Hello <strong>world</strong></div>
    </body>
  </html>"""

  tree = HTMLTree.parse(html)

If your HTML contents are encoded as bytes, use :meth:`~.parse.html.HTMLTree.parse_from_bytes` instead of :meth:`~.parse.html.HTMLTree.parse`, which takes a ``bytes`` object and an encoding:

.. code-block:: python

  from resiliparse.parse.encoding import detect_encoding

  html_bytes = html.encode('utf-16')
  tree = HTMLTree.parse_from_bytes(html_bytes, detect_encoding(html_bytes))

It is sufficient if the encoding name is a "best guess", since the name will be remapped according to the WHATWG specification (using :func:`~.parse.encoding.map_encoding_to_html5`) and the decoding is done with :func:`~.parse.encoding.bytes_to_str`, which tries several fallback encodings if the originally intended encoding fails (see :ref:`parse-bytes-to-str` for more information).


.. _parse-html-benchmark:

Benchmarking Resiliparse HTML Parsing
-------------------------------------

The Resiliparse HTML parser comes with a small benchmarking tool that can measure the parsing engine's performance and compare it to other Python HTML parsing libraries. Supported third-party libraries are `Selectolax <https://github.com/rushter/selectolax>`_ (both the old MyHTML and the new Lexbor engine) and `BeautifulSoup4 <https://www.crummy.com/software/BeautifulSoup/bs4/doc/>`_ (lxml engine only, which is the fastest BS4 backend).

Here are the results of extracting the titles from all web pages in an compressed 42,015-document WARC file on a Ryzen Threadripper 2920X machine:

.. code-block:: bash

  $ python3 -m resiliparse.parse.cli benchmark-html warcfile.warc
  HTML parser benchmark <title> extraction:
  =========================================
  Resiliparse (Lexbor):  42015 documents in 36.55s (1149.56 documents/s)
  Selectolax (Lexbor):   42015 documents in 37.46s (1121.52 documents/s)
  Selectolax (MyHTML):   42015 documents in 53.82s (780.72 documents/s)
  BeautifulSoup4 (lxml): 42015 documents in 874.40s (48.05 documents/s)

Not surprisingly, the two parsers based on the Lexbor engine perform almost identically, whereas lxml is by far the slowest by a factor of 24x.
