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
from libc.stdint cimport uint32_t
from libc.string cimport memcpy
from libcpp.string cimport string, to_string
from libcpp.vector cimport vector

from resiliparse_common.string_util cimport rstrip_str, strip_str
from resiliparse_inc.boost_regex cimport flag_type, regex, regex_search
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
    lxb_dom_node_t* root_node
    lxb_dom_node_t* node
    size_t depth
    size_t list_depth
    size_t pre_depth
    vector[size_t] list_numbering
    size_t space_before_next_block
    size_t newline_before_next_block
    size_t lstrip_next_block
    vector[string] text
    ExtractOpts opts


cdef string _get_collapsed_string(const string& input_str, ExtractContext* ctx):
    """
    Collapse newlines and consecutive white space in a string to single spaces.
    Takes into account previously extracted text from ``ctx.text``.
    """
    cdef string element_text

    if input_str.empty():
        return string()

    element_text.reserve(input_str.size())

    # Pre-formatted context, return string as is, but add list indents
    cdef string tmp
    cdef size_t i
    if ctx.opts.preserve_formatting and ctx.pre_depth > 0:
        if ctx.list_depth > 0:
            tmp.reserve(element_text.size())
            for i in range(element_text.size()):
                tmp.push_back(element_text[i])
                if element_text[i] == <char>b'\n':
                    tmp.append(string(2 * ctx.list_depth + 2, <char>b' '))
            return tmp
        return element_text

    # Otherwise collapse white space
    for i in range(input_str.size()):
        if isspace(input_str[i]):
            if (element_text.empty() and not ctx.text.empty() and not isspace(ctx.text.back().back())) or \
                    (not element_text.empty() and not isspace(element_text.back())):
                element_text.push_back(<char>b' ')
        else:
            element_text.push_back(input_str[i])

    element_text.reserve(element_text.size())    # Shrink to fit
    return element_text


cdef string LIST_BULLET = <char*>b'\xe2\x80\xa2'


cdef inline void _make_block(ExtractContext* ctx):
    """Make a block start or end by inserting the needed number of linefeeds."""

    # Strip previous linefeeds to prevent excess empty lines
    while not ctx.text.empty() and isspace(ctx.text.back().back()):
        ctx.text[ctx.text.size() - 1] = rstrip_str(ctx.text.back())
        if ctx.text.back().empty():
            ctx.text.pop_back()

    if ctx.text.empty():
        return

    # Headings and paragraphs enforce newlines
    cdef bint block_creates_newline = ctx.newline_before_next_block
    if ctx.node.local_name in [LXB_TAG_H1, LXB_TAG_H2, LXB_TAG_H3, LXB_TAG_H4, LXB_TAG_H5, LXB_TAG_H6, LXB_TAG_P]:
        block_creates_newline = True

    if not ctx.lstrip_next_block:
        ctx.text.back().append(<char*>b'\n\n' if block_creates_newline else <char*>b'\n')
    if ctx.space_before_next_block:
        ctx.text.back().push_back(<char>b' ')


cdef bint _is_unprintable_pua(lxb_dom_node_t* node):
    """Whether text node contains only a single unprintable code point from the private use area."""
    if node.first_child and (node.first_child.next or node.first_child.type != LXB_DOM_NODE_TYPE_TEXT):
        # Node has more than one child
        return False
    if not node.first_child and node.type != LXB_DOM_NODE_TYPE_TEXT:
        return False

    cdef string element_text = strip_str(get_node_text(node))
    if element_text.size() > 3:
        return False

    # Pilcrow character (probably an anchor link)
    if element_text == <char*>b'\xc2\xb6':
        return False

    # BMP private use area (probably an icon font)
    cdef uint32_t cp = 0
    if element_text.size() == 3:
        memcpy(&cp, element_text.data(), 3 * sizeof(char))
        if 0x8080ee <= cp <= 0xbfa3ef:
            return True

    return False


cdef void _extract_start_cb(ExtractContext* ctx):
    """Extraction start element callback."""
    cdef lxb_dom_character_data_t* node_char_data = NULL
    cdef string node_attr_data
    cdef string element_text
    cdef size_t i

    if ctx.node.type == LXB_DOM_NODE_TYPE_TEXT:
        node_char_data = <lxb_dom_character_data_t*>ctx.node
        if ctx.space_before_next_block:
            element_text.push_back(<char>b' ')
        element_text.append(<char*>node_char_data.data.data, node_char_data.data.length)
        element_text = _get_collapsed_string(element_text, ctx)
        if not rstrip_str(element_text).empty():
            ctx.newline_before_next_block = False
            ctx.lstrip_next_block = False
            ctx.space_before_next_block = False
        if not element_text.empty():
            ctx.text.push_back(element_text)
        return

    if ctx.node.type != LXB_DOM_NODE_TYPE_ELEMENT:
        return

    # Alternative descriptions
    if ctx.opts.alt_texts and ctx.node.local_name in [LXB_TAG_IMG, LXB_TAG_AREA]:
        node_attr_data = get_node_attr(ctx.node, <char*>b'alt')
        if not node_attr_data.empty():
            element_text.append(_get_collapsed_string(node_attr_data, ctx))
            element_text.push_back(<char>b' ')
            ctx.text.push_back(element_text)
        return

    # Form field elements
    if ctx.opts.form_fields:
        if ctx.node.local_name in [LXB_TAG_TEXTAREA, LXB_TAG_BUTTON] and ctx.node.first_child:
            ctx.pre_depth += <int>ctx.node.local_name == LXB_TAG_TEXTAREA
            element_text.append(<char*>b'[ ')
        elif ctx.node.local_name == LXB_TAG_INPUT and get_node_attr(ctx.node, <char*>b'type') not in \
                [<char*>b'checkbox', <char*>b'color', <char*>b'file', <char*>b'hidden',
                 <char*>b'radio', <char*>b'reset']:
            node_attr_data = get_node_attr(ctx.node, <char*>b'value')
            if node_attr_data.empty():
                node_attr_data = get_node_attr(ctx.node, <char*>b'placeholder')
            if not node_attr_data.empty():
                element_text.append(<char*>b'[ ')
                element_text.append(_get_collapsed_string(node_attr_data, ctx))
                if not isspace(element_text.back()):
                    element_text.push_back(<char>b' ')
                element_text.append(<char*>b'] ')
                ctx.text.push_back(element_text)
            return

    if not ctx.opts.preserve_formatting:
        return

    cdef size_t tag_name_len
    cdef const lxb_char_t* tag_name = lxb_dom_element_qualified_name(<lxb_dom_element_t*>ctx.node, &tag_name_len)

    # Block formatting
    if is_block_element(ctx.node.local_name):
        if ctx.node.local_name == LXB_TAG_LI:
            ctx.lstrip_next_block = False
            ctx.space_before_next_block = False
        _make_block(ctx)

    # Pre-formatted text
    if ctx.node.local_name == LXB_TAG_PRE:
        ctx.pre_depth += 1

    # Lists
    if ctx.node.local_name == LXB_TAG_UL:
        preinc(ctx.list_depth)
    elif ctx.node.local_name == LXB_TAG_OL:
        preinc(ctx.list_depth)
        ctx.list_numbering.push_back(0)

    # List item indents
    if ctx.opts.list_bullets and ctx.list_depth > 0 and not ctx.text.empty() and ctx.text.back().back() == b'\n':
        element_text.append(string(2 * ctx.list_depth, <char>b' '))
        if ctx.node.local_name != LXB_TAG_LI:
            # Add an additional two spaces if element is not the li element itself
            element_text.append(<char*>b'  ')

    # List items
    if ctx.opts.list_bullets and ctx.node.local_name == LXB_TAG_LI:
        if ctx.node.parent.local_name == LXB_TAG_OL:
            element_text.append(to_string(preinc(ctx.list_numbering.back())) + <char*>b'. ')
        else:
            element_text.append(LIST_BULLET)
        ctx.lstrip_next_block = True
        ctx.space_before_next_block = True

    if not element_text.empty():
        ctx.text.push_back(element_text)


cdef void _extract_end_cb(ExtractContext* ctx):
    """Extraction end element callback."""
    if ctx.node.type != LXB_DOM_NODE_TYPE_ELEMENT or not ctx.opts.preserve_formatting:
        return

    # Headings and paragraphs insert newlines
    if ctx.node.local_name in [LXB_TAG_H1, LXB_TAG_H2, LXB_TAG_H3, LXB_TAG_H4, LXB_TAG_H5, LXB_TAG_H6, LXB_TAG_P]:
        ctx.newline_before_next_block = True

    # Pre-formatted text
    if ctx.node.local_name == LXB_TAG_PRE:
        ctx.pre_depth -= 1

    # Forms
    if ctx.opts.form_fields and ctx.node.local_name in [LXB_TAG_TEXTAREA, LXB_TAG_BUTTON]:
        ctx.pre_depth -= <int>ctx.node.local_name == LXB_TAG_TEXTAREA
        if not isspace(ctx.text.back().back()):
            ctx.text.back().push_back(<char>b' ')
        ctx.text.back().append(<char*>b'] ')

    # Add tabs between table cells
    if ctx.node.local_name in [LXB_TAG_TD, LXB_TAG_TH]:
        ctx.text.back().append(<char*>b'\t\t')

    # No additional white space after table rows
    if ctx.node.local_name == LXB_TAG_TR and not ctx.text.empty():
        ctx.newline_before_next_block = False

    # Link targets
    cdef string link_href
    if ctx.opts.links and ctx.node.local_name == LXB_TAG_A:
        link_href = get_node_attr(ctx.node, <char*>b'href')
        if not link_href.empty():
            if not isspace(ctx.text.back().back()):
                ctx.text.back().push_back(<char>b' ')
            ctx.text.back().push_back(<char>b'(')
            ctx.text.back().append(_get_collapsed_string(link_href, ctx))
            ctx.text.back().append(<char*>b')')

    # Lists
    if ctx.node.local_name == LXB_TAG_UL:
        predec(ctx.list_depth)
    elif ctx.node.local_name == LXB_TAG_OL:
        predec(ctx.list_depth)
        ctx.list_numbering.pop_back()

    if ctx.node.local_name == LXB_TAG_LI:
        # Clean up empty list items
        while not ctx.text.empty() and strip_str(ctx.text.back()) == LIST_BULLET:
            ctx.text.pop_back()
        if not ctx.text.empty() and ctx.text.back().back() != <char>b'\n':
            ctx.text.back().push_back(<char>b'\n')
        # No additional white space after list items
        ctx.newline_before_next_block = False

    # Add newline after block elements if next element is text
    if ctx.node.next and ctx.node.next.type == LXB_DOM_NODE_TYPE_TEXT and is_block_element(ctx.node.local_name):
        _make_block(ctx)


cdef regex wrapper_cls_regex = regex(<char*>b'(?:^|[\\s_-])(?>wrap(?>per)?)(?:$|[\\s_-])', flag_type.icase)
cdef regex nav_cls_regex = regex(<char*>b'(?:^|[\\s_-])(?>nav(?>bar|igation)?|menu(?>[_-]item)?)(?:$|\\s)', flag_type.icase)
cdef regex footer_cls_regex = regex(<char*>b'(?:^|[\\s_-])(?:(?:global|page|site)[_-]?)?(?>footer)(?:[_-]?(?:section|wrapper)?)(?:^|\\s)', flag_type.icase)
cdef regex sidebar_cls_regex = regex(<char*>b'(?:^|[\\s_-])(?>nav(?:igation)?-|menu-|global-)sidebar(?:$|\\s)', flag_type.icase)
cdef regex search_cls_regex = regex(<char*>b'(?:^|[\\s_-])(?:(?:global|page|site)[_-]?)?(?>search)(?>[_-]?(?>bar|facility|box))?(?:$|\\s)', flag_type.icase)
cdef regex skip_cls_regex = regex(<char*>b'(?:^|[\\s_-])(?>skip|skip-to|skiplink|scroll-(?>up|down))(?:$|[\\s_-])', flag_type.icase)
cdef regex display_cls_regex = regex(<char*>b'(?:^|\\s)(?:is[_-])?(?>display-none|hidden|invisible|collapsed|h-0)(?:-xs|-sm|-lg|-xl)?(?:$|\\s)', flag_type.icase)
cdef regex display_css_regex = regex(<char*>b'(?:^|;\\s)(?>display\\s?:\\s?none|visibility\\s?:\\s?hidden)(?:$|\\s?;)', flag_type.icase)
cdef regex landmark_id_regex = regex(<char*>b'^(?:global[_-]?)?(?>footer|sidebar|nav(?>igation)?)$', flag_type.icase)
cdef regex modal_cls_regex = regex(<char*>b'(?:^|\\s)(?>modal|popup|lightbox|dropdown)(?:$|\\s)', flag_type.icase)
cdef regex ads_cls_regex = regex(<char*>b'(?:^|[\\s_-])(?:google[_-])?(?>ad(?>vert|vertisement)?|widead|banner|promoted)(?:[_-][a-f0-9]+)?(?:$|\\s)', flag_type.icase)
cdef regex social_cls_regex = regex(<char*>b'(?:^|\\s)(?>social(?>media)?|share|sharing|feedback|facebook|twitter)(?:[_-](?:links|section))?(?:$|\\s)', flag_type.icase)


cdef inline bint regex_search_not_empty(const string& s, const regex& r):
    if s.empty():
        return False
    return regex_search(s, r)


cdef bint _is_main_content_node(lxb_dom_node_t* node) except -1:
    """Check with simple heuristics if node belongs to main content."""

    cdef size_t length_to_body = 0
    cdef lxb_dom_node_t* pnode = node.parent
    while pnode.local_name != LXB_TAG_BODY and pnode.parent:
        preinc(length_to_body)
        pnode = pnode.parent

    # Main elements
    if node.type != LXB_DOM_NODE_TYPE_ELEMENT or node.local_name == LXB_TAG_MAIN:
        return True

    # Global navigation
    if node.local_name in [LXB_TAG_UL, LXB_TAG_NAV] and length_to_body < 3:
        return False

    # Global aside
    if node.local_name == LXB_TAG_ASIDE and length_to_body < 3:
        return False

    # Iframes
    if node.local_name == LXB_TAG_IFRAME:
        return False

    # Hidden elements
    if lxb_dom_element_has_attribute(<lxb_dom_element_t*>node, <lxb_char_t*>b'hidden', 6):
        return False

    # ARIA hidden
    if get_node_attr(node, <char*>b'aria-hidden') == <char*>b'true':
        return False

    # ARIA roles
    if get_node_attr(node, <char*>b'role') in [<char*>b'contentinfo', <char*>b'img', <char*>b'menu', <char*>b'menubar',
                                               <char*>b'navigation', <char*>b'menuitem', <char*>b'alert',
                                               <char*>b'dialog', <char*>b'checkbox', <char*>b'radio']:
        return False

    cdef string cls_attr = get_node_attr(node, <char*>b'class')
    cdef string id_attr = get_node_attr(node, <char*>b'id')

    # From here on only rules depending on id or class attributes
    if cls_attr.empty() and id_attr.empty():
        return True

    cdef string cls_and_id_attr = cls_attr
    if not cls_and_id_attr.empty():
        cls_and_id_attr.push_back(<char>b' ')
    cls_and_id_attr.append(id_attr)

    # Global footer
    cdef bint is_last_body_child = True
    if node.local_name == LXB_TAG_FOOTER or regex_search_not_empty(cls_and_id_attr, footer_cls_regex):
        if length_to_body < 3:
            return False

    # Wrapper elements (whitelist them, they may contain more specific elements)
    if node.local_name in [LXB_TAG_SECTION, LXB_TAG_DIV] and regex_search_not_empty(cls_and_id_attr, wrapper_cls_regex):
        return True

    # Hidden elements with class
    if regex_search_not_empty(cls_attr, display_cls_regex):
        return False

    # Global navigation with class
    if node.local_name in [LXB_TAG_UL, LXB_TAG_HEADER, LXB_TAG_NAV, LXB_TAG_SECTION]:
        if regex_search_not_empty(cls_and_id_attr, nav_cls_regex):
            return False

        # Check if footer is recursive last element node of a direct body child
        pnode = node
        while pnode and pnode.parent and pnode.parent.local_name != LXB_TAG_BODY:
            if pnode.next and pnode.next.type == LXB_DOM_NODE_TYPE_TEXT:
                pnode = pnode.next
            if pnode.next:
                # There is at least one more element node
                is_last_body_child = False
                break
            pnode = pnode.parent
        if is_last_body_child:
            return False

    # Landmark IDs
    if regex_search_not_empty(id_attr, landmark_id_regex):
        return False

    # Global search bar
    if regex_search_not_empty(cls_and_id_attr, search_cls_regex):
        return False

    # Global sidebar
    if length_to_body < 4 and regex_search_not_empty(cls_and_id_attr, sidebar_cls_regex):
        return False

    # Modals
    if regex_search_not_empty(cls_and_id_attr, modal_cls_regex):
        return False

    if regex_search_not_empty(get_node_attr(node, <char*>b'style'), display_css_regex):
        return False

    # Skip links
    if node.local_name in [LXB_TAG_A, LXB_TAG_SPAN, LXB_TAG_LI] and regex_search_not_empty(cls_attr, skip_cls_regex):
        return False

    # Ads
    if regex_search_not_empty(cls_and_id_attr, ads_cls_regex) \
            or lxb_dom_element_has_attribute(<lxb_dom_element_t*>node, <lxb_char_t*>b'data-ad', 7) \
            or lxb_dom_element_has_attribute(<lxb_dom_element_t*>node, <lxb_char_t*>b'data-advertisment', 17) \
            or lxb_dom_element_has_attribute(<lxb_dom_element_t*>node, <lxb_char_t*>b'data-text-ad', 12):
        return False

    # Social media and feedback forms
    if regex_search_not_empty(cls_attr, social_cls_regex):
        return False

    return True


def extract_plain_text(DOMNode base_node, bint preserve_formatting=True, bint main_content=True, bint list_bullets=True,
                       bint alt_texts=True, bint links=False, bint form_fields=False, bint noscript=False,
                       skip_elements=None):
    """
    extract_plain_text(base_node, preserve_formatting=True, preserve_formatting=True, main_content=True, \
        list_bullets=True, alt_texts=False, links=True, form_fields=False, noscript=False, skip_elements=None)

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
    :param main_content: apply simple heuristics for extracting only "main-content" elements
    :type main_content: bool
    :param list_bullets: insert bullets / numbers for list items
    :type list_bullets: bool
    :param alt_texts: preserve alternative text descriptions
    :type alt_texts: bool
    :param links: extract link target URLs
    :type links: bool
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
    ctx.root_node = base_node.node
    ctx.depth = 0
    ctx.list_depth = 0
    ctx.pre_depth = 0
    ctx.space_before_next_block = False
    ctx.newline_before_next_block = False
    ctx.lstrip_next_block = False
    ctx.opts = [
        preserve_formatting,
        list_bullets,
        links,
        alt_texts,
        form_fields,
        noscript]

    cdef bint is_end_tag = False
    ctx.node = base_node.node

    while ctx.node:
        # Skip everything except element and text nodes
        if ctx.node.type != LXB_DOM_NODE_TYPE_ELEMENT and ctx.node.type != LXB_DOM_NODE_TYPE_TEXT:
            is_end_tag = True
            ctx.node = next_node(base_node.node, ctx.node, &ctx.depth, &is_end_tag)
            continue

        # Skip unwanted element nodes
        tag_name = lxb_dom_node_name(ctx.node, &tag_name_len)
        if tag_name[:tag_name_len].lower() in skip_elements \
                or (main_content and not _is_main_content_node(ctx.node)) \
                or _is_unprintable_pua(ctx.node):
            is_end_tag = True
            ctx.node = next_node(base_node.node, ctx.node, &ctx.depth, &is_end_tag)
            continue

        if not is_end_tag:
            _extract_start_cb(&ctx)
        else:
            _extract_end_cb(&ctx)

        ctx.node = next_node(base_node.node, ctx.node, &ctx.depth, &is_end_tag)

    return ''.join(s.decode() for s in ctx.text).rstrip()
