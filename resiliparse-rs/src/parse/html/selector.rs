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

//! Match DOM elements with CSS selectors.
//!
//! [`CssSelector`] parses a selector once and can then test a node or query its
//! descendants. Queries return matching elements in document order, without
//! including the query root. Selector callbacks can stop traversal or report an
//! error through [`TraverseAction`].

use std::error::Error;
use std::ffi::c_void;
use std::fmt::{Display, Formatter};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::ptr;
use std::sync::Arc;

use crate::parse::html::dom::coll::ElementNodeList;
use crate::parse::html::dom::node::{ElementNode, NodeRef};
use crate::parse::html::dom::traits::{NodeInterface, NodeInterfaceBaseImpl};
use crate::parse::html::lexbor::*;
use crate::parse::html::tree::HtmlDocument;
use crate::third_party::lexbor::lexbor_status_t::*;
use crate::third_party::lexbor::*;

/// An error returned when a CSS selector cannot be parsed or evaluated.
#[derive(Debug)]
pub struct CssParserError {
    msg: String,
}

impl Display for CssParserError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "CSS parser error: {}", self.msg)
    }
}

impl Error for CssParserError {}

/// Controls how selector traversal proceeds after a match callback.
pub enum TraverseAction {
    /// Continue visiting matching elements.
    Ok,
    /// Stop visiting matches successfully.
    Stop,
    /// Stop visiting matches and report an error.
    Err,
}

struct MatchContextWrapper<'a, T, F> {
    f: F,
    tree: Arc<HtmlDocument>,
    custom_data: &'a mut T,
    panic: Option<Box<dyn std::any::Any + Send>>,
}

/// A compiled CSS selector for querying elements in a DOM subtree.
///
/// Accepts a CSS selector expression or list of expressions, and compiles
/// it into a reusable selector object.
///
/// Compilation is independent of any document. The same selector can query
/// attached or detached nodes in different documents, even after earlier trees
/// have been dropped. Each operation uses a separate native matching context.
/// Queries exclude the root and return unique descendants in tree order.
///
/// ```
/// use resiliparse::parse::html::selector::CssSelector;
/// use resiliparse::parse::html::dom::traits::*;
/// use resiliparse::parse::html::tree::HtmlTree;
///
/// let selector = CssSelector::new("a[href]")?;
/// let tree = HtmlTree::parse("<body><a href='/'>link</a></body>").unwrap();
/// let body = tree.body().unwrap();
/// let links = selector.query_selector_all(&body.as_noderef())?;
/// assert!(selector.matches(&links[0])?);
/// # Ok::<(), resiliparse::parse::html::selector::CssParserError>(())
/// ```
pub struct CssSelector {
    selector_list: *mut lxb_css_selector_list_t,
    memory: *mut lxb_css_memory_t,
}

struct Matcher(*mut lxb_selectors_t);

impl Drop for Matcher {
    fn drop(&mut self) {
        unsafe {
            lxb_selectors_destroy(self.0, true);
        }
    }
}

impl CssSelector {
    /// Parse and compile a CSS selector or list of CSS selectors.
    ///
    /// # Arguments
    ///
    /// * `selectors` - A CSS selector or comma-separated list of selectors.
    ///
    /// # Returns
    ///
    /// A compiled selector list that can be reused for DOM queries, or an error if the selector list is invalid.
    pub fn new(selectors: &str) -> Result<Self, CssParserError> {
        unsafe {
            // Own the arena explicitly, including on parse failures.
            let mut compiled = Self {
                selector_list: ptr::null_mut(),
                memory: lxb_css_memory_create(),
            };
            if compiled.memory.is_null() || lxb_css_memory_init(compiled.memory, 256) != LXB_STATUS_OK {
                return Err(CssParserError {
                    msg: "Failed to allocate selector memory.".into(),
                });
            }
            let parser = lxb_css_parser_create();
            if parser.is_null() {
                return Err(CssParserError {
                    msg: "Failed to allocate CSS parser.".into(),
                });
            }
            if lxb_css_parser_init(parser, ptr::null_mut()) != LXB_STATUS_OK {
                lxb_css_parser_destroy(parser, true);
                return Err(CssParserError {
                    msg: "Failed to initialize CSS parser.".into(),
                });
            }
            (*parser).memory = compiled.memory;
            compiled.selector_list = lxb_css_selectors_parse(parser, selectors.as_ptr(), selectors.len());
            let failed = (*parser).status != LXB_STATUS_OK || compiled.selector_list.is_null();
            let mut msg = String::new();
            if failed {
                lxb_css_log_serialize(
                    (*parser).log,
                    Some(Self::css_log_serialize_cb),
                    ptr::from_mut(&mut msg).cast(),
                    "".as_ptr(),
                    0,
                );
            }
            // The parser does not own the explicitly supplied arena.
            lxb_css_parser_destroy(parser, true);
            if failed {
                if msg.is_empty() {
                    msg = "Invalid selector list.".into();
                }
                Err(CssParserError { msg })
            } else {
                Ok(compiled)
            }
        }
    }

    unsafe extern "C" fn css_log_serialize_cb(data: *const lxb_char_t, size: usize, ctx: *mut c_void) -> lxb_status_t {
        unsafe {
            if let Some(s) = str_from_lxb_char_t(data, size) {
                (*(ctx as *mut String)).push_str(s);
            }
            LXB_STATUS_OK
        }
    }

    unsafe extern "C" fn match_callback_adapter<T, F>(
        node: *mut lxb_dom_node_t,
        spec: lxb_css_selector_specificity_t,
        ctx: *mut c_void,
    ) -> lxb_status_t
    where
        F: Fn(ElementNode, u32, &mut T) -> TraverseAction,
    {
        unsafe {
            let ctx = &mut *ctx.cast::<MatchContextWrapper<T, F>>();
            // Never unwind through native frames.
            match catch_unwind(AssertUnwindSafe(|| (ctx.f)(ElementNode::new(&ctx.tree, node), spec, ctx.custom_data))) {
                Ok(TraverseAction::Ok) => LXB_STATUS_OK,
                Ok(TraverseAction::Stop) => LXB_STATUS_STOP,
                Ok(TraverseAction::Err) => LXB_STATUS_ERROR,
                Err(panic) => {
                    ctx.panic = Some(panic);
                    LXB_STATUS_STOP
                }
            }
        }
    }

    fn execute_with_callback<T, F>(
        &self,
        node: &NodeRef,
        direct: bool,
        cb: F,
        custom_data: &mut T,
    ) -> Result<(), CssParserError>
    where
        F: Fn(ElementNode, u32, &mut T) -> TraverseAction,
    {
        let tree = node.tree_();
        let _guard = tree.doc_ptr();
        let node = node.node_ptr_();
        if node.is_null() {
            return Ok(());
        }
        unsafe {
            let matcher = Matcher(lxb_selectors_create());
            if matcher.0.is_null() || lxb_selectors_init(matcher.0) != LXB_STATUS_OK {
                return Err(CssParserError {
                    msg: "Failed to initialize selector matching.".into(),
                });
            }
            (*matcher.0).options = lxb_selectors_opt_t::LXB_SELECTORS_OPT_MATCH_FIRST;
            let mut ctx = MatchContextWrapper {
                f: cb,
                tree: tree.clone(),
                custom_data,
                panic: None,
            };
            let callback = Some(Self::match_callback_adapter::<T, F> as unsafe extern "C" fn(_, _, _) -> _);
            let status = if direct {
                lxb_selectors_match_node(matcher.0, node, self.selector_list, callback, ptr::from_mut(&mut ctx).cast())
            } else {
                lxb_selectors_find(matcher.0, node, self.selector_list, callback, ptr::from_mut(&mut ctx).cast())
            };
            if let Some(panic) = ctx.panic {
                resume_unwind(panic);
            }
            if status != LXB_STATUS_OK && status != LXB_STATUS_STOP {
                return Err(CssParserError {
                    msg: format!("Native matching failed with status {status}."),
                });
            }
        }
        Ok(())
    }

    /// Visit unique descendants in tree order. The callback must not mutate the DOM.
    ///
    /// # Arguments
    ///
    /// * `root` - The node whose descendants are searched.
    /// * `cb` - A callback invoked for each matching element.
    /// * `data` - Mutable caller-supplied state passed to the callback.
    ///
    /// # Returns
    ///
    /// No value if traversal completes; otherwise an error describing selector evaluation failure.
    pub fn match_elements<T, F>(&self, root: &NodeRef, cb: F, data: &mut T) -> Result<(), CssParserError>
    where
        F: Fn(ElementNode, u32, &mut T) -> TraverseAction,
    {
        self.execute_with_callback(root, false, cb, data)
    }

    /// Match only the supplied element, including detached elements.
    /// The callback must not mutate the DOM.
    ///
    /// # Arguments
    ///
    /// * `node` - The element to test against this selector list.
    /// * `cb` - A callback invoked for the supplied element if it matches.
    /// * `data` - Mutable caller-supplied state passed to the callback.
    ///
    /// # Returns
    ///
    /// No value if matching completes; otherwise an error describing selector evaluation failure.
    pub fn match_elements_reverse<T, F>(&self, node: &NodeRef, cb: F, data: &mut T) -> Result<(), CssParserError>
    where
        F: Fn(ElementNode, u32, &mut T) -> TraverseAction,
    {
        let tree = node.tree_();
        let _guard = tree.doc_ptr();
        unsafe {
            if node.node_ptr_().is_null() || (*node.node_ptr_()).type_ != lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_ELEMENT
            {
                return Ok(());
            }
        }
        self.execute_with_callback(node, true, cb, data)
    }

    /// Return the first matching descendant element, if any.
    ///
    /// # Arguments
    ///
    /// * `root` - The node whose descendants are searched.
    ///
    /// # Returns
    ///
    /// The first matching descendant element, or `None` if no element matches.
    pub fn query_selector(&self, root: &NodeRef) -> Result<Option<ElementNode>, CssParserError> {
        let mut result = None;
        self.match_elements(
            root,
            |e, _, result| {
                *result = Some(e);
                TraverseAction::Stop
            },
            &mut result,
        )?;
        Ok(result)
    }

    /// Return all matching descendant elements in tree order.
    ///
    /// # Arguments
    ///
    /// * `root` - The node whose descendants are searched.
    ///
    /// # Returns
    ///
    /// All matching descendant elements in tree order.
    pub fn query_selector_all(&self, root: &NodeRef) -> Result<ElementNodeList, CssParserError> {
        let mut result = Vec::new();
        self.match_elements(
            root,
            |e, _, result| {
                result.push(e);
                TraverseAction::Ok
            },
            &mut result,
        )?;
        Ok(result.into())
    }

    /// Return whether the element matches this selector list.
    ///
    /// # Arguments
    ///
    /// * `element` - The element to inspect or match.
    ///
    /// # Returns
    ///
    /// True if the element matches this selector list.
    pub fn matches(&self, element: &ElementNode) -> Result<bool, CssParserError> {
        let mut found = false;
        self.match_elements_reverse(
            &element.as_noderef(),
            |_, _, found| {
                *found = true;
                TraverseAction::Stop
            },
            &mut found,
        )?;
        Ok(found)
    }

    /// Test the receiver first, then its ancestors from nearest to farthest.
    ///
    /// # Arguments
    ///
    /// * `element` - The element to inspect or match.
    ///
    /// # Returns
    ///
    /// The nearest matching element at or above the supplied element, or `None` if none matches.
    pub fn closest(&self, element: &ElementNode) -> Result<Option<ElementNode>, CssParserError> {
        let tree = element.tree_();
        let _guard = tree.doc_ptr();
        let mut current = Some(element.clone());
        while let Some(element) = current {
            if self.matches(&element)? {
                return Ok(Some(element));
            }
            current = element.parent_element();
        }
        Ok(None)
    }
}

impl Drop for CssSelector {
    fn drop(&mut self) {
        unsafe {
            lxb_css_memory_destroy(self.memory, true);
        }
    }
}
