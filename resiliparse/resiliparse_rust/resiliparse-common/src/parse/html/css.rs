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

//! CSS DOM matching utilities.

use std::error::Error;
use std::ffi::c_void;
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;
use std::ptr::addr_of_mut;
use std::sync::{Arc, Weak};

use crate::parse::html::dom::node::{ElementNode, NodeRef};
use crate::parse::html::dom::traits::NodeInterfaceBaseImpl;
use crate::parse::html::lexbor::*;
use crate::parse::html::tree::HTMLDocument;
use crate::third_party::lexbor::*;
use crate::third_party::lexbor::lexbor_status_t::*;
use crate::third_party::lexbor::lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_ELEMENT;
use crate::third_party::lexbor::lxb_html_status_t::*;


#[derive(Debug)]
pub struct CSSParserError {
    msg: String
}

impl Display for CSSParserError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "CSS parser error: {}", self.msg)
    }
}

impl Error for CSSParserError {}

pub enum TraverseAction {
    Ok,
    Stop,
    Err
}

struct MatchContextWrapper<'a, T, F: Fn(ElementNode, u32, &mut T) -> TraverseAction> {
    f: F,
    tree: Weak<HTMLDocument>,
    custom_data: &'a mut T
}

pub struct CSSSelectorList<'a> {
    selector_list: *mut lxb_css_selector_list_t,
    tree: Weak<HTMLDocument>,
    phantom: PhantomData<&'a ()>
}

impl<'a> CSSSelectorList<'a> {
    pub(crate) fn parse_selectors(tree: &Arc<HTMLDocument>, selectors: &str) -> Result<CSSSelectorList<'a>, CSSParserError> {
        unsafe {
            if lxb_html_document_css_init(*tree.doc_ptr()) != LXB_HTML_STATUS_OK {
                return Err(CSSParserError { msg: "Failed to initialize CSS parser.".to_owned() });
            }

            let parser = (*(*tree.doc_ptr())).css.parser;
            let sel_list = lxb_css_selectors_parse(parser, selectors.as_ptr(), selectors.len());
            if (*parser).status != LXB_STATUS_OK {
                let mut msg = String::default();
                lxb_css_log_serialize((*parser).log, Some(Self::css_log_serialize_cb),
                                      addr_of_mut!(msg) as *mut c_void, "".as_ptr(), 0);
                Err(CSSParserError { msg })
            } else {
                Ok(CSSSelectorList { selector_list: sel_list, tree: Arc::downgrade(&tree), phantom: Default::default() })
            }
        }
    }

    unsafe extern "C" fn css_log_serialize_cb(data: *const lxb_char_t, size: usize, ctx: *mut c_void) -> lxb_status_t {
        if let Some(s) = str_from_lxb_char_t(data, size) {
            (*(ctx as *mut String)).push_str(s);
        }
        LXB_STATUS_OK
    }

    pub(crate) unsafe fn match_elements_unchecked<Ctx>(
        &self, root_node: *mut lxb_dom_node_t, cb: lxb_selectors_cb_f, ctx: &mut Ctx) {
        if let Some(t) = self.tree.upgrade() {
            lxb_selectors_find((*(*t.doc_ptr())).css.selectors,
                               root_node, self.selector_list, cb, addr_of_mut!(*ctx).cast());
        }
    }

    pub(crate) unsafe fn match_elements_unchecked_reverse<T>(
        &self, node: *mut lxb_dom_node_t, cb: lxb_selectors_cb_f, ctx: &mut T) {
        if let Some(t) = self.tree.upgrade() {
            lxb_selectors_find_reverse((*(*t.doc_ptr())).css.selectors,
                                       node, self.selector_list, cb, addr_of_mut!(*ctx).cast());
        }
    }

    unsafe extern "C" fn match_cb_adapter<T, F>(
        node: *mut lxb_dom_node_t, spec: lxb_css_selector_specificity_t, ctx: *mut c_void) -> lxb_status_t
        where F: Fn(ElementNode, u32, &mut T) -> TraverseAction {
        if node.is_null() || (*node).type_ != lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_ELEMENT {
            return LXB_STATUS_OK;
        }

        let ctx_cast = ctx as *mut MatchContextWrapper<T, F>;
        let new_element = ElementNode::new(&(*ctx_cast).tree.upgrade().unwrap(), node).unwrap();
        let status = ((*ctx_cast).f)(new_element, spec, &mut (*ctx_cast).custom_data);
        match status {
            TraverseAction::Ok => LXB_STATUS_OK,
            TraverseAction::Stop => LXB_STATUS_STOP,
            TraverseAction::Err => LXB_STATUS_ERROR
        }
    }

    pub fn match_elements<T, F>(&self, root_node: &NodeRef, cb: F, custom_data: &mut T)
        where F: Fn(ElementNode, u32, &mut T) -> TraverseAction {
        let tree = self.tree.upgrade().unwrap();
        unsafe { assert_eq!(*tree.doc_ptr(), *root_node.tree_().doc_ptr()) };
        let mut ctx_wrapper = MatchContextWrapper { f: cb, tree: Arc::downgrade(&tree), custom_data };
        unsafe {
            self.match_elements_unchecked(*root_node.node_ptr_(), Some(Self::match_cb_adapter::<T, F>), &mut ctx_wrapper)
        }
    }

    pub fn match_elements_reverse<T, F>(&self, node: &NodeRef, cb: F, custom_data: &mut T)
        where F: Fn(ElementNode, u32, &mut T) -> TraverseAction {
        unsafe {
            if (*(*node.node_ptr_())).parent.is_null() || (*(*node.node_ptr_())).type_ != LXB_DOM_NODE_TYPE_ELEMENT {
                return;
            }
        }
        let tree = self.tree.upgrade().unwrap();
        unsafe { assert_eq!(*tree.doc_ptr(), *node.tree_().doc_ptr()) };
        let mut ctx_wrapper = MatchContextWrapper { f: cb, tree: Arc::downgrade(&tree), custom_data };
        unsafe {
            self.match_elements_unchecked_reverse(*node.node_ptr_(), Some(Self::match_cb_adapter::<T, F>), &mut ctx_wrapper)
        }
    }
}

impl Drop for CSSSelectorList<'_> {
    fn drop(&mut self) {
        unsafe { lxb_css_selector_list_destroy(self.selector_list); }
    }
}
