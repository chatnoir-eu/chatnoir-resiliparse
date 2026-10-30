# Copyright 2026 Janek Bevendorff
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

import pytest

from resiliparse.parse.html import DOMContext, HTMLTree, traverse_dom
from resiliparse.parse.html.node import ElementNode


@pytest.mark.parametrize('kind', ['fragment', 'element'])
def test_parent_node_exports(kind):
    # Each subclass exports its own PyO3 shims for the shared parent-node API.
    document = HTMLTree.parse('<html><body></body></html>').document
    parent = {
        'fragment': document.create_document_fragment(),
        'element': document.create_element('section'),
    }[kind]
    parent.replace_children()
    first = document.create_element('div')
    first.id = 'target'
    last = document.create_element('span')
    parent.append(first, last)
    parent.prepend(document.create_comment('note'))
    assert parent.child_element_count == 2
    assert parent.first_element_child is first
    assert parent.last_element_child is last
    assert parent.children.values() == [first, last]
    assert parent.query_selector('#target') is first
    assert parent.query_selector_all('#target').item(0) is first
    assert parent.get_element_by_id('target') is first
    assert isinstance(parent.query_selector('span'), ElementNode)
    parent.replace_children(last)
    assert parent.children.values() == [last]


@pytest.mark.parametrize('collection', ['child_nodes', 'children', 'query', 'attributes'])
def test_node_collection_python_protocols(collection):
    tree = HTMLTree.parse('<div id="target"><span></span><b></b></div>')
    element = tree.body.first_element_child
    nodes = {
        'child_nodes': element.child_nodes,
        'children': element.children,
        'query': element.query_selector_all('*'),
        'attributes': element.attributes,
    }[collection]
    first = nodes.item(0)
    assert first is nodes[0]
    assert first in nodes
    assert tree.head not in nodes
    assert object() not in nodes
    assert nodes.values() == list(nodes)
    assert nodes[::-1] == tuple(reversed(list(nodes)))
    assert nodes.item(len(nodes)) is None
    with pytest.raises(IndexError):
        _ = nodes[len(nodes)]
    with pytest.raises(TypeError):
        _ = nodes['invalid']


def test_document_query_exports():
    document = HTMLTree.parse('<div id="target" class="marker"></div>').document
    element = document.get_element_by_id('target')
    assert document.get_elements_by_tag_name('div').item(0) is element
    assert document.get_elements_by_class_name('marker').item(0) is element
    assert document.get_elements_by_attr('id', 'target').item(0) is element


def test_document_parent_node_exports():
    document = HTMLTree.parse('<html></html>').document
    root = document.document_element
    document.replace_children()
    document.append(root)
    document.prepend(document.create_comment('note'))
    assert document.child_element_count == 1
    assert document.first_element_child is root
    assert document.last_element_child is root
    assert document.children.values() == [root]
    assert document.query_selector_all('html').item(0) is root
    document.replace_children(root)
    assert document.children.values() == [root]


@pytest.mark.parametrize('callback_kind', ['start', 'end'])
def test_traversal_preserves_callback_exception(callback_kind):
    tree = HTMLTree.parse('<div><span><b></b></span></div>')
    context = DOMContext()
    error = RuntimeError('callback failed')

    def fail(ctx):
        assert ctx is context
        raise error

    callbacks = {'start_callback': lambda ctx: None}
    callbacks[callback_kind + '_callback'] = fail
    with pytest.raises(RuntimeError) as caught:
        traverse_dom(tree.body, context=context, **callbacks)
    assert caught.value is error


def test_traversal_default_context():
    tree = HTMLTree.parse('<div><span></span></div>')
    contexts = []
    traverse_dom(tree.body, contexts.append)
    assert contexts
    assert all(isinstance(ctx, DOMContext) and ctx is contexts[0] for ctx in contexts)


def test_traversal_filters_non_element_root():
    document = HTMLTree.parse('<p>text</p>').document
    traverse_dom(document.create_text_node('text'),
                 lambda ctx: pytest.fail('Text root must be skipped'), elements_only=True)


@pytest.mark.parametrize('factory, args', [
    ('create_text_node', ('text',)),
    ('create_comment', ('comment',)),
    ('create_cdata_section', ('cdata',)),
    ('create_processing_instruction', ('target', 'data')),
    ('create_element', ('span',)),
])
def test_child_node_exports(factory, args):
    # The mixin methods are independently registered on each Python subclass.
    document = HTMLTree.parse('<html></html>').document
    parent = document.create_element('section')
    node = getattr(document, factory)(*args)
    parent.append(node)
    before = document.create_element('b')
    after = document.create_element('i')
    node.before(before)
    node.after(after)
    assert node.previous_element_sibling is before
    assert node.next_element_sibling is after
    replacement = document.create_element('em')
    node.replace_with(replacement)
    assert replacement.parent is parent
    assert node.parent is None
    parent.append(node)
    node.remove()
    assert node.parent is None


def test_doctype_exports():
    document = HTMLTree.parse('<!DOCTYPE html PUBLIC "public-id" "system-id"><html></html>').document
    doctype = document.doctype
    assert doctype.public_id == 'public-id'
    assert doctype.system_id == 'system-id'
    before = document.create_comment('before')
    after = document.create_comment('after')
    doctype.before(before)
    doctype.after(after)
    assert doctype.previous_sibling is before
    assert doctype.next_sibling is after
    doctype.replace_with(document.create_comment('replacement'))
    assert doctype.parent is None
    document.prepend(doctype)
    doctype.remove()
    assert doctype.parent is None


@pytest.mark.parametrize('method', ['append', 'prepend', 'replace_children', 'before', 'after', 'replace_with'])
def test_node_arguments_reject_python_objects(method):
    tree = HTMLTree.parse('<div></div>')
    with pytest.raises(TypeError):
        getattr(tree.body.first_element_child, method)(object())


def test_element_attribute_mapping_errors():
    element = HTMLTree.parse('<div></div>').body.first_element_child
    element.setattr('title', 'caption')
    assert 'title' in element
    assert element['title'] == 'caption'
    element.delattr('title')
    assert 'title' not in element
    with pytest.raises(KeyError, match='missing'):
        del element['missing']


@pytest.mark.parametrize('method', ['query_selector', 'query_selector_all', 'matches'])
def test_collection_selector_exception(method):
    from resiliparse.parse.html import CSSParserException

    elements = HTMLTree.parse('<div></div>').body.children
    with pytest.raises(CSSParserException):
        getattr(elements, method)('[')
