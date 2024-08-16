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

from typing import Callable, Optional
from typing_extensions import deprecated

from .coll import DOMCollection, DOMElementClassList
from .exception import DOMException
from .node import DocumentNode, ElementNode, Node, NodeType, TextNode


class HTMLTree:
    @staticmethod
    def parse(document: str) -> Optional[HTMLTree]: ...
    @staticmethod
    def parse_from_bytes(document: bytes, encoding: str = 'utf-8', errors: str = 'ignore') -> Optional[HTMLTree]: ...

    @deprecated('Use HTMLTree.document.create_element() instead.')
    def create_element(self, tag_name: str) -> Optional[ElementNode]: ...
    @deprecated('Use HTMLTree.document.create_text_node() instead.')
    def create_text_node(self, text: str) -> Optional[TextNode]: ...

    @property
    def document(self) -> Optional[DocumentNode]: ...
    @property
    def head(self) -> Optional[ElementNode]: ...
    @property
    def body(self) -> Optional[ElementNode]: ...
    @property
    def title(self) -> Optional[str]: ...


class DOMContext:
    def __init__(self):
        self.node: Optional[Node] = None
        self.depth: int = 0


def traverse_dom(base_node: Node,
                 start_callback: Callable[[DOMContext], None],
                 end_callback: Optional[Callable[[DOMContext], None]] = None,
                 context: Optional[DOMContext] = None,
                 elements_only: bool = False): ...
