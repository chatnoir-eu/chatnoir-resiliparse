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

//! DOM API types and methods.

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::ptr;
use std::sync::Arc;

use crate::parse::html::dom::node::*;
use crate::parse::html::dom::traits::{Element, NodeInterface, NodeInterfaceBaseImpl};
use crate::parse::html::tree::HTMLDocument;
use crate::third_party::lexbor::*;
use crate::third_party::lexbor::lxb_dom_node_type_t::*;

pub mod coll;
pub mod iter;
pub mod node;
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


// -------------------------------------- Crate-public Helpers -------------------------------------

pub(crate) fn wrap_any_raw_node(tree: &Arc<HTMLDocument>, node: *mut lxb_dom_node_t) -> Option<Node> {
    match unsafe { node.as_ref()?.type_ } {
        LXB_DOM_NODE_TYPE_ELEMENT =>
            Some(Node::Element(ElementNode::new(&tree, node))),
        LXB_DOM_NODE_TYPE_ATTRIBUTE =>
            Some(Node::Attribute(AttrNode::new(&tree, node))),
        LXB_DOM_NODE_TYPE_TEXT =>
            Some(Node::Text(TextNode::new(&tree, node))),
        LXB_DOM_NODE_TYPE_CDATA_SECTION =>
            Some(Node::CdataSection(CdataSectionNode::new(&tree, node))),
        LXB_DOM_NODE_TYPE_PROCESSING_INSTRUCTION =>
            Some(Node::ProcessingInstruction(ProcessingInstructionNode::new(&tree, node))),
        LXB_DOM_NODE_TYPE_COMMENT =>
            Some(Node::Comment(CommentNode::new(&tree, node))),
        LXB_DOM_NODE_TYPE_DOCUMENT =>
            Some(Node::Document(DocumentNode::new(&tree, node))),
        LXB_DOM_NODE_TYPE_DOCUMENT_TYPE =>
            Some(Node::DocumentType(DocumentTypeNode::new(&tree, node))),
        LXB_DOM_NODE_TYPE_DOCUMENT_FRAGMENT =>
            Some(Node::DocumentFragment(DocumentFragmentNode::new(&tree, node))),
        LXB_DOM_NODE_TYPE_NOTATION =>
            Some(Node::Notation(NotationNode::new(&tree, node))),
        _ => None
    }
}


// ----------------------------------------- Private Helpers ---------------------------------------

unsafe fn next_element_unchecked(tree: &Arc<HTMLDocument>, mut child: *mut lxb_dom_node_t) -> Option<ElementNode> {
    while !child.is_null() {
        if (*child).type_ == LXB_DOM_NODE_TYPE_ELEMENT {
            return Some(ElementNode::new(tree, child))
        }
        child = (*child).next;
    }
    None
}

unsafe fn previous_element_unchecked(tree: &Arc<HTMLDocument>, mut child: *mut lxb_dom_node_t) -> Option<ElementNode> {
    while !child.is_null() {
        if (*child).type_ == LXB_DOM_NODE_TYPE_ELEMENT {
            return Some(ElementNode::new(tree, child))
        }
        child = (*child).prev;
    }
    None
}

unsafe fn create_element_unchecked(doc: &DocumentNode, local_name: &str) -> Result<ElementNode, DOMError> {
    let element = lxb_dom_document_create_element(
        doc.node_ptr_().cast(), local_name.as_ptr(), local_name.len(), ptr::null_mut());
    if element.is_null() {
        return Err(DOMError { msg: "ElementNode allocation failed".to_owned() })
    }
    Ok(ElementNode::new(&doc.tree_(), element.cast()))
}

unsafe fn create_document_fragment_unchecked(doc: &DocumentNode) -> Result<DocumentFragmentNode, DOMError> {
    let doc_frag = lxb_dom_document_create_document_fragment(doc.node_ptr_().cast());
    if doc_frag.is_null() {
        return Err(DOMError { msg: "DocumentFragmentNode allocation failed".to_owned() })
    }
    Ok(DocumentFragmentNode::new(&doc.tree_(), doc_frag.cast()))
}

unsafe fn create_text_node_unchecked(doc: &DocumentNode, data: &str) -> Result<TextNode, DOMError> {
    let text = lxb_dom_document_create_text_node(
        doc.node_ptr_().cast(), data.as_ptr(), data.len());
    if text.is_null() {
        return Err(DOMError { msg: "TextNode allocation failed".to_owned() })
    }
    Ok(TextNode::new(&doc.tree_(), text.cast()))
}

unsafe fn create_cdata_section_unchecked(doc: &DocumentNode, data: &str) -> Result<CdataSectionNode, DOMError> {
    let cdata = lxb_dom_document_create_cdata_section(
        doc.node_ptr_().cast(), data.as_ptr(), data.len());
    if cdata.is_null() {
        return Err(DOMError { msg: "CdataSectionNode allocation failed".to_owned() })
    }
    Ok(CdataSectionNode::new(&doc.tree_(), cdata.cast()))
}

unsafe fn create_comment_unchecked(doc: &DocumentNode, data: &str) -> Result<CommentNode, DOMError> {
    let comment = lxb_dom_document_create_comment(
        doc.node_ptr_().cast(), data.as_ptr(), data.len());
    if comment.is_null() {
        return Err(DOMError { msg: "CommentNode allocation failed".to_owned() })
    }
    Ok(CommentNode::new(&doc.tree_(), comment.cast()))
}

unsafe fn create_processing_instruction_unchecked(doc: &DocumentNode, target: &str, data: &str)
    -> Result<ProcessingInstructionNode, DOMError> {
    let proc = lxb_dom_document_create_processing_instruction(
        doc.node_ptr_().cast(), target.as_ptr(), target.len(), data.as_ptr(), data.len());
    if proc.is_null() {
        return Err(DOMError { msg: "ProcessingInstructionNode allocation failed".to_owned() })
    }
    Ok(ProcessingInstructionNode::new(&doc.tree_(), proc.cast()))
}

unsafe fn create_attribute_unchecked(doc: &DocumentNode, local_name: &str) -> Result<AttrNode, DOMError> {
    let attr = lxb_dom_attr_interface_create(doc.node_ptr_().cast());
    if attr.is_null() {
        return Err(DOMError { msg: "AttrNode allocation failed".to_owned() })
    }
    let status = unsafe {
        lxb_dom_attr_set_name(attr, local_name.as_ptr(), local_name.len(), true)
    };
    if status != lexbor_status_t::LXB_STATUS_OK {
        lxb_dom_attr_interface_destroy(attr);
        return Err(DOMError { msg: "Failed to set attribute name".to_owned() } );
    }
    Ok(AttrNode::new(&doc.tree_(), attr.cast()))
}

unsafe fn get_element_by_id(node: &NodeRef, id: &str, case_insensitive: bool) -> Option<ElementNode> {
    let coll = lxb_dom_collection_create(node.doc_ptr_unchecked());
    if coll.is_null() {
        return None;
    }
    lxb_dom_elements_by_attr(*node.node_ptr_() as *mut lxb_dom_element_t, coll, "id".as_ptr(), 2,
                             id.as_ptr(), id.len(), case_insensitive);
    let matched_node = lxb_dom_collection_node_noi(coll, 0);
    lxb_dom_collection_destroy(coll, true);
    if !matched_node.is_null() {
        Some(ElementNode::new(&node.tree_(), matched_node))
    } else {
        None
    }
}

unsafe fn get_elements_by_attr(node: &NodeRef, name: &str, value: &str, case_insensitive: bool) -> Vec<ElementNode> {
    let coll = lxb_dom_collection_create(node.doc_ptr_unchecked());
    if coll.is_null() {
        return Vec::default();
    }
    lxb_dom_elements_by_attr(*node.node_ptr_() as *mut lxb_dom_element_t, coll, name.as_ptr(),
                             name.len(), value.as_ptr(), value.len(), case_insensitive);
    dom_coll_to_vec(&node.tree_(), coll, true)
}

unsafe fn get_elements_by_tag_name(node: &Node, name: &str) -> Vec<ElementNode> {
    let coll = lxb_dom_collection_create(node.doc_ptr_unchecked());
    if coll.is_null() {
        return Vec::default();
    }
    lxb_dom_node_by_tag_name(*node.node_ptr_(), coll, name.as_ptr(), name.len());
    dom_coll_to_vec(&node.tree_(), coll, true)
}

unsafe fn get_elements_by_class_name(node: &Node, class_names: &str) -> Vec<ElementNode> {
    let coll = lxb_dom_collection_create(node.doc_ptr_unchecked());
    if coll.is_null() {
        return Vec::default();
    }
    lxb_dom_elements_by_class_name(node.node_ptr_().cast(), coll, class_names.as_ptr(), class_names.len());
    dom_coll_to_vec(&node.tree_(), coll, true)
}

unsafe fn dom_coll_to_vec(tree: &Arc<HTMLDocument>, coll: *mut lxb_dom_collection_t,
                          destroy: bool) -> Vec<ElementNode> {
    let mut coll_vec = Vec::<ElementNode>::with_capacity(lxb_dom_collection_length_noi(coll));
    for i in 0..lxb_dom_collection_length_noi(coll) {
        coll_vec.push(ElementNode::new(&tree, lxb_dom_collection_node_noi(coll, i)));
    }
    if destroy {
        lxb_dom_collection_destroy(coll, true);
    }
    coll_vec
}
