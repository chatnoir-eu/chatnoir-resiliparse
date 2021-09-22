import os
import pytest
import html as pyhtml

from fastwarc.warc import ArchiveIterator, WarcRecordType
from fastwarc.stream_io import FileStream
from resiliparse.parse.encoding import detect_encoding
from resiliparse.parse.html import *


DATA_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', 'data'))

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
  </body>
</html>"""


tree = None


def validate_document():
    assert tree is not None

    assert type(tree.document) is DOMNode
    assert tree.document.type == DOCUMENT
    assert tree.document.tag == '#document'
    assert repr(tree.document) == '[HTML Document]'
    assert tree.document.first_child.type == DOCUMENT_TYPE
    assert repr(tree.document.first_child) == '<!DOCTYPE html>'
    assert str(tree) == tree.document.html

    assert type(tree.head) is DOMNode
    assert tree.head.type == ELEMENT
    assert tree.head.tag == 'head'
    assert repr(tree.head) == '<head>'
    assert str(tree.head) == tree.head.html
    assert str(tree.head).startswith('<head>')
    assert str(tree.head).endswith('</head>')

    assert type(tree.body) is DOMNode
    assert tree.body.type == ELEMENT
    assert tree.body.tag == 'body'
    assert repr(tree.body) == '<body>'
    assert str(tree.body) == tree.body.html
    assert str(tree.body).startswith('<body>')
    assert str(tree.body).endswith('</body>')

    assert tree.title == 'Example page'


def test_parse():
    global tree
    tree = HTMLTree.parse(html)
    assert tree is not None

    validate_document()


def test_parse_from_bytes():
    global tree
    tree = HTMLTree.parse_from_bytes(html.encode('utf-16'), 'utf-16')
    assert tree is not None
    validate_document()


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
    assert tree_quirk.title == ''
    assert tree_quirk.body is not None
    assert len(tree_quirk.body.child_nodes) == 0

    tree_quirk = HTMLTree.parse(html_no_title_svg)
    assert tree_quirk.head is not None
    assert tree_quirk.title == ''
    assert tree_quirk.body is not None

    tree_quirk = HTMLTree.parse(html_unclosed_head)
    assert tree_quirk.head is not None
    assert len(tree_quirk.head.child_nodes) == 1
    assert tree_quirk.title == 'Title'
    assert tree_quirk.body is not None
    assert len(tree_quirk.body.child_nodes) == 1


def test_node_equality():
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
    assert tree.body.get_element_by_id('foo').tag == 'main'

    meta = tree.head.get_elements_by_tag_name('meta')
    assert type(meta) is DOMCollection
    assert len(meta) == 1
    assert meta[0].tag == 'meta'

    bar_class = tree.body.get_elements_by_class_name('bar')
    assert type(bar_class) is DOMCollection
    assert len(bar_class) == 2
    assert bar_class[0].tag == 'span'
    assert bar_class[1].tag == 'a'

    lang_en = tree.document.get_elements_by_attr('lang', 'en')
    assert (type(lang_en)) is DOMCollection
    assert len(lang_en) == 1
    assert lang_en[0].hasattr('lang')
    assert lang_en[0].tag == 'html'

    match_css = tree.document.query_selector('body > main p:last-child')
    assert type(match_css) is DOMNode
    assert match_css.tag == 'p'

    match_css_all = tree.body.query_selector_all('main *')
    assert type(match_css_all) is DOMCollection
    assert len(match_css_all) == 4
    assert match_css_all[0].tag == 'p'
    assert match_css_all[1].tag == 'span'
    assert match_css_all[2].tag == 'p'
    assert match_css_all[3].tag == 'a'

    # Check whether there is any element matching this CSS selector:
    assert tree.body.matches('.bar')
    assert not tree.body.matches('.barbaz')


def test_collection():
    coll = tree.body.query_selector_all('main *')

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

    assert coll.get_element_by_id('abc') is None
    assert coll.get_elements_by_class_name('bar')[0] is coll.query_selector('.bar')
    assert len(coll.get_elements_by_attr('href', 'https://example.com')) == 1
    assert len(coll.get_elements_by_tag_name('span')) == 1

    assert coll.query_selector('.bar').tag == 'span'
    assert len(coll.query_selector_all('span, a')) == 2

    assert coll.matches('.bar.baz')
    assert not coll.matches('.foo.bar.baz')


def test_attributes():
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

    # TODO: Uncomment these tests once https://github.com/lexbor/lexbor/issues/139 has been released.
    # assert a.getattr('id') is None
    # assert a.getattr('id', 'default') == 'default'
    # assert a.id == ''
    # a.id = 'abc'
    # assert a.id == 'abc'
    # assert a['id'] == 'abc'
    # assert a.getattr('id') == 'abc'

    with pytest.raises(KeyError):
        # noinspection PyStatementEffect
        a['lang']

    assert a.getattr('lang') is None
    a['lang'] = 'en'
    assert a['lang'] == 'en'
    assert a.getattr('lang') == 'en'

    # TODO: These, too
    # assert len(a.attrs) == 4
    # assert a.attrs == ['href', 'class', 'id', 'lang']

    del a['lang']
    assert a.getattr('lang') is None


def test_serialization():
    assert tree.body.get_element_by_id('a').text == 'Hello world!'
    assert tree.body.get_element_by_id('a').html == '<p id="a">Hello <span class="bar">world</span>!</p>'

    assert str(tree.head) == tree.head.html
    assert repr(tree.head) == '<head>'
    assert repr(tree.head.query_selector('title')) == '<title>'
    assert str(tree.head.query_selector('title')) == '<title>Example page</title>'

    assert str(tree.body) == tree.body.html
    assert repr(tree.body) == '<body>'
    assert repr(tree.body.query_selector('main')) == '<main id="foo">'

    text = tree.body.query_selector('#b').first_child
    assert text.type == TEXT
    assert repr(text) == str(text) == text.text


def test_traversal():
    root = tree.body.get_element_by_id('a')

    tag_names = [e.tag for e in root]
    tag_names_elements_only = [e.tag for e in root if e.type == ELEMENT]

    assert tag_names == ['p', '#text', 'span', '#text', '#text']
    assert tag_names_elements_only == ['p', 'span']

    child_node_tags = [e.tag for e in tree.body.get_element_by_id('foo').child_nodes]
    assert child_node_tags == ['#text', 'p', '#text', 'p', '#text']

    child_node_types = [e.type for e in tree.body.get_element_by_id('foo').child_nodes]
    assert child_node_types == [TEXT, ELEMENT, TEXT, ELEMENT, TEXT]


def test_children():
    element = tree.body.get_element_by_id('a')

    assert element.first_child.parent is element
    assert element.last_child.parent is element
    assert element.first_child.next is element.last_child.prev

    assert element.first_child.type == TEXT
    assert element.first_child.text == 'Hello '
    assert element.last_child.type == TEXT
    assert element.last_child.text == '!'

    assert element.first_child.next.type == ELEMENT
    assert element.first_child.next.tag == 'span'
    assert element.first_child.next.class_name == 'bar'

    assert element.last_child.prev.type == ELEMENT
    assert element.last_child.prev.tag == 'span'
    assert element.last_child.prev.class_name == 'bar'


def test_dom_manipulation():
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
    assert repr(new_element) == '<INVALID ELEMENT>'


def test_inner_html_and_text():
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
    for rec in ArchiveIterator(FileStream(os.path.join(DATA_DIR, 'warcfile.warc')),
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
