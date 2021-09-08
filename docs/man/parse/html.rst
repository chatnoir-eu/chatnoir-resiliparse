.. _parse-html-manual:

HTML Parsing
============

Resiliparse comes with a light-weight and fast HTML parsing and DOM processing library based on the `Lexbor web browser engine <https://www.lexbor.com/>`_ for processing HTML web pages.

To parse a Python Unicode string into a DOM tree, construct a new :class:`~.HTMLTree` object by calling the static factory method :meth:`~.HTMLTree.parse`:

.. code-block:: python

  from resiliparse.parse.html import HTMLTree

  html = """<!doctype html>
  <html lang="en">
    <head>
      <meta charset="utf-8">
      <title>Example page</title>
    </head>
    <body>
      <main id="foo">
        <p id="a">Hello <span class="bar">world</span>!</p>
        <p id="b" class="dom">Hello <span class="bar baz">DOM</span>!</p>
       </main>
    </body>
  </html>"""

  tree = HTMLTree.parse(html)

If your HTML contents are encoded as bytes, use :meth:`~.HTMLTree.parse_from_bytes` instead of :meth:`~.HTMLTree.parse`, which takes a ``bytes`` object and an encoding:

.. code-block:: python

  from resiliparse.parse.encoding import detect_encoding

  html_bytes = html.encode('utf-16')
  tree = HTMLTree.parse_from_bytes(html_bytes, detect_encoding(html_bytes))

It is sufficient if the encoding name is a "best guess", since the name will be remapped according to the WHATWG specification (using :func:`~.parse.encoding.map_encoding_to_html5`) and the decoding is done with :func:`~.parse.encoding.bytes_to_str`, which tries several fallback encodings if the originally intended encoding fails (see :ref:`parse-bytes-to-str` for more information).


.. _parse-html-select-elements:

DOM Element Selection
---------------------

Resiliparse provides the standard basic DOM functions for selecting element nodes in the DOM tree. Supported functions are:

* :meth:`~.DOMNode.get_element_by_id`
* :meth:`~.DOMNode.get_elements_by_tag_name`
* :meth:`~.DOMNode.get_elements_by_class_name`
* :meth:`~.DOMNode.query_selector`
* :meth:`~.DOMNode.query_selector_all`
* :meth:`~.DOMNode.matches`

which behave just like you would expect from other languages or libraries. Each of these returns either a single :class:`~.DOMNode` object or a :class:`.DOMCollection` containing all matching :class:`.DOMNode` objects. The only exception is :meth:`~.DOMNode.matches`, which returns a boolean value indicating whether a matching element exists in the subtree. In addition, Resiliparse provides a generic :meth:`~.DOMNode.get_elements_by_attr` function for matching by arbitrary attribute names and values.

Elements
^^^^^^^^

The document root element node can be accessed with the :attr:`.HTMLTree.document` property. The properties :attr:`.HTMLTree.body` and :attr:`.HTMLTree.head` also exist for directly accessing a document's ``<head>`` or ``<body>`` elements (if they exist).

.. code-block:: python

  # Match single node by ID:
  print(repr(tree.body.get_element_by_id('foo')))
  # >>> <main id="foo">

  # Match multiple nodes by tag name:
  print(repr(tree.head.get_elements_by_tag_name('meta')))
  # >>> {<meta charset="utf-8">}

  # Match multiple nodes by class name:
  print(repr(tree.body.get_elements_by_class_name('bar')))
  # >>> {<span class="bar">, <span class="bar baz">}

  # Match single node by CSS selector:
  print(repr(tree.document.query_selector('body > main p:last-child')))
  # >>> <p id="b" class="dom">

  # Match multiple nodes by CSS selector:
  print(repr(tree.body.query_selector_all('main *')))
  # >>> {<p id="a">, <span class="bar">, <p id="b" class="dom">, <span class="bar baz">}

  # Check whether there is any element matching this CSS selector:
  print(tree.body.matches('.bar'))
  # >>> True

:class:`.DOMCollection` objects are iterable, indexable, and slicable. The size of a collection can be checked with ``len()``. If a slice is requested, the returned object will be another :class:`.DOMCollection`:

.. code-block:: python

  coll = tree.body.query_selector_all('main *')

  # First element
  print(repr(coll[0]))
  # >>> <p id="a">

  # Last element
  print(repr(coll[-1]))
  # >>> <span class="bar">

  # First two elements
  print(repr(coll[:2]))
  # >>> {<p id="a">, <span class="bar">}

:class:`.DOMCollection` objects have the same DOM methods for selecting objects as :class:`.DOMNode` objects. This can be used for efficiently matching elements in the subtree(s) of the previously selected elements. The selection methods behave just like their :class:`.DOMNode` counterparts and return either a single :class:`.DOMNode` or another :class:`.DOMCollection`:

.. code-block:: python

  coll = tree.body.get_elements_by_class_name('dom')

  # Only matches within the subtrees of elements in `coll`:
  print(repr(coll.get_elements_by_class_name('bar')))
  # >>> {<span class="bar baz">}


.. _parse-html-attributes:

Attributes
^^^^^^^^^^

Attributes of element nodes can be accessed either via :meth:`.DOMNode.getattr` or by dict-like access:

.. code-block:: python

  meta = tree.head.query_selector('meta[charset]')
  if meta is not None:
    print(meta.getattr('charset'))
    # >>> utf-8

    # Or:
    print(meta['charset'])
    # >>> utf-8

The dict access method will raise a :exc:`KeyError` exception if the attribute does not exist.

A list of existing attributes on an element is provided by its :attr:`~.DOMNode.attrs` property:

.. code-block:: python

  print(tree.body.query_selector('main').attrs)
  # >>> ['id']

The ``id`` and ``class`` attributes of an element are also available through the :attr:`~.DOMNode.id` and :attr:`~.DOMNode.class_name` properties. If multiple class names are set (separated by spaces), they can be accessed and modified individually via :attr:`~.DOMNode.class_list`:

.. code-block:: python

    tree.body.id = 'foobar'
    print(repr(tree.body))
    # >>> <body id="foobar">

    tree.body.class_name = 'class-a'
    print(tree.body.class_name)
    # >>> class-a

    tree.body.class_list.add('class-b')
    print(tree.body.class_list)
    # >>> ['class-a', 'class-b']

    tree.body.class_list.remove('class-a')
    print(tree.body.class_name)
    # >>> class-b

.. _parse-html-text-serialization:

HTML and Text Serialization
^^^^^^^^^^^^^^^^^^^^^^^^^^^

All :class:`.DOMNode` objects have a :attr:`~.DOMNode.text` and :attr:`~.DOMNode.html` property for accessing their plaintext or HTML serialization:

.. code-block:: python

  print(tree.body.get_element_by_id('a').text)
  # >>> Hello world!

  print(tree.body.get_element_by_id('a').html)
  # >>> <p id="a">Hello <span class="bar">world</span>!</p>

Alternatively, you can also simply cast a :class:`.DOMNode` to ``str``, which is equivalent to :attr:`.DOMNode.html`:

.. code-block:: python

  print(tree.body.get_element_by_id('a'))
  # >>> <p id="a">Hello <span class="bar">world</span>!</p>

For extracting specifically the text contents of the document's ``<title>`` element, there is also the :attr:`.HTMLTree.title` property:

.. code-block:: python

  # Example page
  print(tree.title)


.. _parse-html-traversal:

DOM Tree Traversal
------------------

The DOM subtree of any node can be traversed in pre-order by iterating over a :class:`.DOMNode` instance. Different types of nodes can be distinguished by their :attr:`~.DOMNode.type` property.

.. code-block:: python

  from resiliparse.parse.html import NodeType

  root = tree.body.get_element_by_id('a')

  tag_names = [e.tag for e in root]
  tag_names_elements_only = [e.tag for e in root if e.type == NodeType.ELEMENT]

  print(tag_names)
  # >>> ['p', '#text', 'span', '#text', '#text']

  print(tag_names_elements_only)
  # >>> ['p', 'span']

To iterate only the immediate children of a node, loop over its :attr:`~.DOMNode.child_nodes` property instead of the node itself:

.. code-block:: python

  for e in tree.body.get_element_by_id('foo').child_nodes:
    if e.type == NodeType.ELEMENT:
      print(e.text)
  # >>> Hello DOM!
  # >>> Hello world!

In addition, any :class:`.DOMNode` object also has the following properties:

* :attr:`~.DOMNode.first_child`
* :attr:`~.DOMNode.last_child`
* :attr:`~.DOMNode.prev`
* :attr:`~.DOMNode.next`
* :attr:`~.DOMNode.parent`

which can be used for traversing the tree more flexibly.


.. _parse-html-manipulate:

DOM Tree Manipulation
---------------------

Resiliparse supports DOM manipulation and the creation of new nodes with a basic set of well-known DOM functions.

.. warning::

  A :class:`.DOMNode` object is valid only for as long as its parent tree has not been modified or deallocated. Thus, **DO NOT** use existing instances after any sort of DOM tree manipulation! Doing so may result in Python crashes or (worse) security vulnerabilities due to dangling pointers (*use after free*). This is a `known Lexbor limitation <https://github.com/lexbor/lexbor/issues/132>`_ for which there is no workaround at the moment.

Elements
^^^^^^^^
In the following is an example of how you can create new DOM elements and text nodes and insert them into the tree:

.. code-block:: python

  # Create a new <div> element node
  new_element = tree.create_element('p')

  # Create a new text node
  new_text = tree.create_text_node('Hello Resiliparse!')

  # Insert nodes into DOM tree
  main_element = tree.body.query_selector('main')
  main_element.append_child(new_element)
  new_element.append_child(new_text)

  print(main_element)
  # >>> <main id="foo">
  # >>>   <p id="a">Hello <span class="bar">world</span>!</p>
  # >>>   <p id="b" class="dom">Hello <span class="bar baz">DOM</span>!</p>
  # >>>  <p>Hello Resiliparse!</div></p>

In addition to :meth:`~.DOMNode.append_child`, nodes also provide :meth:`~.DOMNode.insert_before` for inserting a child node before another child instead of appending it at the end, and :meth:`~.DOMNode.replace_child` for replacing an existing child node in the tree with another.

Use :meth:`~.DOMNode.remove_child` to remove a node from the tree:

.. code-block:: python

  main_element.remove_child(new_element)

To fully delete a node, use :meth:`~.DOMNode.decompose()` on the node itself. This will remove it from the tree (if not already done) and delete the node and its entire subtree recursively:

.. code-block:: python

  new_element.decompose()
  # From here on, this element and all elements in its subtree are invalid!!!

Attributes
^^^^^^^^^^
Attributes can be added or modified via :meth:`~.DOMNode.setattr` or by assigning directly to its dict entry:

.. code-block:: python

  new_element['id'] = 'c'
  new_element.setattr('class', 'foobar')

  print(new_element)
  # >>> <p id="c" class="foobar">Hello Resiliparse!</p>


Inner HTML and Inner Text
^^^^^^^^^^^^^^^^^^^^^^^^^
An easier, but less efficient way of manipulating the DOM is to assign a string directly to either its :attr:`~.DOMNode.html` or :attr:`~.DOMNode.text` property. This will replace the inner HTML or inner text of these nodes with the new value:

.. code-block:: python

  main_element.html = '<p>New inner HTML content</p>'
  print(main_element)
  # >>> <main id="foo"><p>New HTML content</p></main>

  main_element.text = '<p>New inner text content</p>'
  print(main_element)
  # >>> <main id="foo">&lt;p&gt;New inner text content&lt;/p&gt;</main>


.. _parse-html-benchmark:

Benchmarking Parser Performance
-------------------------------

The Resiliparse HTML parser comes with a small benchmarking tool that can measure the parsing engine's performance and compare it to other Python HTML parsing libraries. Supported third-party libraries are `Selectolax <https://github.com/rushter/selectolax>`_ (both the old MyHTML and the new Lexbor engine) and `BeautifulSoup4 <https://www.crummy.com/software/BeautifulSoup/bs4/doc/>`_ (lxml engine only, which is the fastest BS4 backend).

Here are the results of extracting the titles from all web pages in an uncompressed 42,015-document WARC file on a Ryzen Threadripper 2920X machine:

.. code-block:: bash

  $ python3 -m resiliparse.parse.cli benchmark-html warcfile.warc
  HTML parser benchmark <title> extraction:
  =========================================
  Resiliparse (Lexbor):  42015 documents in 36.55s (1149.56 documents/s)
  Selectolax (Lexbor):   42015 documents in 37.46s (1121.52 documents/s)
  Selectolax (MyHTML):   42015 documents in 53.82s (780.72 documents/s)
  BeautifulSoup4 (lxml): 42015 documents in 874.40s (48.05 documents/s)

Not surprisingly, the two parsers based on the Lexbor engine perform almost identically, whereas lxml is by far the slowest by a factor of 24x.
