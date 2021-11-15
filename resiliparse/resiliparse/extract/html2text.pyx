# Copyright 2021 Janek Bevendorff
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

# distutils: language = c++

from cython.operator cimport preincrement as preinc, predecrement as predec
from libcpp.string cimport string, to_string
from libcpp.vector cimport vector

from resiliparse.parse.html cimport *
from resiliparse_inc.cctype cimport isspace
from resiliparse_inc.lexbor cimport *

cdef struct ExtractOpts:
    bint preserve_formatting
    bint list_bullets
    bint links
    bint alt_texts
    bint form_fields
    bint noscript


cdef struct ExtractContext:
    lxb_dom_node_t* node
    size_t depth
    size_t list_depth
    vector[size_t] list_numbering
    bint is_pre
    string text
    ExtractOpts opts


cdef void _extract_start_cb(ExtractContext* ctx):
    cdef lxb_dom_character_data_t* node_char_data = NULL
    cdef string txt

    if ctx.node.type == LXB_DOM_NODE_TYPE_TEXT:
        node_char_data = <lxb_dom_character_data_t*>ctx.node
        ctx.text.reserve(ctx.text.size() + node_char_data.data.length)
        if ctx.opts.preserve_formatting and ctx.is_pre:
            ctx.text.append(<char*>node_char_data.data.data, node_char_data.data.length)
        else:
            for i in range(node_char_data.data.length):
                if isspace(<char> node_char_data.data.data[i]):
                    if ctx.text.back() != b' ':
                        ctx.text.push_back(<char> b' ')
                else:
                    ctx.text.push_back(<char> node_char_data.data.data[i])

    if ctx.node.type != LXB_DOM_NODE_TYPE_ELEMENT or not ctx.opts.preserve_formatting:
        return

    cdef size_t tag_name_len
    cdef const lxb_char_t* tag_name = lxb_dom_element_qualified_name(<lxb_dom_element_t*>ctx.node, &tag_name_len)

    # Block formatting
    if is_block_element(ctx.node.local_name) and ctx.text.back() != b'\n':
        ctx.text.push_back(<char>b'\n')

    # Headings
    if ctx.node.local_name in [LXB_TAG_H1, LXB_TAG_H2, LXB_TAG_H3, LXB_TAG_H4, LXB_TAG_H5, LXB_TAG_H6]:
        if ctx.text.back() != b'\n':
            ctx.text.push_back(<char>b'\n')

    # Lists
    if ctx.node.local_name == LXB_TAG_UL:
        preinc(ctx.list_depth)
    elif ctx.node.local_name == LXB_TAG_OL:
        preinc(ctx.list_depth)
        ctx.list_numbering.push_back(0)

    # List items
    if ctx.opts.list_bullets and ctx.node.local_name == LXB_TAG_LI:
        ctx.text.append(string(2 * ctx.list_depth, <char> b' '))
        if ctx.node.parent.local_name == LXB_TAG_OL:
            ctx.text.append(to_string(preinc(ctx.list_numbering.back())) + b'. ')
        else:
            ctx.text.append(b'\xe2\x80\xa2 ')


cdef void _extract_end_cb(ExtractContext* ctx):
    if ctx.node.type != LXB_DOM_NODE_TYPE_ELEMENT or not ctx.opts.preserve_formatting:
        return

    # Headings
    if ctx.node.local_name in [LXB_TAG_H1, LXB_TAG_H2, LXB_TAG_H3, LXB_TAG_H4, LXB_TAG_H5, LXB_TAG_H6]:
        ctx.text.push_back(<char>b'\n')

    # Paragraphs
    if ctx.node.local_name == LXB_TAG_P and ctx.text.back() != b'\n':
        ctx.text.push_back(<char>b'\n')

    # Lists
    if ctx.node.local_name == LXB_TAG_UL:
        predec(ctx.list_depth)
    elif ctx.node.local_name == LXB_TAG_OL:
        predec(ctx.list_depth)
        ctx.list_numbering.pop_back()


def extract_plain_text(DOMNode base_node, bint preserve_formatting=True, bint list_bullets=True, bint links=False,
                       bint alt_texts=False, bint form_fields=False, bint noscript=False, skip_elements=None):
    """
    extract_plain_text(base_node, preserve_formatting=True, preserve_formatting=True, list_bullets=True, \
                       links=False, alt_texts=False, form_fields=False, noscript=False, skip_elements=None)

    Perform a simple plain-text extraction from the given DOM node and its children.

    Extracts all visible text (excluding script/style elements, comment nodes etc.)
    and collapses consecutive white space characters. If ``preserve_formatting`` is
    ``True``, line breaks, paragraphs, other block-level elements, list elements, and
    ``<pre>``-formatted text will be preserved.

    Extraction of particular elements and attributes such as links, alt texts, or form fields
    can be be configured individually by setting the corresponding parameter to ``True``.
    Defaults to ``False`` for most elements (i.e., only basic text will be extracted).

    :param base_node: base DOM node of which to extract sub tree
    :type base_node: DOMNode
    :param preserve_formatting: preserve basic block-level formatting
    :type preserve_formatting: bool
    :param list_bullets: insert bullets / numbers for list items
    :type list_bullets: bool
    :param links: extract link target URLs
    :type links: bool
    :param alt_texts: preserve alternative text descriptions
    :type alt_texts: bool
    :param form_fields: extract form fields and their values
    :type form_fields: bool
    :param noscript: extract contents of <noscript> elements
    :param skip_elements: names of elements to skip (defaults to ``head``, ``script``, ``style``)
    :type skip_elements: t.Iterable[str] or None
    :type noscript: bool
    :return: extracted plain text
    :rtype: str
    """
    if not check_node(base_node):
        return ''

    skip_elements = {e.encode() for e in skip_elements or []}
    if not skip_elements:
        skip_elements = {b'head', b'script', b'style'}
    if not alt_texts:
        skip_elements.update({b'object', b'video', b'audio', b'embed' b'img', b'area'})
    if not noscript:
        skip_elements.add(b'noscript')
    if not form_fields:
        skip_elements.update({b'textarea', b'input', b'button'})

    cdef size_t tag_name_len = 0
    cdef const lxb_char_t* tag_name = NULL

    cdef ExtractContext ctx
    ctx.is_pre = False
    ctx.depth = 0
    ctx.list_depth = 0
    ctx.opts = [
        preserve_formatting,
        list_bullets,
        links,
        alt_texts,
        form_fields,
        noscript]

    cdef bint is_end_tag = False
    ctx.node = base_node.node

    while ctx.node != NULL:
        # Skip everything except element and text nodes
        if ctx.node.type != LXB_DOM_NODE_TYPE_ELEMENT and ctx.node.type != LXB_DOM_NODE_TYPE_TEXT:
            ctx.node = next_node(base_node.node, ctx.node, &ctx.depth, &is_end_tag)
            continue

        # Skip unwanted element nodes
        if tag_name[:tag_name_len] in skip_elements:
            ctx.node = next_node(base_node.node, ctx.node, &ctx.depth, &is_end_tag)
            continue

        if not is_end_tag:
            _extract_start_cb(&ctx)
        else:
            _extract_end_cb(&ctx)

        ctx.node = next_node(base_node.node, ctx.node, &ctx.depth, &is_end_tag)

    return ctx.text.decode().strip()
