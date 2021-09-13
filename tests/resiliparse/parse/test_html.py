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
      <p id="b" class="dom">Hello <span class="bar baz">DOM</span>!</p>
     </main>
  </body>
</html>"""


tree = None


def test_parse():
    global tree

    tree = HTMLTree.parse(html)
    assert tree is not None


def test_document():
    assert type(tree.document) is DOMNode
    assert tree.document.type == DOCUMENT

    assert type(tree.head) is DOMNode
    assert tree.head.type == ELEMENT
    assert tree.head.tag == 'head'

    assert type(tree.body) is DOMNode
    assert tree.body.type == ELEMENT
    assert tree.body.tag == 'body'

    assert tree.title == 'Example page'


def test_parse_from_bytes():
    global tree
    tree = HTMLTree.parse_from_bytes(html.encode('utf-16'), 'utf-16')
    assert tree is not None

    test_document()


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
    assert bar_class[1].tag == 'span'

    match_css = tree.document.query_selector('body > main p:last-child')
    assert type(match_css) is DOMNode
    assert match_css.tag == 'p'

    match_css_all = tree.body.query_selector_all('main *')
    assert type(match_css_all) is DOMCollection
    assert len(match_css_all) == 4
    assert match_css_all[0].tag == 'p'
    assert match_css_all[1].tag == 'span'
    assert match_css_all[2].tag == 'p'
    assert match_css_all[3].tag == 'span'

    # Check whether there is any element matching this CSS selector:
    assert tree.body.matches('.bar')
    assert not tree.body.matches('.barbaz')


def test_collection():
    coll = tree.body.query_selector_all('main *')

    assert coll[0].id == 'a'
    assert coll[-1].class_name == 'bar baz'

    assert len(coll[:2]) == 2
    assert coll[:2][0].id == 'a'
    assert coll[:2][1].class_name == 'bar'

    coll = tree.body.get_elements_by_class_name('dom')
    assert len(coll.get_elements_by_class_name('bar')) == 1
    assert coll.get_elements_by_class_name('bar')[0].class_name == 'bar baz'


def test_attributes():
    span = tree.body.query_selector('#b span')
    assert span.class_name == 'bar baz'
    assert len(span.class_list) == 2
    assert span.class_list == ['bar', 'baz']

    span.class_list.add('abc')
    assert len(span.class_list) == 3
    assert span.class_list == ['bar', 'baz', 'abc']
    assert span.class_name == 'bar baz abc'

    assert span.getattr('id') is None
    assert span.getattr('id', 'default') == 'default'
    assert span.id == ''
    span.id = 'abc'
    assert span.id == 'abc'
    assert span['id'] == 'abc'
    assert span.getattr('id') == 'abc'

    with pytest.raises(KeyError):
        # noinspection PyStatementEffect
        span['lang']

    assert span.getattr('lang') is None
    span['lang'] = 'en'
    assert span['lang'] == 'en'
    assert span.getattr('lang') == 'en'

    assert len(span.attrs) == 3
    assert span.attrs == ['class', 'id', 'lang']

    del span['lang']
    assert span.getattr('lang') is None


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
