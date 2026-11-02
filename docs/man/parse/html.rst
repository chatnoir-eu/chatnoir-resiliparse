.. _parse-html-manual:

HTML Parsing
============

Resiliparse comes with a light-weight and fast HTML parsing and DOM processing library based on the `Lexbor web browser
engine <https://www.lexbor.com/>`_ for processing HTML web pages.

To parse a Python Unicode string into a DOM tree, construct a new :class:`~.HTMLTree` object by calling the static
factory method :meth:`~.HTMLTree.parse`:

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

If your HTML contents are encoded as bytes, use :meth:`~.HTMLTree.parse_from_bytes` instead of :meth:`~.HTMLTree.parse`,
which takes a ``bytes`` object and an encoding:

.. code-block:: python

    from resiliparse.parse.encoding import detect_encoding

    html_bytes = html.encode('utf-16')
    tree = HTMLTree.parse_from_bytes(html_bytes, detect_encoding(html_bytes))

It is sufficient if the encoding name is a "best guess", since the name will be remapped according to the WHATWG
specification (using :func:`~.parse.encoding.map_encoding_to_html5`) and the decoding is done with
:func:`~.parse.encoding.bytes_to_str`, which tries several fallback encodings if the originally intended encoding fails
(see :ref:`parse-bytes-to-str` for more information).

Node Types and Compatibility
----------------------------
All DOM nodes inherit from :class:`.Node`. Elements, text, comments, attributes, documents, and document fragments are
represented by specialised classes such as :class:`.ElementNode`, :class:`.TextNode`, and :class:`.DocumentNode`.
Element-specific methods such as attribute access are available only on element nodes. Use ``isinstance(node,
ElementNode)`` or :attr:`.Node.node_type` when processing mixed node types.

.. _parse-html-select-elements:

DOM Selection
-------------

Resiliparse provides a basic set of standard DOM functions for selecting element nodes and attributes in the DOM tree.

Elements
^^^^^^^^
The :attr:`.HTMLTree.document` property returns a :class:`.DocumentNode`. To access the root ``<html>`` element,
use :attr:`.DocumentNode.document_element`. The properties
:attr:`.HTMLTree.body` and :attr:`.HTMLTree.head` also exist for conveniently accessing a document's ``<head>`` or
``<body>`` elements (if they exist). The head, body, and other element nodes are represented by :class:`.ElementNode`
objects, which inherit from :class:`.Node` and support the following selectors for matching further elements by
attributes or CSS selectors:

* :meth:`~.ElementNode.get_element_by_id`
* :meth:`~.ElementNode.get_elements_by_tag_name`
* :meth:`~.ElementNode.get_elements_by_class_name`
* :meth:`~.ElementNode.query_selector`
* :meth:`~.ElementNode.query_selector_all`
* :meth:`~.ElementNode.matches`

These element selectors behave just like you would expect from other languages or libraries and return either a single
:class:`~.ElementNode` object (or ``None``) or an :class:`.ElementNodeList` of matching elements. The exception is
:meth:`~.ElementNode.matches`, which returns a boolean value indicating whether the element itself matches the given CSS
selector, following DOM ``Element.matches()`` semantics. In addition to these standard DOM functions, Resiliparse
provides :meth:`~.ElementNode.get_elements_by_attr` for selecting elements by arbitrary attribute names and values.

.. versionchanged:: 2.0.0

   ``node.matches(selector)`` checks the element itself instead of searching its
   descendants. To retain the previous descendant-search behaviour, use
   ``node.query_selector(selector) is not None``.

.. note::

  If you want to match only a single element, always use the dedicated single-match selectors (e.g., use
  :meth:`~.ElementNode.query_selector` instead of :meth:`~.ElementNode.query_selector_all` etc.). These functions stop
  after the first match and are therefore more efficient than matching all elements and discarding unwanted results.

Here are a few examples of how to match elements by ID, tag name, class name, or CSS selector:

.. code-block:: python

    # Match single node by ID:
    print(repr(tree.body.get_element_by_id('foo')))
    # >>> Element(<main id="foo">)

    # Match multiple nodes by tag name:
    print(repr(tree.head.get_elements_by_tag_name('meta')))
    # >>> [Element(<meta charset="utf-8">)]

    # Match multiple nodes by class name:
    print(repr(tree.body.get_elements_by_class_name('bar')))
    # >>> [Element(<span class="bar">), Element(<span class="bar baz">)]

    # Match single node by CSS selector:
    print(repr(tree.document.query_selector('body > main p:last-child')))
    # >>> Element(<p id="b" class="dom">)

    # Match multiple nodes by CSS selector:
    print([node.tag for node in tree.body.query_selector_all('main *')])
    # >>> ['p', 'span', 'p', 'span']

    # Check whether the element itself matches this CSS selector:
    print(tree.body.matches('body'))
    # >>> True
    print(tree.body.matches('.bar'))
    # >>> False

    # Check whether any descendant matches this CSS selector:
    print(tree.body.query_selector('.bar') is not None)
    # >>> True

:class:`.ElementNodeList` objects are iterable, indexable, and slicable. The size of a collection can be checked with
``len()``. Slices return tuples, so collection selection methods are available on the original collection,
but not on a slice:

.. code-block:: python

    coll = tree.body.query_selector_all('main *')

    # First element
    print(repr(coll[0]))
    # >>> Element(<p id="a">)

    # Last element
    print(repr(coll[-1]))
    # >>> Element(<span class="bar baz">)

    # First two elements
    print(repr(coll[:2]))
    # >>> (Element(<p id="a">), Element(<span class="bar">))

:class:`.ElementNodeList` objects provide selection methods similar to those on :class:`.ElementNode`. These methods
search the descendants of the elements in the collection and return either a single :class:`.ElementNode` (or ``None``)
or another :class:`.ElementNodeList`:

.. code-block:: python

    coll = tree.body.get_elements_by_class_name('dom')

    # Search within the subtrees of elements in `coll`:
    print(repr(coll.get_elements_by_class_name('bar')))
    # >>> [Element(<span class="bar baz">)]


.. versionchanged:: 2.0.0

   ``collection.matches(selector)`` checks whether any of the collection's
   elements themselves match, instead of searching their descendants. Use
   ``collection.query_selector(selector) is not None`` for the previous behaviour.
   Collection ``matches()`` is a Resiliparse extension, not a standard DOM method.


.. _parse-html-attributes:

Attributes
^^^^^^^^^^

Attributes of element nodes can be accessed either via :meth:`.ElementNode.get_attribute` or by dict-like access.
The method returns ``None`` (or the supplied default) for a missing attribute:

.. code-block:: python

    meta = tree.head.query_selector('meta[charset]')
    if meta is not None:
        print(meta.get_attribute('charset'))
        # >>> utf-8

        # Or:
        print(meta['charset'])
        # >>> utf-8

The dict access method will raise a :exc:`KeyError` exception if the attribute does not exist.

The ``id`` and ``class`` attributes of an element are also available through the :attr:`~.ElementNode.id` and
:attr:`~.ElementNode.class_name` or :attr:`~.ElementNode.class_list` properties:

.. code-block:: python

    p = tree.body.get_element_by_id('b')
    print(p.id)
    # >>> b

    span = p.query_selector('span')
    print(span.class_name)
    # >>> bar baz

    print(span.class_list)
    # >>> ['bar', 'baz']

A tuple of existing attribute names on an element is provided by its :attr:`~.ElementNode.attribute_names` property:

.. code-block:: python

    a = tree.create_element('div')

    a.id = 'a-id'
    a.class_name = 'a-class'
    a['href'] = 'https://example.com'

    print(a.attribute_names)
    # >>> ('id', 'class', 'href')

The :attr:`.ElementNode.attributes` property provides a :class:`.NamedNodeMap` of :class:`.AttrNode` objects instead.
The :attr:`.ElementNode.class_list` property is a :class:`.DOMTokenList`, with ``add()``, ``remove()``, ``replace()``,
and ``toggle()`` methods for modifying class names. Use ``list(element.class_list)`` to obtain a Python list.


.. _parse-html-text-serialization:

HTML and Text Serialization
^^^^^^^^^^^^^^^^^^^^^^^^^^^

Use :attr:`.Node.text_content` for the text contents of a node and :attr:`.ElementNode.outer_html` for an
element's HTML serialisation. :attr:`.ElementNode.inner_html` returns only the markup inside the element.
``text_content`` can be ``None`` for nodes without text content, such as the document node:

.. code-block:: python

    print(tree.body.get_element_by_id('a').text_content)
    # >>> Hello world!

    print(tree.body.get_element_by_id('a').outer_html)
    # >>> <p id="a">Hello <span class="bar">world</span>!</p>

Alternatively, you can also simply cast a :class:`.Node` to ``str``, which is its HTML serialisation:

.. code-block:: python

    print(tree.body.get_element_by_id('a'))
    # >>> <p id="a">Hello <span class="bar">world</span>!</p>

For extracting specifically the text contents of the document's ``<title>`` element, there is also the
:attr:`.HTMLTree.title` property:

.. code-block:: python

    # Example page
    print(tree.title)


.. _parse-html-traversal:

DOM Tree Traversal
------------------

The DOM subtree of any node can be traversed in pre-order by iterating over a :class:`.Node` instance. Different
types of nodes can be distinguished by their :attr:`~.Node.node_type` property.

.. code-block:: python

    from resiliparse.parse.html import NodeType

    root = tree.body.get_element_by_id('a')

    tag_names = [e.tag for e in root]
    tag_names_elements_only = [e.tag for e in root if e.node_type == NodeType.ELEMENT]

    print(tag_names)
    # >>> ['p', '#text', 'span', '#text', '#text']

    print(tag_names_elements_only)
    # >>> ['p', 'span']

To iterate only the immediate children of a node, loop over its :attr:`~.Node.child_nodes` property instead of the
node itself:

.. code-block:: python

    for e in tree.body.get_element_by_id('foo').child_nodes:
        if e.node_type == NodeType.ELEMENT:
            print(e.text)

Output:

::

    Hello world!
    Hello DOM!

Use :attr:`.ElementNode.children` to iterate only immediate element children, omitting text and comment nodes.

In addition, any :class:`.Node` object also has the following properties:

* :attr:`~.Node.first_child`
* :attr:`~.Node.last_child`
* :attr:`~.Node.prev`
* :attr:`~.Node.next`
* :attr:`~.Node.parent`

These can be used for traversing in a custom order or with custom logic, though the callback-based traversal mechanism
described in the next section :ref:`parse-html-traversal-advanced` is usually more convenient.


.. _parse-html-traversal-advanced:

Advanced Traversal
^^^^^^^^^^^^^^^^^^
Besides the simple iterable interface, Resiliparse also supports a more advanced and flexible callback-based method for
traversing the DOM tree. The :func:`.traverse_dom` helper function accepts a :class:`.Node` and a callback function
that is called for each individual DOM node with a :class:`.DOMContext` context object as its only parameter.

The following example prints the values of all text nodes:

.. code-block:: python

    from resiliparse.parse.html import *

    def start_cb(ctx: DOMContext):
        if ctx.node.node_type == NodeType.TEXT and ctx.node.value.strip():
            print(ctx.node.value.strip(), end=' ')

    traverse_dom(tree.body, start_cb)

Output:

::

    Hello world ! Hello DOM !

In addition to the start element callback, you can also specify an end element callback that will be invoked every time
the DOM tree traverser encounters an element's end tag (i.e., every time the tree traverser goes up one node level):

The following example prints all start and end tags without their textual contents or attributes:

.. code-block:: python

    def start_cb(ctx: DOMContext):
        if ctx.node.node_type == NodeType.ELEMENT:
            print(f'<{ctx.node.tag}>', end='')

    def end_cb(ctx: DOMContext):
        if ctx.node.node_type == NodeType.ELEMENT:
            print(f'</{ctx.node.tag}>', end='')

    traverse_dom(tree.body, start_cb, end_cb)

Output:

.. code-block:: html

    <body><main><p><span></span></p><p><span></span></p></main></body>

Besides a reference to the current node, the context objects also keeps track of the traversal depth, so the following
is possible:

.. code-block:: python

    def start_cb(ctx: DOMContext):
        if ctx.node.node_type == NodeType.ELEMENT:
            print(f'{"  " * ctx.depth}<{ctx.node.tag}>')
        if ctx.node.node_type == NodeType.TEXT and ctx.node.value.strip():
            print(f'{"  " * ctx.depth}{ctx.node.text.strip()}')

    def end_cb(ctx: DOMContext):
        if ctx.node.node_type == NodeType.ELEMENT:
            print(f'{"  " * ctx.depth}</{ctx.node.tag}>')

    traverse_dom(tree.body, start_cb, end_cb)

Output:

.. code-block:: html

    <body>
      <main>
        <p>
          Hello
          <span>
            world
          </span>
          !
        </p>
        <p>
          Hello
          <span>
            DOM
          </span>
          !
        </p>
      </main>
    </body>

The context object is the same object throughout the whole traversal process, so besides the node ``node`` and ``depth``
attributes, it can be mutated arbitrarily to maintain your own state. If you need to, you can also pass a
pre-initialized context object to :func:`.traverse_dom`. The following example converts the HTML body into nested Python
lists:

.. code-block:: python

    def start_cb(ctx: DOMContext):
        if ctx.node.node_type == NodeType.ELEMENT:
            t = (ctx.node.tag, [])
            ctx.list_stack_current[-1].append(t)
            ctx.list_stack_current.append(t[1])
        elif ctx.node.node_type == NodeType.TEXT:
            txt = ctx.node.value.strip()
            if txt:
                ctx.list_stack_current[-1].append(txt)

    def end_cb(ctx: DOMContext):
        if ctx.node.node_type == NodeType.ELEMENT:
            ctx.list_stack_current.pop()

    ctx = DOMContext()
    ctx.list_stack = []
    ctx.list_stack_current = [ctx.list_stack]
    traverse_dom(tree.body, start_cb, end_cb, ctx)

    print(ctx.list_stack)

Output:

.. code-block:: python

    [('body', [('main', [('p', ['Hello', ('span', ['world']), '!']), ('p', ['Hello', ('span', ['DOM']), '!'])])])]

.. _parse-html-manipulate:

DOM Tree Manipulation
---------------------

Resiliparse supports DOM manipulation and the creation of new nodes with a basic set of well-known DOM functions.

Elements
^^^^^^^^
In the following is an example of how you can create new DOM elements and text nodes and insert them into the tree:

.. code-block:: python

    # Create a new <p> element node
    new_element = tree.create_element('p')

    # Create a new text node
    new_text = tree.create_text_node('Hello Resiliparse!')

    # Insert nodes into DOM tree
    main_element = tree.body.query_selector('main')
    main_element.append_child(new_element)
    new_element.append_child(new_text)

    print(main_element)

Output:

.. code-block:: html

    <main id="foo">
          <p id="a">Hello <span class="bar">world</span>!</p>
          <p id="b" class="dom">Hello <span class="bar baz">DOM</span>!</p>
         <p>Hello Resiliparse!</p></main>

In addition to :meth:`~.Node.append_child`, nodes also provide :meth:`~.Node.insert_before` for inserting a child
node before another child instead of appending it at the end, and :meth:`~.Node.replace_child` for replacing an
existing child node in the tree with another.

Elements also support :meth:`.ElementNode.append`, :meth:`.ElementNode.prepend`, and
:meth:`.ElementNode.replace_children` for inserting several nodes at once. The methods :meth:`.ElementNode.before`,
:meth:`.ElementNode.after`, :meth:`.ElementNode.replace_with`, and :meth:`.ElementNode.remove` operate relative to an
element's position in its parent. :meth:`.Node.clone_node` makes a detached copy; pass ``deep=True`` to copy the subtree
as well. Use :meth:`.DocumentNode.create_document_fragment` to construct a fragment before inserting its contents.

Use :meth:`~.Node.remove_child` to remove a node from the tree:

.. code-block:: python

    main_element.remove_child(new_element)

To fully delete a node, use :meth:`~.Node.decompose()` on the node itself. This will remove it from the tree (if not
already done) and delete the node and its entire subtree recursively:

.. code-block:: python

    new_element.decompose()
    # From here on, this element and all elements in its subtree are invalid!!!

Attributes
^^^^^^^^^^
Attributes can be added or modified via :meth:`~.ElementNode.set_attribute` or by assigning directly to its dict entry.
Use :meth:`~.ElementNode.remove_attribute` or ``del element['name']`` to remove an attribute:

.. code-block:: python

    element = tree.create_element('img')
    element['src'] = 'https://example.com/foo.png'
    element.set_attribute('alt', 'Foo')

    print(element)
    # >>> <img src="https://example.com/foo.png" alt="Foo">

For ``id`` and ``class`` attributes, you can also use :attr:`~.ElementNode.id` and :attr:`~.ElementNode.class_name` or
:attr:`~.ElementNode.class_list`:

.. code-block:: python

    element = tree.create_element('div')

    element.id = 'my-id'
    element.class_name = 'class-a'
    element.class_list.add('class-b')

    print(element)
    # >>> <div id="my-id" class="class-a class-b"></div>

    print(element.class_list)
    # >>> ['class-a', 'class-b']

    element.class_list.remove('class-a')
    print(element)
    # >>> <div id="my-id" class="class-b"></div>


Inner HTML and Inner Text
^^^^^^^^^^^^^^^^^^^^^^^^^
An easier, but less efficient way of manipulating the DOM is to assign a string directly to either its
:attr:`~.ElementNode.inner_html` or :attr:`~.Node.text_content` property. This replaces the element's children
with parsed markup or a text node, respectively:

.. code-block:: python

    main_element.inner_html = '<p>New inner HTML content</p>'
    print(main_element)
    # >>> <main id="foo"><p>New inner HTML content</p></main>

    main_element.text_content = '<p>New inner text content</p>'
    print(main_element)
    # >>> <main id="foo">&lt;p&gt;New inner text content&lt;/p&gt;</main>


.. _parse-html-benchmark:


Benchmarks
----------

The :ref:`resiliparse-cli` parser comes with a small HTML parser benchmarking tool that can measure the parsing engine's
performance and compare it to other Python HTML parsing libraries. Supported third-party libraries are `Selectolax
<https://github.com/rushter/selectolax>`__ (Lexbor engine) and `BeautifulSoup4
<https://www.crummy.com/software/BeautifulSoup/bs4/doc/>`__ (lxml engine only, which is the fastest BS4 backend).

Here are the results of extracting the titles from all web pages in an uncompressed 42,015-document WARC file on a Ryzen
Threadripper 2920X machine:

.. code-block:: console

    $ resiliparse html benchmark CC-MAIN-*.warc

    HTML parser benchmark <title> extraction:
    =========================================
    Resiliparse (Lexbor):  42015 documents in 36.55s (1149.56 documents/s)
    Selectolax (Lexbor):   42015 documents in 37.46s (1121.52 documents/s)
    BeautifulSoup4 (lxml): 42015 documents in 874.40s (48.05 documents/s)

Not surprisingly, the two parsers based on the Lexbor engine perform almost identically, whereas lxml is by far the
slowest by a factor of 24x.
