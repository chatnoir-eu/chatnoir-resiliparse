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

from collections.abc import Container, Iterable, Iterator
from typing import Any, Callable, Optional, Tuple, final
from typing_extensions import disjoint_base

from .coll import DOMTokenList, ElementNodeList, NamedNodeMap, NodeList

__all__ = [
    "DOMTokenList",
    "ElementNodeList",
    "NamedNodeMap",
    "NodeList",
    "NodeType",
    "AttrNode",
    "CDATASectionNode",
    "CommentNode",
    "DocumentFragmentNode",
    "DocumentNode",
    "DocumentTypeNode",
    "DOMContext",
    "ElementNode",
    "Node",
    "NotationNode",
    "ProcessingInstructionNode",
    "TextNode",
    "traverse_dom",
    "ELEMENT",
    "ATTRIBUTE",
    "TEXT",
    "CDATA_SECTION",
    "ENTITY_REFERENCE",
    "ENTITY",
    "PROCESSING_INSTRUCTION",
    "COMMENT",
    "DOCUMENT",
    "DOCUMENT_TYPE",
    "DOCUMENT_FRAGMENT",
    "NOTATION",
    "DOMNode",
]


@final
class NodeType:
    ELEMENT: NodeType
    ATTRIBUTE: NodeType
    TEXT: NodeType
    CDATA_SECTION: NodeType
    ENTITY_REFERENCE: NodeType
    ENTITY: NodeType
    PROCESSING_INSTRUCTION: NodeType
    COMMENT: NodeType
    DOCUMENT: NodeType
    DOCUMENT_TYPE: NodeType
    DOCUMENT_FRAGMENT: NodeType
    NOTATION: NodeType


ELEMENT: NodeType
ATTRIBUTE: NodeType
TEXT: NodeType
CDATA_SECTION: NodeType
ENTITY_REFERENCE: NodeType
ENTITY: NodeType
PROCESSING_INSTRUCTION: NodeType
COMMENT: NodeType
DOCUMENT: NodeType
DOCUMENT_TYPE: NodeType
DOCUMENT_FRAGMENT: NodeType
NOTATION: NodeType


@disjoint_base
class Node(Iterable[Node], Container[Node]):
    @property
    def node_type(self) -> Optional[NodeType]: ...

    @property
    def type(self) -> Optional[NodeType]: ...

    @property
    def node_name(self) -> Optional[str]: ...

    @property
    def tag(self) -> Optional[str]: ...

    @property
    def node_value(self) -> Optional[str]: ...

    @node_value.setter
    def node_value(self, value: str) -> None: ...

    @property
    def value(self) -> Optional[str]: ...

    @value.setter
    def value(self, value: str) -> None: ...

    @property
    def text_content(self) -> Optional[str]: ...

    @text_content.setter
    def text_content(self, value: str) -> None: ...

    @property
    def text(self) -> Optional[str]: ...

    @text.setter
    def text(self, value: str) -> None: ...

    @property
    def html(self) -> str: ...

    @html.setter
    def html(self, value: str) -> None: ...

    @property
    def owner_document(self) -> Optional[DocumentNode]: ...

    @property
    def parent_node(self) -> Optional[Node]: ...

    @property
    def parent(self) -> Optional[Node]: ...

    @property
    def parent_element(self) -> Optional[ElementNode]: ...

    def has_child_nodes(self) -> bool: ...

    def contains(self, node: Node) -> bool: ...

    @property
    def child_nodes(self) -> NodeList[Node]: ...

    @property
    def first_child(self) -> Optional[Node]: ...

    @property
    def last_child(self) -> Optional[Node]: ...

    @property
    def previous_sibling(self) -> Optional[Node]: ...

    @property
    def prev(self) -> Optional[Node]: ...

    @property
    def next_sibling(self) -> Optional[Node]: ...

    @property
    def next(self) -> Optional[Node]: ...

    @property
    def next_element(self) -> Optional[ElementNode]: ...

    @property
    def prev_element(self) -> Optional[ElementNode]: ...

    def clone_node(self, deep: bool = False) -> Optional[Node]: ...

    def insert_before(self, node: Node, reference: Optional[Node] = None) -> Optional[Node]: ...

    def append_child(self, node: Node) -> Optional[Node]: ...

    def replace_child(self, new_child: Node, old_child: Node) -> Optional[Node]: ...

    def remove_child(self, child: Node) -> Optional[Node]: ...

    def decompose(self) -> None: ...

    def __iter__(self) -> Iterator[Node]: ...

    def __contains__(self, node: object, /) -> bool: ...

    def __str__(self) -> str: ...

    def __repr__(self) -> str: ...


@final
class DOMContext:
    node: object | None
    depth: int


def traverse_dom(
    base_node: Node,
    start_callback: Callable[[DOMContext], Any],
    end_callback: Callable[[DOMContext], Any] | None = None,
    context: DOMContext | None = None,
    elements_only: bool = False,
) -> None: ...


# Legacy name.
DOMNode = Node


class _ParentNode(Node):
    @property
    def children(self) -> ElementNodeList: ...

    @property
    def first_element_child(self) -> Optional[ElementNode]: ...

    @property
    def last_element_child(self) -> Optional[ElementNode]: ...

    @property
    def child_element_count(self) -> int: ...

    def prepend(self, *nodes: Node) -> None: ...

    def append(self, *nodes: Node) -> None: ...

    def replace_children(self, *nodes: Node) -> None: ...

    def query_selector(self, selectors: str) -> Optional[ElementNode]: ...

    def query_selector_all(self, selectors: str) -> ElementNodeList: ...


class _ChildNode(Node):
    def before(self, *nodes: Node) -> None: ...

    def after(self, *nodes: Node) -> None: ...

    def replace_with(self, *nodes: Node) -> None: ...

    def remove(self) -> None: ...


class _NonDocumentTypeChildNode(Node):
    @property
    def next_element_sibling(self) -> Optional[ElementNode]: ...

    @property
    def previous_element_sibling(self) -> Optional[ElementNode]: ...


class _NonElementParentNode(Node):
    def get_element_by_id(self, element_id: str, case_insensitive: bool = False) -> Optional[ElementNode]: ...


class _Element(_ParentNode, _ChildNode, _NonDocumentTypeChildNode):
    @property
    def tag_name(self) -> Optional[str]: ...

    @property
    def local_name(self) -> Optional[str]: ...

    @property
    def id(self) -> Optional[str]: ...

    @id.setter
    def id(self, value: str) -> None: ...

    @property
    def class_name(self) -> Optional[str]: ...

    @class_name.setter
    def class_name(self, value: str) -> None: ...

    @property
    def class_list(self) -> DOMTokenList: ...

    def get_attribute(self, qualified_name: str, default_value: Optional[str] = None) -> Optional[str]: ...

    def attribute_node(self, qualified_name: str) -> Optional[AttrNode]: ...

    @property
    def attribute_names(self) -> Tuple[str, ...]: ...

    @property
    def attributes(self) -> NamedNodeMap: ...

    def has_attribute(self, qualified_name: str) -> bool: ...

    def set_attribute(self, qualified_name: str, value: str) -> None: ...

    def remove_attribute(self, qualified_name: str) -> None: ...

    def toggle_attribute(self, qualified_name: str, force: Optional[bool] = None) -> bool: ...

    @property
    def attrs(self) -> Tuple[str, ...]: ...

    def hasattr(self, qualified_name: str) -> bool: ...

    def getattr(self, qualified_name: str, default_value: Optional[str] = None) -> Optional[str]: ...

    def __contains__(self, qualified_name: object, /) -> bool: ...

    def __getitem__(self, qualified_name: str, /) -> str: ...

    def setattr(self, qualified_name: str, value: str) -> None: ...

    def __setitem__(self, qualified_name: str, value: str, /) -> None: ...

    def delattr(self, qualified_name: str) -> None: ...

    def __delitem__(self, qualified_name: str, /) -> None: ...

    def closest(self, selectors: str) -> Optional[ElementNode]: ...

    def matches(self, selectors: str) -> bool: ...

    def get_elements_by_tag_name(self, name: str) -> ElementNodeList: ...

    def get_elements_by_class_name(self, class_names: str, case_insensitive: bool = False) -> ElementNodeList: ...

    def get_elements_by_attr(self, name: str, value: str, case_insensitive: bool = False) -> ElementNodeList: ...

    def get_element_by_id(self, element_id: str, case_insensitive: bool = False) -> Optional[ElementNode]: ...

    @property
    def inner_html(self) -> str: ...

    @inner_html.setter
    def inner_html(self, value: str) -> None: ...

    @property
    def outer_html(self) -> str: ...

    @outer_html.setter
    def outer_html(self, value: str) -> None: ...

    @property
    def inner_text(self) -> str: ...

    @inner_text.setter
    def inner_text(self, value: str) -> None: ...

    @property
    def outer_text(self) -> str: ...

    @outer_text.setter
    def outer_text(self, value: str) -> None: ...


class _DocumentType(_ChildNode):
    @property
    def name(self) -> Optional[str]: ...

    @property
    def public_id(self) -> Optional[str]: ...

    @property
    def system_id(self) -> Optional[str]: ...


class _Document(_ParentNode, _NonElementParentNode):
    @property
    def doctype(self) -> Optional[DocumentTypeNode]: ...

    @property
    def document_element(self) -> Optional[ElementNode]: ...

    def get_elements_by_tag_name(self, name: str) -> ElementNodeList: ...

    def get_elements_by_class_name(self, class_names: str, case_insensitive: bool = False) -> ElementNodeList: ...

    def get_elements_by_attr(self, name: str, value: str, case_insensitive: bool = False) -> ElementNodeList: ...

    def create_element(self, local_name: str) -> ElementNode: ...

    def create_document_fragment(self) -> DocumentFragmentNode: ...

    def create_text_node(self, data: str) -> TextNode: ...

    def create_cdata_section(self, data: str) -> CDATASectionNode: ...

    def create_comment(self, data: str) -> CommentNode: ...

    def create_processing_instruction(self, target: str, local_name: str) -> ProcessingInstructionNode: ...

    def create_attribute(self, local_name: str) -> AttrNode: ...


class _DocumentFragment(_ParentNode, _NonElementParentNode): ...


class _DocumentOrShadowRoot(Node): ...


class _ShadowRoot(_DocumentOrShadowRoot): ...


class _Attr(_ChildNode):
    @property
    def local_name(self) -> Optional[str]: ...

    @property
    def name(self) -> Optional[str]: ...

    @property
    def value(self) -> Optional[str]: ...

    @value.setter
    def value(self, value: str) -> None: ...

    @property
    def owner_element(self) -> Optional[ElementNode]: ...


class _CharacterData(_ChildNode, _NonDocumentTypeChildNode):
    def __len__(self) -> int: ...

    def is_empty(self) -> bool: ...

    @property
    def data(self) -> Optional[str]: ...

    @data.setter
    def data(self, value: str) -> None: ...

    def substring_data(self, offset: int, count: int) -> str: ...

    def append_data(self, data: str) -> None: ...

    def insert_data(self, offset: int, data: str) -> None: ...

    def delete_data(self, offset: int, count: int) -> None: ...

    def replace_data(self, offset: int, count: int, data: str) -> None: ...


class _Text(_CharacterData): ...


class _CDATASection(_CharacterData): ...


class _ProcessingInstruction(_CharacterData):
    @property
    def target(self) -> Optional[str]: ...


class _Comment(_CharacterData): ...


@final
class ElementNode(_Element): ...


@final
class AttrNode(_Attr): ...


@final
class TextNode(_Text): ...


@final
class CDATASectionNode(_CDATASection): ...


@final
class ProcessingInstructionNode(_ProcessingInstruction): ...


@final
class CommentNode(_Comment): ...


@final
class DocumentNode(_Document): ...


@final
class DocumentTypeNode(_DocumentType): ...


@final
class DocumentFragmentNode(_DocumentFragment): ...


@final
class NotationNode(Node): ...
