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

use std::ptr;
use crate::parse::html::dom::node::{ElementNode, Node};
use crate::parse::html::dom::node_base::NodeBase;
use crate::third_party::lexbor::*;

pub(super) struct NodeIteratorRaw {
    root: *mut lxb_dom_node_t,
    next_node: *mut lxb_dom_node_t,
}

impl NodeIteratorRaw {
    pub(super) unsafe fn new(root: *mut lxb_dom_node_t) -> Self {
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

pub struct NodeIterator<'a> {
    root: &'a NodeBase,
    iterator_raw: NodeIteratorRaw
}

impl<'a> NodeIterator<'a> {
    pub(super) fn new(root: &'a NodeBase) -> Self {
        Self { root, iterator_raw: unsafe { NodeIteratorRaw::new(root.node) } }
    }
}

impl Iterator for NodeIterator<'_> {
    type Item = Node;

    fn next(&mut self) -> Option<Self::Item> {
        NodeBase::wrap_node(&self.root.tree, self.iterator_raw.next()?)
    }
}

pub struct ElementIterator<'a> {
    root: &'a NodeBase,
    iterator_raw: NodeIteratorRaw
}

impl<'a> ElementIterator<'a> {
    pub(super) fn new(root: &'a NodeBase) -> Self {
        Self { root, iterator_raw: unsafe { NodeIteratorRaw::new(root.node) } }
    }
}

impl Iterator for ElementIterator<'_> {
    type Item = ElementNode;

    fn next(&mut self) -> Option<Self::Item> {
        let tree = &self.root.tree;
        while let Some(next) = unsafe { self.iterator_raw.next()?.as_ref() } {
            if next.type_ != lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_ELEMENT {
                continue
            }
            if let Some(Node::Element(e)) = NodeBase::wrap_node(tree, self.iterator_raw.next()?) {
                return Some(e)
            }
        }
        None
    }
}
