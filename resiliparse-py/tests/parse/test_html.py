import os
import pytest
import html as pyhtml

from fastwarc.warc import ArchiveIterator, WarcRecordType
from fastwarc.stream_io import FileStream
from resiliparse.parse.encoding import detect_encoding
from resiliparse.parse.html import *

DATA_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', '..', 'test-data'))

html = """<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <title>Example page</title>
  </head>
  <body>
    <main id="foo">
      <p id="a">Hello <span class="bar">world</span>!</p>
      <p id="b" class="dom">Hello <a href="https://example.com" class="bar baz">DOM</a>!</p>
     </main>
     <!-- A comment -->
  </body>
</html>"""


def validate_document(tree):
    assert tree is not None

    assert type(tree.document) is DocumentNode
    assert tree.document.type == DOCUMENT
    assert tree.document.tag == '#document'
    assert repr(tree.document) == 'Document(#document)'
    assert tree.document.first_child.type == DOCUMENT_TYPE
    assert repr(tree.document.first_child) == 'DocumentType(html)'
    assert str(tree) == tree.document.html

    assert type(tree.head) is ElementNode
    assert tree.head.type == ELEMENT
    assert tree.head.tag == 'head'
    assert repr(tree.head) == 'Element(<head>)'
    assert str(tree.head) == tree.head.html
    assert str(tree.head).startswith('<head>')
    assert str(tree.head).endswith('</head>')

    assert type(tree.body) is ElementNode
    assert tree.body.type == ELEMENT
    assert tree.body.tag == 'body'
    assert repr(tree.body) == 'Element(<body>)'
    assert str(tree.body) == tree.body.html
    assert str(tree.body).startswith('<body>')
    assert str(tree.body).endswith('</body>')

    assert tree.title == 'Example page'


def test_parse():
    tree = HTMLTree.parse(html)
    validate_document(tree)


def test_parse_from_bytes():
    tree = HTMLTree.parse_from_bytes(html.encode('utf-16'), 'utf-16')
    assert tree is not None
    validate_document(tree)


html_no_head = """<!doctype html><body><span></span></body>"""
html_no_body = """<!doctype html><head><title>Title</title></head>"""
html_no_title = """<!doctype html><head></head></body>"""
html_no_title_svg = """<!doctype html><svg xmlns="http://www.w3.org/2000/svg"><title>SVG Title</title></svg>"""
html_unclosed_head = """<!doctype html><head><title>Title</title><span></span>"""


# noinspection DuplicatedCode
def test_parse_quirks():
    tree_quirk = HTMLTree.parse(html_no_head)
    assert tree_quirk.head is not None
    assert len(tree_quirk.head.child_nodes) == 0
    assert tree_quirk.body is not None
    assert len(tree_quirk.body.child_nodes) == 1

    tree_quirk = HTMLTree.parse(html_no_body)
    assert tree_quirk.head is not None
    assert len(tree_quirk.head.child_nodes) == 1
    assert tree_quirk.title == 'Title'
    assert tree_quirk.body is not None
    assert len(tree_quirk.body.child_nodes) == 0

    tree_quirk = HTMLTree.parse(html_no_title)
    assert tree_quirk.head is not None
    assert len(tree_quirk.head.child_nodes) == 0
    assert tree_quirk.title is None
    assert tree_quirk.body is not None
    assert len(tree_quirk.body.child_nodes) == 0

    tree_quirk = HTMLTree.parse(html_no_title_svg)
    assert tree_quirk.head is not None
    assert tree_quirk.title is None
    assert tree_quirk.body is not None

    tree_quirk = HTMLTree.parse(html_unclosed_head)
    assert tree_quirk.head is not None
    assert len(tree_quirk.head.child_nodes) == 1
    assert tree_quirk.title == 'Title'
    assert tree_quirk.body is not None
    assert len(tree_quirk.body.child_nodes) == 1


def test_node_equality():
    tree = HTMLTree.parse(html)

    assert tree.body is not tree.head
    assert tree.body != tree.head
    assert tree.body is tree.body
    assert tree.body == tree.body

    a1 = tree.body.query_selector('#a')
    a2 = tree.body.query_selector('#a')
    b1 = tree.body.query_selector('#b')
    b2 = tree.body.query_selector('#b')

    assert a1 is not b1
    assert a1 != b1

    assert a2 is not b2
    assert a2 != b2

    assert a1 is a2
    assert a1 == a2
    assert hash(a1) == hash(a2)

    assert b2 is b2
    assert b2 == b2
    assert hash(b1) == hash(b2)


def test_selection():
    tree = HTMLTree.parse(html)

    assert tree.body.get_element_by_id('foo').tag == 'main'

    meta = tree.head.get_elements_by_tag_name('meta')
    assert isinstance(meta, DOMCollection)
    assert len(meta) == 1
    assert meta[0].tag == 'meta'

    bar_class = tree.body.get_elements_by_class_name('bar')
    assert isinstance(bar_class, DOMCollection)
    assert len(bar_class) == 2
    assert bar_class[0].tag == 'span'
    assert bar_class[1].tag == 'a'

    lang_en = tree.document.get_elements_by_attr('lang', 'en')
    assert isinstance(lang_en, DOMCollection)
    assert len(lang_en) == 1
    assert lang_en[0].hasattr('lang')
    assert lang_en[0].tag == 'html'

    match_css = tree.document.query_selector('body > main p:last-child')
    assert type(match_css) is ElementNode
    assert match_css.tag == 'p'

    match_css_all = tree.body.query_selector_all('main *')
    assert isinstance(match_css_all, DOMCollection)
    assert len(match_css_all) == 4
    assert match_css_all[0].tag == 'p'
    assert match_css_all[1].tag == 'span'
    assert match_css_all[2].tag == 'p'
    assert match_css_all[3].tag == 'a'

    # Check whether there is any element matching this CSS selector:
    assert tree.body.matches('body')
    assert not tree.body.matches('.bar')
    assert tree.body.query_selector('.bar') is not None
    assert tree.body.query_selector('.barbaz') is None

    # Invalid CSS selector
    with pytest.raises(ValueError):
        tree.body.query_selector('..abc')


def test_collection():
    tree = HTMLTree.parse(html)

    coll = tree.body.query_selector_all('main *')
    assert coll.item(0) is coll[0]
    assert coll.item(len(coll)) is None
    assert coll.values() == list(coll)
    assert isinstance(repr(coll), str)

    # Basic element attributes
    assert coll[0].id == 'a'
    assert coll[-1].class_name == 'bar baz'
    assert len(coll[:2]) == 2
    assert coll[:2][0].id == 'a'
    assert coll[:2][1].class_name == 'bar'

    # Iteration
    count = 0
    for el in coll:
        assert el.tag
        count += 1
    assert count == len(coll)

    # Collection match forwarding
    coll = tree.body.query_selector_all('p')

    # Negative indexing
    assert coll[-1] is coll[len(coll) - 1]
    assert coll[-2] is coll[0]

    assert coll.get_element_by_id('abc') is None
    assert coll.get_elements_by_class_name('bar')[0] is coll.query_selector('.bar')
    assert coll.get_elements_by_class_name('bar')[-1] is coll.query_selector('.bar.baz')
    assert len(coll.get_elements_by_attr('href', 'https://example.com')) == 1
    assert len(coll.get_elements_by_tag_name('span')) == 1

    assert coll.query_selector('.bar').tag == 'span'
    assert len(coll.query_selector_all('span, a')) == 2

    assert coll.matches('p.dom')
    assert not coll.matches('.bar.baz')
    assert coll.query_selector('.bar.baz') is not None
    assert coll.query_selector('.foo.bar.baz') is None


def test_attributes():
    tree = HTMLTree.parse(html)

    a = tree.body.query_selector('#b a')
    assert a.hasattr('class')
    assert a.class_name == 'bar baz'
    assert len(a.class_list) == 2
    assert a.class_list == ['bar', 'baz']

    a.class_list.add('abc')
    assert len(a.class_list) == 3
    assert a.class_list == ['bar', 'baz', 'abc']
    assert a.class_name == 'bar baz abc'
    a.class_list.remove('baz')
    assert a.class_list == ['bar', 'abc']
    assert a.class_name == 'bar abc'

    assert a.getattr('id') is None
    assert a.getattr('id', 'default') == 'default'
    assert a.id is None
    a.id = 'abc'
    assert a.id == 'abc'
    assert a['id'] == 'abc'
    assert a.getattr('id') == 'abc'

    with pytest.raises(KeyError):
        # noinspection PyStatementEffect
        a['lang']

    assert a.getattr('lang') is None
    a['lang'] = 'en'
    assert a['lang'] == 'en'
    assert a.getattr('lang') == 'en'

    assert len(a.attrs) == 4
    assert a.attrs == ('href', 'class', 'id', 'lang')

    del a['lang']
    assert a.getattr('lang') is None


def test_dom_token_list_python_sequence_and_mutation_methods():
    element = HTMLTree.parse('<div class="one two"></div>').body.first_element_child
    tokens = element.class_list

    assert tokens.value == 'one two'
    assert tokens.values() == ['one', 'two']
    assert tokens.item(0) == 'one'
    assert tokens.item(2) is None
    assert tokens[1] == 'two'
    assert tokens[:] == ('one', 'two')
    assert tokens.contains('one')
    assert 'two' in tokens
    assert len(tokens) == 2
    assert tokens.replace('two', 'three')
    assert not tokens.replace('missing', 'ignored')
    assert tokens.toggle('three', False) is False
    assert tokens.toggle('four', True) is True
    assert tokens.toggle('one') is False
    assert tokens == ['four']
    assert repr(tokens) == "['four']"
    tokens.value = 'alpha beta'
    assert tokens.values() == ['alpha', 'beta']


def test_element_attribute_selector_and_markup_properties():
    tree = HTMLTree.parse('<main><div id="target" data-kind="one"><span>text</span></div></main>')
    element = tree.body.query_selector('#target')

    assert element.closest('main').tag == 'main'
    assert element.closest('article') is None
    assert element.attribute_names == ('id', 'data-kind')
    assert [attr.name for attr in element.attributes] == ['id', 'data-kind']
    attribute = element.attribute_node('data-kind')
    assert attribute.owner_element == element
    assert attribute.value == 'one'
    attribute.value = 'two'
    assert element.get_attribute('data-kind') == 'two'
    assert element.has_attribute('data-kind')
    assert element.toggle_attribute('hidden')
    assert element.toggle_attribute('hidden', True)
    assert not element.toggle_attribute('hidden', False)
    element.set_attribute('title', 'caption')
    assert element.get_attribute('title') == 'caption'
    element.remove_attribute('title')
    assert not element.has_attribute('title')

    assert element.outer_html == '<div id="target" data-kind="two"><span>text</span></div>'
    assert element.inner_text == 'text'
    element.inner_text = 'replacement'
    assert element.inner_html == 'replacement'
    assert element.outer_text == 'replacement'
    element.outer_html = '<section id="replacement">next</section>'
    replacement = tree.body.query_selector('#replacement')
    assert replacement.outer_html == '<section id="replacement">next</section>'
    replacement.outer_text = 'plain'
    assert tree.body.inner_text.strip() == 'plain'


def test_document_factories_and_character_data_binding_methods():
    document = HTMLTree.parse('<!doctype html><html><body></body></html>').document
    assert document.doctype.name == 'html'
    assert document.document_element.tag == 'html'

    fragment = document.create_document_fragment()
    element = document.create_element('div')
    text = document.create_text_node('abcd')
    comment = document.create_comment('note')
    cdata = document.create_cdata_section('raw')
    instruction = document.create_processing_instruction('target', 'data')
    attribute = document.create_attribute('title')

    assert fragment.type == NodeType.DOCUMENT_FRAGMENT
    assert fragment.append_child(element) is element
    assert element.parent is fragment
    assert text.data == 'abcd'
    assert len(text) == 4
    assert not text.is_empty()
    assert text.substring_data(1, 2) == 'bc'
    text.insert_data(2, 'X')
    text.delete_data(1, 1)
    text.replace_data(1, 1, 'Y')
    text.append_data('!')
    assert text.data == 'aYcd!'
    assert comment.data == 'note'
    assert cdata.data == 'raw'
    assert instruction.target == 'target'
    assert instruction.data == 'data'
    assert attribute.name == attribute.local_name == 'title'
    assert attribute.owner_element is None
    attribute.value = 'caption'
    assert attribute.value == 'caption'


@pytest.mark.parametrize('markup, expected', [
    ('<input>', None),
    ('<input checked>', ''),
    ('<input checked="">', ''),
    ('<input checked="checked">', 'checked'),
])
def test_get_attribute_boolean_value(markup, expected):
    element = HTMLTree.parse(markup).body.query_selector('input')
    assert element.get_attribute('checked') == expected
    assert element.get_attribute('checked', 'fallback') == (expected if expected is not None else 'fallback')


def test_empty_attributes():
    input_tree = HTMLTree.parse('''<div>
    <input type="checkbox" checked>
    <div class="foo"></div>
    <div class></div>
    <div class=""></div>
    <div id="foo"></div>
    <div id></div>
    <div id=""></div>
    <div foo></div>
    <div foo=""></div>''')

    input_element = input_tree.body.query_selector('input')

    assert input_element.hasattr('type')
    assert input_element['type'] == 'checkbox'

    assert input_element.hasattr('checked')
    assert input_element['checked'] == ''

    assert not input_element.hasattr('checkedx')
    with pytest.raises(KeyError):
        assert input_element['checkedx']

    # Test empty attribute selection
    # There used to be a Lexbor crash with this: https://github.com/lexbor/lexbor/pull/148
    assert len(input_tree.body.query_selector_all('.foo')) == 1
    assert len(input_tree.body.query_selector_all('#foo')) == 1
    assert len(input_tree.body.query_selector_all('[class]')) == 3
    assert len(input_tree.body.query_selector_all('[id]')) == 3
    assert input_tree.body.get_element_by_id('foo') is not None
    assert input_tree.body.get_element_by_id('foox') is None
    assert len(input_tree.body.get_elements_by_class_name('foo')) == 1
    assert len(input_tree.body.get_elements_by_class_name('')) == 0  # This doesn't match anything
    assert len(input_tree.body.get_elements_by_attr('class', 'foo')) == 1
    assert len(input_tree.body.get_elements_by_attr('class', '')) == 2
    assert len(input_tree.body.get_elements_by_attr('id', '')) == 2
    assert len(input_tree.body.get_elements_by_attr('foo', '')) == 2


def test_serialization():
    tree = HTMLTree.parse(html)

    assert tree.body.get_element_by_id('a').text == 'Hello world!'
    assert tree.body.get_element_by_id('a').html == '<p id="a">Hello <span class="bar">world</span>!</p>'

    assert str(tree.head) == tree.head.html
    assert repr(tree.head) == 'Element(<head>)'
    assert repr(tree.head.query_selector('title')) == 'Element(<title>)'
    assert str(tree.head.query_selector('title')) == '<title>Example page</title>'

    assert str(tree.body) == tree.body.html
    assert repr(tree.body) == 'Element(<body>)'
    assert repr(tree.body.query_selector('main')) == 'Element(<main id="foo">)'

    text = tree.body.query_selector('#b').first_child
    assert text.type == TEXT
    assert isinstance(text, TextNode)
    assert repr(text) == 'Text("Hello ")'
    assert str(text) == text.text


def test_traversal():
    tree = HTMLTree.parse(html)
    root = tree.body.get_element_by_id('a')

    tag_names = [e.tag for e in root]
    tag_names_elements_only = [e.tag for e in root if e.type == ELEMENT]

    assert tag_names == ['p', '#text', 'span', '#text', '#text']
    assert tag_names_elements_only == ['p', 'span']

    child_node_tags = [e.tag for e in tree.body.get_element_by_id('foo').child_nodes]
    assert child_node_tags == ['#text', 'p', '#text', 'p', '#text']

    child_node_types = [e.type for e in tree.body.get_element_by_id('foo').child_nodes]
    assert child_node_types == [TEXT, ELEMENT, TEXT, ELEMENT, TEXT]


def test_callback_traversal():
    def start_cb(ctx: DOMContext):
        if ctx.node.type == NodeType.ELEMENT:
            t = (ctx.node.tag, [])
            ctx.list_stack_current[-1].append(t)
            ctx.list_stack_current.append(t[1])
        elif ctx.node.type == NodeType.TEXT:
            txt = ctx.node.value.strip()
            if txt:
                ctx.list_stack_current[-1].append(txt)

    def end_cb(ctx: DOMContext):
        if ctx.node.type == NodeType.ELEMENT:
            ctx.list_stack_current.pop()

    ctx = DOMContext()
    ctx.list_stack = []
    ctx.list_stack_current = [ctx.list_stack]
    traverse_dom(HTMLTree.parse(html).body, start_cb, end_cb, ctx)
    assert ctx.list_stack == [('body',
                               [('main',
                                 [('p', ['Hello', ('span', ['world']), '!']),
                                  ('p', ['Hello', ('a', ['DOM']), '!'])])])]

    ctx = DOMContext()
    ctx.list_stack = []
    ctx.list_stack_current = [ctx.list_stack]
    traverse_dom(HTMLTree.parse(html).body, start_cb, end_cb, ctx, elements_only=True)
    assert ctx.list_stack == [('body', [('main', [('p', [('span', [])]), ('p', [('a', [])])])])]


def test_children():
    # <main id="foo">
    #   <p id="a">Hello <span class="bar">world</span>!</p>
    #   <p id="b" class="dom">Hello <a href="https://example.com" class="bar baz">DOM</a>!</p>
    #  </main>
    tree = HTMLTree.parse(html)
    element = tree.body.get_element_by_id('a')

    assert element.first_child.parent is element
    assert element.last_child.parent is element
    assert element.first_child.next is element.last_child.prev

    assert element.first_child.type == TEXT
    assert element.first_child.first_child is None
    assert element.first_child.text == 'Hello '
    assert element.first_element_child.type == ELEMENT
    assert element.first_element_child.text == 'world'

    assert element.last_child.type == TEXT
    assert element.last_child.text == '!'
    assert element.last_child.last_child is None
    assert element.last_element_child.type == ELEMENT
    assert element.last_element_child is element.first_element_child

    assert element.first_child.next.type == ELEMENT
    assert element.first_child.next.tag == 'span'
    assert element.first_child.next.class_name == 'bar'

    assert element.last_child.prev.type == ELEMENT
    assert element.last_child.prev.tag == 'span'
    assert element.last_child.prev.class_name == 'bar'


def test_siblings():
    # <main id="foo">
    #   <p id="a">Hello <span class="bar">world</span>!</p>
    #   <p id="b" class="dom">Hello <a href="https://example.com" class="bar baz">DOM</a>!</p>
    #  </main>
    tree = HTMLTree.parse(html)

    element1 = tree.body.get_element_by_id('foo').first_element_child
    assert element1.type == ELEMENT
    assert element1.id == 'a'

    assert element1.next.type == TEXT
    assert element1.next.text.strip() == ''
    assert element1.next_element.type == ELEMENT
    assert element1.next_element.text == 'Hello DOM!'
    assert element1.prev.prev is None
    assert element1.prev_element is None

    element2 = tree.body.get_element_by_id('foo').last_element_child
    assert element2.type == ELEMENT
    assert element2.id == 'b'

    assert element2.prev.type == TEXT
    assert element2.prev.text.strip() == ''
    assert element2.prev_element.type == ELEMENT
    assert element2.prev_element.text == 'Hello world!'
    assert element2.next.next is None
    assert element2.next_element is None

    assert element1.next_element is element2
    assert element2.prev_element is element1


def test_dom_manipulation():
    tree = HTMLTree.parse(html)

    new_element = tree.create_element('p')
    assert new_element.type == ELEMENT
    assert new_element.tag == 'p'
    assert new_element.parent is None
    assert len(new_element.child_nodes) == 0

    # Create a new text node
    new_text = tree.create_text_node('Hello Resiliparse!')
    assert new_text.type == TEXT
    assert new_text.text == 'Hello Resiliparse!'

    new_element.append_child(new_text)
    assert len(new_element.child_nodes) == 1
    assert new_element.text == new_text.text
    assert new_element.last_child is new_text

    assert len(tree.body.query_selector_all('main > *')) == 2

    main_element = tree.body.query_selector('main')
    main_element.append_child(new_element)
    assert new_element.parent is main_element
    assert len(tree.body.query_selector_all('main > *')) == 3
    assert main_element.last_child is new_element

    assert main_element.remove_child(new_element) is new_element
    assert new_element.parent is None
    assert len(tree.body.query_selector_all('main > *')) == 2

    new_element2 = tree.create_element('div')
    main_element.append_child(new_element)
    assert main_element.last_child is new_element
    main_element.replace_child(new_element2, new_element)
    assert main_element.last_child is new_element2

    main_element.insert_before(new_element, new_element2)
    assert main_element.last_child is new_element2
    assert main_element.last_child.prev is new_element

    assert main_element.remove_child(main_element.last_child) is new_element2
    assert main_element.remove_child(main_element.last_child) is new_element

    new_element.decompose()
    assert new_element.type is None
    assert repr(new_element) == 'Element(<#undef>)'


def test_node_value():
    tree = HTMLTree.parse(html)

    for node in tree.document:
        if node.type in [NodeType.TEXT, NodeType.COMMENT]:
            assert node.value == node.text
        else:
            assert node.value is None


def test_inner_html_and_text():
    tree = HTMLTree.parse(html)

    element = tree.create_element('div')
    assert element.html == '<div></div>'

    new_content = '<p>New inner content</p>'
    element.html = new_content
    assert element.html == f'<div>{new_content}</div>'

    element.text = new_content
    assert element.text == new_content
    assert element.html == f'<div>{pyhtml.escape(new_content)}</div>'

    text = tree.create_text_node('xyz')
    assert text.text == 'xyz'
    text.text = 'abc'
    assert text.text == 'abc'


def test_real_world_data():
    count = 0
    for rec in ArchiveIterator(os.path.join(DATA_DIR, 'warcfile.warc'),
                               parse_http=True, record_types=WarcRecordType.response):
        content = rec.reader.read()
        tree = HTMLTree.parse_from_bytes(content, rec.http_charset or detect_encoding(content))
        assert tree.document
        assert tree.head
        assert tree.body
        assert tree.title
        assert tree.head.query_selector('style, link')
        assert tree.body.query_selector('div')

        count += 1

    assert count == 16


@pytest.mark.parametrize('document, expected', [
    ('<head></head>', None),
    ('<title></title>', None),
    ('<title> </title>', ''),
    ('<title>Example</title>', 'Example'),
])
def test_title_missing_vs_empty(document, expected):
    assert HTMLTree.parse(document).title == expected


@pytest.mark.parametrize('attribute, property_name', [('id', 'id'), ('class', 'class_name')])
def test_attribute_missing_vs_empty(attribute, property_name):
    tree = HTMLTree.parse(f'<div></div><div {attribute}=""></div>')
    elements = tree.body.query_selector_all('div')
    assert getattr(elements[0], property_name) is None
    assert getattr(elements[1], property_name) == ''
