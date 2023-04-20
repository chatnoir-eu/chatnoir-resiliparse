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
use std::fmt::{Display, Formatter};
use std::rc::Rc;

use crate::parse::html::dom::node::*;
use crate::parse::html::dom::node_base::NodeBase;
use crate::parse::html::tree::HTMLDocument;
use crate::third_party::lexbor::*;

pub mod coll;
pub mod iter;
pub mod node;
pub mod node_base;
pub mod traits;


#[derive(Debug)]
pub struct DOMError {
    msg: String
}

impl Display for DOMError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "DOM error: {}", self.msg)
    }
}

impl Error for DOMError {}


// --------------------------------------------- Helpers -------------------------------------------


unsafe fn element_by_id(node: &NodeBase, id: &str) -> Option<ElementNode> {
    let coll = lxb_dom_collection_create(node.doc_ptr_unchecked());
    if coll.is_null() {
        return None;
    }
    lxb_dom_elements_by_attr(node.node as *mut lxb_dom_element_t, coll, "id".as_ptr(), 2,
                             id.as_ptr(), id.len(), false);
    let matched_node = lxb_dom_collection_node_noi(coll, 0);
    lxb_dom_collection_destroy(coll, true);
    Some(ElementNode { node_base: NodeBase::new_base(&node.tree, matched_node)? })
}

unsafe fn elements_by_attr(node: &NodeBase, qualified_name: &str, value: &str) -> Vec<ElementNode> {
    let coll = lxb_dom_collection_create(node.doc_ptr_unchecked());
    if coll.is_null() {
        return Vec::default();
    }
    lxb_dom_elements_by_attr(node.node as *mut lxb_dom_element_t, coll, qualified_name.as_ptr(),
                             qualified_name.len(), value.as_ptr(), value.len(), false);
    dom_coll_to_vec(&node.tree, coll, true)
}

unsafe fn elements_by_tag_name(node: &NodeBase, qualified_name: &str) -> Vec<ElementNode> {
    let coll = lxb_dom_collection_create(node.doc_ptr_unchecked());
    if coll.is_null() {
        return Vec::default();
    }
    lxb_dom_node_by_tag_name(node.node, coll, qualified_name.as_ptr(), qualified_name.len());
    dom_coll_to_vec(&node.tree, coll, true)
}

unsafe fn elements_by_class_name(node: &NodeBase, class_name: &str) -> Vec<ElementNode> {
    let coll = lxb_dom_collection_create(node.doc_ptr_unchecked());
    if coll.is_null() {
        return Vec::default();
    }
    lxb_dom_elements_by_class_name(node.node.cast(), coll, class_name.as_ptr(), class_name.len());
    dom_coll_to_vec(&node.tree, coll, true)
}

unsafe fn dom_coll_to_vec(tree: &Rc<HTMLDocument>, coll: *mut lxb_dom_collection_t,
                          destroy: bool) -> Vec<ElementNode> {
    let mut v = Vec::<ElementNode>::with_capacity(lxb_dom_collection_length_noi(coll));
    for i in 0..lxb_dom_collection_length_noi(coll) {
        if let Some(b) = NodeBase::new_base(&tree, lxb_dom_collection_node_noi(coll, i)) {
            v.push(ElementNode { node_base: b });
        }
    }
    if destroy {
        lxb_dom_collection_destroy(coll, true);
    }
    v
}
