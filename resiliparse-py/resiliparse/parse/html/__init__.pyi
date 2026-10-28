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

from .coll import (
    DOMCollection as DOMCollection,
    DOMElementClassList,
    DOMTokenList,
    ElementNodeList,
    HTMLCollection,
    NamedNodeMap,
    NodeList,
)

__all__: list[str]
from .exception import CSSParserException, DOMException, HTMLParserException
from .node import (
    DOMNode as DOMNode,
    DOMContext as DOMContext,
    AttrNode,
    CDATASectionNode,
    CommentNode,
    DocumentFragmentNode,
    DocumentNode,
    DocumentTypeNode,
    ElementNode,
    Node,
    NodeType,
    NotationNode,
    ProcessingInstructionNode,
    TextNode,
    traverse_dom as traverse_dom,
)

from .tree import HTMLTree as HTMLTree

__all__ = [
    "HTMLTree",
    "NodeType",
    "Node",
    "DOMNode",
    "ElementNode",
    "AttrNode",
    "TextNode",
    "CDATASectionNode",
    "ProcessingInstructionNode",
    "CommentNode",
    "DocumentNode",
    "DocumentTypeNode",
    "DocumentFragmentNode",
    "NotationNode",
    "DOMContext",
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
    "NodeList",
    "ElementNodeList",
    "DOMCollection",
    "HTMLCollection",
    "NamedNodeMap",
    "DOMTokenList",
    "DOMElementClassList",
    "DOMException",
    "HTMLParserException",
    "CSSParserException",
]

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
