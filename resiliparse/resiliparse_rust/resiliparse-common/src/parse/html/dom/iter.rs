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

//! Tree traversal tools.
//!
//! Tools for iterating DOM (sub) trees.

use std::ptr;

use crate::parse::html::dom::wrap_raw_node;
use crate::parse::html::dom::node::{ElementNode, Node, NodeRef};
use crate::parse::html::dom::traits::NodeInterface;
use crate::third_party::lexbor::*;


pub(crate) struct NodeIteratorRaw {
    root: *mut lxb_dom_node_t,
    next_node: *mut lxb_dom_node_t,
}

unsafe impl Send for NodeIteratorRaw {}
unsafe impl Sync for NodeIteratorRaw {}


impl NodeIteratorRaw {
    pub(crate) unsafe fn new(root: *mut lxb_dom_node_t) -> Self {
        if root.is_null() || unsafe { (*root).first_child }.is_null() {
            Self { root: ptr::null_mut(), next_node: ptr::null_mut() }
        } else {
            Self { root, next_node: root }
        }
    }
}

impl Iterator for NodeIteratorRaw {
    type Item = *mut lxb_dom_node_t;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_node.is_null() {
            return None;
        }

        let return_node = self.next_node;
        unsafe {
            if !(*self.next_node).first_child.is_null() {
                self.next_node = (*self.next_node).first_child;
            } else {
                while self.next_node != self.root && (*self.next_node).next.is_null() {
                    self.next_node = (*self.next_node).parent;
                }
                if self.next_node == self.root {
                    self.next_node = ptr::null_mut();
                    return Some(return_node);
                }
                self.next_node = (*self.next_node).next;
            }
        }
        Some(return_node)
    }
}

// -------------------------------------- Generic Iterators ----------------------------------------

macro_rules! impl_iterator_for {
    ($Self: ty) => {
        impl Iterator for $Self {
            type Item = Node;

            fn next(&mut self) -> Option<Self::Item> {
                wrap_raw_node(&self.root.tree_(), self.iterator_raw.next()?)
            }
        }
    };
}

pub struct NodeIterator<'a> {
    root: NodeRef<'a>,
    iterator_raw: NodeIteratorRaw
}

impl<'a> NodeIterator<'a> {
    pub(crate) fn new(root: NodeRef<'a>) -> Self {
        Self { root: root.clone(), iterator_raw: unsafe { NodeIteratorRaw::new(root.node_ptr_()) } }
    }
}

impl_iterator_for!(NodeIterator<'_>);


pub struct NodeIteratorOwned {
    root: Node,
    iterator_raw: NodeIteratorRaw
}

impl NodeIteratorOwned {
    pub(crate) fn new(root: Node) -> Self {
        let ptr = root.node_ptr_();
        Self { root: root, iterator_raw: unsafe { NodeIteratorRaw::new(ptr) } }
    }
}

impl_iterator_for!(NodeIteratorOwned);


// --------------------------------------- ElementIterator -----------------------------------------

pub struct ElementIterator<'a> {
    root: &'a dyn NodeInterface,
    iterator_raw: NodeIteratorRaw
}

impl<'a> ElementIterator<'a> {
    pub(crate) fn new(root: &'a dyn NodeInterface) -> Self {
        Self { root, iterator_raw: unsafe { NodeIteratorRaw::new(root.node_ptr_()) } }
    }
}

impl Iterator for ElementIterator<'_> {
    type Item = ElementNode;

    fn next(&mut self) -> Option<Self::Item> {
        let tree = &self.root.tree_();
        while let Some(next) = unsafe { self.iterator_raw.next()?.as_ref() } {
            if next.type_ != lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_ELEMENT {
                continue
            }
            if let Some(Node::Element(e)) = wrap_raw_node(tree, self.iterator_raw.next()?) {
                return Some(e)
            }
        }
        None
    }
}
