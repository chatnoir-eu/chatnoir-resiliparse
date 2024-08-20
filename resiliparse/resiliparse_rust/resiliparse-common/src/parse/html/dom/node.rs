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
use std::ptr::addr_of_mut;
use std::sync::Arc;
use crate::parse::html::css::*;
use crate::parse::html::dom::coll::*;
use crate::parse::html::dom::*;
use crate::parse::html::dom::iter::*;
use crate::parse::html::dom::node_base::NodeBase;
use crate::parse::html::dom::traits::*;
use crate::parse::html::serialize::{node_format_visible_text, node_serialize_html};

// Re-export NodeType publicly
pub use crate::parse::html::dom::traits::NodeType;


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


impl Deref for Node {
    type Target = NodeBase;

    fn deref(&self) -> &Self::Target {
        match self {
            Node::Element(n) => &n.node_base,
            Node::Attribute(n) => &n.node_base,
            Node::Text(n) => &n.node_base,
            Node::CdataSection(n) => &n.node_base,
            Node::ProcessingInstruction(n) => &n.node_base,
            Node::Comment(n) => &n.node_base,
            Node::Document(n) => &n.node_base,
            Node::DocumentType(n) => &n.node_base,
            Node::DocumentFragment(n) => &n.node_base,
            Node::Notation(n) => &n.node_base, // legacy
        }
    }
}

impl DerefMut for Node {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Node::Element(n) => &mut n.node_base,
            Node::Attribute(n) => &mut n.node_base,
            Node::Text(n) => &mut n.node_base,
            Node::CdataSection(n) => &mut n.node_base,
            Node::ProcessingInstruction(n) => &mut n.node_base,
            Node::Comment(n) => &mut n.node_base,
            Node::Document(n) => &mut n.node_base,
            Node::DocumentType(n) => &mut n.node_base,
            Node::DocumentFragment(n) => &mut n.node_base,
            Node::Notation(n) => &mut n.node_base,
        }
    }
}

impl From<NodeRef<'_>> for Node {
    fn from(value: NodeRef<'_>) -> Self {
        NodeBase::wrap_node(&value.tree, value.node).unwrap()
    }
}

impl From<&NodeBase> for Node {
    fn from(value: &NodeBase) -> Self {
        NodeBase::wrap_node(&value.tree, value.node).unwrap()
    }
}

impl From<NodeBase> for Node {
    #[inline]
    fn from(value: NodeBase) -> Self {
        Self::from(&value)
    }
}

impl PartialEq<NodeBase> for Node {
    fn eq(&self, other: &NodeBase) -> bool {
        **self == *other
    }
}

impl Display for Node {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self.deref(), f)
    }
}


#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    Notation(&'a NotationNode),
}

impl<'a> Deref for NodeRef<'a> {
    type Target = NodeBase;

    fn deref(&self) -> &Self::Target {
        match self {
            NodeRef::Element(n) => &n.node_base,
            NodeRef::Attribute(n) => &n.node_base,
            NodeRef::Text(n) => &n.node_base,
            NodeRef::CdataSection(n) => &n.node_base,
            NodeRef::ProcessingInstruction(n) => &n.node_base,
            NodeRef::Comment(n) => &n.node_base,
            NodeRef::Document(n) => &n.node_base,
            NodeRef::DocumentType(n) => &n.node_base,
            NodeRef::DocumentFragment(n) => &n.node_base,
            NodeRef::Notation(n) => &n.node_base,
        }
    }
}

impl<'a> From<&'a Node> for NodeRef<'a> {
    fn from(value: &'a Node) -> Self {
        match value {
            Node::Element(n) => NodeRef::Element(n),
            Node::Attribute(n) => NodeRef::Attribute(n),
            Node::Text(n) => NodeRef::Text(n),
            Node::CdataSection(n) => NodeRef::CdataSection(n),
            Node::ProcessingInstruction(n) => NodeRef::ProcessingInstruction(n),
            Node::Comment(n) => NodeRef::Comment(n),
            Node::Document(n) => NodeRef::Document(n),
            Node::DocumentType(n) => NodeRef::DocumentType(n),
            Node::DocumentFragment(n) => NodeRef::DocumentFragment(n),
            Node::Notation(n) => NodeRef::Notation(n),
        }
    }
}

impl PartialEq<NodeBase> for NodeRef<'_> {
    fn eq(&self, other: &NodeBase) -> bool {
        **self == *other
    }
}

impl Display for NodeRef<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self.deref(), f)
    }
}

macro_rules! check_node {
    ($node: expr) => {
        if $node.node.is_null() {
            return Default::default();
        }
    }
}

pub(super) use check_node;

macro_rules! check_nodes {
    ($node1: expr, $node2: expr) => {
        {
            if !Arc::ptr_eq(&$node1.tree, &$node2.tree) ||
               $node1.node.is_null() || $node2.node.is_null() || $node1 == $node2 {
                return Default::default();
            }
        }
    }
}

pub(super) use check_nodes;
use crate::parse::html::lexbor::{str_from_lxb_char_t, str_from_lxb_str_cb, str_from_lxb_str_t};


macro_rules! define_node_type {
    ($Self: ident, $EnumType: ident) => {
        #[derive(Clone, PartialEq, Eq)]
        pub struct $Self {
            // TODO: Pass state up instead of down
            pub(in super::super) node_base: NodeBase
        }

        impl NodeInterface for $Self {
            #[inline(always)]
            unsafe fn node_name_unchecked(&self) -> Option<&str> { self.node_base.node_name_unchecked() }
            #[inline(always)]
            unsafe fn node_value_unchecked(&self) -> Option<&str> { self.node_base.node_value_unchecked() }

            #[inline(always)]
            fn upcast(&self) -> &NodeBase { &self.node_base }
            #[inline(always)]
            fn upcast_mut(&mut self) -> &mut NodeBase { &mut self.node_base }
            #[inline(always)]
            fn as_noderef(&self) -> NodeRef { NodeRef::$EnumType(self) }
            #[inline(always)]
            fn as_node(&self) -> Node { self.clone().into() }
            #[inline(always)]
            fn into_node(self) -> Node { self.into() }

            #[inline(always)]
            fn node_type(&self) -> Option<NodeType> { self.node_base.node_type() }
            #[inline(always)]
            fn node_name(&self) -> Option<String> { self.node_base.node_name() }
            #[inline(always)]
            fn node_value(&self) -> Option<String> { self.node_base.node_value() }
            #[inline(always)]
            fn set_node_value(&mut self, value: &str) { self.node_base.set_node_value(value) }
            #[inline(always)]
            fn text_content(&self) -> Option<String> { self.node_base.text_content() }
            #[inline(always)]
            fn set_text_content(&mut self, content: &str) { self.node_base.set_text_content(content) }

            #[inline(always)]
            fn owner_document(&self) -> Option<DocumentNode> { self.node_base.owner_document() }
            #[inline(always)]
            fn parent_node(&self) -> Option<Node> { self.node_base.parent_node() }
            #[inline(always)]
            fn parent_element(&self) -> Option<ElementNode> { self.node_base.parent_element() }

            #[inline(always)]
            fn has_child_nodes(&self) -> bool { self.node_base.has_child_nodes() }
            #[inline(always)]
            fn contains(&self, node: &Node) -> bool { self.node_base.contains(node) }
            #[inline(always)]
            fn child_nodes(&self) -> NodeList { self.node_base.child_nodes() }
            #[inline(always)]
            fn first_child(&self) -> Option<Node> { self.node_base.first_child() }
            #[inline(always)]
            fn last_child(&self) -> Option<Node> { self.node_base.last_child() }
            #[inline(always)]
            fn previous_sibling(&self) -> Option<Node> { self.node_base.previous_sibling() }
            #[inline(always)]
            fn next_sibling(&self) -> Option<Node> { self.node_base.next_sibling() }
            #[inline(always)]
            fn clone_node(&self, deep: bool) -> Option<Node> { self.node_base.clone_node(deep) }

            #[inline(always)]
            fn insert_before<'a>(&mut self, node: &'a Node, child: Option<&'a Node>) -> Option<&'a Node> {
                self.node_base.insert_before(node, child) }
            #[inline(always)]
            fn append_child<'a>(&mut self, node: &'a Node) -> Option<&'a Node> {
                self.node_base.append_child(node) }
            #[inline(always)]
            fn replace_child<'a>(&mut self, node: &'a Node, child: &'a Node) -> Option<&'a Node> {
                self.node_base.replace_child(node, child) }
            #[inline(always)]
            fn remove_child<'a>(&mut self, node: &'a Node) -> Option<&'a Node> {
                self.node_base.remove_child(node) }

            #[inline(always)]
            fn iter(&self) -> NodeIterator { self.node_base.iter() }
            #[inline(always)]
            fn iter_elements(&self) -> ElementIterator { self.node_base.iter_elements() }
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
            fn from(value: &'a $Self) -> Self {
                NodeRef::$EnumType(value)
            }
        }

        impl IntoIterator for $Self {
            type Item = Node;
            type IntoIter = NodeIteratorOwned;

            fn into_iter(self) -> Self::IntoIter {
                self.node_base.into_iter()
            }
        }

        impl Debug for $Self {
            #[inline]
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                Debug::fmt(&self.node_base, f)
            }
        }

        impl Display for $Self {
            #[inline]
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                Display::fmt(&self.node_base, f)
            }
        }
    }
}


// --------------------------------------- DocumentType impl ---------------------------------------


define_node_type!(DocumentTypeNode, DocumentType);

impl DocumentType for DocumentTypeNode {
    unsafe fn name_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_base.node, lxb_dom_document_type_name_noi)
    }

    unsafe fn public_id_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_base.node, lxb_dom_document_type_public_id_noi)
    }

    unsafe fn system_id_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_base.node, lxb_dom_document_type_system_id_noi)
    }

    #[inline]
    fn name(&self) -> Option<String> {
        check_node!(self.node_base);
        unsafe { Some(self.name_unchecked()?.to_owned()) }
    }

    #[inline]
    fn public_id(&self) -> Option<String> {
        check_node!(self.node_base);
        unsafe { Some(self.public_id_unchecked()?.to_owned()) }
    }

    #[inline]
    fn system_id(&self) -> Option<String> {
        check_node!(self.node_base);
        unsafe { Some(self.system_id_unchecked()?.to_owned()) }
    }
}

impl ChildNode for DocumentTypeNode {}


// ----------------------------------------- Document impl -----------------------------------------


define_node_type!(DocumentNode, Document);


impl Document for DocumentNode {
    fn doctype(&self) -> Option<DocumentTypeNode> {
        check_node!(self.node_base);
        unsafe {
            let doctype = (*self.node_base.doc_ptr_unchecked().as_ref()?).doctype;
            let base = NodeBase::new_base(&self.node_base.tree, doctype.cast())?;
            Some(DocumentTypeNode { node_base: base })
        }
    }

    fn document_element(&self) -> Option<DocumentNode> {
        check_node!(self.node_base);
        let base = NodeBase::new_base(&self.node_base.tree, self.node_base.node)?;
        Some(DocumentNode { node_base: base })
    }

    fn get_elements_by_tag_name(&self, qualified_name: &str) -> HTMLCollection {
        check_node!(self.node_base);
        HTMLCollection::new_live(self.as_node(), Some(Box::new([qualified_name.to_owned()])), |n, qn| {
            unsafe { elements_by_tag_name(n, &qn.unwrap_unchecked()[0]) }
        })
    }

    fn get_elements_by_class_name(&self, qualified_name: &str) -> HTMLCollection {
        check_node!(self.node_base);
        HTMLCollection::new_live(self.as_node(), Some(Box::new([qualified_name.to_owned()])), |n, cls| {
            unsafe { elements_by_class_name(n, &cls.unwrap_unchecked()[0]) }
        })
    }

    #[inline(always)]
    fn get_elements_by_attr(&self, qualified_name: &str, value: &str) -> HTMLCollection {
        self.get_elements_by_attr_case(qualified_name, value, false)
    }

    fn get_elements_by_attr_case(&self, qualified_name: &str, value: &str, case_insensitive: bool) -> HTMLCollection {
        check_node!(self.node_base);
        let user_data = Box::new([
            qualified_name.to_owned(),
            value.to_owned(),
            case_insensitive.to_string()]);
        HTMLCollection::new_live(self.as_node(), Some(user_data), |n, attr| {
            unsafe {
                elements_by_attr(
                    n,
                    &attr.unwrap_unchecked()[0],
                    &attr.unwrap_unchecked()[1],
                    &attr.unwrap_unchecked()[2] == "true")
            }
        })
    }

    fn create_element(&mut self, local_name: &str) -> Result<ElementNode, DOMError> {
        if !self.node_base.node.is_null() {
            unsafe { NodeBase::create_element_unchecked(&self.node_base, local_name) }
        } else {
            Err(DOMError { msg: "Invalid document.".to_owned()} )
        }
    }

    fn create_document_fragment(&mut self) -> Result<DocumentFragmentNode, DOMError> {
        if !self.node_base.node.is_null() {
            unsafe { NodeBase::create_document_fragment_unchecked(&self.node_base) }
        } else {
            Err(DOMError { msg: "Invalid document.".to_owned()} )
        }
    }

    fn create_text_node(&mut self, data: &str) -> Result<TextNode, DOMError> {
        if !self.node_base.node.is_null() {
            unsafe { NodeBase::create_text_node_unchecked(&self.node_base, data) }
        } else {
            Err(DOMError { msg: "Invalid document.".to_owned()} )
        }
    }

    fn create_cdata_section(&mut self, data: &str) -> Result<CdataSectionNode, DOMError> {
        if !self.node_base.node.is_null() {
            unsafe { NodeBase::create_cdata_section_unchecked(&self.node_base, data) }
        } else {
            Err(DOMError { msg: "Invalid document.".to_owned()} )
        }
    }

    fn create_comment(&mut self, data: &str) -> Result<CommentNode, DOMError> {
        if !self.node_base.node.is_null() {
            unsafe { NodeBase::create_comment_unchecked(&self.node_base, data) }
        } else {
            Err(DOMError { msg: "Invalid document.".to_owned()} )
        }
    }

    fn create_processing_instruction(&mut self, target: &str, data: &str) -> Result<ProcessingInstructionNode, DOMError> {
        if !self.node_base.node.is_null() {
            unsafe { NodeBase::create_processing_instruction_unchecked(&self.node_base, target, data) }
        } else {
            Err(DOMError { msg: "Invalid document.".to_owned()} )
        }
    }

    fn create_attribute(&mut self, local_name: &str) -> Result<AttrNode, DOMError> {
        if !self.node_base.node.is_null() {
            unsafe { NodeBase::create_attribute_unchecked(&self.node_base, local_name) }
        } else {
            Err(DOMError { msg: "Invalid document.".to_owned()} )
        }
    }
}

impl DocumentOrShadowRoot for DocumentNode {}

impl ParentNode for DocumentNode {
    fn children(&self) -> HTMLCollection {
        HTMLCollection::new_live(self.as_node(), None, |n, _| {
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
        HTMLCollection::new_live(self.as_node(), None, |n, _| {
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
        str_from_lxb_str_cb(self.node_base.node, lxb_dom_element_tag_name)
    }

    unsafe fn local_name_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_base.node, lxb_dom_element_local_name)
    }

    unsafe fn id_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_base.node, lxb_dom_element_id_noi)
    }

    #[inline]
    unsafe fn name_unchecked(&self) -> Option<&str> {
        self.attribute_unchecked("name")
    }

    unsafe fn class_name_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_base.node, lxb_dom_element_class_noi)
    }

    unsafe fn attribute_unchecked(&self, qualified_name: &str) -> Option<&str> {
        let mut size = 0;
        let name = lxb_dom_element_get_attribute(
            self.node_base.node.cast(),
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
        let mut attr =  lxb_dom_element_first_attribute_noi(self.node_base.node.cast());
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
        check_node!(self.node_base);
        unsafe { Some(self.tag_name_unchecked()?.to_owned()) }
    }

    #[inline]
    fn local_name(&self) -> Option<String> {
        check_node!(self.node_base);
        unsafe { Some(self.local_name_unchecked()?.to_owned()) }
    }

    #[inline]
    fn id(&self) -> Option<String> {
        check_node!(self.node_base);
        unsafe { Some(self.id_unchecked()?.to_owned()) }
    }

    #[inline]
    fn set_id(&mut self, id: &str) {
        self.set_attribute("id", id);
    }

    #[inline]
    fn class_name(&self) -> Option<String> {
        check_node!(self.node_base);
        unsafe { Some(self.class_name_unchecked()?.to_owned()) }
    }

    #[inline]
    fn set_class_name(&mut self, class_name: &str) {
        self.set_attribute("class", class_name);
    }

    fn class_list(&self) -> DOMTokenList {
        DOMTokenList::new(self)
    }

    fn class_list_mut(&mut self) -> DOMTokenListMut {
        DOMTokenListMut::new(self)
    }

    fn attribute(&self, qualified_name: &str) -> Option<String> {
        check_node!(self.node_base);
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
        check_node!(self.node_base);
        let attr = unsafe {
            lxb_dom_element_attr_by_name(
                self.node_base.node.cast(), qualified_name.as_ptr(), qualified_name.len())
        };
        if attr.is_null() {
            return None;
        }
        let base = NodeBase::new_base(&self.node_base.tree, attr.cast())?;
        Some(AttrNode { node_base: base })
    }

    fn attribute_names(&self) -> Vec<String> {
        check_node!(self.node_base);
        unsafe { self.attribute_names_unchecked().into_iter().map(|s| s.to_owned()).collect() }
    }

    fn attributes(&self) -> NamedNodeMap {
        check_node!(self.node_base);
        NamedNodeMap::new_live(self.as_node(), None, |n, _| {
            let mut v = Vec::new();
            unsafe {
                let mut attr = lxb_dom_element_first_attribute_noi(n.node.cast());
                while !attr.is_null() {
                    v.push(AttrNode { node_base: NodeBase::new_base(&n.tree, attr.cast()).unwrap_unchecked() });
                    attr = lxb_dom_element_next_attribute_noi(attr);
                }
            };
            v
        })
    }

    fn set_attribute(&mut self, qualified_name: &str, value: &str) {
        check_node!(self.node_base);
        unsafe {
             lxb_dom_element_set_attribute(self.node_base.node.cast(),
                                           qualified_name.as_ptr(), qualified_name.len(),
                                           value.as_ptr(), value.len());
        }
    }

    fn set_attribute_node(&mut self, attribute: &AttrNode) {
        check_nodes!(self.node_base, attribute.node_base);
        unsafe {
            lxb_dom_element_attr_append(self.node_base.node.cast(), attribute.node_base.node.cast());
        }
    }

    fn remove_attribute(&mut self, qualified_name: &str) {
        check_node!(self.node_base);
        unsafe {
             lxb_dom_element_remove_attribute(
                 self.node_base.node.cast(), qualified_name.as_ptr(), qualified_name.len());
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
        check_node!(self.node_base);
        unsafe { lxb_dom_element_has_attribute(self.node_base.node.cast(),
                                               qualified_name.as_ptr(), qualified_name.len()) }
    }

    fn closest(&self, selectors: &str) -> Result<Option<ElementNode>, CSSParserError> {
        let sel_list = CSSSelectorList::parse_selectors(&self.upcast().tree, selectors)?;
        let mut found = None;
        let mut node = Some(self.as_node());
        while let Some(n) = &node {
            sel_list.match_elements_reverse(n.as_noderef(), |e, _, found| {
                *found = Some(e.clone());
                TraverseAction::Stop
            }, &mut found);
            if found.is_some() {
                return Ok(found);
            }
            node = n.parent_node();
        }
        Ok(None)
    }

    fn matches(&self, selectors: &str) -> Result<bool, CSSParserError> {
        let sel_list = CSSSelectorList::parse_selectors(&self.upcast().tree, selectors)?;
        let mut found = false;
        sel_list.match_elements_reverse(self.into(), |_, _, found| {
            *found = true;
            TraverseAction::Stop
        }, &mut found);
        Ok(found)
    }

    fn get_elements_by_tag_name(&self, qualified_name: &str) -> HTMLCollection {
        HTMLCollection::new_live(self.as_node(), Some(Box::new([qualified_name.to_owned()])), |n, qn| {
            unsafe { elements_by_tag_name(&n, &qn.unwrap_unchecked()[0]) }
        })
    }

    fn get_elements_by_class_name(&self, class_names: &str) -> HTMLCollection {
        HTMLCollection::new_live(self.as_node(), Some(Box::new([class_names.to_owned()])), |n, cls| {
            unsafe { elements_by_class_name(&n, &cls.unwrap_unchecked()[0]) }
        })
    }

    fn get_elements_by_attr(&self, qualified_name: &str, value: &str) -> HTMLCollection {
        self.get_elements_by_attr_case(qualified_name, value, false)
    }

    fn get_elements_by_attr_case(&self, qualified_name: &str, value: &str, case_insensitive: bool) -> HTMLCollection {
        let user_data = Box::new([
            qualified_name.to_owned(),
            value.to_owned(),
            case_insensitive.to_string()]);
        HTMLCollection::new_live(self.as_node(), Some(user_data), |n, attr| {
            unsafe {
                elements_by_attr(
                    &n,
                    &attr.unwrap_unchecked()[0],
                    &attr.unwrap_unchecked()[1],
                    &attr.unwrap_unchecked()[2] == "true")
            }
        })
    }

    /// Inner HTML of this DOM node's children.
    fn inner_html(&self) -> String {
        check_node!(self.node_base);
        unsafe {
            let html_str = lexbor_str_create();
            if html_str.is_null() {
                return String::default();
            }
            let mut next = lxb_dom_node_first_child_noi(self.node_base.node);
            while !next.is_null() {
                lxb_html_serialize_tree_str(next, html_str);
                next = lxb_dom_node_next_noi(next);
            };
            let s = str_from_lxb_str_t(html_str).unwrap_or_default().to_owned();
            lexbor_str_destroy(html_str, (*self.node_base.doc_ptr_unchecked()).text, true);
            s
        }
    }

    fn set_inner_html(&mut self, html: &str) {
        check_node!(self.node_base);
        unsafe { lxb_html_element_inner_html_set(self.node_base.node.cast(), html.as_ptr(), html.len()); }
    }

    fn outer_html(&self) -> String {
        check_node!(self.node_base);
        node_serialize_html(self.node_base.node)
    }

    fn set_outer_html(&mut self, html: &str) {
        check_node!(self.node_base);
        if unsafe { *self.node_base.node }.parent.is_null() {
            return;
        }
        self.set_inner_html(html);
        unsafe {
            self.node_base.iter_raw().for_each(|n| {
                lxb_dom_node_insert_before(self.node_base.node, n);
            })
        }
        self.remove()
    }

    fn inner_text(&self) -> String {
        check_node!(self.node_base);
        node_format_visible_text(self.node_base.node)
    }

    #[inline]
    fn set_inner_text(&mut self, text: &str) {
        self.node_base.set_text_content(text);
    }

    #[inline]
    fn outer_text(&self) -> String {
        self.inner_text()
    }

    fn set_outer_text(&mut self, text: &str) {
        check_node!(self.node_base);
        self.node_base.set_text_content(text);
        unsafe {
            let fc = lxb_dom_node_first_child_noi(self.node_base.node);
            if !fc.is_null() {
                lxb_dom_node_insert_before(self.node_base.node, fc);
            }
        }
        self.remove()
    }
}

impl ParentNode for ElementNode {
    fn children(&self) -> HTMLCollection {
        HTMLCollection::new_live(self.as_node(), None, |n, _| {
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
        str_from_lxb_str_cb(self.node_base.node, lxb_dom_attr_qualified_name)
    }

    unsafe fn local_name_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_base.node, lxb_dom_attr_local_name_noi)
    }

    unsafe fn value_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_base.node, lxb_dom_attr_value_noi)
    }

    #[inline]
    fn local_name(&self) -> Option<String> {
        check_node!(self.node_base);
        unsafe { Some(self.local_name_unchecked()?.to_owned()) }
    }

    #[inline]
    fn name(&self) -> Option<String> {
        check_node!(self.node_base);
        unsafe { Some(self.name_unchecked()?.to_owned()) }
    }

    #[inline]
    fn value(&self) -> Option<String> {
        check_node!(self.node_base);
        unsafe { Some(self.value_unchecked()?.to_owned()) }
    }

    fn set_value(&mut self, value: &str) {
        check_node!(self.node_base);
        unsafe { lxb_dom_node_text_content_set(self.node_base.node, value.as_ptr(), value.len()); }
    }

    fn owner_element(&self) -> Option<ElementNode> {
        check_node!(self.node_base);
        unsafe {
            let attr = self.node_base.node as *mut lxb_dom_attr_t;
            if attr.is_null() || (*attr).owner.is_null() {
                return None;
            }
            Some(NodeBase::wrap_node(&self.node_base.tree, (*attr).owner.cast())?.into())
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
        check_node!(self.node_base);
        unsafe {
            Some(str_from_lxb_str_cb(self.node_base.node,
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

impl Notation for NotationNode {
    unsafe fn public_id_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_base.node, lxb_dom_document_type_public_id_noi)
    }

    unsafe fn system_id_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_base.node, lxb_dom_document_type_system_id_noi)
    }

    #[inline]
    fn public_id(&self) -> Option<String> {
        check_node!(self.node_base);
        unsafe { Some(self.public_id_unchecked()?.to_owned()) }
    }

    #[inline]
    fn system_id(&self) -> Option<String> {
        check_node!(self.node_base);
        unsafe { Some(self.system_id_unchecked()?.to_owned()) }
    }
}
