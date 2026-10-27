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

"""The original uniform-node API, backed by the native typed DOM.

The typed API remains available from ``html.tree``, ``html.node`` and
``html.coll``. This adapter only translates the legacy entry point's values.
"""

from functools import wraps
from weakref import WeakValueDictionary

from .tree import HTMLTree as _HTMLTree
from .node import Node as _Node, NodeType, traverse_dom as _traverse_dom
from .coll import NodeList as _NodeList

__all__ = ['HTMLTree', 'DOMNode', 'DOMCollection', 'traverse_dom']

_nodes = WeakValueDictionary()


def _unwrap(value):
    return value._native if isinstance(value, _Proxy) else value


def _wrap(value):
    if isinstance(value, _Node):
        # Native nodes are interned while alive, so object identity is sufficient
        # and remains stable even after decompose() invalidates a node.
        key = id(value)
        cached = _nodes.get(key)
        if cached is None:
            cached = DOMNode(value)
            _nodes[key] = cached
        return cached
    if isinstance(value, _NodeList):
        return DOMCollection(value)
    if isinstance(value, (tuple, list)):
        return type(value)(_wrap(item) for item in value)
    return value


class _Proxy:
    __slots__ = ('_native', '__weakref__')

    def __init__(self, native):
        object.__setattr__(self, '_native', native)

    def __getattr__(self, name):
        value = getattr(self._native, name)
        if callable(value):
            @wraps(value)
            def call(*args, **kwargs):
                return _wrap(value(*(_unwrap(arg) for arg in args),
                                   **{key: _unwrap(arg) for key, arg in kwargs.items()}))
            return call
        return _wrap(value)

    def __setattr__(self, name, value):
        setattr(self._native, name, _unwrap(value))

    def __str__(self):
        return str(self._native)


class HTMLTree(_Proxy):
    """HTML tree exposing the original uniform DOMNode interface."""
    __slots__ = ()

    @classmethod
    def parse(cls, document):
        return cls(_HTMLTree.parse(document))

    @classmethod
    def parse_from_bytes(cls, document, encoding='utf-8', errors='ignore'):
        return cls(_HTMLTree.parse_from_bytes(document, encoding, errors))


class DOMNode(_Proxy):
    """Uniform view of a native DOM node, preserving legacy return values."""
    __slots__ = ()

    def __getattr__(self, name):
        value = super().__getattr__(name)
        if name in ('id', 'class_name'):
            return value or ''
        if name == 'attrs':
            return list(value)
        return value

    def __iter__(self):
        return (_wrap(node) for node in self._native)

    def __contains__(self, value):
        return _unwrap(value) in self._native

    def __getitem__(self, name):
        return self._native[name]

    def __setitem__(self, name, value):
        self._native[name] = value

    def __delitem__(self, name):
        del self._native[name]

    def __eq__(self, other):
        return isinstance(other, DOMNode) and self._native == other._native

    def __hash__(self):
        return hash(self._native)

    def matches(self, selectors):
        # The legacy method searches descendants; the typed ElementNode method
        # follows the DOM standard and tests the element itself.
        return self._native.query_selector(selectors) is not None

    def __repr__(self):
        kind = self._native.type
        if kind is None:
            return '<INVALID ELEMENT>'
        if kind == NodeType.ELEMENT:
            attrs = ''.join(f' {name}="{self[name]}"' for name in self.attrs)
            return f'<{self.tag}{attrs}>'
        if kind == NodeType.TEXT:
            return self.text
        if kind == NodeType.DOCUMENT:
            return '[HTML Document]'
        if kind == NodeType.DOCUMENT_TYPE:
            return '<!DOCTYPE html>'
        return '<DOMNode Element>'


class DOMCollection(_Proxy):
    """Collection whose elements use the original DOMNode interface."""
    __slots__ = ()

    def __len__(self):
        return len(self._native)

    def __iter__(self):
        return (_wrap(node) for node in self._native)

    def __getitem__(self, index):
        return _wrap(self._native[index])

    def __contains__(self, value):
        return _unwrap(value) in self._native

    def matches(self, selectors):
        return self._native.query_selector(selectors) is not None

    def __repr__(self):
        return '{' + ', '.join(repr(node) for node in self) + '}'


def traverse_dom(base_node, start_callback, end_callback=None, context=None, elements_only=False):
    def adapt(callback):
        if callback is None:
            return None

        @wraps(callback)
        def invoke(ctx):
            ctx.node = _wrap(ctx.node)
            return callback(ctx)
        return invoke

    return _traverse_dom(_unwrap(base_node), adapt(start_callback), adapt(end_callback), context, elements_only)
