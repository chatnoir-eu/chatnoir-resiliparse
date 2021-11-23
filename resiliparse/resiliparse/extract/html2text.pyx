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
from resiliparse_inc.cctype cimport isspace
from resiliparse.parse.html cimport *
from resiliparse_inc.lexbor cimport *
from resiliparse_inc.re2 cimport Options as RE2Options, RE2Stack as RE2, StringPiece, PartialMatch
from resiliparse_inc.utility cimport move


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


cdef string _get_collapsed_string(const string_view& input_str, ExtractContext* ctx) nogil:
    """
    Collapse newlines and consecutive white space in a string to single spaces.
    Takes into account previously extracted text from ``ctx.text``.
    """
    if input_str.empty():
        return string()

    cdef string element_text

    # Pre-formatted context, return string as is, but add list indents
    cdef size_t i
    if ctx.opts.preserve_formatting and ctx.pre_depth > 0:
        if ctx.list_depth > 0:
            element_text.reserve(input_str.size())
            for i in range(input_str.size()):
                element_text.push_back(input_str[i])
                if input_str[i] == b'\n':
                    insert_list_indent(element_text, ctx)
            return element_text
        return <string>input_str

    # Otherwise collapse white space
    element_text.reserve(input_str.size())
    for i in range(input_str.size()):
        if isspace(input_str[i]):
            if (element_text.empty() and not ctx.text.empty() and not isspace(ctx.text.back().back())) or \
                    (not element_text.empty() and not isspace(element_text.back())):
                element_text.push_back(b' ')
        else:
            element_text.push_back(input_str[i])

    if element_text.capacity() > element_text.size() // 2:
        element_text.reserve(element_text.size())    # Shrink to fit
    return element_text


cdef string LIST_BULLET = <const char*>b'\xe2\x80\xa2'


cdef inline void insert_list_indent(string& element_text, ExtractContext* ctx) nogil:
    if ctx.list_depth == 0 \
            or (not element_text.empty() and element_text.back() != b'\n') \
            or (element_text.empty() and not ctx.text.empty() and ctx.text.back().back() != b'\n'):
        return

    element_text.append(string(2 * ctx.list_depth, <char>b' '))
    cdef lxb_dom_node_t* node = ctx.node if ctx.node.type != LXB_DOM_NODE_TYPE_TEXT else ctx.node.parent
    if ctx.opts.list_bullets and node.local_name != LXB_TAG_LI:
        # Add an additional two spaces if element is not the li element itself or a direct text child
        element_text.append(b'  ')


cdef inline void _add_space(ExtractContext* ctx) nogil:
    """Add space if last character is a non-space character."""
    if not ctx.text.empty() and not isspace(ctx.text.back().back()):
        ctx.text.back().push_back(<char>b' ')


cdef inline void _make_block(ExtractContext* ctx) nogil:
    """Make a block start or end by inserting the needed number of linefeeds."""

    # Strip previous linefeeds to prevent excess empty lines
    while not ctx.text.empty() and isspace(ctx.text.back().back()):
        ctx.text[ctx.text.size() - 1] = rstrip_str(ctx.text.back())
        if ctx.text.back().empty():
            ctx.text.pop_back()

    if not ctx.opts.preserve_formatting:
        _add_space(ctx)
        return

    cdef bint block_creates_newline = ctx.newline_before_next_block and not ctx.lstrip_next_block
    cdef string ws
    if not ctx.text.empty() and (not ctx.lstrip_next_block or not ctx.opts.list_bullets):
        if ctx.node.local_name in [LXB_TAG_H1, LXB_TAG_H2, LXB_TAG_H3, LXB_TAG_H4, LXB_TAG_H5, LXB_TAG_H6, LXB_TAG_P]:
            block_creates_newline = not ctx.lstrip_next_block
        ws = <const char*>b'\n\n' if block_creates_newline else <const char*>b'\n'

    insert_list_indent(ws, ctx)
    if ctx.space_before_next_block:
        ws.push_back(b' ')
    if not ws.empty():
        ctx.text.push_back(move(ws))


cdef bint _is_unprintable_pua(lxb_dom_node_t* node) nogil:
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
    if element_text == b'\xc2\xb6':
        return False

    # BMP private use area (probably an icon font)
    cdef uint32_t cp = 0
    if element_text.size() == 3:
        memcpy(&cp, element_text.data(), 3 * sizeof(char))
        if 0x8080ee <= cp <= 0xbfa3ef:
            return True

    return False


cdef void _extract_start_cb(ExtractContext* ctx) nogil:
    """Extraction start element callback."""
    cdef lxb_dom_character_data_t* node_char_data = NULL
    cdef string_view node_attr_data
    cdef string element_text
    cdef size_t i

    if ctx.node.type == LXB_DOM_NODE_TYPE_TEXT:
        if ctx.space_before_next_block:
            _add_space(ctx)

        node_char_data = <lxb_dom_character_data_t*>ctx.node
        element_text.append(<const char*>node_char_data.data.data, node_char_data.data.length)
        element_text = _get_collapsed_string(<string_view>element_text, ctx)
        if not rstrip_str(element_text).empty():
            ctx.newline_before_next_block = False
            ctx.lstrip_next_block = False
            ctx.space_before_next_block = False
            ctx.text.push_back(move(element_text))
        else:
            _add_space(ctx)
        return

    if ctx.node.type != LXB_DOM_NODE_TYPE_ELEMENT:
        return

    cdef bint last_was_space = not ctx.text.empty() and isspace(ctx.text.back().back())

    # Alternative descriptions
    if ctx.opts.alt_texts and ctx.node.local_name in [LXB_TAG_IMG, LXB_TAG_AREA]:
        node_attr_data = get_node_attr_sv(ctx.node, b'alt')
        if not node_attr_data.empty():
            if not last_was_space:
                element_text.push_back(b' ')
            element_text.append(_get_collapsed_string(node_attr_data, ctx))
            element_text.push_back(b' ')
            ctx.text.push_back(move(element_text))
        return

    # Form field elements
    if ctx.opts.form_fields:
        if ctx.node.local_name in [LXB_TAG_TEXTAREA, LXB_TAG_BUTTON] and ctx.node.first_child:
            ctx.pre_depth += <int>ctx.node.local_name == LXB_TAG_TEXTAREA
            if not last_was_space:
                element_text.push_back(<char>b' ')
            element_text.append(b'[ ')
        elif ctx.node.local_name == LXB_TAG_INPUT:
            node_attr_data = get_node_attr_sv(ctx.node, b'type')
            if node_attr_data.empty() or node_attr_data not in \
                    [b'checkbox', b'color', b'file', b'hidden', b'radio', b'reset']:
                node_attr_data = get_node_attr_sv(ctx.node, b'value')
                if node_attr_data.empty():
                    node_attr_data = get_node_attr_sv(ctx.node, b'placeholder')
                if not node_attr_data.empty():
                    if not last_was_space:
                        element_text.push_back(<char>b' ')
                    element_text.append(b'[ ')
                    element_text.append(_get_collapsed_string(node_attr_data, ctx))
                    if not isspace(element_text.back()):
                        element_text.push_back(b' ')
                    element_text.append(b'] ')
                    ctx.text.push_back(move(element_text))
                return

    # Short-circuit empty blocks (such as <br>, <hr>, or empty <divs>)
    if not ctx.opts.preserve_formatting or not ctx.node.first_child:
        if is_block_element(ctx.node.local_name):
            _make_block(ctx)
        return

    cdef size_t tag_name_len
    cdef const lxb_char_t* tag_name = lxb_dom_element_qualified_name(<lxb_dom_element_t*>ctx.node, &tag_name_len)

    # Block formatting
    if is_block_element(ctx.node.local_name):
        if ctx.node.local_name == LXB_TAG_LI:
            ctx.lstrip_next_block = False
            ctx.space_before_next_block = False
        if ctx.node.local_name == LXB_TAG_UL and ctx.node.parent and ctx.node.parent.local_name == LXB_TAG_LI:
            ctx.newline_before_next_block = False
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

    # List items
    if ctx.node.local_name == LXB_TAG_LI:
        if ctx.opts.list_bullets:
            insert_list_indent(element_text, ctx)
            if ctx.node.parent.local_name == LXB_TAG_OL:
                element_text.append(to_string(preinc(ctx.list_numbering.back())) + <const char*>b'. ')
            else:
                element_text.append(LIST_BULLET)
            ctx.space_before_next_block = True
        ctx.lstrip_next_block = True

    if not element_text.empty():
        ctx.text.push_back(move(element_text))


cdef void _extract_end_cb(ExtractContext* ctx) nogil:
    """Extraction end element callback."""
    if ctx.node.type != LXB_DOM_NODE_TYPE_ELEMENT or not ctx.opts.preserve_formatting:
        return

    if ctx.text.empty():
        ctx.text.push_back(<const char*>b'')

    # Headings and paragraphs insert newlines
    if ctx.node.local_name in [LXB_TAG_H1, LXB_TAG_H2, LXB_TAG_H3, LXB_TAG_H4, LXB_TAG_H5, LXB_TAG_H6, LXB_TAG_P]:
        ctx.newline_before_next_block = True

    # Pre-formatted text
    if ctx.node.local_name == LXB_TAG_PRE:
        ctx.pre_depth -= 1

    # Lists
    if ctx.node.local_name == LXB_TAG_UL:
        predec(ctx.list_depth)
    elif ctx.node.local_name == LXB_TAG_OL:
        predec(ctx.list_depth)
        ctx.list_numbering.pop_back()

    # Forms
    if ctx.opts.form_fields and ctx.node.local_name in [LXB_TAG_TEXTAREA, LXB_TAG_BUTTON]:
        ctx.pre_depth -= <int>ctx.node.local_name == LXB_TAG_TEXTAREA
        if not isspace(ctx.text.back().back()):
            ctx.text.back().push_back(b' ')
        ctx.text.back().append(b'] ')

    # Add tabs between table cells
    if ctx.node.local_name in [LXB_TAG_TD, LXB_TAG_TH]:
        ctx.text.back().append(b'\t\t')

    # No additional white space after table rows
    if ctx.node.local_name == LXB_TAG_TR and not ctx.text.empty():
        ctx.newline_before_next_block = False

    # Link targets
    cdef string_view link_href
    if ctx.opts.links and ctx.node.local_name == LXB_TAG_A:
        link_href = get_node_attr_sv(ctx.node, b'href')
        if not link_href.empty():
            if not isspace(ctx.text.back().back()):
                ctx.text.back().push_back(b' ')
            ctx.text.back().push_back(b'(')
            ctx.text.back().append(_get_collapsed_string(link_href, ctx))
            ctx.text.back().append(b')')

    cdef string stripped
    if ctx.node.local_name == LXB_TAG_LI:
        # Clean up empty list items
        while not ctx.text.empty():
            stripped = strip_str(ctx.text.back())
            if not stripped.empty() and stripped != LIST_BULLET:
                break
            ctx.text.pop_back()

        # No additional white space after list items
        ctx.newline_before_next_block = False
        ctx.lstrip_next_block = False

    if is_block_element(ctx.node.local_name):
        _make_block(ctx)


cdef RE2Options re_opts
re_opts.set_case_sensitive(False)

cdef RE2 wrapper_cls_regex = RE2(b'(?:^|[\\s_-])wrap(?:per)?(?:$|[\\s_-])', re_opts)
cdef RE2 nav_cls_regex = RE2(b'(?:^|[\\s_-])(?:nav(?:bar|igation)?|menu(?:[_-]item)?)(?:$|\\s)', re_opts)
cdef RE2 footer_cls_regex = RE2(b'(?:^|\\s)(?:(?:global|page|site|copyright)[_-]?)?footer(?:[_-]?(?:section|wrapper)?)(?:^|\\s)', re_opts)
cdef RE2 sidebar_cls_regex = RE2(b'(?:^|[\\s_-])sidebar(?:$|\\s)', re_opts)
cdef RE2 search_cls_regex = RE2(b'(?:^|[\\s_-])search(?:[_-]?(?:bar|facility|box))?(?:$|\\s)', re_opts)
cdef RE2 skip_cls_regex = RE2(b'(?:^|[\\s_-])(?:skip|skip-to|skiplink|scroll-(?:up|down))(?:$|[\\s_-])', re_opts)
cdef RE2 display_cls_regex = RE2(b'(?:^|\\s)(?:is[_-])?(?:display-none|hidden|invisible|collapsed|h-0)(?:-xs|-sm|-lg|-xl)?(?:$|\\s)', re_opts)
cdef RE2 display_css_regex = RE2(b'(?:^|;\\s)(?:display\\s?:\\s?none|visibility\\s?:\\s?hidden)(?:$|\\s?;)', re_opts)
cdef RE2 modal_cls_regex = RE2(b'(?:^|\\s)(?:modal|popup|lightbox|dropdown)(?:$|\\s)', re_opts)
cdef RE2 ads_cls_regex = RE2(b'(?:^|[\\s_-])(?:google[_-])?(?:ad(?:vert|vertisement)?|widead|banner|promoted)(?:[_-][a-f0-9]+)?(?:$|\\s)', re_opts)
cdef RE2 social_cls_regex = RE2(b'(?:^|\\s)(?:social(?:media)?|share|sharing|feedback|facebook|twitter)(?:[_-](?:links|section))?(?:$|\\s)', re_opts)


cdef inline bint regex_search_not_empty(const StringPiece& s, const RE2& r) nogil:
    if s.empty():
        return False
    return PartialMatch(s, r())


cdef bint _is_main_content_node(lxb_dom_node_t* node) nogil:
    """Check with simple heuristics if node belongs to main content."""

    if node.type != LXB_DOM_NODE_TYPE_ELEMENT:
        return True

    cdef bint is_block = is_block_element(node.local_name)


    # ------ Section 1: Inline and block elements ------

    if not is_block and _is_unprintable_pua(node):
        return False

    # Hidden elements
    if lxb_dom_element_has_attribute(<lxb_dom_element_t*>node, <lxb_char_t*>b'hidden', 6):
        return False

    # ARIA hidden
    if get_node_attr_sv(node, b'aria-hidden') == b'true':
        return False

    # ARIA roles
    cdef string_view role_attr = get_node_attr_sv(node, b'role')
    if not role_attr.empty() and role_attr in [b'contentinfo', b'img', b'menu', b'menubar', b'navigation', b'menuitem',
                                               b'alert', b'dialog', b'checkbox', b'radio']:
        return False

    # Block element matching based only on tag name
    cdef size_t length_to_body = 0
    cdef lxb_dom_node_t* pnode = node.parent
    cdef bint footer_is_last_body_child = True
    if is_block:
        while pnode.local_name != LXB_TAG_BODY and pnode.parent:
            preinc(length_to_body)
            pnode = pnode.parent

        # Main elements
        if node.local_name == LXB_TAG_MAIN:
            return True

        # Global footer
        if node.local_name == LXB_TAG_FOOTER:
            if length_to_body < 3:
                return False

            # Check if footer is recursive last element node of a direct body child
            pnode = node
            while pnode and pnode.parent and pnode.parent.local_name != LXB_TAG_BODY:
                if pnode.next and pnode.next.type == LXB_DOM_NODE_TYPE_TEXT:
                    pnode = pnode.next
                if pnode.next:
                    # There is at least one more element node
                    footer_is_last_body_child = False
                    break
                pnode = pnode.parent
            if footer_is_last_body_child:
                return False

        # Global navigation
        if node.local_name in [LXB_TAG_UL, LXB_TAG_NAV] and length_to_body < 3:
            return False

        # Global aside
        if node.local_name == LXB_TAG_ASIDE and length_to_body < 3:
            return False

        # Iframes
        if node.local_name == LXB_TAG_IFRAME:
            return False


    # ------ Section 2: General class and id matching ------

    cdef StringPiece cls_attr = get_node_attr_sp(node, b'class')
    cdef StringPiece id_attr = get_node_attr_sp(node, b'id')
    if cls_attr.empty() and id_attr.empty():
        return True

    cdef string cls_and_id_attr_str = cls_attr.as_string()
    if not cls_and_id_attr_str.empty():
        cls_and_id_attr_str.push_back(b' ')
    cls_and_id_attr_str.append(id_attr.as_string())
    cdef StringPiece cls_and_id_attr = StringPiece(cls_and_id_attr_str)

    # Hidden elements
    if regex_search_not_empty(cls_attr, display_cls_regex) \
            or regex_search_not_empty(get_node_attr_sp(node, b'style'), display_css_regex):
        return False

    # Skip links
    if node.local_name in [LXB_TAG_A, LXB_TAG_SPAN, LXB_TAG_LI] and regex_search_not_empty(cls_attr, skip_cls_regex):
        return False

    # Social media and feedback forms
    if regex_search_not_empty(cls_attr, social_cls_regex):
        return False

    # Ads
    if regex_search_not_empty(cls_and_id_attr, ads_cls_regex) \
            or lxb_dom_element_has_attribute(<lxb_dom_element_t*>node, <lxb_char_t*>b'data-ad', 7) \
            or lxb_dom_element_has_attribute(<lxb_dom_element_t*>node, <lxb_char_t*>b'data-advertisement', 18) \
            or lxb_dom_element_has_attribute(<lxb_dom_element_t*>node, <lxb_char_t*>b'data-text-ad', 12):
        return False


    # ------ Section 3: Class and id matching of block elements only ------

    if not is_block:
        return True

    # Global footer
    if regex_search_not_empty(cls_and_id_attr, footer_cls_regex) and length_to_body < 8:
        return False

    # Wrapper elements (whitelist them, they may contain more specific elements)
    if node.local_name in [LXB_TAG_SECTION, LXB_TAG_DIV] and regex_search_not_empty(cls_and_id_attr, wrapper_cls_regex):
        return True

    # Global navigation
    if length_to_body < 12 and node.local_name in [LXB_TAG_UL, LXB_TAG_HEADER, LXB_TAG_NAV, LXB_TAG_SECTION]:
        if regex_search_not_empty(cls_and_id_attr, nav_cls_regex):
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

    return True


def extract_plain_text(DOMNode base_node, bint preserve_formatting=True, bint main_content=False, bint list_bullets=True,
                       bint alt_texts=True, bint links=False, bint form_fields=False, bint noscript=False,
                       skip_elements=None):
    """
    extract_plain_text(base_node, preserve_formatting=True, preserve_formatting=True, main_content=False, \
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

    ctx.node = base_node.node

    cdef vector[string] skip_elements_vec = list(skip_elements)
    cdef const lxb_char_t* tag_name = NULL
    cdef size_t tag_name_len
    cdef string tag_name_str
    cdef size_t i
    cdef bint skip = False
    cdef bint is_end_tag = False

    if ctx.node.type == LXB_DOM_NODE_TYPE_DOCUMENT:
        ctx.root_node = next_element_node(ctx.node, ctx.node.first_child)
        ctx.node = ctx.root_node

    with nogil:
        while ctx.node:
            # Skip everything except element and text nodes
            if ctx.node.type != LXB_DOM_NODE_TYPE_ELEMENT and ctx.node.type != LXB_DOM_NODE_TYPE_TEXT:
                is_end_tag = True
                ctx.node = next_node(ctx.root_node, ctx.node, &ctx.depth, &is_end_tag)
                continue

            # Skip unwanted element nodes
            if ctx.node.type == LXB_DOM_NODE_TYPE_ELEMENT:
                tag_name = lxb_dom_element_qualified_name(<lxb_dom_element_t*>ctx.node, &tag_name_len)
                tag_name_str = string(<const char*>tag_name, tag_name_len)
                skip = False
                for i in range(skip_elements_vec.size()):
                    if skip_elements_vec[i] == tag_name_str:
                        skip = True
                        break

                if skip or (main_content and not _is_main_content_node(ctx.node)):
                    is_end_tag = True
                    ctx.node = next_node(ctx.root_node, ctx.node, &ctx.depth, &is_end_tag)
                    continue

            if not is_end_tag:
                _extract_start_cb(&ctx)
            else:
                _extract_end_cb(&ctx)

            ctx.node = next_node(ctx.root_node, ctx.node, &ctx.depth, &is_end_tag)

    return ''.join(s.decode() for s in ctx.text).rstrip()
