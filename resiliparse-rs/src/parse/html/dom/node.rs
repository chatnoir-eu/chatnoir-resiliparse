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

//! Node types.
//!
//! Concrete node type implementations.

use crate::parse::html::css::*;
use crate::parse::html::dom::coll::*;
use crate::parse::html::dom::iter::*;
use crate::parse::html::dom::traits::*;
use crate::parse::html::dom::*;
use crate::parse::html::lexbor::{str_from_lxb_char_t, str_from_lxb_str_cb, str_from_lxb_str_t};
use crate::parse::html::serialize::{node_format_visible_text, node_serialize_html};
use std::cell::Cell;
use std::fmt::{Debug, Display, Formatter};
use std::ops::{Deref, DerefMut};
use std::ptr;
use std::ptr::addr_of_mut;
use std::sync::Arc;

// Re-export NodeType publicly
pub use crate::parse::html::dom::traits::NodeType;

// ----------------------------------------- Node Base Enum ----------------------------------------

// Keep the owned and borrowed node variants and their common dispatch together.
macro_rules! define_node_enums {
    ($($Variant:ident($Type:ty)),+ $(,)?) => {
        /// An owned handle to a generic DOM node.
        ///
        /// Concrete types' `TryFrom<Node>` implementations provide checked downcasts; a mismatch
        /// returns `Err(original_node)`. Borrowed access is available through the `as_*` views,
        /// while common operations and traversal use [`NodeInterface`].
        #[derive(Clone, PartialEq, Debug)]
        pub enum Node {
            $(#[doc = concat!("An owned node handle containing a ", stringify!($Variant), " node.")] $Variant($Type)),+
        }

        /// A borrowed view of an owned DOM node handle.
        #[derive(Clone, PartialEq, Debug)]
        pub enum NodeRef<'a> {
            $(#[doc = concat!("A borrowed ", stringify!($Variant), " node handle.")] $Variant(&'a $Type)),+
        }

        /// A mutable borrowed view of an owned DOM node handle.
        #[derive(PartialEq, Debug)]
        pub enum NodeRefMut<'a> {
            $(#[doc = concat!("A mutable borrowed ", stringify!($Variant), " node handle.")] $Variant(&'a mut $Type)),+
        }

        impl NodeInterfaceBaseImpl for Node {
            fn tree_(&self) -> Arc<HTMLDocument> {
                match self { $(Self::$Variant(node) => node.tree_()),+ }
            }

            fn node_ptr_(&self) -> *mut lxb_dom_node_t {
                match self { $(Self::$Variant(node) => node.node_ptr_()),+ }
            }

            fn reset_node_ptr_(&self) {
                match self { $(Self::$Variant(node) => node.reset_node_ptr_()),+ }
            }
        }

        impl NodeInterface for Node {
            fn as_noderef(&self) -> NodeRef<'_> {
                match self { $(Self::$Variant(node) => NodeRef::$Variant(node)),+ }
            }

            fn as_noderef_mut(&mut self) -> NodeRefMut<'_> {
                match self { $(Self::$Variant(node) => NodeRefMut::$Variant(node)),+ }
            }

            fn as_node(&self) -> Node { self.clone() }
            fn into_node(self) -> Node { self }
        }

        impl Display for Node {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                match self { $(Self::$Variant(node) => Display::fmt(node, f)),+ }
            }
        }

        impl Deref for NodeRef<'_> {
            type Target = dyn NodeInterface;
            fn deref(&self) -> &Self::Target {
                match self { $(Self::$Variant(node) => *node),+ }
            }
        }

        impl Deref for NodeRefMut<'_> {
            type Target = dyn NodeInterface;
            fn deref(&self) -> &Self::Target {
                match self { $(Self::$Variant(node) => *node),+ }
            }
        }

        impl DerefMut for NodeRefMut<'_> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                match self { $(Self::$Variant(node) => *node),+ }
            }
        }
    };
}

define_node_enums! {
    Element(ElementNode),
    Attribute(AttrNode),
    Text(TextNode),
    CdataSection(CdataSectionNode),
    ProcessingInstruction(ProcessingInstructionNode),
    Comment(CommentNode),
    Document(DocumentNode),
    DocumentType(DocumentTypeNode),
    DocumentFragment(DocumentFragmentNode),
    Notation(NotationNode), // legacy
}

/// A borrowed view of a node that can contain children.
///
/// Constructing or copying this view does not clone a native handle. Operations use `ParentNode`
/// and the shared document lock.
///
/// ```
/// use resiliparse::parse::html::{dom::traits::NodeInterface, tree::HTMLTree};
///
/// let tree = HTMLTree::parse("<a href='/page'>link</a>").unwrap();
/// let node = tree.body().unwrap().into_node();
/// if let Some(parent) = node.as_parent() {
///     assert_eq!(parent.query_selector_all("a[href]").unwrap().len(), 1);
/// }
/// ```
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ParentNodeRef<'a> {
    /// An element, which can contain child nodes.
    Element(&'a ElementNode),
    /// A document node, which can contain child nodes.
    Document(&'a DocumentNode),
    /// A document fragment, which can contain child nodes.
    DocumentFragment(&'a DocumentFragmentNode),
}

impl<'a> Deref for ParentNodeRef<'a> {
    type Target = dyn ParentNode + 'a;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Element(node) => *node,
            Self::Document(node) => *node,
            Self::DocumentFragment(node) => *node,
        }
    }
}

/// A borrowed view of character data.
///
/// Constructing or copying this view does not clone a native handle.
/// Operations use `CharacterData`'s scalar indices and the shared document lock.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CharacterDataRef<'a> {
    /// A text node.
    Text(&'a TextNode),
    /// A CDATA section node.
    CdataSection(&'a CdataSectionNode),
    /// A comment node.
    Comment(&'a CommentNode),
    /// A processing instruction node.
    ProcessingInstruction(&'a ProcessingInstructionNode),
}

impl<'a> Deref for CharacterDataRef<'a> {
    type Target = dyn CharacterData + 'a;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Text(node) => *node,
            Self::CdataSection(node) => *node,
            Self::Comment(node) => *node,
            Self::ProcessingInstruction(node) => *node,
        }
    }
}

macro_rules! impl_node_views {
    ($($Node:ty),+ $(,)?) => {$(
        impl $Node {
            /// Borrow this node as an element, if it is one.
            ///
            /// # Returns
            ///
            /// A borrowed element node, or `None` if this node is not an element.
            pub fn as_element(&self) -> Option<&ElementNode> {
                match self { Self::Element(node) => Some(node), _ => None }
            }

            /// Borrow this node as text. CDATA has its own concrete node type.
            ///
            /// # Returns
            ///
            /// A borrowed text node, or `None` if this node is not a text node.
            pub fn as_text(&self) -> Option<&TextNode> {
                match self { Self::Text(node) => Some(node), _ => None }
            }

            /// Borrow this node as a parent node without cloning its handle.
            ///
            /// # Returns
            ///
            /// A borrowed parent-node view, or `None` if this node cannot contain children.
            pub fn as_parent(&self) -> Option<ParentNodeRef<'_>> {
                match self {
                    Self::Element(node) => Some(ParentNodeRef::Element(node)),
                    Self::Document(node) => Some(ParentNodeRef::Document(node)),
                    Self::DocumentFragment(node) => Some(ParentNodeRef::DocumentFragment(node)),
                    _ => None,
                }
            }

            /// Borrow this node as character data without cloning its handle.
            ///
            /// # Returns
            ///
            /// A borrowed character-data view, or `None` if this node has no character data.
            pub fn as_character_data(&self) -> Option<CharacterDataRef<'_>> {
                match self {
                    Self::Text(node) => Some(CharacterDataRef::Text(node)),
                    Self::CdataSection(node) => Some(CharacterDataRef::CdataSection(node)),
                    Self::Comment(node) => Some(CharacterDataRef::Comment(node)),
                    Self::ProcessingInstruction(node) => Some(CharacterDataRef::ProcessingInstruction(node)),
                    _ => None,
                }
            }
        }
    )+};
}

impl_node_views!(Node, NodeRef<'_>, NodeRefMut<'_>);

impl Display for NodeRef<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self.deref(), f)
    }
}

impl Display for NodeRefMut<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self.deref(), f)
    }
}

impl IntoIterator for Node {
    type Item = Result<Node, TraversalError>;
    type IntoIter = NodeIteratorOwned;

    fn into_iter(self) -> Self::IntoIter {
        NodeIteratorOwned::new(self)
    }
}

// --------------------------------------- Node Type Macros ----------------------------------------

macro_rules! check_node {
    ($node: expr) => {
        // Hold the document lock for the caller's entire operation, before
        // accessing native memory or changing a handle's validity.
        let document = $node.tree_();
        let _document_guard = document.doc_ptr();
        if $node.node_ptr_().is_null() {
            return Default::default();
        }
    };
}
pub(crate) use check_node;

macro_rules! check_nodes {
    ($node1: expr, $node2: expr) => {
        // Identity is valid for some operations (e.g. contains and replacement).
        // Only validate handles and document ownership here.
        if !Arc::ptr_eq(&$node1.tree_(), &$node2.tree_()) {
            return Default::default();
        }
        let document = $node1.tree_();
        let _document_guard = document.doc_ptr();
        if ($node1.node_ptr_()).is_null() || ($node2.node_ptr_()).is_null() {
            return Default::default();
        }
    };
}
pub(crate) use check_nodes;

/// A shared native-node handle. The document owns native storage; handles keep
/// that owner alive and retain native references. Identity is the document and
/// native node pointer, independent of Rust wrapper addresses or tree position.
/// Decomposition invalidates this handle; clones remain separately retained.
/// Only live handles have a comparable native identity.
struct NodeHandle {
    document: Arc<HTMLDocument>,
    node: Cell<*mut lxb_dom_node_t>,
}

// SAFETY: all native access, including reference counts and destruction, uses
// the document lock. Cell keeps individual handles !Sync; clones can be sent.
unsafe impl Send for NodeHandle {}

impl NodeHandle {
    fn new(document: &Arc<HTMLDocument>, node: *mut lxb_dom_node_t, kind: NodeType) -> Self {
        let _guard = document.doc_ptr();
        if !node.is_null() {
            unsafe {
                assert_eq!((*node).type_, kind as u32, "Incorrect concrete DOM node kind");
                (*node).ref_count += 1;
            }
        }
        Self {
            document: document.clone(),
            node: Cell::new(node),
        }
    }
}

impl Clone for NodeHandle {
    fn clone(&self) -> Self {
        let _guard = self.document.doc_ptr();
        let node = self.node.get();
        if !node.is_null() {
            unsafe {
                (*node).ref_count += 1;
            }
        }
        Self {
            document: self.document.clone(),
            node: Cell::new(node),
        }
    }
}

impl Drop for NodeHandle {
    fn drop(&mut self) {
        let _guard = self.document.doc_ptr();
        let node = self.node.get();
        if node.is_null() {
            return;
        }
        unsafe {
            if (*node).parent.is_null() && (*node).ref_count < 2 {
                self.document.mark_mutated();
                lxb_dom_node_destroy_deep(node);
            } else {
                (*node).ref_count -= 1;
            }
        }
    }
}

impl PartialEq for NodeHandle {
    fn eq(&self, other: &Self) -> bool {
        if !Arc::ptr_eq(&self.document, &other.document) {
            return false;
        }
        let _guard = self.document.doc_ptr();
        let node = self.node.get();
        !node.is_null() && node == other.node.get()
    }
}

macro_rules! define_node_type {
    ($Self: ident, $EnumType: ident) => {
        #[derive(Clone, PartialEq)]
        #[doc = concat!("A handle to a DOM ", stringify!($Self), ".")]
        pub struct $Self {
            handle: NodeHandle,
        }

        impl $Self {
            pub(crate) fn new(tree: &Arc<HTMLDocument>, node: *mut lxb_dom_node_t) -> Self {
                Self {
                    handle: NodeHandle::new(tree, node, NodeType::$EnumType),
                }
            }
        }

        impl NodeInterfaceBaseImpl for $Self {
            #[inline(always)]
            fn tree_(&self) -> Arc<HTMLDocument> {
                self.handle.document.clone()
            }

            #[inline(always)]
            fn node_ptr_(&self) -> *mut lxb_dom_node_t {
                self.handle.node.get()
            }

            #[inline(always)]
            fn reset_node_ptr_(&self) {
                self.handle.node.set(ptr::null_mut());
            }
        }

        impl NodeInterface for $Self {
            #[inline(always)]
            fn as_noderef(&self) -> NodeRef<'_> {
                NodeRef::$EnumType(self)
            }

            #[inline(always)]
            fn as_noderef_mut(&mut self) -> NodeRefMut<'_> {
                NodeRefMut::$EnumType(self)
            }

            #[inline(always)]
            fn as_node(&self) -> Node {
                Node::$EnumType(self.clone())
            }

            #[inline(always)]
            fn into_node(self) -> Node {
                Node::$EnumType(self)
            }
        }

        impl PartialEq<Node> for $Self {
            fn eq(&self, other: &Node) -> bool {
                self.as_noderef() == other.as_noderef()
            }
        }

        impl PartialEq<$Self> for Node {
            fn eq(&self, other: &$Self) -> bool {
                self.as_noderef() == other.as_noderef()
            }
        }

        impl PartialEq<NodeRef<'_>> for $Self {
            fn eq(&self, other: &NodeRef) -> bool {
                &self.as_noderef() == other
            }
        }

        impl PartialEq<$Self> for NodeRef<'_> {
            fn eq(&self, other: &$Self) -> bool {
                self == &other.as_noderef()
            }
        }

        impl PartialEq<NodeRefMut<'_>> for $Self {
            fn eq(&self, other: &NodeRefMut) -> bool {
                self.as_noderef() == other.as_noderef()
            }
        }

        impl PartialEq<$Self> for NodeRefMut<'_> {
            fn eq(&self, other: &$Self) -> bool {
                self.as_noderef() == other.as_noderef()
            }
        }

        impl TryFrom<Node> for $Self {
            type Error = Node;

            fn try_from(value: Node) -> Result<Self, Self::Error> {
                match value {
                    Node::$EnumType(node) => Ok(node),
                    node => Err(node),
                }
            }
        }

        impl From<$Self> for Node {
            fn from(value: $Self) -> Node {
                value.into_node()
            }
        }

        impl IntoIterator for $Self {
            type Item = Result<Node, TraversalError>;
            type IntoIter = NodeIteratorOwned;

            fn into_iter(self) -> Self::IntoIter {
                NodeIteratorOwned::new(self.into_node())
            }
        }

        impl Debug for $Self {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                let _guard = self.handle.document.doc_ptr();
                if self.node_ptr_().is_null() {
                    return f.write_str("<#undef>");
                }

                let mut repr = self.node_name().unwrap();
                let node = self.as_noderef();
                if let NodeRef::Element(element) = node {
                    repr = format!("<{}", repr.to_lowercase());
                    element.attributes().iter().for_each(|attr| {
                        repr.push(' ');
                        repr.push_str(&attr.name().unwrap());
                        if attr.value().is_some() {
                            repr.push_str(&format!("={:?}", attr.value().unwrap()));
                        }
                    });
                    repr.push('>');
                } else if let NodeRef::Attribute(attr) = node {
                    repr = format!("[{}={:?}]", repr, attr.node_value());
                } else if let NodeRef::Text(text) = node {
                    repr = format!("{:?}", text.node_value().unwrap_or_else(String::new));
                } else if let NodeRef::Comment(comment) = node {
                    repr = format!("<-- {} -->", comment.node_value().unwrap_or_else(String::new));
                }
                f.write_str(&repr)
            }
        }

        impl Display for $Self {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                let _guard = self.handle.document.doc_ptr();
                f.write_str(&node_serialize_html(self.node_ptr_()))
            }
        }
    };
}

// --------------------------------------- DocumentType impl ---------------------------------------

define_node_type!(DocumentTypeNode, DocumentType);

//noinspection DuplicatedCode
// Private native borrows; callers hold the document lock until strings are copied.
impl DocumentTypeNode {
    unsafe fn name_unchecked(&self) -> Option<&str> {
        unsafe { str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_document_type_name_noi) }
    }

    unsafe fn public_id_unchecked(&self) -> Option<&str> {
        unsafe { str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_document_type_public_id_noi) }
    }

    unsafe fn system_id_unchecked(&self) -> Option<&str> {
        unsafe { str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_document_type_system_id_noi) }
    }
}

impl DocumentType for DocumentTypeNode {
    #[inline]
    fn name(&self) -> Option<String> {
        check_node!(self);
        unsafe { Some(self.name_unchecked()?.to_owned()) }
    }

    #[inline]
    fn public_id(&self) -> Option<String> {
        check_node!(self);
        unsafe { Some(self.public_id_unchecked()?.to_owned()) }
    }

    #[inline]
    fn system_id(&self) -> Option<String> {
        check_node!(self);
        unsafe { Some(self.system_id_unchecked()?.to_owned()) }
    }
}

impl ChildNode for DocumentTypeNode {}

// ----------------------------------------- Document impl -----------------------------------------

define_node_type!(DocumentNode, Document);

impl Document for DocumentNode {
    fn doctype(&self) -> Option<DocumentTypeNode> {
        check_node!(self);
        unsafe {
            let doctype = self.doc_ptr_unchecked().as_ref()?.doctype;
            Some(DocumentTypeNode::new(&self.tree_(), doctype.cast()))
        }
    }

    fn document_element(&self) -> Option<ElementNode> {
        self.owner_document()?.first_element_child()
    }

    fn get_elements_by_tag_name(&self, name: &str) -> HTMLCollection {
        HTMLCollection::new_live(&self.as_noderef(), Some(Box::new([name.to_owned()])), |n, qn| unsafe {
            get_elements_by_tag_name(n, &qn.unwrap_unchecked()[0])
        })
    }

    fn get_elements_by_class_name(&self, class_names: &str) -> HTMLCollection {
        HTMLCollection::new_live(&self.as_noderef(), Some(Box::new([class_names.to_owned()])), |n, cls| unsafe {
            get_elements_by_class_name(n, &cls.unwrap_unchecked()[0])
        })
    }

    #[inline(always)]
    fn get_elements_by_attr(&self, name: &str, value: &str) -> HTMLCollection {
        self.get_elements_by_attr_case(name, value, false)
    }

    fn get_elements_by_attr_case(&self, name: &str, value: &str, case_insensitive: bool) -> HTMLCollection {
        let user_data = Box::new([name.to_owned(), value.to_owned(), case_insensitive.to_string()]);
        HTMLCollection::new_live(&self.as_noderef(), Some(user_data), |n, attr| unsafe {
            get_elements_by_attr(
                &n.as_noderef(),
                &attr.unwrap_unchecked()[0],
                &attr.unwrap_unchecked()[1],
                &attr.unwrap_unchecked()[2] == "true",
            )
        })
    }

    fn create_element(&self, local_name: &str) -> Result<ElementNode, DOMError> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        if !self.node_ptr_().is_null() {
            unsafe { create_element_unchecked(&self, local_name) }
        } else {
            Err(DOMError {
                msg: "Invalid document.".to_owned(),
            })
        }
    }

    fn create_document_fragment(&self) -> Result<DocumentFragmentNode, DOMError> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        if !self.node_ptr_().is_null() {
            unsafe { create_document_fragment_unchecked(&self) }
        } else {
            Err(DOMError {
                msg: "Invalid document.".to_owned(),
            })
        }
    }

    fn create_text_node(&self, data: &str) -> Result<TextNode, DOMError> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        if !self.node_ptr_().is_null() {
            unsafe { create_text_node_unchecked(&self, data) }
        } else {
            Err(DOMError {
                msg: "Invalid document.".to_owned(),
            })
        }
    }

    fn create_cdata_section(&self, data: &str) -> Result<CdataSectionNode, DOMError> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        if !self.node_ptr_().is_null() {
            unsafe { create_cdata_section_unchecked(&self, data) }
        } else {
            Err(DOMError {
                msg: "Invalid document.".to_owned(),
            })
        }
    }

    fn create_comment(&self, data: &str) -> Result<CommentNode, DOMError> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        if !self.node_ptr_().is_null() {
            unsafe { create_comment_unchecked(&self, data) }
        } else {
            Err(DOMError {
                msg: "Invalid document.".to_owned(),
            })
        }
    }

    fn create_processing_instruction(&self, target: &str, data: &str) -> Result<ProcessingInstructionNode, DOMError> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        if !self.node_ptr_().is_null() {
            unsafe { create_processing_instruction_unchecked(&self, target, data) }
        } else {
            Err(DOMError {
                msg: "Invalid document.".to_owned(),
            })
        }
    }

    fn create_attribute(&self, local_name: &str) -> Result<AttrNode, DOMError> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        if !self.node_ptr_().is_null() {
            unsafe { create_attribute_unchecked(&self, local_name) }
        } else {
            Err(DOMError {
                msg: "Invalid document.".to_owned(),
            })
        }
    }
}

impl DocumentOrShadowRoot for DocumentNode {}

impl ParentNode for DocumentNode {
    fn children(&self) -> HTMLCollection {
        HTMLCollection::new_live(&self.as_noderef(), None, |n, _| {
            let mut nodes: Vec<ElementNode> = Vec::new();
            if let Node::Document(d) = n {
                let mut child = d.first_element_child();
                while let Some(c) = child {
                    child = c.next_element_sibling();
                    nodes.push(c);
                }
            }
            nodes
        })
    }
}

impl NonElementParentNode for DocumentNode {}

// ------------------------------------- DocumentFragment impl -------------------------------------

define_node_type!(DocumentFragmentNode, DocumentFragment);

impl DocumentFragment for DocumentFragmentNode {}

impl DocumentOrShadowRoot for DocumentFragmentNode {}

impl ParentNode for DocumentFragmentNode {
    fn children(&self) -> HTMLCollection {
        HTMLCollection::new_live(&self.as_noderef(), None, |n, _| {
            let mut nodes: Vec<ElementNode> = Vec::new();
            if let Node::DocumentFragment(d) = n {
                let mut child = d.first_element_child();
                while let Some(c) = child {
                    child = c.next_element_sibling();
                    nodes.push(c.into());
                }
            }
            nodes
        })
    }
}

impl NonElementParentNode for DocumentFragmentNode {}

// ------------------------------------------ Element impl -----------------------------------------

define_node_type!(ElementNode, Element);

// Private native borrows; callers hold the document lock until strings are copied.
impl ElementNode {
    /// DOM element tag or node name.
    unsafe fn tag_name_unchecked(&self) -> Option<&str> {
        unsafe { str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_element_tag_name) }
    }

    unsafe fn local_name_unchecked(&self) -> Option<&str> {
        unsafe { str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_element_local_name) }
    }

    unsafe fn id_unchecked(&self) -> Option<&str> {
        unsafe { str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_element_id_noi) }
    }

    unsafe fn class_name_unchecked(&self) -> Option<&str> {
        unsafe { str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_element_class_noi) }
    }

    unsafe fn attribute_unchecked(&self, qualified_name: &str) -> Option<&str> {
        unsafe {
            let mut size = 0;
            let name = lxb_dom_element_get_attribute(
                self.node_ptr_().cast(),
                qualified_name.as_ptr().cast(),
                qualified_name.len(),
                addr_of_mut!(size),
            );
            if name.is_null() {
                None
            } else {
                str_from_lxb_char_t(name, size)
            }
        }
    }

    unsafe fn attribute_names_unchecked(&self) -> Vec<&str> {
        unsafe {
            let mut attr = lxb_dom_element_first_attribute_noi(self.node_ptr_().cast());
            let mut name_vec = Vec::new();
            while !attr.is_null() {
                if let Some(qname) = str_from_lxb_str_cb(attr, lxb_dom_attr_qualified_name) {
                    name_vec.push(qname);
                }
                attr = lxb_dom_element_next_attribute_noi(attr);
            }
            name_vec
        }
    }
}

impl Element for ElementNode {
    /// DOM element tag or node name.
    #[inline]
    fn tag_name(&self) -> Option<String> {
        check_node!(self);
        unsafe { Some(self.tag_name_unchecked()?.to_owned()) }
    }

    #[inline]
    fn local_name(&self) -> Option<String> {
        check_node!(self);
        unsafe { Some(self.local_name_unchecked()?.to_owned()) }
    }

    #[inline]
    fn id(&self) -> Option<String> {
        check_node!(self);
        unsafe { Some(self.id_unchecked()?.to_owned()) }
    }

    #[inline]
    fn set_id(&self, id: &str) {
        self.set_attribute("id", id);
    }

    #[inline]
    fn class_name(&self) -> Option<String> {
        check_node!(self);
        unsafe { Some(self.class_name_unchecked()?.to_owned()) }
    }

    #[inline]
    fn set_class_name(&self, class_name: &str) {
        self.set_attribute("class", class_name);
    }

    fn class_list(&self) -> DOMTokenList<'_> {
        DOMTokenList::new(self)
    }

    fn attribute(&self, qualified_name: &str) -> Option<String> {
        check_node!(self);
        unsafe { Some(self.attribute_unchecked(qualified_name)?.to_owned()) }
    }

    #[inline]
    fn attribute_or(&self, qualified_name: &str, default: &str) -> String {
        self.attribute(qualified_name).unwrap_or(default.to_owned())
    }

    #[inline]
    fn attribute_or_default(&self, qualified_name: &str) -> String {
        self.attribute(qualified_name).unwrap_or_default()
    }

    fn attribute_node(&self, qualified_name: &str) -> Option<AttrNode> {
        check_node!(self);
        let attr = unsafe {
            lxb_dom_element_attr_by_name(self.node_ptr_().cast(), qualified_name.as_ptr(), qualified_name.len())
        };
        if attr.is_null() {
            return None;
        }
        Some(AttrNode::new(&self.tree_(), attr.cast()))
    }

    fn attribute_names(&self) -> Vec<String> {
        check_node!(self);
        unsafe {
            self.attribute_names_unchecked()
                .into_iter()
                .map(|s| s.to_owned())
                .collect()
        }
    }

    fn attributes(&self) -> NamedNodeMap {
        NamedNodeMap::new(
            self.clone(),
            LiveCollection::new_live(&self.as_noderef(), None, |n, _| {
                let document = n.tree_();
                let _guard = document.doc_ptr();
                let mut v = Vec::new();
                unsafe {
                    let mut attr = lxb_dom_element_first_attribute_noi(n.node_ptr_().cast());
                    while !attr.is_null() {
                        v.push(AttrNode::new(&n.tree_(), attr.cast()));
                        attr = lxb_dom_element_next_attribute_noi(attr);
                    }
                };
                v
            }),
        )
    }

    fn set_attribute(&self, qualified_name: &str, value: &str) {
        check_node!(self);
        unsafe {
            self.tree_().mark_mutated();
            lxb_dom_element_set_attribute(
                self.node_ptr_().cast(),
                qualified_name.as_ptr(),
                qualified_name.len(),
                value.as_ptr(),
                value.len(),
            );
        }
    }

    fn set_attribute_node(&self, attribute: &AttrNode) {
        check_nodes!(self, attribute);
        unsafe {
            self.tree_().mark_mutated();
            lxb_dom_element_attr_append(self.node_ptr_().cast(), attribute.node_ptr_().cast());
        }
    }

    fn remove_attribute(&self, qualified_name: &str) {
        check_node!(self);
        unsafe {
            self.tree_().mark_mutated();
            lxb_dom_element_remove_attribute(self.node_ptr_().cast(), qualified_name.as_ptr(), qualified_name.len());
        }
    }

    fn toggle_attribute(&self, qualified_name: &str, force: Option<bool>) -> bool {
        let on = match force {
            Some(f) => f,
            None => !self.has_attribute(qualified_name),
        };
        if on {
            self.set_attribute(qualified_name, &self.attribute(qualified_name).unwrap_or_default());
            true
        } else {
            self.remove_attribute(qualified_name);
            false
        }
    }

    fn has_attribute(&self, qualified_name: &str) -> bool {
        check_node!(self);
        unsafe { lxb_dom_element_has_attribute(self.node_ptr_().cast(), qualified_name.as_ptr(), qualified_name.len()) }
    }

    fn closest(&self, selectors: &str) -> Result<Option<ElementNode>, CSSParserError> {
        Selector::new(selectors)?.closest(self)
    }

    fn matches(&self, selectors: &str) -> Result<bool, CSSParserError> {
        Selector::new(selectors)?.matches(self)
    }

    fn get_elements_by_tag_name(&self, name: &str) -> HTMLCollection {
        HTMLCollection::new_live(&self.as_noderef(), Some(Box::new([name.to_owned()])), |n, qn| unsafe {
            get_elements_by_tag_name(&n, &qn.unwrap_unchecked()[0])
        })
    }

    fn get_elements_by_class_name(&self, class_names: &str) -> HTMLCollection {
        HTMLCollection::new_live(&self.as_noderef(), Some(Box::new([class_names.to_owned()])), |n, cls| unsafe {
            get_elements_by_class_name(&n, &cls.unwrap_unchecked()[0])
        })
    }

    fn get_elements_by_attr(&self, name: &str, value: &str) -> HTMLCollection {
        self.get_elements_by_attr_case(name, value, false)
    }

    fn get_elements_by_attr_case(&self, name: &str, value: &str, case_insensitive: bool) -> HTMLCollection {
        let user_data = Box::new([name.to_owned(), value.to_owned(), case_insensitive.to_string()]);
        HTMLCollection::new_live(&self.as_noderef(), Some(user_data), |n, attr| unsafe {
            get_elements_by_attr(
                &n.as_noderef(),
                &attr.unwrap_unchecked()[0],
                &attr.unwrap_unchecked()[1],
                &attr.unwrap_unchecked()[2] == "true",
            )
        })
    }

    /// Inner HTML of this DOM node's children.
    fn inner_html(&self) -> String {
        check_node!(self);
        unsafe {
            let html_str = lexbor_str_create();
            if html_str.is_null() {
                return String::default();
            }
            let mut next = lxb_dom_node_first_child_noi(self.node_ptr_());
            while !next.is_null() {
                lxb_html_serialize_tree_str(next, html_str);
                next = lxb_dom_node_next_noi(next);
            }
            let s = str_from_lxb_str_t(html_str).unwrap_or_default().to_owned();
            lexbor_str_destroy(html_str, self.doc_ptr_unchecked().as_ref().unwrap().text, true);
            s
        }
    }

    fn set_inner_html(&self, html: &str) {
        check_node!(self);
        unsafe {
            self.tree_().mark_mutated();
            lxb_html_element_inner_html_set(self.node_ptr_().cast(), html.as_ptr(), html.len());
        }
    }

    fn outer_html(&self) -> String {
        check_node!(self);
        node_serialize_html(self.node_ptr_())
    }

    fn set_outer_html(&self, html: &str) {
        check_node!(self);
        let Some(parent) = self.parent_node() else {
            return;
        };
        let context = match parent {
            Node::Element(element) => element,
            Node::DocumentFragment(_) => {
                let Some(document) = self.owner_document() else {
                    return;
                };
                let Ok(body) = document.create_element("body") else {
                    return;
                };
                body
            }
            // The document element cannot be replaced through outer HTML.
            _ => return,
        };
        unsafe {
            // Parse before modifying the existing tree, in the parent's context
            // (important for table rows and other context-sensitive elements).
            self.tree_().mark_mutated();
            let fragment = lxb_html_document_parse_fragment(
                self.doc_ptr_unchecked().cast(),
                context.node_ptr_().cast(),
                html.as_ptr(),
                html.len(),
            );
            if fragment.is_null() {
                return;
            }
            while !(*fragment).first_child.is_null() {
                let child = (*fragment).first_child;
                move_node_before_unchecked((*self.node_ptr_()).parent, child, self.node_ptr_());
            }
            lxb_dom_node_destroy(fragment);
        }
        self.remove()
    }

    fn inner_text(&self) -> String {
        check_node!(self);
        node_format_visible_text(self.node_ptr_())
    }

    #[inline]
    fn set_inner_text(&self, text: &str) {
        self.set_text_content(text);
    }

    #[inline]
    fn outer_text(&self) -> String {
        self.inner_text()
    }

    fn set_outer_text(&self, text: &str) {
        check_node!(self);
        if !matches!(self.parent_node(), Some(Node::Element(_) | Node::DocumentFragment(_))) {
            return;
        }
        self.set_text_content(text);
        unsafe {
            let fc = lxb_dom_node_first_child_noi(self.node_ptr_());
            if !fc.is_null() {
                move_node_before_unchecked((*self.node_ptr_()).parent, fc, self.node_ptr_());
            }
        }
        self.remove()
    }
}

impl ParentNode for ElementNode {
    fn children(&self) -> HTMLCollection {
        HTMLCollection::new_live(&self.as_noderef(), None, |n, _| {
            let mut nodes: Vec<ElementNode> = Vec::new();
            if let Node::Element(e) = n {
                let mut child = e.first_element_child();
                while let Some(c) = child {
                    child = c.next_element_sibling();
                    nodes.push(c);
                }
            }
            nodes
        })
    }
}

impl ChildNode for ElementNode {}

impl NonDocumentTypeChildNode for ElementNode {}

// ------------------------------------------- Attr impl -------------------------------------------

define_node_type!(AttrNode, Attribute);

// Private native borrows; callers hold the document lock until strings are copied.
impl AttrNode {
    unsafe fn name_unchecked(&self) -> Option<&str> {
        unsafe { str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_attr_qualified_name) }
    }

    unsafe fn local_name_unchecked(&self) -> Option<&str> {
        unsafe { str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_attr_local_name_noi) }
    }

    unsafe fn value_unchecked(&self) -> Option<&str> {
        unsafe { str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_attr_value_noi) }
    }
}

impl Attr for AttrNode {
    #[inline]
    fn local_name(&self) -> Option<String> {
        check_node!(self);
        unsafe { Some(self.local_name_unchecked()?.to_owned()) }
    }

    #[inline]
    fn name(&self) -> Option<String> {
        check_node!(self);
        unsafe { Some(self.name_unchecked()?.to_owned()) }
    }

    #[inline]
    fn value(&self) -> Option<String> {
        check_node!(self);
        unsafe { Some(self.value_unchecked()?.to_owned()) }
    }

    fn set_value(&self, value: &str) {
        check_node!(self);
        unsafe {
            self.tree_().mark_mutated();
            lxb_dom_node_text_content_set(self.node_ptr_(), value.as_ptr(), value.len());
        }
    }

    fn owner_element(&self) -> Option<ElementNode> {
        check_node!(self);
        unsafe {
            let attr = (self.node_ptr_()) as *mut lxb_dom_attr_t;
            if attr.is_null() || (*attr).owner.is_null() {
                return None;
            }
            Some(ElementNode::new(&self.tree_(), (*attr).owner.cast()))
        }
    }
}

// -------------------------------------------- Text impl ------------------------------------------

define_node_type!(TextNode, Text);

impl Text for TextNode {}

impl CharacterData for TextNode {}

impl ChildNode for TextNode {}

impl NonDocumentTypeChildNode for TextNode {}

// ---------------------------------------- CdataSection impl --------------------------------------

define_node_type!(CdataSectionNode, CdataSection);

impl CdataSection for CdataSectionNode {}

impl CharacterData for CdataSectionNode {}

impl ChildNode for CdataSectionNode {}

impl NonDocumentTypeChildNode for CdataSectionNode {}

// ----------------------------------- ProcessingInstruction impl ----------------------------------

define_node_type!(ProcessingInstructionNode, ProcessingInstruction);

impl ProcessingInstruction for ProcessingInstructionNode {
    fn target(&self) -> Option<String> {
        check_node!(self);
        unsafe { Some(str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_processing_instruction_target_noi)?.to_owned()) }
    }
}

impl CharacterData for ProcessingInstructionNode {}

impl ChildNode for ProcessingInstructionNode {}

impl NonDocumentTypeChildNode for ProcessingInstructionNode {}

// ------------------------------------------ Comment impl -----------------------------------------

define_node_type!(CommentNode, Comment);

impl Comment for CommentNode {}

impl CharacterData for CommentNode {}

impl ChildNode for CommentNode {}

impl NonDocumentTypeChildNode for CommentNode {}

// ------------------------------------------ Notation impl -----------------------------------------

define_node_type!(NotationNode, Notation);

//noinspection DuplicatedCode
// Private native borrows; callers hold the document lock until strings are copied.
impl NotationNode {
    unsafe fn public_id_unchecked(&self) -> Option<&str> {
        unsafe { str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_document_type_public_id_noi) }
    }

    unsafe fn system_id_unchecked(&self) -> Option<&str> {
        unsafe { str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_document_type_system_id_noi) }
    }
}

impl Notation for NotationNode {
    #[inline]
    fn public_id(&self) -> Option<String> {
        check_node!(self);
        unsafe { Some(self.public_id_unchecked()?.to_owned()) }
    }

    #[inline]
    fn system_id(&self) -> Option<String> {
        check_node!(self);
        unsafe { Some(self.system_id_unchecked()?.to_owned()) }
    }
}
