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

from cython.operator cimport dereference as deref, preincrement as preinc, predecrement as predec
from libcpp.set cimport set as stl_set
from libc.string cimport memcpy
from libcpp.memory cimport make_shared, shared_ptr
from libcpp.string cimport string, to_string
from libcpp.vector cimport vector

from resiliparse_common.string_util cimport lstrip_str, rstrip_str, strip_str, strip_sv
from resiliparse_inc.cctype cimport isspace
from resiliparse.parse.html cimport *
from resiliparse_inc.lexbor cimport *
from resiliparse_inc.re2 cimport Options as RE2Options, RE2Stack as RE2, PartialMatch
from resiliparse_inc.string_view cimport string_view
from resiliparse_inc.utility cimport move


__all__ = [
    'extract_plain_text',
]


cdef extern from * nogil:
    """
    enum FormattingOpts {
        FORMAT_OFF = 0,
        FORMAT_BASIC = 1,
        FORMAT_MINIMAL_HTML = 2
    };

    struct ExtractOpts {
        FormattingOpts preserve_formatting = FORMAT_BASIC;
        bool list_bullets = true;
        bool links = false;
        bool alt_texts = true;
        bool form_fields = false;
        bool noscript = false;
    };

    struct ExtractContext {
        lxb_dom_node_t* root_node = NULL;
        lxb_dom_node_t* node = NULL;
        size_t depth = 0;
        ExtractOpts opts;
    };

    struct ExtractNode {
        lxb_dom_node_t* reference_node = NULL;
        lxb_tag_id_t tag_id = LXB_TAG__UNDEF;
        size_t depth = 0;
        size_t pre_depth = 0;
        bool space_after = false;
        bool collapse_margins = true;
        bool make_block = true;
        bool make_big_block = false;
        bool is_end_tag = false;
        bool escape_text_contents = false;
        std::shared_ptr<std::string> text_contents = NULL;
    };
    """
    ctypedef enum FormattingOpts:
        FORMAT_OFF
        FORMAT_BASIC
        FORMAT_MINIMAL_HTML

    cdef struct ExtractOpts:
        FormattingOpts preserve_formatting
        bint list_bullets
        bint links
        bint alt_texts
        bint form_fields
        bint noscript

    cdef struct ExtractContext:
        lxb_dom_node_t * root_node
        lxb_dom_node_t * node
        size_t depth
        ExtractOpts opts

    cdef struct ExtractNode:
        lxb_dom_node_t* reference_node
        lxb_tag_id_t tag_id
        size_t depth
        size_t pre_depth
        bint space_after
        bint collapse_margins
        bint make_block
        bint make_big_block
        bint is_end_tag
        bint escape_text_contents
        shared_ptr[string] text_contents


cdef string _get_collapsed_string(const string& input_str) noexcept nogil:
    """
    Collapse newlines and consecutive white space in a string to single spaces.
    Takes into account previously extracted text from ``ctx.text``.
    """
    if input_str.empty():
        return input_str

    cdef string element_text
    element_text.reserve(input_str.size())
    for i in range(input_str.size()):
        if isspace(input_str[i]):
            if element_text.empty() or not isspace(element_text.back()):
                element_text.push_back(b' ')
        else:
            element_text.push_back(input_str[i])

    return element_text


cdef string LIST_BULLET = <const char*>b'\xe2\x80\xa2'


cdef inline void _ensure_text_contents(vector[shared_ptr[ExtractNode]]& extract_nodes) noexcept nogil:
    if not deref(extract_nodes.back()).text_contents:
        deref(extract_nodes.back()).text_contents = make_shared[string]()


cdef inline void _ensure_space(string& in_str, char space_char) noexcept nogil:
    if in_str.empty() or not isspace(in_str.back()):
        in_str.push_back(space_char)


cdef string _escape_html(const char* data, size_t length) noexcept nogil:
    cdef size_t i
    cdef char c
    cdef string data_escaped
    data_escaped.reserve(length)
    for i in range(length):
        c = data[i]
        if c == b'&':
            data_escaped.append(b'&amp;')
        elif c == b'"':
            data_escaped.append(b'&quot;')
        elif c == b'<':
            data_escaped.append(b'&lt;')
        elif c == b'>':
            data_escaped.append(b'&gt;')
        else:
            data_escaped.push_back(data[i])
    return data_escaped


cdef void _extract_cb(vector[shared_ptr[ExtractNode]]& extract_nodes, ExtractContext& ctx, bint is_end_tag) noexcept nogil:
    cdef shared_ptr[ExtractNode] current_node
    cdef shared_ptr[ExtractNode] last_node

    if not extract_nodes.empty():
        last_node = extract_nodes.back()
        current_node = last_node

    cdef bint is_block = ctx.node.type == LXB_DOM_NODE_TYPE_ELEMENT and is_block_element(ctx.node.local_name)

    if not last_node or is_block or ctx.depth < deref(last_node).depth or \
            (ctx.opts.links and ctx.node.local_name == LXB_TAG_A) or ctx.node.local_name == LXB_TAG_TEXTAREA:
        current_node = make_shared[ExtractNode]()
        extract_nodes.push_back(current_node)
        deref(current_node).reference_node = ctx.node
        deref(current_node).depth = ctx.depth
        deref(current_node).make_block = is_block
        deref(current_node).make_big_block = ctx.node.local_name in [LXB_TAG_P, LXB_TAG_H1, LXB_TAG_H2, LXB_TAG_H3, LXB_TAG_H4]
        deref(current_node).tag_id = ctx.node.local_name
        deref(current_node).pre_depth = deref(last_node).pre_depth if last_node else 0
        if ctx.node.local_name in [LXB_TAG_PRE, LXB_TAG_TEXTAREA]:
            current_node.get().pre_depth += 1 if not is_end_tag else -1
        deref(current_node).is_end_tag = is_end_tag
        deref(current_node).escape_text_contents = ctx.opts.preserve_formatting == FormattingOpts.FORMAT_MINIMAL_HTML

    cdef lxb_dom_character_data_t* node_char_data = NULL
    cdef string node_char_data_escaped
    cdef string element_text
    cdef string_view element_text_sv

    if ctx.node.type == LXB_DOM_NODE_TYPE_TEXT:
        _ensure_text_contents(extract_nodes)
        node_char_data = <lxb_dom_character_data_t*>ctx.node
        element_text = string(<const char*>node_char_data.data.data, node_char_data.data.length)

        if deref(current_node).tag_id == LXB_TAG_A and ctx.opts.preserve_formatting >= FormattingOpts.FORMAT_MINIMAL_HTML:
            # Escape <a> inner text
            element_text = _escape_html(element_text.data(), element_text.size())

        if not element_text.empty():
            deref(deref(current_node).text_contents).append(element_text)

    elif ctx.node.type != LXB_DOM_NODE_TYPE_ELEMENT:
        return

    elif ctx.node.local_name == LXB_TAG_BR and ctx.opts.preserve_formatting == FormattingOpts.FORMAT_BASIC:
        _ensure_text_contents(extract_nodes)
        deref(current_node).collapse_margins = False

    elif ctx.opts.links and ctx.node.local_name == LXB_TAG_A:
        element_text_sv = strip_sv(get_node_attr_sv(ctx.node, b'href'))
        _ensure_text_contents(extract_nodes)
        deref(current_node).make_block = False

        if ctx.opts.preserve_formatting == FormattingOpts.FORMAT_MINIMAL_HTML:
            if not is_end_tag:
                element_text.append(b'<a href="')
                element_text.append(_escape_html(element_text_sv.data(), element_text_sv.size()))
                element_text.append(b'">')
            else:
                element_text.append(b'</a>')
            deref(deref(current_node).text_contents).append(element_text)
            deref(current_node).escape_text_contents = False

        elif is_end_tag:
            element_text.append(b' (')
            element_text.append(<string>element_text_sv)
            element_text.push_back(b')')
            deref(deref(current_node).text_contents).append(element_text)

    elif ctx.opts.alt_texts and ctx.node.local_name in [LXB_TAG_IMG, LXB_TAG_AREA]:
        _ensure_text_contents(extract_nodes)
        element_text_sv = get_node_attr_sv(ctx.node, b'alt')
        if not element_text_sv.empty():
            deref(deref(current_node).text_contents).append(<string>element_text_sv)

    elif ctx.opts.form_fields and ctx.node.local_name in [LXB_TAG_TEXTAREA, LXB_TAG_BUTTON]:
        if not is_end_tag:
            element_text.append(b'[ ')
        else:
            element_text.append(b' ] ')
        _ensure_text_contents(extract_nodes)
        deref(deref(current_node).text_contents).append(element_text)

    elif ctx.opts.form_fields and ctx.node.local_name == LXB_TAG_INPUT:
        element_text_sv = strip_sv(get_node_attr_sv(ctx.node, b'type'))
        if element_text_sv.empty() or element_text_sv not in \
                [b'checkbox', b'color', b'file', b'hidden', b'radio', b'reset']:
            element_text_sv = strip_sv(get_node_attr_sv(ctx.node, b'value'))
            if element_text_sv.empty():
                element_text_sv = strip_sv(get_node_attr_sv(ctx.node, b'placeholder'))
            if not element_text_sv.empty():
                _ensure_text_contents(extract_nodes)
                element_text.append(b'[ ')
                element_text.append(<string> element_text_sv)
                element_text.append(b' ] ')
                _ensure_text_contents(extract_nodes)
                deref(deref(current_node).text_contents).append(element_text)


cdef inline void _make_indent(string& output, size_t list_depth, const ExtractNode* current_node, const ExtractOpts& opts) noexcept nogil:
    if not list_depth:
        return
    if opts.preserve_formatting == FormattingOpts.FORMAT_OFF:
        output = rstrip_str(move(output))
    output.append(list_depth * 2u, <char>b' ')


cdef inline void _make_margin(string& output, size_t& margin_size, const ExtractNode* current_node, const ExtractOpts& opts) noexcept nogil:
    if not margin_size:
        return
    if not current_node.pre_depth or opts.preserve_formatting == FormattingOpts.FORMAT_OFF:
        output = rstrip_str(move(output))
    if opts.preserve_formatting == FormattingOpts.FORMAT_OFF and not output.empty():
        output.push_back(<char>b' ')
    elif opts.preserve_formatting >= FormattingOpts.FORMAT_BASIC and not output.empty():
        output.append(margin_size, <char>b'\n')
    margin_size = 0


cdef string _serialize_extract_nodes(vector[shared_ptr[ExtractNode]]& extract_nodes,
                                     const ExtractOpts& opts, size_t reserve_size) noexcept nogil:
    cdef size_t i
    cdef string output
    cdef string element_text
    cdef string element_text_prefix
    cdef ExtractNode* current_node = NULL
    cdef bint bullet_inserted = False
    cdef size_t list_depth = 0
    cdef size_t margin_size = 0
    cdef size_t uncollapsed_margin_count = 0
    cdef vector[size_t] list_numbering
    cdef string list_item_indent = <const char*>b' '
    cdef const char* element_name = NULL
    cdef size_t element_name_len = 0

    output.reserve(reserve_size)

    for i in range(extract_nodes.size()):
        current_node = extract_nodes[i].get()

        # Basic and minimal HTML formatting
        if opts.preserve_formatting >= FormattingOpts.FORMAT_BASIC:
            if current_node.make_block and not current_node.collapse_margins:
                uncollapsed_margin_count += 1

            # List tags
            if (current_node.tag_id in [LXB_TAG_UL, LXB_TAG_OL]
                    or (current_node.tag_id == LXB_TAG_LI and list_depth == 0)):
                if current_node.is_end_tag:
                    predec(list_depth)
                    list_numbering.pop_back()
                    bullet_inserted = False
                    element_text_prefix.clear()
                else:
                    preinc(list_depth)
                    list_numbering.push_back(<size_t>(current_node.tag_id == LXB_TAG_OL))

            # List item tags
            if opts.list_bullets and current_node.tag_id == LXB_TAG_LI:
                if opts.preserve_formatting == FormattingOpts.FORMAT_BASIC:
                    if list_numbering.back() == 0:
                        element_text_prefix = LIST_BULLET + <const char*>b' '
                    else:
                        element_text_prefix = to_string(list_numbering.back()) + <const char*>b'. '
                        if not current_node.is_end_tag:
                            preinc(list_numbering.back())
                    bullet_inserted = not current_node.is_end_tag

                elif opts.list_bullets and opts.preserve_formatting == FormattingOpts.FORMAT_MINIMAL_HTML:
                    _make_margin(output, margin_size, current_node, opts)
                    if not current_node.is_end_tag:
                        output.append(2 * list_depth, <char>b' ')
                        output.append(b'<li>')
                        margin_size = 0
                        current_node.make_block = False
                    else:
                        if not current_node.pre_depth:
                            output = rstrip_str(move(output))
                        output.append(b'</li>\n')

        # Minimal HTML formatting only
        if opts.preserve_formatting == FormattingOpts.FORMAT_MINIMAL_HTML:
            # Add <pre> tags immediately with newlines and skip usual block logic for opening tags
            if current_node.tag_id == LXB_TAG_PRE and opts.preserve_formatting == FormattingOpts.FORMAT_MINIMAL_HTML:
                if not current_node.is_end_tag:
                    _make_margin(output, margin_size, current_node, opts)
                output.append(b'</pre>' if current_node.is_end_tag else b'<pre>')
                margin_size = 0

            if current_node.pre_depth:
                current_node.make_block = False

            # Explicit line breaks
            if current_node.tag_id == LXB_TAG_BR:
                output.append(b'<br>')

            # Add a select number of start/end tags if minimal HTML formatting is on.
            if opts.preserve_formatting == FormattingOpts.FORMAT_MINIMAL_HTML and \
                    current_node.reference_node.first_child != NULL and (
                    current_node.tag_id in [LXB_TAG_H1, LXB_TAG_H2, LXB_TAG_H3, LXB_TAG_H4, LXB_TAG_H5, LXB_TAG_H6, LXB_TAG_P]
                    or (current_node.tag_id in [LXB_TAG_UL, LXB_TAG_OL] and opts.list_bullets)):

                # Add margin before start tag and skip after
                if (not current_node.is_end_tag and not current_node.pre_depth) or (
                        uncollapsed_margin_count and current_node.collapse_margins):
                    if current_node.collapse_margins:
                        margin_size = max(margin_size, <size_t>(current_node.make_block + current_node.make_big_block))
                    else:
                        margin_size += <size_t>(current_node.make_block + current_node.make_big_block)
                    _make_margin(output, margin_size, current_node, opts)
                    current_node.make_block = False
                    uncollapsed_margin_count = 0

                # Indent if in list (indent ul and ol start tags on level less)
                if opts.list_bullets:
                    _make_indent(output, list_depth - (<size_t>(current_node.tag_id in [LXB_TAG_UL, LXB_TAG_OL])
                                                       if list_depth > 0 and not current_node.is_end_tag else 0u),
                                 current_node, opts)
                output.push_back(b'<')
                if current_node.is_end_tag:
                    output.push_back(b'/')
                element_name = <const char*>lxb_dom_element_qualified_name(<lxb_dom_element_t*>current_node.reference_node, &element_name_len)
                output.append(element_name, element_name_len)
                output.push_back(b'>')

                # Add extra newline after opening <ul> / <ol>
                if not output.empty() and current_node.tag_id in [LXB_TAG_UL, LXB_TAG_OL] \
                        and not current_node.is_end_tag and not current_node.pre_depth:
                    output.push_back(b'\n')


        # Record size follow-up margins
        if current_node.make_block:
            if current_node.collapse_margins:
                margin_size = max(margin_size, 2u if current_node.make_big_block and not current_node.pre_depth else 1u)
            else:
                margin_size += 2u if current_node.make_big_block else 1u

        # From here on process only text nodes
        if current_node.text_contents.get() == NULL:
            continue

        element_text = deref(current_node.text_contents)
        if not current_node.pre_depth or opts.preserve_formatting == FormattingOpts.FORMAT_OFF:
            element_text = _get_collapsed_string(element_text)
            if current_node.make_block or (not output.empty() and isspace(output.back())):
                # Strip inline elements only if previous text ended with space
                element_text = lstrip_str(move(element_text))

        if element_text.empty():
            continue

        if current_node.escape_text_contents:
            element_text = _escape_html(element_text.data(), element_text.size())

        # Make margins and indents
        _make_margin(output, margin_size, current_node, opts)
        uncollapsed_margin_count = 0

        # Indent list items if basic formatting is used (follow-up lines without bullets are indented more)
        if list_depth and opts.preserve_formatting == FormattingOpts.FORMAT_BASIC:
            _make_indent(output, list_depth + <size_t>(opts.list_bullets and not bullet_inserted),
                         current_node, opts)
            bullet_inserted = False


        if opts.preserve_formatting >= FormattingOpts.FORMAT_BASIC and current_node.tag_id in [LXB_TAG_TD, LXB_TAG_TH]:
            if not output.empty() and output.back() != b'\n':
                output.append(b'\t\t')

        output.append(element_text_prefix)
        element_text_prefix.clear()
        output.append(element_text)

    return output


cdef inline bint _is_unprintable_pua(lxb_dom_node_t* node) noexcept nogil:
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
        return True

    # BMP private use area (probably an icon font)
    cdef uint32_t cp = 0
    if element_text.size() == 3:
        memcpy(&cp, element_text.data(), 3 * sizeof(char))
        if 0x8080ee <= cp <= 0xbfa3ef:
            return True

    return False


cdef RE2Options re_opts
re_opts.set_case_sensitive(False)

cdef RE2 article_cls_regex = RE2(rb'(?:^|[\s_-])(?:article|entry|post|story|single[_-]?post|(?:main[_-])?content|body|text|page)?(?:$|[\s_-])', re_opts)
cdef RE2 nav_cls_regex = RE2(rb'(?:^|\s)(?:(?:l|m|wp|main|site|page|sub|article|global|sticky|main)[_-]*)?(?:nav(?:igation)?|menu(?:[_-]item)?|drop[_-]?down|bread[_-]?crumbs?)|(?:links?[_-]?(?:bar|box|list|container|section|wrapp(?:er))?)(?:$|[\s_-])', re_opts)
cdef RE2 recommended_cls_regex = RE2(rb'(?:^|[\s_-])(?:trends|trending|recommended|featured|popular|editors?[_-]picks|related|read-next|(?:related|more|other)[_-]?(?:links|articles|posts|guides|stories))(?:$|[\s_-])', re_opts)
cdef RE2 landmark_id_regex = RE2(rb'^(?:(?:l|wp|global|page|site|full|sticky)[_-]*)?(?:(?:head|foot)(?:er)?|right)$', re_opts)
cdef RE2 header_cls_regex = RE2(rb'(?:^|\s)(?:l|m|wp|global|page|site|full|sticky)[_-]*header(?:[_-]?wrap(?:per)?|bar)?(?:$|\s)', re_opts)
cdef RE2 footer_cls_regex = RE2(rb'(?:^|[\s_-])(?:global|page|site|copyright)?(?:footer|copyright|cookie|consent|legal|fcontainer)(?:$|[\s_-])', re_opts)
cdef RE2 post_meta_cls_regex = RE2(rb'(?:^|[\s_-])(?:(?:post|entry|article(?:page)?|content|story|section)[_-]*(?:text[_-]*)?(?:footer|teaser|meta(?:[_-]?data)?|subline|sidebar|author(?:name)?|published|timestamp|date|posted[_-]?on|info|labels?|tags?|keywords|category)|by[_-]?line|date[_-]?line|author-date|submitted(?:-by)?)|meta[_-]?data(?:$|[\s_-])', re_opts)
cdef RE2 sidebar_cls_regex = RE2(rb'(?:^|\s)(?:(?:l|wp|right|left|global|sticky)[_-]*)?(?:(?:side|sticky)[_-]?(?:bars?|box)|one-third)(?:$|[\s_-])', re_opts)
cdef RE2 search_cls_regex = RE2(rb'(?:^|[\s_-])search(?:[_-]?(?:bar|facility|box))?(?:$|\s)', re_opts)
cdef RE2 skip_link_cls_regex = RE2(rb'(?:^|\s)(?:link[_-]?)?(?:skip(?:[_-]?(?:to|link))?|scroll[_-]?(?:up|down)|next|prev(?:ious)?|permalink|pagination|skip-to-(?:main-)?content)(?:$|\s|[_-]?(?:post|article))', re_opts)
cdef RE2 display_cls_regex = RE2(rb'(?:^|\s)(?:(?:is|visually)[_-])?(?:display-none|hidden|invisible|collapsed|h-0|nocontent|expandable)(?:-xs|-sm|-lg|-2?xl)?(?:$|\s)', re_opts)
cdef RE2 display_css_regex = RE2(rb'(?:^|;\s*)(?:display\s?:\s?none|visibility\s?:\s?hidden)(?:$|\s?;)', re_opts)
cdef RE2 modal_cls_regex = RE2(rb'(?:^|\s)(?:wp-|p-|-l)?(?:modal|popup|lightbox)(?:[_-]*(?:window|pane|box))?(?:$|[\s_-])', re_opts)
cdef RE2 gallery_cls_regex = RE2(rb'(?:^|[\s_-])(?:gallery|carousel)(?:$|[\s_-])', re_opts)
cdef RE2 signin_cls_regex = RE2(rb'(?:^|[\s_-])(?:(?:log[_-]?in|sign[_-]?(?:in|up)|account)|user[_-](?:info|profile|settings|actions))(?:$|[\s_-])', re_opts)
cdef RE2 ads_cls_regex = RE2(rb'(?:^|\s)(?:(?:google|wide)[_-]?ads?|ad(?:vert|vertise(?:ment|link)?|$|_[a-f0-9]+)|sponsor(?:ed)?|promoted|paid|(?:wide)?banner|donate)(?:$|[\s_-])', re_opts)
cdef RE2 social_cls_regex = RE2(rb'(?:^|\s|__|--|mobile-|desktop-|l-|m-|c-)(?:social(?:media|search)?|share(?:daddy)?|syndication|newsletter|sharing|follow|email|likes?|(?:give[_-]?)?feedback|(?:brand[_-])?engagement|facebook|twitter|subscribe|wa|jp|aptf-follow)(?:[_-]?(?:post|links?|section|icons?|btn|buttons?|target))?(?:$|[\s_-])', re_opts)
cdef RE2 comments_cls_regex = RE2(rb'(?:^|[\s_-])(?:(?:article|user|post)[_-]*)?(?:(?:no[_-]?)?comments?|comment[_-]?list|reply)(?:$|[\s_-])', re_opts)
cdef RE2 logo_cls_regex = RE2(rb'(?:brand(?:ing)?[_-]*)?logo(?:$|\s)', re_opts)
cdef RE2 print_cls_regex = RE2(rb'(?:^|\s)print[_-]', re_opts)
cdef RE2 other_junk_cls_regex = RE2(rb'(?:^|\s)short-view-count|spinner(?:$|[\s_-])')


cdef inline bint regex_search_not_empty(const string_view s, const RE2& r) noexcept nogil:
    if s.empty():
        return False
    return PartialMatch(s, r())


cdef inline bint _is_link_cluster(lxb_dom_node_t* node, double max_link_ratio, size_t max_length) noexcept nogil:
    """
    Check if element contains an excessive number of links compared to the whole content length.
    
    :param node: input node
    :param max_link_ratio: maximum ratio of link chars / all chars
    :param max_length: do not check ratio if content length is larger than this (0 to disable limit)
    :return: true if element is a link cluster
    """
    cdef string element_text = _get_collapsed_string(get_node_text(node))
    if max_length and element_text.size() > max_length:
        return False
    dom_coll = lxb_dom_collection_make(node.owner_document, 20)
    lxb_dom_elements_by_tag_name(<lxb_dom_element_t *> node, dom_coll, <const lxb_char_t *> b'a', 1)
    cdef size_t i
    cdef string link_texts
    link_texts.reserve(element_text.size())
    for i in range(lxb_dom_collection_length(dom_coll)):
        link_texts.append(_get_collapsed_string(get_node_text(lxb_dom_collection_node(dom_coll, i))))
    lxb_dom_collection_destroy(dom_coll, True)
    if not link_texts.empty() and link_texts.size() / <double> element_text.size() > max_link_ratio:
        return True
    return False


cdef stl_set[string] blacklist_aria_roles = [b'alert', b'banner', b'checkbox', b'comment', b'complementary',
                                             b'contentinfo', b'dialog', b'img', b'menu', b'menubar', b'menuitem',
                                             b'navigation', b'presentation', b'radio', b'search', b'searchbox',
                                             b'separator', b'tab', b'toolbar', b'tooltip']


# noinspection DuplicatedCode
cdef inline bint _is_main_content_node(lxb_dom_node_t* node, size_t body_depth, bint keep_comments,
                                       bint keep_post_meta, bint keep_hidden) noexcept nogil:
    """
    Perform a rule-based check whether the given element is a "main-content" element.
    
    :param node: node to check
    :param body_depth: DOM depth of element counted from the document's BODY
    :param keep_comments: treat comment sections as main content
    :param keep_post_meta: treat article / blog post meta data as main content
    :param keep_hidden: keep elements that are hidden by classes or inline CSS
    :return: true if element is a main content element
    """

    if node.type == LXB_DOM_NODE_TYPE_TEXT:
        return not _is_unprintable_pua(node)
    elif node.type != LXB_DOM_NODE_TYPE_ELEMENT:
        return True


    # ------ Section 1: Tag name matching ------

    # Main elements and headings
    if node.local_name in [LXB_TAG_BODY, LXB_TAG_MAIN, LXB_TAG_H1]:
        return True

    # Global footer
    elif node.local_name == LXB_TAG_FOOTER:
        if body_depth < 3 or _is_link_cluster(node, 0.2, 0):
            return False

        # Check if footer is recursive last element node of a direct body child
        pnode = node
        while pnode and pnode.parent and pnode.parent.local_name != LXB_TAG_BODY:
            if pnode.next and pnode.next.type == LXB_DOM_NODE_TYPE_TEXT:
                pnode = pnode.next
            if pnode.next:
                # There is at least one more element node
                return True
            pnode = pnode.parent
        return False

    elif node.local_name == LXB_TAG_UL:
        if body_depth < 4 or _is_link_cluster(node, 0.2, 0):
            return False

    # Teaser articles
    elif node.local_name == LXB_TAG_ARTICLE:
        if body_depth > 2 and _is_link_cluster(node, 0.2, 500):
            return False

    # Navigation, sidebar, other hard-blacklisted elements
    elif node.local_name in [LXB_TAG_NAV, LXB_TAG_ASIDE, LXB_TAG_AUDIO, LXB_TAG_VIDEO, LXB_TAG_TIME]:
        return False


    # ------ Section 2: Rel and ARIA attribute matching ------

    # Hidden elements
    if lxb_dom_element_has_attribute(<lxb_dom_element_t*>node, <const lxb_char_t*>b'hidden', 6):
        return False

    # rel attributes
    cdef string_view rel_attr = strip_sv(get_node_attr_sv(node, b'rel'))
    if not rel_attr.empty() and rel_attr in [b'author', b'icon', b'search', b'prev', b'next', b'tag']:
        return False

    # itemprop attributes
    cdef string_view itemprop_attr = strip_sv(get_node_attr_sv(node, b'itemprop'))
    if not itemprop_attr.empty() and itemprop_attr in [b'datePublished', b'author', b'url']:
        return False

    # ARIA hidden
    if strip_sv(get_node_attr_sv(node, b'aria-hidden')) == b'true':
        return False

    # ARIA expanded
    if strip_sv(get_node_attr_sv(node, b'aria-expanded')) == b'false':
        return False


    # ------ Section 3: General class and ID matching ------

    cdef string_view cls_attr = get_node_attr_sv(node, b'class')
    cdef string_view id_attr = get_node_attr_sv(node, b'id')
    # Only elements with class or id attributes from here on
    if cls_attr.empty() and id_attr.empty():
        if node.local_name == LXB_TAG_DIV:
            return body_depth <= 5 or not _is_link_cluster(node, 0.6, 800)
        return True

    cdef string cls_and_id_attr_str = <string>cls_attr
    if not cls_and_id_attr_str.empty():
        cls_and_id_attr_str.push_back(b' ')
    cls_and_id_attr_str.append(<string>id_attr)
    cdef string_view cls_and_id_attr = <string_view>cls_and_id_attr_str

    # Hidden elements
    if not keep_hidden and regex_search_not_empty(cls_attr, display_cls_regex) \
            or regex_search_not_empty(get_node_attr_sv(node, b'style'), display_css_regex):
        return False

    # Skip links
    if node.local_name in [LXB_TAG_A, LXB_TAG_DIV, LXB_TAG_LI] and \
            regex_search_not_empty(cls_and_id_attr, skip_link_cls_regex):
        return False

    if body_depth > 2:
        # Sign-in links
        if regex_search_not_empty(cls_attr, signin_cls_regex):
            return False

        # Post meta
        if not keep_post_meta and regex_search_not_empty(cls_attr, post_meta_cls_regex):
            return False

        # Social media and feedback forms
        if regex_search_not_empty(cls_attr, social_cls_regex):
            return False

    # Logos
    if regex_search_not_empty(cls_and_id_attr, logo_cls_regex):
        return False

    # Ads
    if regex_search_not_empty(cls_and_id_attr, ads_cls_regex) \
            or lxb_dom_element_has_attribute(<lxb_dom_element_t*>node, <const lxb_char_t*>b'data-ad', 7) \
            or lxb_dom_element_has_attribute(<lxb_dom_element_t*>node, <const lxb_char_t*>b'data-advertisement', 18) \
            or lxb_dom_element_has_attribute(<lxb_dom_element_t*>node, <const lxb_char_t*>b'data-text-ad', 12):
        return False

    # Other junk
    if regex_search_not_empty(cls_attr, other_junk_cls_regex):
        return False


    # ------ Section 4: Class and ID matching of block elements only ------

    if not is_block_element(node.local_name) and node.local_name != LXB_TAG_TD:
        return True

    # ARIA roles
    cdef string_view role_attr = strip_sv(get_node_attr_sv(node, b'role'))
    if rel_attr == b'main':
        return True
    if not role_attr.empty() and blacklist_aria_roles.find(<string>role_attr) != blacklist_aria_roles.end():
        return False

    # Whitelist article elements
    if regex_search_not_empty(cls_and_id_attr, article_cls_regex):
        return True

    # Global landmarks by ID
    if regex_search_not_empty(id_attr, landmark_id_regex):
        return False

    # Global header
    if regex_search_not_empty(cls_and_id_attr, header_cls_regex):
        return False

    # Global footer
    if regex_search_not_empty(cls_and_id_attr, footer_cls_regex):
        return False

    # Global navigation
    if regex_search_not_empty(cls_and_id_attr, nav_cls_regex):
        return False

    # Recommended articles
    if regex_search_not_empty(cls_and_id_attr, recommended_cls_regex):
        return False

    # Comments section
    if not keep_comments and node.local_name and regex_search_not_empty(cls_and_id_attr, comments_cls_regex):
        return False

    # Global search bar
    if regex_search_not_empty(cls_and_id_attr, search_cls_regex):
        return False

    # Global sidebar
    if regex_search_not_empty(cls_and_id_attr, sidebar_cls_regex):
        return False

    # Modals
    if regex_search_not_empty(cls_and_id_attr, modal_cls_regex):
        return False

    # Image galleries and carousels
    if regex_search_not_empty(cls_and_id_attr, gallery_cls_regex):
        return False

    # Print content
    if regex_search_not_empty(cls_and_id_attr, print_cls_regex):
        return False

    if body_depth > 2 and node.local_name == LXB_TAG_DIV and _is_link_cluster(node, 0.6, 1500):
        return False

    return True


cdef inline lxb_status_t _exists_cb(lxb_dom_node_t *node, lxb_css_selector_specificity_t *spec, void *ctx) noexcept nogil:
    (<bint*>ctx)[0] = True
    return LXB_STATUS_STOP


def extract_plain_text(html,
                       preserve_formatting=True,
                       bint main_content=False,
                       bint list_bullets=True,
                       bint alt_texts=True,
                       bint links=False,
                       bint form_fields=False,
                       bint noscript=False,
                       bint comments=True,
                       bint post_meta=True,
                       bint hidden_elements=False,
                       skip_elements=None):
    """
    extract_plain_text(html, preserve_formatting=True, main_content=False, list_bullets=True, alt_texts=False, \
                       links=True, form_fields=False, noscript=False, comments=None, skip_elements=None)

    Perform a simple plain-text extraction from the given DOM node and its children.

    Extracts all visible text (excluding script/style elements, comment nodes etc.)
    and collapses consecutive white space characters.

    If ``preserve_formatting`` is ``True``, line breaks, paragraphs, other block-level elements,
    list elements, and pre-formatted text will be preserved. Use the special value ``'minimal_html'`` to
    add reduced HTML markup to the formatted output, preserving headings (``<h1-6>``), paragraphs (``<p>``),
    lists (``<ul>``, ``<ol>``), ``<pre>`` text, ``<br>`` line breaks, and links (``<a>``, if ``links=True``).

    Extraction of particular elements and attributes such as links, alt texts, or form fields
    can be configured individually by setting the corresponding parameter to ``True``.
    Defaults to ``False`` for most elements (i.e., only basic text will be extracted).

    :param html: HTML as DOM tree or Unicode string
    :type html: HTMLTree or str
    :param preserve_formatting: preserve basic block-level formatting (use ``'minimal_html'`` for minimal HTML
                                markup in output)
    :type preserve_formatting: bool or t.Literal['minimal_html']
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
    :param comments: treat comment sections as main content
    :type comments: bool
    :param post_meta: preserve blog post / article meta data in main content extract
    :type post_meta: bool
    :param hidden_elements: keep elements hidden by inline CSS or class names
    :type hidden_elements: bool
    :param skip_elements: list of CSS selectors for elements to skip
    :type skip_elements: t.Iterable[str] or None
    :type noscript: bool
    :return: extracted plain text
    :rtype: str
    """

    cdef HTMLTree tree
    if isinstance(html, str):
        tree = HTMLTree.parse(html)
    elif isinstance(html, HTMLTree):
        tree = <HTMLTree>html
    else:
        raise TypeError('Parameter "html" is neither string nor HTMLTree.')

    if not check_node(tree.body):
        return ''

    skip_selectors = {e.encode() for e in skip_elements or []}
    skip_selectors.update({b'script', b'style', b'iframe', b'frame', b'template'})
    if not alt_texts:
        skip_selectors.update({b'object', b'video', b'audio', b'embed' b'img', b'area',
                               b'svg', b'figcaption', b'figure'})
    if not noscript:
        skip_selectors.add(b'noscript')
    if not form_fields:
        skip_selectors.update({b'textarea', b'input', b'button', b'select', b'option', b'label', })
    cdef string skip_selector = <string>b','.join(skip_selectors)

    cdef FormattingOpts formatting_opts = FormattingOpts.FORMAT_OFF
    if preserve_formatting == 'minimal_html':
        formatting_opts = FormattingOpts.FORMAT_MINIMAL_HTML
    elif preserve_formatting:
        formatting_opts = FormattingOpts.FORMAT_BASIC

    cdef string extracted
    with nogil:
        extracted = _extract_plain_text_impl(
            tree,
            formatting_opts,
            main_content,
            list_bullets,
            alt_texts,
            links,
            form_fields,
            noscript,
            comments,
            post_meta,
            hidden_elements,
            skip_selector)
    return extracted.decode(errors='ignore')

cdef string _extract_plain_text_impl(HTMLTree tree,
                                     FormattingOpts preserve_formatting,
                                     bint main_content,
                                     bint list_bullets,
                                     bint alt_texts,
                                     bint links,
                                     bint form_fields,
                                     bint noscript,
                                     bint comments,
                                     bint post_meta,
                                     bint hidden_elements,
                                     string skip_selector) noexcept nogil:
    """Internal extractor implementation not requiring GIL."""

    cdef ExtractContext ctx
    ctx.root_node = <lxb_dom_node_t*>tree.dom_document.body
    ctx.node = ctx.root_node
    ctx.opts = [
        preserve_formatting,
        list_bullets,
        links,
        alt_texts,
        form_fields,
        noscript]

    cdef const lxb_char_t* tag_name = NULL
    cdef size_t tag_name_len
    cdef string tag_name_str
    cdef size_t i
    cdef bint skip = False
    cdef bint is_end_tag = False

    if ctx.node.type == LXB_DOM_NODE_TYPE_DOCUMENT:
        ctx.root_node = next_element_node(ctx.node, ctx.node.first_child)
        ctx.node = ctx.root_node

    cdef string main_content_selector
    cdef lxb_dom_collection_t* root_candidates = NULL
    if main_content:
        main_content_selector = string(b'.article-body, .articleBody, .contentBody, .article-text,'
                                       b'.main-content, .postcontent, .post-content, .single-post,'
                                       b'[role="main"]')
        root_candidates = query_selector_all_impl(ctx.node, tree,
                                                  main_content_selector.data(), main_content_selector.size(), 5)
        if root_candidates != NULL:
            if lxb_dom_collection_length(root_candidates) == 1:
                # Use result only if there is exactly one match
                ctx.root_node = lxb_dom_collection_node(root_candidates, 0)
                ctx.node = ctx.root_node
            lxb_dom_collection_destroy(root_candidates, True)
            root_candidates = NULL

    # Select all blacklisted elements and store them in a set
    cdef lxb_dom_collection_t* blacklist_coll = query_selector_all_impl(ctx.root_node, tree,
                                                                        skip_selector.data(), skip_selector.size(), 30)
    cdef stl_set[lxb_dom_node_t*] blacklisted_nodes
    if blacklist_coll != NULL:
        for i in range(lxb_dom_collection_length(blacklist_coll)):
            blacklisted_nodes.insert(lxb_dom_collection_node(blacklist_coll, i))
        lxb_dom_collection_destroy(blacklist_coll, True)

    cdef size_t base_depth = 0
    cdef lxb_dom_node_t* pnode = ctx.node
    while pnode.local_name != LXB_TAG_BODY and pnode.parent:
        preinc(base_depth)
        pnode = pnode.parent

    cdef vector[shared_ptr[ExtractNode]] extract_nodes
    cdef size_t chars_extracted = 0
    cdef size_t nodes_extracted = 0
    extract_nodes.reserve(150)
    while ctx.node:
        # Skip everything except element and text nodes
        if ctx.node.type != LXB_DOM_NODE_TYPE_ELEMENT and ctx.node.type != LXB_DOM_NODE_TYPE_TEXT:
            is_end_tag = True
            ctx.node = next_node(ctx.root_node, ctx.node, &ctx.depth, &is_end_tag)
            continue

        # Skip blacklisted or non-main-content nodes
        if blacklisted_nodes.find(ctx.node) != blacklisted_nodes.end() or \
                (main_content and not _is_main_content_node(ctx.node, ctx.depth + base_depth, comments,
                                                            post_meta, hidden_elements)):
            is_end_tag = True
            ctx.node = next_node(ctx.root_node, ctx.node, &ctx.depth, &is_end_tag)
            continue

        _extract_cb(extract_nodes, ctx, is_end_tag)
        if extract_nodes.size() > nodes_extracted and deref(extract_nodes.back()).text_contents:
            chars_extracted += deref(deref(extract_nodes.back()).text_contents).size()
            preinc(nodes_extracted)

        ctx.node = next_node(ctx.root_node, ctx.node, &ctx.depth, &is_end_tag)

    return rstrip_str(_serialize_extract_nodes(extract_nodes, ctx.opts, <size_t>(chars_extracted * 1.2)))
