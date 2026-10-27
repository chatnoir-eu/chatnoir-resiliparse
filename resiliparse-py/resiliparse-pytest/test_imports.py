import importlib
from pathlib import Path
from types import ModuleType

import pytest


@pytest.mark.parametrize('name', [
    'resiliparse.parse',
    'resiliparse.parse.html',
    'resiliparse.parse.html.tree',
    'resiliparse.parse.html.node',
    'resiliparse.parse.html.coll',
    'resiliparse.parse.html.exception',
])
def test_submodule_import(name):
    module = importlib.import_module(name)
    parent_name, _, attr = name.rpartition('.')
    parent = importlib.import_module(parent_name)
    assert getattr(parent, attr) is module
    namespace = {}
    exec(f'from {parent_name} import {attr}', namespace)
    assert namespace[attr] is module


@pytest.mark.parametrize('name', [
    'resiliparse',
    'resiliparse.parse',
    'resiliparse.parse.html',
])
def test_wildcard_import_excludes_submodules(name):
    module = importlib.import_module(name)
    namespace = {}
    exec(f'from {name} import *', namespace)
    assert set(namespace) - {'__builtins__'} == set(module.__all__)
    assert not any(isinstance(namespace[attr], ModuleType) for attr in module.__all__)


def test_native_package_paths():
    import resiliparse
    from resiliparse.parse import html

    root = Path(resiliparse.__file__).parent
    assert resiliparse.parse.__path__ == [str(root / 'parse')]
    assert html.__path__ == [str(root / 'parse' / 'html')]
    assert html.HTMLTree is html.tree.HTMLTree
    assert html.Node is html.node.Node
    assert html.DOMNode is html.Node
    assert html.DOMCollection is html.coll.ElementNodeList
    assert html.traverse_dom is html.node.traverse_dom

    tree = html.HTMLTree.parse('<p>Example</p>')
    assert type(tree.document) is html.DocumentNode
    assert type(tree.body) is html.ElementNode
    text = tree.body.query_selector('p').first_child
    assert type(text) is html.TextNode
