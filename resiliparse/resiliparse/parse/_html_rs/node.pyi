# Copyright 2024 Janek Bevendorff
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

import enum
from typing import Container, Iterator, Iterable, Optional, Tuple, Sized

from typing_extensions import deprecated

from .coll import DOMTokenList, HTMLCollection, NodeList, ElementNodeList, NamedNodeMap


class NodeType(enum.Enum):
    ELEMENT: int = 0x01
    ATTRIBUTE: int = 0x02
    TEXT: int = 0x03
    CDATA_SECTION: int = 0x04
    ENTITY_REFERENCE: int = 0x05
    ENTITY: int = 0x06
    PROCESSING_INSTRUCTION: int = 0x07
    COMMENT: int = 0x08
    DOCUMENT: int = 0x09
    DOCUMENT_TYPE: int = 0x0A
    DOCUMENT_FRAGMENT: int = 0x0B
    NOTATION: int = 0x0C


class _Node(Iterable[Node], Container[Node]):
    @property
    def type(self) -> NodeType: ...
    @property
    def node_type(self) -> NodeType: ...

    @property
    def name(self) -> Optional[str]: ...
    @property
    def node_name(self) -> Optional[str]: ...
    @property
    def value(self) -> Optional[str]: ...
    @property
    def node_value(self) -> Optional[str]: ...

    @property
    def text(self) -> Optional[str]: ...
    @property
    def text_content(self) -> Optional[str]: ...

    @property
    def owner_document(self) -> Optional[DocumentNode]: ...
    @property
    def parent(self) -> Optional[Node]: ...
    @property
    def parent_node(self) -> Optional[Node]: ...
    @property
    def parent_element(self) -> Optional[ElementNode]: ...

    def has_child_nodes(self) -> bool: ...
    def contains(self, node: Node) -> bool: ...
    @property
    def child_nodes(self) -> NodeList: ...
    @property
    def first_child(self) -> Optional[Node]: ...
    @property
    def last_child(self) -> Optional[Node]: ...
    @property
    def prev(self) -> Optional[Node]: ...
    @property
    def previous_sibling(self) -> Optional[Node]: ...
    @property
    def next(self) -> Optional[Node]: ...
    @property
    def next_sibling(self) -> Optional[Node]: ...
    def clone_node(self, deep: bool = False) -> Optional[Node]: ...

    def insert_before(self, node: Node, reference: Optional[Node] = None) -> Node: ...
    def append_child(self, node: Node) -> Node: ...
    def replace_child(self, new_child: Node, old_child: Node) -> Node: ...
    def remove_child(self, child: Node) -> Node: ...
    def decompose(self): ...

    def __iter__(self) -> Iterator[Node]: ...
    def __contains__(self, __x: object) -> bool: ...


class _DocumentType(_ChildNode):
    @property
    def name(self) -> Optional[str]: ...
    def public_id(self) -> Optional[str]: ...
    def system_id(self) -> Optional[str]: ...


class _DocumentOrShadowRoot(_Node): ...
class _ShadowRoot(_DocumentOrShadowRoot): ...


class _Document(_DocumentOrShadowRoot, _ParentNode, _NonElementParentNode):
    @property
    def doctype(self) -> Optional[DocumentTypeNode]: ...
    def document_element(self) -> Optional[DocumentNode]: ...

    def get_elements_by_tag_name(self, qualified_name: str) -> HTMLCollection: ...
    def get_elements_by_class_name(self, qualified_name: str) -> HTMLCollection: ...
    def get_elements_by_attr(self, qualified_name: str, value: str, case_insensitive: bool = False) -> HTMLCollection: ...

    def create_element(self, local_name: str) -> ElementNode: ...
    def create_document_fragment(self, local_name: str) -> DocumentFragmentNode: ...
    def create_text_node(self, local_name: str) -> TextNode: ...
    def create_cdata_section(self, local_name: str) -> CDATASectionNode: ...
    def create_comment(self, local_name: str) -> CommentNode: ...
    def create_processing_instruction(self, local_name: str) -> ProcessingInstructionNode: ...
    def create_attribute(self, local_name: str) -> AttrNode: ...


class _DocumentFragment(_DocumentOrShadowRoot, _ParentNode, _NonElementParentNode): ...


class _ParentNode(_Node):
    @property
    def children(self) -> HTMLCollection: ...
    @property
    def first_element_child(self) -> Optional[ElementNode]: ...
    @property
    def last_element_child(self) -> Optional[ElementNode]: ...
    @property
    def child_element_count(self) -> int: ...

    @property
    def prepend(self, nodes: Iterable[Node]): ...
    @property
    def append(self, nodes: Iterable[Node]): ...
    @property
    def replace_children(self, nodes: Iterable[Node]): ...

    def query_selector(self, selector: str) -> Optional[ElementNode]: ...
    def query_selector_all(self, selector: str) -> ElementNodeList: ...


class _NonElementParentNode(_Node):
    def get_element_by_id(self, element_id: str, case_insensitive: bool = False) -> Optional[ElementNode]: ...


class _ChildNode(_Node):
    def before(self, nodes: Iterable[Node]): ...
    def after(self, nodes: Iterable[Node]): ...
    def replace_with(self, nodes: Iterable[Node]): ...
    def remove(self): ...


class _NonDocumentTypeChildNode(_Node):
    @property
    def next_element_sibling(self) -> Optional[ElementNode]: ...
    @property
    def next_element(self) -> Optional[ElementNode]: ...
    @property
    def previous_element_sibling(self) -> Optional[ElementNode]: ...
    @property
    def prev_element(self) -> Optional[ElementNode]: ...


class _Element(_ParentNode, _ChildNode, _NonDocumentTypeChildNode):
    @property
    def tag(self) -> Optional[str]: ...
    @property
    def tag_name(self) -> Optional[str]: ...
    @property
    def local_name(self) -> Optional[str]: ...
    @property
    def id(self) -> Optional[str]: ...
    @property
    def class_name(self) -> Optional[str]: ...
    @property
    def class_list(self) -> DOMTokenList: ...

    def attribute(self, qualified_name: str) -> Optional[str]: ...
    def attribute_node(self, qualified_name: str) -> Optional[AttrNode]: ...
    @property
    def attribute_names(self) -> Tuple[str]: ...
    @property
    def attributes(self) -> NamedNodeMap: ...
    def has_attribute(self, qualified_name: str) -> bool: ...
    def set_attribute(self, qualified_name: str, value: str): ...
    def remove_attribute(self, qualified_name: str): ...
    def toggle_attribute(self, qualified_name: str, force: bool = False): ...

    @property
    def attrs(self) -> Tuple[str]: ...
    def hasattr(self, qualified_name: str) -> bool: ...
    def __contains__(self, qualified_name: str) -> bool: ...
    def getattr(self, qualified_name: str, default_value: Optional[str] = None) -> Optional[str]: ...
    def __getitem__(self, qualified_name: str) -> str: ...
    def setattr(self, qualified_name: str, value: str): ...
    def __setitem__(self, qualified_name: str, value: str): ...
    def delattr(self, qualified_name: str): ...
    def __delitem__(self, qualified_name: str): ...

    def closest(self, selector: str) -> Optional[ElementNode]: ...
    def matches(self, selector: str) -> bool: ...
    @deprecated('Use DocumentNode.get_element_by_id() instead.')
    def get_element_by_id(self, element_id: str, case_insensitive: bool = False) -> Optional[Node]: ...
    def get_elements_by_tag_name(self, tag_name: str) -> HTMLCollection: ...
    def get_elements_by_class_name(self, class_name: str) -> HTMLCollection: ...
    def get_elements_by_attr(self, attr_name: str, attr_value: str, case_insensitive: bool = False) -> HTMLCollection: ...

    @deprecated("Inconsistent get/set behaviour, use ElementNode.inner_html or ElementNode.outer_html instead.")
    @property
    def html(self) -> Optional[str]: ...
    @property
    def inner_html(self) -> Optional[str]: ...
    @property
    def outer_html(self) -> Optional[str]: ...
    @property
    def inner_text(self) -> Optional[str]: ...
    @property
    def outer_text(self) -> Optional[str]: ...


class _Attr(_Node):
    @property
    def local_name(self) -> Optional[str]: ...
    @property
    def name(self) -> Optional[str]: ...
    @property
    def value(self) -> Optional[str]: ...
    @property
    def owner_element(self) -> Optional[ElementNode]: ...


class _CharacterData(_Node, _ChildNode, _NonDocumentTypeChildNode):
    @property
    def len(self) -> int: ...
    @property
    def data(self) -> Optional[str]: ...
    @property
    def substring_data(self) -> Optional[str]: ...
    def append_data(self, data: str): ...
    def insert_data(self, offset: int, data: str): ...
    def delete_data(self, offset: int, count: int): ...
    def replace_data(self, offset: int, count: int, data: str): ...


class _Text(_CharacterData): ...
class _CDATASection(_CharacterData): ...


class _ProcessingInstruction(_CharacterData):
    @property
    def target(self) -> Optional[str]: ...


class _Comment(_CharacterData): ...


class Node(_Node): ...
class ElementNode(Node, _Element, _ParentNode, _ChildNode, _NonDocumentTypeChildNode): ...
class AttrNode(Node, _Attr): ...
class TextNode(Node, _Text, _CharacterData, _ChildNode, _NonDocumentTypeChildNode): ...
class CDATASectionNode(Node, _CDATASection, _CharacterData, _ChildNode, _NonDocumentTypeChildNode): ...
class ProcessingInstructionNode(Node, _ProcessingInstruction, _CharacterData, _ChildNode, _NonDocumentTypeChildNode): ...
class DocumentNode(Node, _Document, _DocumentOrShadowRoot, _ParentNode, _NonElementParentNode): ...
class DocumentTypeNode(Node, _DocumentType, _ChildNode): ...
class DocumentFragmentNode(Node, _DocumentFragment, _DocumentOrShadowRoot, _ParentNode, _NonElementParentNode): ...
class CommentNode(Node, _Comment, _CharacterData, _ChildNode, _NonDocumentTypeChildNode): ...

DOMNode = Node
