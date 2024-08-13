// Copyright 2023 Janek Bevendorff
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


//! Internal helper tools for serializing to HTML5 bytes.


use crate::parse::html::lexbor::*;
use crate::third_party::lexbor::*;


pub(super) fn node_serialize_html(node: *mut lxb_dom_node_t) -> String {
    unsafe {
        let html_str = lexbor_str_create();
        if html_str.is_null() {
            return String::default();
        }
        lxb_html_serialize_tree_str(node, html_str);
        let s = str_from_lxb_str_t(html_str).unwrap_or_default().to_owned();
        lexbor_str_destroy(html_str, (*(*node).owner_document).text, true);
        s
    }
}

pub(super) fn node_format_visible_text(node: *mut lxb_dom_node_t) -> String {
    let mut ctx = WalkCtx { text: String::new() };
    unsafe { dom_node_walk(node, node_format_cb_begin, node_format_cb_end, &mut ctx); }
    ctx.text
}

struct WalkCtx {
    text: String,
}

type WalkCbFn = unsafe fn(*mut lxb_dom_node_t, &mut WalkCtx) -> lexbor_action_t::Type;


unsafe fn dom_node_walk(root: *mut lxb_dom_node_t, begin_fn: WalkCbFn, end_fn: WalkCbFn, ctx: &mut WalkCtx) {
    if root.is_null() {
        return;
    }

    let mut action: lexbor_action_t::Type;
    let mut node = (*root).first_child;

    use lexbor_action_t::*;
    while !node.is_null() {
        action = begin_fn(node, ctx);
        if action == LEXBOR_ACTION_STOP {
            return;
        }
        if !(*node).first_child.is_null() && action != LEXBOR_ACTION_NEXT {
            node = (*node).first_child;
        } else {
            while node != root && (*node).next.is_null() {
                end_fn(node, ctx);
                node = (*node).parent;
            }
            if node == root {
                break;
            }
            node = (*node).next;
        }
    }
}

fn is_block_element(local_name: lxb_tag_id_enum_t::Type) -> bool {
    use lxb_tag_id_enum_t::*;
    matches!(
        local_name,
        LXB_TAG_ADDRESS | LXB_TAG_ARTICLE | LXB_TAG_ASIDE | LXB_TAG_BLOCKQUOTE | LXB_TAG_CANVAS |
        LXB_TAG_DD | LXB_TAG_DIV | LXB_TAG_DL | LXB_TAG_DT | LXB_TAG_FIELDSET | LXB_TAG_FIGCAPTION |
        LXB_TAG_FIGURE | LXB_TAG_FOOTER | LXB_TAG_FORM | LXB_TAG_H1 | LXB_TAG_H2 | LXB_TAG_H3 |
        LXB_TAG_H4 | LXB_TAG_H5 | LXB_TAG_H6 | LXB_TAG_HEADER | LXB_TAG_HR | LXB_TAG_LI |
        LXB_TAG_MAIN | LXB_TAG_NAV | LXB_TAG_NOSCRIPT | LXB_TAG_OL | LXB_TAG_P |
        LXB_TAG_PRE | LXB_TAG_SECTION | LXB_TAG_TABLE | LXB_TAG_TFOOT | LXB_TAG_UL | LXB_TAG_VIDEO)
}

unsafe fn node_format_cb_begin(node: *mut lxb_dom_node_t, ctx: &mut WalkCtx) -> lexbor_action_t::Type {
    use lexbor_action_t::*;
    use lxb_dom_node_type_t::*;

    if node.is_null() {
        return LEXBOR_ACTION_STOP;
    }

    if (*node).type_ == LXB_DOM_NODE_TYPE_TEXT {
        let s = collapse_whitespace(str_from_dom_node(node).unwrap_or_default());
        if ctx.text.chars().last().unwrap_or(' ').is_ascii_whitespace() {
            ctx.text.push_str(s.trim_start());
        } else {
            ctx.text.push_str(s.as_str());
        }
        return LEXBOR_ACTION_OK;
    }
    if (*node).type_ != LXB_DOM_NODE_TYPE_ELEMENT {
        return LEXBOR_ACTION_NEXT;
    }

    use lxb_tag_id_enum_t::*;
    let lname = (*node).local_name as lxb_tag_id_enum_t::Type;
    match lname {
        LXB_TAG_SCRIPT | LXB_TAG_STYLE | LXB_TAG_NOSCRIPT => LEXBOR_ACTION_NEXT,
        LXB_TAG_PRE => {
            let mut l = 0;
            let t = lxb_dom_node_text_content(node, &mut l);
            str_from_lxb_char_t(t, l).map(|s| { ctx.text.push_str(s) });
            lxb_dom_document_destroy_text_noi((*node).owner_document, t);
            LEXBOR_ACTION_NEXT
        },
        LXB_TAG_BR => {
            ctx.text.push('\n');
            LEXBOR_ACTION_NEXT
        },
        _ => {
            insert_block(lname, &mut ctx.text);
            LEXBOR_ACTION_OK
        }
    }
}

unsafe fn node_format_cb_end(node: *mut lxb_dom_node_t, ctx: &mut WalkCtx) -> lexbor_action_t::Type {
    use lexbor_action_t::*;
    if node.is_null() {
        return LEXBOR_ACTION_STOP;
    }
    insert_block((*node).local_name as lxb_tag_id_enum_t::Type, &mut ctx.text);
    LEXBOR_ACTION_OK
}

#[inline]
fn insert_block(tag_name: lxb_tag_id_enum_t::Type, text: &mut String) {
    if !is_block_element(tag_name) {
        return
    }
    text.truncate(text.trim_end_matches(|c: char| c.is_ascii_whitespace()).len());
    text.push('\n');
    if tag_name == lxb_tag_id_enum_t::LXB_TAG_P {
        text.push('\n');
    }
}

fn collapse_whitespace(s: &str) -> String {
    let mut last_char = 'x';
    s.chars()
        .into_iter()
        .map(|c| if c.is_ascii_whitespace() { ' ' } else { c })
        .filter(|c| {
            let keep = c != &' ' || last_char != ' ';
            last_char = c.clone();
            keep
        })
        .collect()
}
