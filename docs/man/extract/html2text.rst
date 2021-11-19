.. _extract-html2text-manual:

HTML2Text
=========

Resiliparse HTML2Text is a very fast and rule-based plain text extractor for HTML pages. HTML2Text uses the :ref:`Resilparse DOM parser <parse-html-manual>`.


Basic Plain Text Conversion
---------------------------
This is the simplest and fastest way to convert an HTML page to plain text. It will extract all visible text nodes inside the HTML ``<body>``. Only ``<script>``, ``<style>`` and a few other (generally) invisible elements are skipped and very basic ASCII formatting is applied:

.. code-block:: python

    from resiliparse.parse.html import HTMLTree
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

    tree = HTMLTree.parse(html)
    print(extract_plain_text(tree.body))

Output:

::

      • Index
      • Contact

    foo Link

    baz
    bar

    Some image

    Copyright (C) 2021 Foo Bar

The function :func:`~.extract.html2text.extract_plain_text` accepts several parameters for customizing the output, such as the extraction of ``alt`` texts (enabled by default), link ``href`` targets, form fields, or ``noscript`` elements.

.. code-block:: python

    # Without alt texts:
    extract_plain_text(tree.body, alt_texts=False)
    # Skips: "Some image"

    # With href targets:
    extract_plain_text(tree.body, links=True)
    # Adds:
    #   • Index (/)
    #   • Contact (/contact)

    # With form fields:
    extract_plain_text(tree.body, form_fields=True)
    # Adds:
    # [ Some text ] [ Insert text ]

    # With noscript
    extract_plain_text(tree.body, noscript=True)
    # Adds:
    # Sorry, your browser doesn't support JavaScript!

If you don't like list bullets, you can turn them off as well:

.. code-block:: python

    print(extract_plain_text(tree.body, list_bullets=False))

Output:

::

      Index
      Contact

    foo Link

    baz
    bar

    Some image

    Copyright (C) 2021 Foo Bar

If you want the most compact extraction possible without any formatting, set ``preserve_formatting=False``:


.. code-block:: python

    print(extract_plain_text(tree.body, preserve_formatting=False))

Output:

::

    Index Contact foo Link baz Some image Copyright (C) 2021 Foo Bar


Main Content Extraction
-----------------------
HTML2Text can also do very simple and fast rule-based main content extraction (also called boilerplate removal). Setting ``main_content=True`` will apply a set of rules for removing page elements such as navigation blocks, sidebars, footers, some ads, and (as far as they are possible to detect without rendering the page) invisible elements:

.. code-block:: python

    print(extract_plain_text(tree.body, main_content=True))

Output:

::

    foo

    baz
    bar

    Some image

Of course, the same options for adjusting the output as above can be applied here as well:

.. code-block:: python

    print(extract_plain_text(tree.body,
                             main_content=True,
                             alt_texts=False,
                             preserve_formatting=False,
                             noscript=True))

Output:

::

    foo baz bar Sorry, your browser doesn't support JavaScript!
