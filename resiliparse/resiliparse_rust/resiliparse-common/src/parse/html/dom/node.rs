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

use std::fmt::{Debug, Display, Formatter};
use std::ops::{Deref, DerefMut};
use parking_lot::ReentrantMutex;
use std::ptr;
use std::ptr::addr_of_mut;
use std::sync::Arc;
use std::sync::atomic::{AtomicPtr, Ordering};
use crate::parse::html::css::*;
use crate::parse::html::dom::*;
use crate::parse::html::dom::coll::*;
use crate::parse::html::dom::iter::*;
use crate::parse::html::dom::traits::*;
use crate::parse::html::lexbor::{str_from_lxb_char_t, str_from_lxb_str_cb, str_from_lxb_str_t};
use crate::parse::html::serialize::{node_format_visible_text, node_serialize_html};

// Re-export NodeType publicly
pub use crate::parse::html::dom::traits::NodeType;



// ----------------------------------------- Node Base Enum ----------------------------------------

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Node {
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

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum NodeRef<'a> {
    Element(&'a ElementNode),
    Attribute(&'a AttrNode),
    Text(&'a TextNode),
    CdataSection(&'a CdataSectionNode),
    ProcessingInstruction(&'a ProcessingInstructionNode),
    Comment(&'a CommentNode),
    Document(&'a DocumentNode),
    DocumentType(&'a DocumentTypeNode),
    DocumentFragment(&'a DocumentFragmentNode),
    Notation(&'a NotationNode), // legacy
}

#[derive(PartialEq, Eq, Debug)]
pub enum NodeRefMut<'a> {
    Element(&'a mut ElementNode),
    Attribute(&'a mut AttrNode),
    Text(&'a mut TextNode),
    CdataSection(&'a mut CdataSectionNode),
    ProcessingInstruction(&'a mut ProcessingInstructionNode),
    Comment(&'a mut CommentNode),
    Document(&'a mut DocumentNode),
    DocumentType(&'a mut DocumentTypeNode),
    DocumentFragment(&'a mut DocumentFragmentNode),
    Notation(&'a mut NotationNode), // legacy
}


macro_rules! unwrap_node {
    ($node: ident) => {
        match $node {
            Node::Element(n) => n,
            Node::Attribute(n) => n,
            Node::Text(n) => n,
            Node::CdataSection(n) => n,
            Node::ProcessingInstruction(n) => n,
            Node::Comment(n) => n,
            Node::Document(n) => n,
            Node::DocumentType(n) => n,
            Node::DocumentFragment(n) => n,
            Node::Notation(n) => n, // legacy
        }
    };
}

macro_rules! unwrap_noderef {
    ($Enum: ident, $node: ident) => {
        match $node {
            $Enum::Element(n) => *n,
            $Enum::Attribute(n) => *n,
            $Enum::Text(n) => *n,
            $Enum::CdataSection(n) => *n,
            $Enum::ProcessingInstruction(n) => *n,
            $Enum::Comment(n) => *n,
            $Enum::Document(n) => *n,
            $Enum::DocumentType(n) => *n,
            $Enum::DocumentFragment(n) => *n,
            $Enum::Notation(n) => *n, // legacy
        }
    };
}

impl Deref for Node {
    type Target = dyn NodeInterface;

    fn deref(&self) -> &Self::Target {
        unwrap_node!(self)
    }
}

impl DerefMut for Node {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unwrap_node!(self)
    }
}

impl<'a> Deref for NodeRef<'a> {
    type Target = dyn NodeInterface;

    fn deref(&self) -> &Self::Target {
        unwrap_noderef!(NodeRef, self)
    }
}

impl<'a> Deref for NodeRefMut<'a> {
    type Target = dyn NodeInterface;

    fn deref(&self) -> &Self::Target {
        unwrap_noderef!(NodeRefMut, self)
    }
}

impl<'a> DerefMut for NodeRefMut<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unwrap_noderef!(NodeRefMut, self)
    }
}

impl Display for Node {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self.deref(), f)
    }
}

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
    type Item = Node;
    type IntoIter = NodeIteratorOwned;

    fn into_iter(self) -> Self::IntoIter {
        NodeIteratorOwned::new(self)
    }
}


// --------------------------------------- Node Type Macros ----------------------------------------

macro_rules! check_node {
    ($node: expr) => {
        if $node.node_ptr_().is_null() {
            return Default::default();
        }
    }
}
pub(crate) use check_node;

macro_rules! check_nodes {
    ($node1: expr, $node2: expr) => {
        {
            if !Arc::ptr_eq(&$node1.tree_(), &$node2.tree_()) ||
               $node1.node_ptr_().is_null() || $node2.node_ptr_().is_null() || $node1.node_ptr_() == $node2.node_ptr_() {
                return Default::default();
            }
        }
    }
}
pub(crate) use check_nodes;

macro_rules! define_node_type {
    ($Self: ident, $EnumType: ident) => {
        pub struct $Self {
            pub(crate) tree: Arc<HTMLDocument>,
            pub(crate) node: ReentrantMutex<AtomicPtr<lxb_dom_node_t>>,
        }

        unsafe impl Send for $Self {}
        unsafe impl Sync for $Self {}

        impl NodeInterfaceBaseImpl for $Self {
            fn new(tree: &Arc<HTMLDocument>, node: *mut lxb_dom_node_t) -> Option<Self> {
                if !node.is_null() {
                    Some(Self { tree: tree.clone(), node: ReentrantMutex::new(AtomicPtr::new(node)) })
                } else {
                    None
                }
            }

            #[inline(always)]
            fn tree_(&self) -> Arc<HTMLDocument> {
                self.tree.clone()
            }

            #[inline(always)]
            fn node_ptr_(&self) -> *mut lxb_dom_node_t {
                self.node.lock().load(Ordering::Relaxed)
            }

            #[inline(always)]
            fn reset_node_ptr_(&mut self) {
                *self.node.get_mut().get_mut() = ptr::null_mut();
            }
        }

        impl NodeInterface for $Self {
            #[inline(always)]
            fn as_noderef(&self) -> NodeRef {
                NodeRef::$EnumType(self)
            }

            #[inline(always)]
            fn as_noderef_mut(&mut self) -> NodeRefMut {
                NodeRefMut::$EnumType(self)
            }

            #[inline(always)]
            fn as_node(&self) -> Node {
                self.clone().into()
            }

            #[inline(always)]
            fn into_node(self) -> Node {
                self.into()
            }
        }

        impl Clone for $Self {
            fn clone(&self) -> Self {
                $Self::new(&self.tree_(), self.node_ptr_()).unwrap()
            }
        }

        impl Eq for $Self {}

        impl PartialEq<$Self> for $Self {
            fn eq(&self, other: &Self) -> bool {
                self.node_ptr_() == other.node_ptr_()
            }
        }

        impl From<Node> for $Self {
            fn from(value: Node) -> $Self {
                match value {
                    Node::$EnumType(d) => d,
                    _ => panic!("Illegal DOM Node type coercion.")
                }
            }
        }

        impl From<$Self> for Node {
            fn from(value: $Self) -> Node {
                Node::$EnumType(value)
            }
        }

        impl<'a> From<&'a $Self> for NodeRef<'a> {
            fn from(value: &'a $Self) -> NodeRef<'a> {
                NodeRef::$EnumType(value)
            }
        }

        impl<'a> From<&'a mut $Self> for NodeRefMut<'a> {
            fn from(value: &'a mut $Self) -> NodeRefMut<'a> {
                NodeRefMut::$EnumType(value)
            }
        }

        impl IntoIterator for $Self {
            type Item = Node;
            type IntoIter = NodeIteratorOwned;

            fn into_iter(self) -> Self::IntoIter {
                NodeIteratorOwned::new(self.into_node())
            }
        }

        impl Debug for $Self {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                let mut repr = self.node_name().unwrap_or_else(|| "#undef".to_owned());
                let node: Node = self.as_node();
                if let Node::Element(element) = node {
                    repr = format!("<{}", repr.to_lowercase());
                    element.attributes().iter().for_each(|attr| {
                        repr.push(' ');
                        repr.push_str(&attr.name().unwrap());
                        if attr.value().is_some() {
                            repr.push_str(&format!("={:?}", attr.value().unwrap()));
                        }
                    });
                    repr.push('>');
                } else if let Node::Attribute(attr) = node {
                    repr = format!("[{}={:?}]", repr, attr.node_value());
                } else if let Node::Text(text) = node {
                    repr = format!("{:?}", text.node_value().unwrap_or_else(String::new));
                } else if let Node::Comment(comment) = node {
                    repr = format!("<-- {} -->", comment.node_value().unwrap_or_else(String::new));
                }
                f.write_str(&repr)
            }
        }

        impl Display for $Self {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                f.write_str(&node_serialize_html(self.node_ptr_()))
            }
        }
    }
}


// --------------------------------------- DocumentType impl ---------------------------------------


define_node_type!(DocumentTypeNode, DocumentType);

//noinspection DuplicatedCode
impl DocumentType for DocumentTypeNode {
    unsafe fn name_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_document_type_name_noi)
    }

    unsafe fn public_id_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_document_type_public_id_noi)
    }

    unsafe fn system_id_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_document_type_system_id_noi)
    }

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
            Some(DocumentTypeNode::new(&self.tree_(),doctype.cast())?)
        }
    }

    fn document_element(&self) -> Option<ElementNode> {
        self.owner_document()?.first_element_child()
    }

    fn get_elements_by_tag_name(&self, name: &str) -> HTMLCollection {
        check_node!(self);
        HTMLCollection::new_live(&self.as_noderef(), Some(Box::new([name.to_owned()])), |n, qn| {
            unsafe { get_elements_by_tag_name(n, &qn.unwrap_unchecked()[0]) }
        })
    }

    fn get_elements_by_class_name(&self, class_names: &str) -> HTMLCollection {
        check_node!(self);
        HTMLCollection::new_live(&self.as_noderef(), Some(Box::new([class_names.to_owned()])), |n, cls| {
            unsafe { get_elements_by_class_name(n, &cls.unwrap_unchecked()[0]) }
        })
    }

    #[inline(always)]
    fn get_elements_by_attr(&self, name: &str, value: &str) -> HTMLCollection {
        self.get_elements_by_attr_case(name, value, false)
    }

    fn get_elements_by_attr_case(&self, name: &str, value: &str, case_insensitive: bool) -> HTMLCollection {
        check_node!(self);
        let user_data = Box::new([
            name.to_owned(),
            value.to_owned(),
            case_insensitive.to_string()]);
        HTMLCollection::new_live(&self.as_noderef(), Some(user_data), |n, attr| {
            unsafe {
                get_elements_by_attr(
                    &n.as_noderef(),
                    &attr.unwrap_unchecked()[0],
                    &attr.unwrap_unchecked()[1],
                    &attr.unwrap_unchecked()[2] == "true")
            }
        })
    }

    fn create_element(&mut self, local_name: &str) -> Result<ElementNode, DOMError> {
        if !self.node_ptr_().is_null() {
            unsafe { create_element_unchecked(&self, local_name) }
        } else {
            Err(DOMError { msg: "Invalid document.".to_owned()} )
        }
    }

    fn create_document_fragment(&mut self) -> Result<DocumentFragmentNode, DOMError> {
        if !self.node_ptr_().is_null() {
            unsafe { create_document_fragment_unchecked(&self) }
        } else {
            Err(DOMError { msg: "Invalid document.".to_owned()} )
        }
    }

    fn create_text_node(&mut self, data: &str) -> Result<TextNode, DOMError> {
        if !self.node_ptr_().is_null() {
            unsafe { create_text_node_unchecked(&self, data) }
        } else {
            Err(DOMError { msg: "Invalid document.".to_owned()} )
        }
    }

    fn create_cdata_section(&mut self, data: &str) -> Result<CdataSectionNode, DOMError> {
        if !self.node_ptr_().is_null() {
            unsafe { create_cdata_section_unchecked(&self, data) }
        } else {
            Err(DOMError { msg: "Invalid document.".to_owned()} )
        }
    }

    fn create_comment(&mut self, data: &str) -> Result<CommentNode, DOMError> {
        if !self.node_ptr_().is_null() {
            unsafe { create_comment_unchecked(&self, data) }
        } else {
            Err(DOMError { msg: "Invalid document.".to_owned()} )
        }
    }

    fn create_processing_instruction(&mut self, target: &str, data: &str) -> Result<ProcessingInstructionNode, DOMError> {
        if !self.node_ptr_().is_null() {
            unsafe { create_processing_instruction_unchecked(&self, target, data) }
        } else {
            Err(DOMError { msg: "Invalid document.".to_owned()} )
        }
    }

    fn create_attribute(&mut self, local_name: &str) -> Result<AttrNode, DOMError> {
        if !self.node_ptr_().is_null() {
            unsafe { create_attribute_unchecked(&self, local_name) }
        } else {
            Err(DOMError { msg: "Invalid document.".to_owned()} )
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

impl Element for ElementNode {
    /// DOM element tag or node name.
    unsafe fn tag_name_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_element_tag_name)
    }

    unsafe fn local_name_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_element_local_name)
    }

    unsafe fn id_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_element_id_noi)
    }

    #[inline]
    unsafe fn name_unchecked(&self) -> Option<&str> {
        self.attribute_unchecked("name")
    }

    unsafe fn class_name_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_element_class_noi)
    }

    unsafe fn attribute_unchecked(&self, qualified_name: &str) -> Option<&str> {
        let mut size = 0;
        let name = lxb_dom_element_get_attribute(
            self.node_ptr_().cast(),
            qualified_name.as_ptr().cast(),
            qualified_name.len(),
            addr_of_mut!(size));
        if name.is_null() {
            None
        } else {
            str_from_lxb_char_t(name, size)
        }
    }

    unsafe fn attribute_names_unchecked(&self) -> Vec<&str> {
        let mut attr =  lxb_dom_element_first_attribute_noi(self.node_ptr_().cast());
        let mut name_vec = Vec::new();
        while !attr.is_null() {
            if let Some(qname) = str_from_lxb_str_cb(attr, lxb_dom_attr_qualified_name) {
                name_vec.push(qname);
            }
            attr = lxb_dom_element_next_attribute_noi(attr);
        }
        name_vec
    }

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
    fn set_id(&mut self, id: &str) {
        self.set_attribute("id", id);
    }

    #[inline]
    fn class_name(&self) -> Option<String> {
        check_node!(self);
        unsafe { Some(self.class_name_unchecked()?.to_owned()) }
    }

    #[inline]
    fn set_class_name(&mut self, class_name: &str) {
        self.set_attribute("class", class_name);
    }

    fn class_list(&self) -> DOMTokenList {
        DOMTokenList::new(self)
    }

    fn class_list_mut(&mut self) -> DOMTokenListMut<'_> {
        DOMTokenListMut::new(self)
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
            lxb_dom_element_attr_by_name(
                self.node_ptr_().cast(), qualified_name.as_ptr(), qualified_name.len())
        };
        if attr.is_null() {
            return None;
        }
        Some(AttrNode::new(&self.tree_(), attr.cast())?)
    }

    fn attribute_names(&self) -> Vec<String> {
        check_node!(self);
        unsafe { self.attribute_names_unchecked().into_iter().map(|s| s.to_owned()).collect() }
    }

    fn attributes(&self) -> NamedNodeMap {
        check_node!(self);
        NamedNodeMap::new_live(&self.as_noderef(), None, |n, _| {
            let mut v = Vec::new();
            unsafe {
                let mut attr = lxb_dom_element_first_attribute_noi(n.node_ptr_().cast());
                while !attr.is_null() {
                    v.push(AttrNode::new(&n.tree_(), attr.cast()).unwrap());
                    attr = lxb_dom_element_next_attribute_noi(attr);
                }
            };
            v
        })
    }

    fn set_attribute(&mut self, qualified_name: &str, value: &str) {
        check_node!(self);
        unsafe {
             lxb_dom_element_set_attribute(self.node_ptr_().cast(),
                                           qualified_name.as_ptr(), qualified_name.len(),
                                           value.as_ptr(), value.len());
        }
    }

    fn set_attribute_node(&mut self, attribute: &AttrNode) {
        check_nodes!(self, attribute);
        unsafe {
            lxb_dom_element_attr_append(self.node_ptr_().cast(), attribute.node_ptr_().cast());
        }
    }

    fn remove_attribute(&mut self, qualified_name: &str) {
        check_node!(self);
        unsafe {
             lxb_dom_element_remove_attribute(
                 self.node_ptr_().cast(), qualified_name.as_ptr(), qualified_name.len());
        }
    }

    fn toggle_attribute(&mut self, qualified_name: &str, force: Option<bool>) -> bool {
        let on = match force {
            Some(f) => f,
            None => !self.has_attribute(qualified_name)
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
        unsafe { lxb_dom_element_has_attribute(self.node_ptr_().cast(),
                                               qualified_name.as_ptr(), qualified_name.len()) }
    }

    fn closest(&self, selectors: &str) -> Result<Option<ElementNode>, CSSParserError> {
        let sel_list = CSSSelectorList::parse_selectors(&self.tree_(), selectors)?;
        let mut found = None;
        let mut node = Some(self.clone());
        while let Some(n) = node {
            sel_list.match_elements_reverse(&n.as_noderef(), |e, _, found| {
                *found = Some(e);
                TraverseAction::Stop
            }, &mut found);
            if found.is_some() {
                return Ok(found);
            }
            node = n.parent_element();
        }
        Ok(None)
    }

    fn matches(&self, selectors: &str) -> Result<bool, CSSParserError> {
        let sel_list = CSSSelectorList::parse_selectors(&self.tree_(), selectors)?;
        let mut found = false;
        sel_list.match_elements_reverse(&self.as_noderef(), |_, _, found| {
            *found = true;
            TraverseAction::Stop
        }, &mut found);
        Ok(found)
    }

    fn get_elements_by_tag_name(&self, name: &str) -> HTMLCollection {
        HTMLCollection::new_live(&self.as_noderef(), Some(Box::new([name.to_owned()])), |n, qn| {
            unsafe { get_elements_by_tag_name(&n, &qn.unwrap_unchecked()[0]) }
        })
    }

    fn get_elements_by_class_name(&self, class_names: &str) -> HTMLCollection {
        HTMLCollection::new_live(&self.as_noderef(), Some(Box::new([class_names.to_owned()])), |n, cls| {
            unsafe { get_elements_by_class_name(&n, &cls.unwrap_unchecked()[0]) }
        })
    }

    fn get_elements_by_attr(&self, name: &str, value: &str) -> HTMLCollection {
        self.get_elements_by_attr_case(name, value, false)
    }

    fn get_elements_by_attr_case(&self, name: &str, value: &str, case_insensitive: bool) -> HTMLCollection {
        let user_data = Box::new([
            name.to_owned(),
            value.to_owned(),
            case_insensitive.to_string()]);
        HTMLCollection::new_live(&self.as_noderef(), Some(user_data), |n, attr| {
            unsafe {
                get_elements_by_attr(
                    &n.as_noderef(),
                    &attr.unwrap_unchecked()[0],
                    &attr.unwrap_unchecked()[1],
                    &attr.unwrap_unchecked()[2] == "true")
            }
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
            };
            let s = str_from_lxb_str_t(html_str).unwrap_or_default().to_owned();
            lexbor_str_destroy(html_str, self.doc_ptr_unchecked().as_ref().unwrap().text, true);
            s
        }
    }

    fn set_inner_html(&mut self, html: &str) {
        check_node!(self);
        unsafe { lxb_html_element_inner_html_set(self.node_ptr_().cast(), html.as_ptr(), html.len()); }
    }

    fn outer_html(&self) -> String {
        check_node!(self);
        node_serialize_html(self.node_ptr_())
    }

    fn set_outer_html(&mut self, html: &str) {
        check_node!(self);
        if unsafe { *self.node_ptr_() }.parent.is_null() {
            return;
        }
        self.set_inner_html(html);
        unsafe {
            self.iter_raw().for_each(|n| {
                lxb_dom_node_insert_before(self.node_ptr_(), n);
            })
        }
        self.remove()
    }

    fn inner_text(&self) -> String {
        check_node!(self);
        node_format_visible_text(self.node_ptr_())
    }

    #[inline]
    fn set_inner_text(&mut self, text: &str) {
        self.set_text_content(text);
    }

    #[inline]
    fn outer_text(&self) -> String {
        self.inner_text()
    }

    fn set_outer_text(&mut self, text: &str) {
        check_node!(self);
        self.set_text_content(text);
        unsafe {
            let fc = lxb_dom_node_first_child_noi(self.node_ptr_());
            if !fc.is_null() {
                lxb_dom_node_insert_before(self.node_ptr_(), fc);
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

impl Attr for AttrNode {
    unsafe fn name_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_attr_qualified_name)
    }

    unsafe fn local_name_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_attr_local_name_noi)
    }

    unsafe fn value_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_attr_value_noi)
    }

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

    fn set_value(&mut self, value: &str) {
        check_node!(self);
        unsafe { lxb_dom_node_text_content_set(self.node_ptr_(), value.as_ptr(), value.len()); }
    }

    fn owner_element(&self) -> Option<ElementNode> {
        check_node!(self);
        unsafe {
            let attr = self.node_ptr_() as *mut lxb_dom_attr_t;
            if attr.is_null() || (*attr).owner.is_null() {
                return None;
            }
            ElementNode::new(&self.tree_(), (*attr).owner.cast())
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
        unsafe {
            Some(str_from_lxb_str_cb(self.node_ptr_(),
                                     lxb_dom_processing_instruction_target_noi)?.to_owned())
        }
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
impl Notation for NotationNode {
    unsafe fn public_id_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_document_type_public_id_noi)
    }

    unsafe fn system_id_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_document_type_system_id_noi)
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
