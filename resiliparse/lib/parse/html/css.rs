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
use crate::parse::html::dom::{Node, NodeBase, str_from_lxb_char_t};
use crate::third_party::lexbor::*;

#[derive(Debug)]
pub struct CSSParserError {
    error: String
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
                Err(CSSParserError { error: s })
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

struct MatchContextWrapper<'a, Ctx> {
    f: &'a dyn Fn(Node, u32, &mut Ctx) -> TraverseAction,
    root_node: &'a Node,
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

    unsafe extern "C" fn match_cb_adapter<Ctx>(node: *mut lxb_dom_node_t,
                                               spec: lxb_css_selector_specificity_t,
                                               ctx: *mut c_void) -> lxb_status_t {
        let ctx_cast = ctx as *mut MatchContextWrapper<Ctx>;
        let tree = &(*ctx_cast).root_node.tree.upgrade();
        if !tree.is_some() {
            return lexbor_status_t::LXB_STATUS_ERROR;
        }
        let new_node = NodeBase::create_node(tree.as_ref().unwrap(), node);
        if !new_node.is_some() {
            return lexbor_status_t::LXB_STATUS_ERROR;
        }
        let status = ((*ctx_cast).f)(new_node.unwrap(), spec, &mut (*ctx_cast).ctx);
        match status {
            TraverseAction::Ok => lexbor_status_t::LXB_STATUS_OK,
            TraverseAction::Stop => lexbor_status_t::LXB_STATUS_STOP,
            TraverseAction::Err => lexbor_status_t::LXB_STATUS_ERROR
        }
    }

    pub fn match_elements<Ctx>(&self, root_node: &Node, cb: &dyn Fn(Node, u32, &mut Ctx) -> TraverseAction, ctx: &mut Ctx) {
        let mut ctx_wrapper = MatchContextWrapper { f: cb, root_node, ctx };
        unsafe {
            self.match_elements_unchecked(root_node.node, Some(Self::match_cb_adapter::<Ctx>), &mut ctx_wrapper);
        }
    }
}
