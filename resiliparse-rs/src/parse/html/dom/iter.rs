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

use crate::parse::html::dom::node::{ElementNode, Node, NodeRef};
use crate::parse::html::dom::traits::NodeInterface;
use crate::parse::html::dom::wrap_any_raw_node;
use crate::third_party::lexbor::*;
use parking_lot::ReentrantMutex;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::iter::FusedIterator;
use std::ptr;
use std::sync::Arc;

use crate::parse::html::tree::HTMLDocument;

pub(crate) struct NodeIteratorRaw {
    root: ReentrantMutex<*mut lxb_dom_node_t>,
    next_node: ReentrantMutex<*mut lxb_dom_node_t>,
}

unsafe impl Send for NodeIteratorRaw {}
unsafe impl Sync for NodeIteratorRaw {}

impl NodeIteratorRaw {
    fn is_exhausted(&self) -> bool {
        self.next_node.lock().is_null()
    }

    pub(crate) unsafe fn new(root: *mut lxb_dom_node_t) -> Self {
        Self {
            root: ReentrantMutex::new(root),
            next_node: ReentrantMutex::new(root),
        }
    }
}

impl Iterator for NodeIteratorRaw {
    type Item = *mut lxb_dom_node_t;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.next_node.get_mut();
        let root = self.root.get_mut();
        if next.is_null() || root.is_null() {
            return None;
        }

        let return_node = next.clone();
        unsafe {
            if !(*(*next)).first_child.is_null() {
                *next = (*(*next)).first_child;
            } else {
                while next != root && (*(*next)).next.is_null() {
                    *next = (*(*next)).parent;
                }
                if next == root {
                    *next = ptr::null_mut();
                    return Some(return_node);
                }
                *next = (*(*next)).next;
            }
        }
        Some(return_node)
    }
}

// -------------------------------------- Generic Iterators ----------------------------------------

/// A lazy traversal was invalidated by mutation of its document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraversalError {
    TreeModified,
}

impl Display for TraversalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TreeModified => f.write_str("DOM document modified during traversal"),
        }
    }
}

impl Error for TraversalError {}

struct TraversalState {
    document: Arc<HTMLDocument>,
    expected_epoch: u64,
    cursor: NodeIteratorRaw,
    finished: bool,
}

impl TraversalState {
    fn new(root: NodeRef<'_>) -> Self {
        let document = root.tree_();
        let (expected_epoch, cursor) = {
            let _guard = document.doc_ptr();
            (document.mutation_epoch(), unsafe { NodeIteratorRaw::new(*root.node_ptr_()) })
        };
        Self {
            document,
            expected_epoch,
            cursor,
            finished: false,
        }
    }

    fn next(&mut self) -> Option<Result<Node, TraversalError>> {
        if self.finished {
            return None;
        }
        let _guard = self.document.doc_ptr();
        // Testing a saved pointer for null does not dereference native memory.
        // Already exhausted iterators remain exhausted even after mutation.
        if self.cursor.is_exhausted() {
            self.finished = true;
            return None;
        }
        if self.document.mutation_epoch() != self.expected_epoch {
            self.finished = true;
            return Some(Err(TraversalError::TreeModified));
        }
        // Keep the document locked through cursor advancement and ref-counting
        // the result, so a writer cannot free either pointer between these steps.
        let result = self
            .cursor
            .next()
            .and_then(|node| wrap_any_raw_node(&self.document, node));
        if result.is_none() || self.cursor.is_exhausted() {
            self.finished = true;
        }
        result.map(Ok)
    }
}

macro_rules! impl_iterator_for {
    ($Self: ty) => {
        impl Iterator for $Self {
            type Item = Result<Node, TraversalError>;

            fn next(&mut self) -> Option<Self::Item> {
                self.state.next()
            }
        }

        impl FusedIterator for $Self {}
    };
}

/// Lazy pre-order traversal of a subtree, including its root.
///
/// Construction and iterator storage are O(1). Mutations anywhere in the owning
/// document invalidate traversal: the next step yields [TraversalError::TreeModified]
/// once, then the iterator is exhausted. Already exhausted iterators stay exhausted.
/// Native reference-count changes alone do not invalidate traversal, but node
/// destruction and value/attribute setters conservatively do.
pub struct NodeIterator<'a> {
    state: TraversalState,
    _root: NodeRef<'a>,
}

impl<'a> NodeIterator<'a> {
    pub(crate) fn new(root: NodeRef<'a>) -> Self {
        Self {
            state: TraversalState::new(root.clone()),
            _root: root,
        }
    }
}

impl_iterator_for!(NodeIterator<'_>);

/// Owned lazy traversal with the same invalidation semantics as [NodeIterator].
pub struct NodeIteratorOwned {
    state: TraversalState,
    _root: Node,
}

impl NodeIteratorOwned {
    pub(crate) fn new(root: Node) -> Self {
        Self {
            state: TraversalState::new(root.as_noderef()),
            _root: root,
        }
    }
}

impl_iterator_for!(NodeIteratorOwned);

// --------------------------------------- ElementIterator -----------------------------------------

/// Lazy element-only pre-order traversal, including an element root.
///
/// Invalidation errors are forwarded once, as described by [NodeIterator].
pub struct ElementIterator<'a> {
    nodes: NodeIterator<'a>,
}

impl<'a> ElementIterator<'a> {
    pub(crate) fn new(root: &'a dyn NodeInterface) -> Self {
        Self {
            nodes: NodeIterator::new(root.as_noderef()),
        }
    }
}

impl Iterator for ElementIterator<'_> {
    type Item = Result<ElementNode, TraversalError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.nodes.find_map(|node| match node {
            Ok(Node::Element(element)) => Some(Ok(element)),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
    }
}

impl FusedIterator for ElementIterator<'_> {}
