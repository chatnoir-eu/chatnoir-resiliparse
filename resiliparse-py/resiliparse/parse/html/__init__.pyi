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

from collections.abc import Callable
from typing import Optional

from .coll import (
    DOMCollection,
    DOMElementClassList,
    DOMTokenList,
    ElementNodeList,
    HTMLCollection,
    NamedNodeMap,
    NodeList,
)
from .exception import CSSParserException, DOMException, HTMLParserException
from .node import (
    AttrNode,
    CDATASectionNode,
    CommentNode,
    DocumentFragmentNode,
    DocumentNode,
    DocumentTypeNode,
    DOMNode,
    ElementNode,
    Node,
    NodeType,
    NotationNode,
    ProcessingInstructionNode,
    TextNode,
)


class HTMLTree:
    @staticmethod
    def parse(document: str) -> HTMLTree: ...

    @staticmethod
    def parse_from_bytes(document: bytes, encoding: str = "utf-8", errors: str = "ignore") -> HTMLTree: ...

    def create_element(self, tag_name: str) -> Optional[Node]: ...

    def create_text_node(self, text: str) -> Optional[Node]: ...

    @property
    def document(self) -> Optional[DocumentNode]: ...

    @property
    def head(self) -> Optional[ElementNode]: ...

    @property
    def body(self) -> Optional[ElementNode]: ...

    @property
    def title(self) -> Optional[str]: ...

    def __str__(self) -> str: ...


class DOMContext:
    node: Optional[Node]
    depth: int

    def __init__(self) -> None: ...


def traverse_dom(
        base_node: Node,
        start_callback: Callable[[DOMContext], object],
        end_callback: Optional[Callable[[DOMContext], object]] = None,
        context: Optional[DOMContext] = None,
        elements_only: bool = False,
) -> None: ...


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
