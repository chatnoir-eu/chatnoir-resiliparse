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

from collections.abc import Container, Sequence
from typing import Generic, Optional, TypeVar, overload
from typing_extensions import disjoint_base, final

from .node import AttrNode, ElementNode, Node

_T = TypeVar("_T", covariant=True)

__all__ = [
    "DOMTokenList",
    "ElementNodeList",
    "NamedNodeMap",
    "NodeList",
    "DOMCollection",
    "HTMLCollection",
    "DOMElementClassList",
]


@disjoint_base
class NodeList(Sequence[_T], Container[_T], Generic[_T]):
    def item(self, index: int) -> Optional[_T]: ...

    def values(self) -> list[_T]: ...

    @overload
    def __getitem__(self, index: int, /) -> _T: ...

    @overload
    def __getitem__(self, index: slice, /) -> Sequence[_T]: ...

    def __len__(self) -> int: ...

    def __contains__(self, item: object, /) -> bool: ...


@final
class ElementNodeList(NodeList[ElementNode]):
    def get_element_by_id(self, element_id: str, case_insensitive: bool = False) -> Optional[ElementNode]: ...

    def get_elements_by_attr(self, attr_name: str, attr_value: str,
                             case_insensitive: bool = False) -> ElementNodeList: ...

    def get_elements_by_class_name(self, class_name: str, case_insensitive: bool = False) -> ElementNodeList: ...

    def get_elements_by_tag_name(self, tag_name: str) -> ElementNodeList: ...

    def query_selector(self, selectors: str) -> Optional[ElementNode]: ...

    def query_selector_all(self, selectors: str) -> ElementNodeList: ...

    def matches(self, selectors: str) -> bool: ...


@final
class NamedNodeMap(NodeList[AttrNode]): ...


@final
class DOMTokenList(Sequence[str], Container[str]):
    @property
    def value(self) -> str: ...

    @value.setter
    def value(self, value: str) -> None: ...

    def values(self) -> list[str]: ...

    def item(self, index: int) -> Optional[str]: ...

    def contains(self, token: str) -> bool: ...

    def add(self, *tokens: str) -> None: ...

    def remove(self, *tokens: str) -> None: ...

    def replace(self, old_token: str, new_token: str) -> bool: ...

    def toggle(self, token: str, force: Optional[bool] = None) -> bool: ...

    @overload
    def __getitem__(self, index: int, /) -> str: ...

    @overload
    def __getitem__(self, index: slice, /) -> Sequence[str]: ...

    def __len__(self) -> int: ...

    def __contains__(self, token: object, /) -> bool: ...


# Names retained for source compatibility with the Cython DOM API.
HTMLCollection = ElementNodeList
DOMCollection = ElementNodeList
DOMElementClassList = DOMTokenList
