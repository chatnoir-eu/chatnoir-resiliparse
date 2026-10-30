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

from typing import Iterable, Literal, Optional, Union

from ..parse.html import HTMLTree

__all__ = ["extract_plain_text"]


def extract_plain_text(
    html: Union[str, HTMLTree],
    preserve_formatting: Union[bool, Literal["minimal_html"]] = ...,
    main_content: bool = False,
    list_bullets: bool = True,
    alt_texts: bool = True,
    links: bool = False,
    form_fields: bool = False,
    noscript: bool = False,
    comments: bool = True,
    post_meta: bool = True,
    hidden_elements: bool = False,
    skip_elements: Optional[Iterable[str]] = None,
) -> str: ...
