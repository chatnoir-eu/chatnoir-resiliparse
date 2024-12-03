.. _extract-html2text-manual:

HTML2Text
=========

Resiliparse HTML2Text is a very fast and rule-based plain text extractor for HTML pages. HTML2Text uses the :ref:`Resiliparse DOM parser <parse-html-manual>`.


Basic Plain Text Conversion
---------------------------
The simplest and fastest way to convert an HTML page to plain text is to use the :func:`~.extract_plain_text` helper without any further parameters. This will extract all visible text nodes inside the HTML document's ``<body>``. Only ``<script>``, ``<style>`` and a few other (generally) invisible elements are skipped and very basic ASCII formatting is applied:

.. code-block:: python

    from resiliparse.extract.html2text import extract_plain_text

    html = """<!doctype html>
    <head>
        <title>Foo</title>
        <meta charset="utf-8">
    </head>
    <body>
        <section id="wrapper">
            <nav>
                <ul>
                    <li><a href="/">Index</a></li>
                    <li><a href="/contact">Contact</a></li>
                </ul>
            </nav>
            <main id="foo">
                <h1>foo <a href="#foo" aria-hidden="true">Link</a></h1>

                <p>baz<br>bar</p>

                <img src="" alt="Some image">

                <input type="hidden" value="foo">
                <input type="text" value="Some text" placeholder="Insert text">
                <input type="text" placeholder="Insert text">
            </main>
            <script>alert('Hello World!');</script>
            <noscript>Sorry, your browser doesn't support JavaScript!</noscript>
            <div><div><div><footer id="global-footer">
                Copyright (C) 2021 Foo Bar
            </footer></div></div></div>
        </section>
    </body>
    </html>"""

    print(extract_plain_text(html))

Output:

::

      • Index
      • Contact

    foo Link

    baz
    bar

    Some image
    Copyright (C) 2021 Foo Bar

Instead of the raw HTML as a string, you can also pass an :class:`~resiliparse.parse.html.HTMLTree` instance.

For customization of the generated plain text, the function :func:`~.extract.html2text.extract_plain_text` accepts several parameters controlling individual aspects of its output, such as the extraction of ``alt`` texts (enabled by default), link ``href`` targets, form fields, or ``noscript`` elements.

.. code-block:: python

    # Without alt texts:
    extract_plain_text(html, alt_texts=False)
    # Skips: "Some image"

    # With href targets:
    extract_plain_text(html, links=True)
    # Adds:
    #   • Index (/)
    #   • Contact (/contact)
    #
    # foo Link (#foo)

    # With form fields:
    extract_plain_text(html, form_fields=True)
    # Adds:
    # [ Some text ] [ Insert text ]

    # With noscript
    extract_plain_text(html, noscript=True)
    # Adds:
    # Sorry, your browser doesn't support JavaScript!

If you don't like list bullets, you can turn them off as well:

.. code-block:: python

    print(extract_plain_text(html, list_bullets=False))

Output:

::

      Index
      Contact

    foo Link

    baz
    bar

    Some image
    Copyright (C) 2021 Foo Bar

For the most compact extraction without any formatting, set ``preserve_formatting=False``:

.. code-block:: python

    print(extract_plain_text(html, preserve_formatting=False))

Output:

::

    Index Contact foo Link baz bar Some image Copyright (C) 2021 Foo Bar


Minimal HTML Conversion
-----------------------

Instead of rendering a pure plain text version of the source document, Resiliparse can also spice up the plain text output with minimal HTML markup to retain some of the document's structural information. For a minimal HTML rendering, set ``preserve_formatting='minimal_html'`` (instead of the default ``preserve_formatting=True``):

.. code-block:: python

    print(extract_plain_text(html, preserve_formatting='minimal_html', links=True))

Output:

.. code-block:: html

    <ul>
      <li><a href="/">Index</a></li>
      <li><a href="/contact">Contact</a></li>
    </ul>

    <h1>foo <a href="#foo">Link</a></h1>

    <p>baz<br>
    bar</p>

    Some image
    Copyright (C) 2021 Foo Bar

With ``preserve_formatting='minimal_html'``, Resiliparse will retain headings (``<h1>`` -- ``<h6>``), paragraphs (``<p>``), pre-formatted text (``<pre>``), explicit line breaks (``<br>``), list items (``<ul>`` / ``<ol>`` and ``<li>``; unless ``list_bullets=False``), and anchor links (``<a href="...">``; only if ``links=True``, defaults to ``False``). Any additional attributes on those elements will not be preserved.


Main Content Extraction
-----------------------
Resiliparse HTML2Text can also do very simple and fast rule-based main content extraction (also called boilerplate removal). Setting ``main_content=True`` will apply a set of rules for removing page elements such as navigation blocks, sidebars, footers, some ads, and (as far as they are possible to detect without rendering the page) invisible elements:

.. code-block:: python

    print(extract_plain_text(html, main_content=True))

Output:

::

    foo

    baz
    bar

    Some image

Of course, the same options for adjusting the output as above can be applied here as well:

.. code-block:: python

    print(extract_plain_text(html,
                             main_content=True,
                             alt_texts=False,
                             preserve_formatting=False,
                             noscript=True))

Output:

::

    foo baz bar Sorry, your browser doesn't support JavaScript!
