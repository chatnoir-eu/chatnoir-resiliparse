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


use std::error::Error;
use std::ffi::c_void;
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;
use std::ptr;
use std::ptr::addr_of_mut;
use std::rc::Rc;
use crate::parse::html::dom::{ElementNode, Node, NodeBase, NodeRef, str_from_lxb_char_t};
use crate::parse::html::dom::Node::Element;
use crate::parse::html::tree::HTMLTreeRc;
use crate::third_party::lexbor::*;

#[derive(Debug)]
pub struct CSSParserError {
    error: String
}

impl CSSParserError {
    pub(super) fn new(s: &str) -> Self {
        Self { error: s.to_owned() }
    }
}

impl Display for CSSParserError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "CSS parser error: {}", self.error)
    }
}

impl Error for CSSParserError {}

pub struct CSSParser {
    parser: *mut lxb_css_parser_t,
    memory: *mut lxb_css_memory_t,
}

impl CSSParser {
    pub fn new() -> Self {
        unsafe {
            let memory = lxb_css_memory_create();
            lxb_css_memory_init(memory, 64);
            let parser = lxb_css_parser_create();
            lxb_css_parser_init(parser, ptr::null_mut());
            (*parser).memory = memory;
            (*parser).selectors = ptr::null_mut();
            Self { parser, memory }
        }
    }

    unsafe fn create_css_selectors(&self) {
        self.destroy_css_selectors();
        (*self.parser).selectors = lxb_css_selectors_create();
        lxb_css_selectors_init((*self.parser).selectors);
    }

    unsafe fn destroy_css_selectors(&self) {
        if !(*self.parser).selectors.is_null() {
            lxb_css_selectors_destroy((*self.parser).selectors, true);
            (*self.parser).selectors = ptr::null_mut();
        }
    }

    unsafe extern "C" fn css_log_serialize_cb(data: *const lxb_char_t, size: usize, ctx: *mut c_void) -> lxb_status_t {
        if let Some(s) = str_from_lxb_char_t(data, size) {
            (*(ctx as *mut String)).push_str(s);
        }
        lexbor_status_t::LXB_STATUS_OK
    }

    pub fn parse_css_selectors<'a>(&'a self, selector: &'_ str) -> Result<CSSSelectorList<'a>, CSSParserError> {
        unsafe {
            self.create_css_selectors();

            let sel_list = lxb_css_selectors_parse(self.parser, selector.as_ptr(), selector.len());
            if (*self.parser).status != lexbor_status_t::LXB_STATUS_OK {
                let mut s = String::default();
                lxb_css_log_serialize((*self.parser).log, Some(Self::css_log_serialize_cb), s.as_mut_ptr().cast(), "".as_ptr(), 0);
                Err(CSSParserError::new(s.as_str()))
            } else {
                Ok(CSSSelectorList { selector_list: sel_list, phantom: Default::default() })
            }
        }
    }
}

impl Drop for CSSParser {
    fn drop(&mut self) {
        unsafe {
            assert!(!self.parser.is_null());
            assert!(!self.memory.is_null());
            self.destroy_css_selectors();
            lxb_css_parser_destroy(self.parser, true);
            lxb_css_memory_destroy(self.memory, true);
        }
    }
}

pub enum TraverseAction {
    Ok,
    Stop,
    Err
}

struct MatchContextWrapper<'a, Ctx, F: Fn(ElementNode, u32, &mut Ctx) -> TraverseAction> {
    f: F,
    tree: &'a Rc<HTMLTreeRc>,
    ctx: &'a mut Ctx
}

pub struct CSSSelectorList<'a> {
    selector_list: *mut lxb_css_selector_list_t,
    phantom: PhantomData<&'a ()>
}

impl CSSSelectorList<'_> {
    pub(super) unsafe fn match_elements_unchecked<Ctx>(&self, root_node: *mut lxb_dom_node_t, cb: lxb_selectors_cb_f, ctx: &mut Ctx) {
        if root_node.is_null() {
            return;
        }
        let selectors = lxb_selectors_create();
        lxb_selectors_init(selectors);
        lxb_selectors_find(selectors, root_node, self.selector_list, cb, addr_of_mut!(*ctx).cast());
        lxb_selectors_destroy(selectors, true);
    }

    pub(super) unsafe fn match_elements_unchecked_reverse<Ctx>(&self, root_node: *mut lxb_dom_node_t, cb: lxb_selectors_cb_f, ctx: &mut Ctx) {
        if root_node.is_null() {
            return;
        }
        let selectors = lxb_selectors_create();
        lxb_selectors_init(selectors);
        lxb_selectors_find_reverse(selectors, root_node, self.selector_list, cb, addr_of_mut!(*ctx).cast());
        lxb_selectors_destroy(selectors, true);
    }

    unsafe extern "C" fn match_cb_adapter<Ctx, F>(node: *mut lxb_dom_node_t, spec: lxb_css_selector_specificity_t, ctx: *mut c_void) -> lxb_status_t
        where F: Fn(ElementNode, u32, &mut Ctx) -> TraverseAction {
        if node.is_null() || (*node).type_ != lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_ELEMENT {
            return lexbor_status_t::LXB_STATUS_OK;
        }

        let ctx_cast = ctx as *mut MatchContextWrapper<Ctx, F>;
        let new_element = NodeBase::create_node((*ctx_cast).tree, node).unwrap().into();
        let status = ((*ctx_cast).f)(new_element, spec, &mut (*ctx_cast).ctx);
        match status {
            TraverseAction::Ok => lexbor_status_t::LXB_STATUS_OK,
            TraverseAction::Stop => lexbor_status_t::LXB_STATUS_STOP,
            TraverseAction::Err => lexbor_status_t::LXB_STATUS_ERROR
        }
    }

    pub fn match_elements<Ctx, F: Fn(ElementNode, u32, &mut Ctx) -> TraverseAction>(&self, root_node: NodeRef, cb: F, ctx: &mut Ctx) {
        if let Some(tree) = &root_node.tree.upgrade() {
            let mut ctx_wrapper = MatchContextWrapper { f: cb, tree, ctx };
            unsafe {
                self.match_elements_unchecked(root_node.node, Some(Self::match_cb_adapter::<Ctx, F>), &mut ctx_wrapper);
            }
        }
    }

    pub fn match_elements_reverse<Ctx, F: Fn(ElementNode, u32, &mut Ctx) -> TraverseAction>(&self, root_node: NodeRef, cb: F, ctx: &mut Ctx) {
        if let Some(tree) = &root_node.tree.upgrade() {
            let mut ctx_wrapper = MatchContextWrapper { f: cb, tree, ctx };
            unsafe {
                self.match_elements_unchecked_reverse(root_node.node, Some(Self::match_cb_adapter::<Ctx, F>), &mut ctx_wrapper);
            }
        }
    }
}
