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

//! Node base type (abstract).
//!
//! Common DOM node base methods supported by all concrete node types.

use std::fmt::{Debug, Display, Formatter};
use std::ptr;
use std::ptr::addr_of;
use std::sync::Arc;

use crate::parse::html::dom::coll::NodeList;
use crate::parse::html::dom::*;
use crate::parse::html::dom::iter::*;
use crate::parse::html::dom::traits::{Attr, Element, NodeInterface, NodeType};
use crate::parse::html::serialize::node_serialize_html;
use crate::parse::html::lexbor::*;
use crate::parse::html::tree::HTMLDocument;

/// Base DOM node implementation.
///
/// A `NodeBase` is not supposed to be instantiated or used directly. Use types from
/// the [node] module instead.
#[derive(Clone)]
pub struct NodeBase {
    pub(in super::super) tree: Arc<HTMLDocument>,
    pub(in super::super) node: *mut lxb_dom_node_t,
}

// TODO: Make this actually thread-safe
unsafe impl Send for NodeBase {}
unsafe impl Sync for NodeBase {}

// Cannot be done until https://github.com/lexbor/lexbor/issues/132 is fixed
// If you create lots of unparented DOMNodes, we may leak memory
// impl Drop for NodeBase {
//     fn drop(&mut self) {
//         unsafe {
//             if !self.node.is_null() && (*self.node).parent.is_null() && (*self.node).type_ != LXB_DOM_NODE_TYPE_DOCUMENT {
//                 lxb_dom_node_destroy_deep(self.node);
//                 self.node = ptr::null_mut();
//             }
//         }
//     }
// }

impl PartialEq<NodeBase> for NodeBase {
    fn eq(&self, other: &Self) -> bool {
        self.node == other.node
    }
}

impl PartialEq<Node> for NodeBase {
    fn eq(&self, other: &Node) -> bool {
        (*self).node == (**other).node
    }
}

impl PartialEq<NodeRef<'_>> for NodeBase {
    fn eq(&self, other: &NodeRef<'_>) -> bool {
        (*self).node == (**other).node
    }
}

impl PartialEq<NodeRef<'_>> for &NodeBase {
    fn eq(&self, other: &NodeRef<'_>) -> bool {
        (**self).node == (**other).node
    }
}

impl Debug for NodeBase {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut tag_repr = self.node_name().unwrap_or_else(|| "#undef".to_owned());
        if let Node::Element(element) = self.into() {
            tag_repr = format!("<{}", tag_repr.to_lowercase());
            element.attributes().iter().for_each(|attr| {
                tag_repr.push(' ');
                tag_repr.push_str(&attr.name().unwrap());
                if attr.value().is_some() {
                    tag_repr.push_str(&format!("={:?}", attr.value().unwrap()));
                }
            });
            tag_repr.push('>');
        }
        f.write_str(&tag_repr)
    }
}

impl Display for NodeBase {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&node_serialize_html(self.node))
    }
}

impl Eq for NodeBase {}

impl NodeBase {
    pub(in super::super) fn wrap_node(tree: &Arc<HTMLDocument>, node: *mut lxb_dom_node_t) -> Option<Node> {
        if node.is_null() {
            return None;
        }
        let node_base = Self { tree: tree.clone(), node };
        use crate::third_party::lexbor::lxb_dom_node_type_t::*;
        match unsafe { (*node).type_ } {
            LXB_DOM_NODE_TYPE_ELEMENT => Some(Node::Element(ElementNode { node_base })),
            LXB_DOM_NODE_TYPE_ATTRIBUTE => Some(Node::Attribute(AttrNode { node_base })),
            LXB_DOM_NODE_TYPE_TEXT => Some(Node::Text(TextNode { node_base })),
            LXB_DOM_NODE_TYPE_CDATA_SECTION => Some(Node::CdataSection(CdataSectionNode { node_base })),
            LXB_DOM_NODE_TYPE_PROCESSING_INSTRUCTION => Some(Node::ProcessingInstruction(
                ProcessingInstructionNode { node_base })),
            LXB_DOM_NODE_TYPE_COMMENT => Some(Node::Comment(CommentNode { node_base })),
            LXB_DOM_NODE_TYPE_DOCUMENT => Some(Node::Document(DocumentNode { node_base })),
            LXB_DOM_NODE_TYPE_DOCUMENT_TYPE => Some(Node::DocumentType(DocumentTypeNode { node_base })),
            LXB_DOM_NODE_TYPE_DOCUMENT_FRAGMENT => Some(Node::DocumentFragment(
                DocumentFragmentNode { node_base })),
            LXB_DOM_NODE_TYPE_NOTATION => Some(Node::Notation(NotationNode { node_base })),
            _ => None
        }
    }

    #[inline]
    pub(in super::super) fn new_base(tree: &Arc<HTMLDocument>, node: *mut lxb_dom_node_t) -> Option<Self> {
        if node.is_null() {
            None
        } else {
            Some(Self { tree: tree.clone(), node })
        }
    }

    pub(super) unsafe fn create_element_unchecked(doc: &NodeBase, local_name: &str) -> Result<ElementNode, DOMError> {
        debug_assert_eq!((*doc.node).type_, lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_DOCUMENT);
        let element = lxb_dom_document_create_element(
            doc.node.cast(), local_name.as_ptr(), local_name.len(), ptr::null_mut());
        if let Some(e) = Self::new_base(&doc.tree, element.cast()) {
            Ok(ElementNode { node_base: e })
        } else {
            Err(DOMError { msg: "ElementNode allocation failed".to_owned() })
        }
    }

    pub(super) unsafe fn create_document_fragment_unchecked(doc: &NodeBase) -> Result<DocumentFragmentNode, DOMError> {
        debug_assert_eq!((*doc.node).type_, lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_DOCUMENT);
        let doc_frag = lxb_dom_document_create_document_fragment(doc.node.cast());
        if let Some(e) = Self::new_base(&doc.tree, doc_frag.cast()) {
            Ok(DocumentFragmentNode { node_base: e })
        } else {
            Err(DOMError { msg: "DocumentFragmentNode allocation failed".to_owned() })
        }
    }

    pub(super) unsafe fn create_text_node_unchecked(doc: &NodeBase, data: &str) -> Result<TextNode, DOMError> {
        debug_assert_eq!((*doc.node).type_, lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_DOCUMENT);
        let text = lxb_dom_document_create_text_node(
            doc.node.cast(), data.as_ptr(), data.len());
        if let Some(t) = Self::new_base(&doc.tree, text.cast()) {
            Ok(TextNode { node_base: t })
        } else {
            Err(DOMError { msg: "TextNode allocation failed".to_owned() })
        }
    }

    pub(super) unsafe fn create_cdata_section_unchecked(doc: &NodeBase, data: &str) -> Result<CdataSectionNode, DOMError> {
        debug_assert_eq!((*doc.node).type_, lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_DOCUMENT);
        let cdata = lxb_dom_document_create_cdata_section(
            doc.node.cast(), data.as_ptr(), data.len());
        if let Some(c) = Self::new_base(&doc.tree, cdata.cast()) {
            Ok(CdataSectionNode { node_base: c })
        } else {
            Err(DOMError { msg: "TextNode allocation failed".to_owned() })
        }
    }

    pub(super) unsafe fn create_comment_unchecked(doc: &NodeBase, data: &str) -> Result<CommentNode, DOMError> {
        debug_assert_eq!((*doc.node).type_, lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_DOCUMENT);
        let comment = lxb_dom_document_create_comment(
            doc.node.cast(), data.as_ptr(), data.len());
        if let Some(c) = Self::new_base(&doc.tree, comment.cast()) {
            Ok(CommentNode { node_base: c })
        } else {
            Err(DOMError { msg: "CommentNode allocation failed".to_owned() })
        }
    }

    pub(super) unsafe fn create_processing_instruction_unchecked(doc: &NodeBase, target: &str, data: &str)
        -> Result<ProcessingInstructionNode, DOMError> {
        debug_assert_eq!((*doc.node).type_, lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_DOCUMENT);
        let proc = lxb_dom_document_create_processing_instruction(
            doc.node.cast(), target.as_ptr(), target.len(), data.as_ptr(), data.len());
        if let Some(p) = Self::new_base(&doc.tree, proc.cast()) {
            Ok(ProcessingInstructionNode { node_base: p })
        } else {
            Err(DOMError { msg: "ProcessingInstructionNode allocation failed".to_owned() })
        }
    }

    pub(super) unsafe fn create_attribute_unchecked(doc: &NodeBase, local_name: &str) -> Result<AttrNode, DOMError> {
        debug_assert_eq!((*doc.node).type_, lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_DOCUMENT);
        let attr = lxb_dom_attr_interface_create(doc.node.cast());
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
        Ok(AttrNode { node_base: Self::new_base(&doc.tree, attr.cast()).unwrap_unchecked() })
    }

    unsafe fn can_have_children_unchecked(&self) -> bool {
        matches!((*self.node).type_,
            lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_ELEMENT
                | lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_DOCUMENT
                | lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_DOCUMENT_FRAGMENT
        )
    }

    unsafe fn insert_before_unchecked<'a>(&mut self, node: &'a Node, child: Option<&Node>) -> Option<&'a Node> {
        if let Some(c) = child {
            if c.parent_node()? != *self || !self.can_have_children_unchecked() ||  node.contains(c) {
                return None;
            }
            if node == c {
                return Some(node);
            }
            // TODO: Insert fragment itself once Lexbor bug is fixed: https://github.com/lexbor/lexbor/issues/180
            if let Node::DocumentFragment(d) = node {
                d.child_nodes().iter().for_each(|c2| {
                    lxb_dom_node_insert_before(c.node, c2.node);
                });
                // Lexbor doesn't reset the child pointers upon moving elements from DocumentFragments
                (*d.node_base.node).first_child = ptr::null_mut();
                (*d.node_base.node).last_child = ptr::null_mut();
            } else {
                lxb_dom_node_insert_before(c.node, node.node);
            }
            Some(node)
        } else {
            self.append_child_unchecked(node)
        }
    }

    unsafe fn append_child_unchecked<'a>(&mut self, node: &'a Node) -> Option<&'a Node> {
        if !self.can_have_children_unchecked() {
            return None;
        }
        // TODO: Insert fragment itself once Lexbor bug is fixed: https://github.com/lexbor/lexbor/issues/180
        if let Node::DocumentFragment(d) = node {
            d.child_nodes().iter().for_each(|c| {
                lxb_dom_node_insert_child(self.node, c.node);
            });
            // Lexbor doesn't reset the child pointers upon moving elements from DocumentFragments
            (*d.node_base.node).first_child = ptr::null_mut();
            (*d.node_base.node).last_child = ptr::null_mut();
        } else {
            lxb_dom_node_insert_child(self.node, node.node);
        }
        Some(node)
    }

    unsafe fn replace_child_unchecked<'a>(&mut self, node: &'a Node, child: &'a Node) -> Option<&'a Node> {
        if child.parent_node()? != *self || !self.can_have_children_unchecked() {
            return None;
        }
        if node == child {
            return Some(node);
        }
        self.insert_before_unchecked(node, Some(child))?;
        self.remove_child_unchecked(child)?;
        Some(node)
    }

    unsafe fn remove_child_unchecked<'a>(&mut self, node: &'a Node) -> Option<&'a Node> {
        if node.parent_node()? != *self || !self.can_have_children_unchecked() {
            return None;
        }
        lxb_dom_node_remove(node.node);
        Some(node)
    }

    #[inline(always)]
    pub(super) unsafe fn doc_ptr_unchecked(&self) -> *mut lxb_dom_document_t {
        (*self.node).owner_document
    }

    pub(super) unsafe fn iter_raw(&self) -> NodeIteratorRaw {
        NodeIteratorRaw::new(self.node)
    }
}

impl NodeInterface for NodeBase {
    unsafe fn node_name_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node, lxb_dom_node_name)
    }

    /// Node text value.
    unsafe fn node_value_unchecked(&self) -> Option<&str> {
        use crate::third_party::lexbor::lxb_dom_node_type_t::*;
        match (*self.node).type_ {
            LXB_DOM_NODE_TYPE_ATTRIBUTE => str_from_lxb_str_cb(self.node, lxb_dom_attr_value_noi),
            LXB_DOM_NODE_TYPE_TEXT | LXB_DOM_NODE_TYPE_CDATA_SECTION |
            LXB_DOM_NODE_TYPE_COMMENT | LXB_DOM_NODE_TYPE_PROCESSING_INSTRUCTION =>
                str_from_lxb_str_t(addr_of!((*(self.node as *const lxb_dom_character_data_t)).data)),
            _ => None
        }
    }

    #[inline(always)]
    fn upcast(&self) -> &NodeBase {
        self
    }

    #[inline(always)]
    fn upcast_mut(&mut self) -> &mut NodeBase {
        self
    }

    #[inline(always)]
    fn as_noderef(&self) -> NodeRef {
        unreachable!();
    }

    #[inline(always)]
    fn to_node(&self) -> Node {
        unreachable!();
    }

    fn node_type(&self) -> Option<NodeType> {
        use lxb_dom_node_type_t::*;
        match unsafe { self.node.as_ref()? }.type_ {
            LXB_DOM_NODE_TYPE_ELEMENT => Some(NodeType::Element),
            LXB_DOM_NODE_TYPE_ATTRIBUTE => Some(NodeType::Attribute),
            LXB_DOM_NODE_TYPE_TEXT => Some(NodeType::Text),
            LXB_DOM_NODE_TYPE_CDATA_SECTION => Some(NodeType::CdataSection),
            LXB_DOM_NODE_TYPE_ENTITY_REFERENCE => Some(NodeType::EntityReference),
            LXB_DOM_NODE_TYPE_ENTITY => Some(NodeType::Entity),
            LXB_DOM_NODE_TYPE_PROCESSING_INSTRUCTION => Some(NodeType::ProcessingInstruction),
            LXB_DOM_NODE_TYPE_COMMENT => Some(NodeType::Comment),
            LXB_DOM_NODE_TYPE_DOCUMENT => Some(NodeType::Document),
            LXB_DOM_NODE_TYPE_DOCUMENT_TYPE => Some(NodeType::DocumentType),
            LXB_DOM_NODE_TYPE_DOCUMENT_FRAGMENT => Some(NodeType::DocumentFragment),
            LXB_DOM_NODE_TYPE_NOTATION => Some(NodeType::Notation),
            _ => None
        }
    }

    /// DOM element tag or node name.
    fn node_name(&self) -> Option<String> {
        check_node!(self);
        unsafe { Some(self.node_name_unchecked()?.to_owned()) }
    }

    /// Node text value.
    #[inline]
    fn node_value(&self) -> Option<String> {
        check_node!(self);
        unsafe { Some(self.node_value_unchecked()?.to_owned()) }
    }

    fn set_node_value(&mut self, value: &str) {
        check_node!(self);
        unsafe { lxb_dom_node_text_content_set(self.node, value.as_ptr(), value.len()); }
    }

    /// Text contents of this DOM node and its children.
    fn text_content(&self) -> Option<String> {
        check_node!(self);
        let ret_value;
        unsafe {
            let mut l = 0;
            let t = lxb_dom_node_text_content(self.node, &mut l);
            ret_value = str_from_lxb_char_t(t, l).map(String::from);
            lxb_dom_document_destroy_text_noi(self.doc_ptr_unchecked(), t);
        }
        ret_value
    }

    #[inline]
    fn set_text_content(&mut self, content: &str) {
        self.set_node_value(content)
    }

    fn owner_document(&self) -> Option<DocumentNode> {
        check_node!(self);
        let d = unsafe { self.doc_ptr_unchecked() };
        if !d.is_null() {
            Some(Self::wrap_node(&self.tree, d.cast())?.into())
        } else {
            None
        }
    }

    /// Parent of this node.
    fn parent_node(&self) -> Option<Node> {
        Self::wrap_node(&self.tree, unsafe { self.node.as_ref()?.parent })
    }

    #[inline]
    fn parent_element(&self) -> Option<ElementNode> {
        if let Node::Element(n) = self.parent_node()? {
            Some(n)
        } else {
            None
        }
    }

    #[inline]
    fn has_child_nodes(&self) -> bool {
        self.first_child().is_some()
    }

    fn contains(&self, node: &Node) -> bool {
        check_nodes!(self, node);
        if self == node {
            return true;
        }
        unsafe {
            self.iter_raw()
                .find(|&n| n == node.node)
                .is_some()
        }
    }

    /// List of child nodes.
    fn child_nodes(&self) -> NodeList {
        NodeList::new_live(self.into(), None, |n, _| {
            let mut nodes = Vec::new();
            let mut child = n.first_child();
            while let Some(c) = child {
                child = c.next_sibling();
                nodes.push(c);
            }
            nodes
        })
    }

    /// First child element of this DOM node.
    fn first_child(&self) -> Option<Node> {
        Self::wrap_node(&self.tree, unsafe { self.node.as_ref()?.first_child })
    }

    /// Last child element of this DOM node.
    fn last_child(&self) -> Option<Node> {
        Self::wrap_node(&self.tree, unsafe { self.node.as_ref()?.last_child })
    }

    /// Previous sibling node.
    fn previous_sibling(&self) -> Option<Node> {
        Self::wrap_node(&self.tree, unsafe { self.node.as_ref()?.prev })
    }

    /// Next sibling node.
    fn next_sibling(&self) -> Option<Node> {
        Self::wrap_node(&self.tree, unsafe { self.node.as_ref()?.next })
    }

    fn clone_node(&self, deep: bool) -> Option<Node> {
        check_node!(self);
        Self::wrap_node(&self.tree, unsafe { lxb_dom_node_clone(self.node, deep) })
    }

    fn insert_before<'a>(&mut self, node: &'a Node, child: Option<&'a Node>) -> Option<&'a Node> {
        check_nodes!(self, node);
        if child.is_some() {
            check_nodes!(node, child?);
            if self == child? {
                return None;
            }
        }
        unsafe { self.insert_before_unchecked(node, child) }
    }

    fn append_child<'a>(&mut self, node: &'a Node) -> Option<&'a Node> {
        check_nodes!(self, node);
        unsafe { self.append_child_unchecked(node) }
    }

    fn replace_child<'a>(&mut self, node: &'a Node, child: &'a Node) -> Option<&'a Node> {
        check_nodes!(self, node);
        check_nodes!(node, child);
        unsafe { self.replace_child_unchecked(node, child) }
    }

    fn remove_child<'a>(&mut self, node: &'a Node) -> Option<&'a Node> {
        check_nodes!(self, node);
        unsafe { self.remove_child_unchecked(node) }
    }

    fn iter(&self) -> NodeIterator {
        NodeIterator::new(&self)
    }

    fn iter_elements(&self) -> ElementIterator {
        ElementIterator::new(&self)
    }
}

impl IntoIterator for NodeBase {
    type Item = Node;
    type IntoIter = NodeIteratorOwned;

    fn into_iter(self) -> Self::IntoIter {
        NodeIteratorOwned::new(self)
    }
}
