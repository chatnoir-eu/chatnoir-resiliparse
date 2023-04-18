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


use std::{ptr, slice, vec};
use std::cmp::max;
use std::collections::HashSet;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::ops::{Add, Deref, DerefMut};
use std::ptr::{addr_of, addr_of_mut};
use std::rc::Rc;
use crate::parse::html::css::{CSSParserError, CSSSelectorList, TraverseAction};
use crate::parse::html::serialize::node_serialize_html;

use crate::third_party::lexbor::*;
use super::serialize::node_format_visible_text;
use super::tree::{HTMLDocument};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Node {
    Element(ElementNode),
    Attr(AttrNode),
    Text(TextNode),
    CDataSection(CDataSectionNode),
    ProcessingInstruction(ProcessingInstructionNode),
    Comment(CommentNode),
    Document(DocumentNode),
    DocumentType(DocumentTypeNode),
    DocumentFragment(DocumentFragmentNode)
}

impl Deref for Node {
    type Target = NodeBase;

    fn deref(&self) -> &Self::Target {
        match self {
            Node::Element(n) => &n.node_base,
            Node::Attr(n) => &n.node_base,
            Node::Text(n) => &n.node_base,
            Node::CDataSection(n) => &n.node_base,
            Node::ProcessingInstruction(n) => &n.node_base,
            Node::Comment(n) => &n.node_base,
            Node::Document(n) => &n.node_base,
            Node::DocumentType(n) => &n.node_base,
            Node::DocumentFragment(n) => &n.node_base
        }
    }
}

impl DerefMut for Node {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Node::Element(n) => &mut n.node_base,
            Node::Attr(n) => &mut n.node_base,
            Node::Text(n) => &mut n.node_base,
            Node::CDataSection(n) => &mut n.node_base,
            Node::ProcessingInstruction(n) => &mut n.node_base,
            Node::Comment(n) => &mut n.node_base,
            Node::Document(n) => &mut n.node_base,
            Node::DocumentType(n) => &mut n.node_base,
            Node::DocumentFragment(n) => &mut n.node_base
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
    Attr(&'a AttrNode),
    Text(&'a TextNode),
    CDataSection(&'a CDataSectionNode),
    ProcessingInstruction(&'a ProcessingInstructionNode),
    Comment(&'a CommentNode),
    Document(&'a DocumentNode),
    DocumentType(&'a DocumentTypeNode),
    DocumentFragment(&'a DocumentFragmentNode),
    Undefined(&'a NodeBase)
}

impl<'a> Deref for NodeRef<'a> {
    type Target = NodeBase;

    fn deref(&self) -> &Self::Target {
        match self {
            NodeRef::Element(n) => &n.node_base,
            NodeRef::Attr(n) => &n.node_base,
            NodeRef::Text(n) => &n.node_base,
            NodeRef::CDataSection(n) => &n.node_base,
            NodeRef::ProcessingInstruction(n) => &n.node_base,
            NodeRef::Comment(n) => &n.node_base,
            NodeRef::Document(n) => &n.node_base,
            NodeRef::DocumentType(n) => &n.node_base,
            NodeRef::DocumentFragment(n) => &n.node_base,
            NodeRef::Undefined(n) => n,
        }
    }
}

impl<'a> From<&'a Node> for NodeRef<'a> {
    fn from(value: &'a Node) -> Self {
        match value {
            Node::Element(n) => NodeRef::Element(n),
            Node::Attr(n) => NodeRef::Attr(n),
            Node::Text(n) => NodeRef::Text(n),
            Node::CDataSection(n) => NodeRef::CDataSection(n),
            Node::ProcessingInstruction(n) => NodeRef::ProcessingInstruction(n),
            Node::Comment(n) => NodeRef::Comment(n),
            Node::Document(n) => NodeRef::Document(n),
            Node::DocumentType(n) => NodeRef::DocumentType(n),
            Node::DocumentFragment(n) => NodeRef::DocumentFragment(n)
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

macro_rules! check_nodes {
    ($node1: expr, $node2: expr) => {
        {
            if !Rc::ptr_eq(&$node1.tree, &$node2.tree) ||
               $node1.node.is_null() || $node2.node.is_null() || $node1 == $node2 {
                return Default::default();
            }
        }
    }
}

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


/// Base DOM node.
pub trait NodeInterface: Debug + Display {
    unsafe fn node_name_unchecked(&self) -> Option<&str>;
    unsafe fn node_value_unchecked(&self) -> Option<&str>;

    fn upcast(&self) -> &NodeBase;
    fn upcast_mut(&mut self) -> &mut NodeBase;
    fn as_noderef(&self) -> NodeRef;
    fn to_node(&self) -> Node;

    fn node_name(&self) -> Option<String>;
    fn node_value(&self) -> Option<String>;
    fn set_node_value(&mut self, value: &str);
    fn text_content(&self) -> Option<String>;
    fn set_text_content(&mut self, content: &str);

    fn owner_document(&self) -> Option<DocumentNode>;
    fn parent_node(&self) -> Option<Node>;
    fn parent_element(&self) -> Option<ElementNode>;

    fn has_child_nodes(&self) -> bool;
    fn contains(&self, node: &Node) -> bool;
    fn child_nodes(&self) -> NodeList;
    fn first_child(&self) -> Option<Node>;
    fn last_child(&self) -> Option<Node>;
    fn previous_sibling(&self) -> Option<Node>;
    fn next_sibling(&self) -> Option<Node>;
    fn clone_node(&self, deep: bool) -> Option<Node>;

    fn insert_before<'a>(&mut self, node: &'a Node, child: Option<&'a Node>) -> Option<&'a Node>;
    fn append_child<'a>(&mut self, node: &'a Node) -> Option<&'a Node>;
    fn replace_child<'a>(&mut self, node: &'a Node, child: &'a Node) -> Option<&'a Node>;
    fn remove_child<'a>(&mut self, node: &'a Node) -> Option<&'a Node>;

    fn iter(&self) -> NodeIterator;
    fn iter_elements(&self) -> ElementIterator;
}

/// DocumentType node.
pub trait DocumentType: ChildNode {
    unsafe fn name_unchecked(&self) -> Option<&str>;
    unsafe fn public_id_unchecked(&self) -> Option<&str>;
    unsafe fn system_id_unchecked(&self) -> Option<&str>;

    fn name(&self) -> Option<String>;
    fn public_id(&self) -> Option<String>;
    fn system_id(&self) -> Option<String>;
}

pub trait DocumentOrShadowRoot: NodeInterface {}

pub trait ShadowRoot: DocumentOrShadowRoot {}

/// Document node.
pub trait Document: DocumentOrShadowRoot + ParentNode + NonElementParentNode {
    fn doctype(&self) -> Option<DocumentTypeNode>;
    fn document_element(&self) -> Option<DocumentNode>;

    fn elements_by_tag_name(&self, qualified_name: &str) -> HTMLCollection;
    fn elements_by_class_name(&self, qualified_name: &str) -> HTMLCollection;
    fn elements_by_attr(&self, qualified_name: &str, value: &str) -> HTMLCollection;

    fn create_element(&mut self, local_name: &str) -> Result<ElementNode, DOMError>;
    fn create_document_fragment(&mut self) -> Result<DocumentFragmentNode, DOMError>;
    fn create_text_node(&mut self, data: &str) -> Result<TextNode, DOMError>;
    fn create_cdata_section(&mut self, data: &str) -> Result<CDataSectionNode, DOMError>;
    fn create_comment(&mut self, data: &str) -> Result<CommentNode, DOMError>;
    fn create_processing_instruction(&mut self, target: &str, data: &str) -> Result<ProcessingInstructionNode, DOMError>;
    fn create_attribute(&mut self, local_name: &str) -> Result<AttrNode, DOMError>;
}

pub trait DocumentFragment: DocumentOrShadowRoot + ParentNode + NonElementParentNode {}

/// ParentNode mixin trait.
pub trait ParentNode: NodeInterface {

    /// List of child element nodes.
    fn children(&self) -> HTMLCollection;

    /// First element child of this DOM node.
    fn first_element_child(&self) -> Option<ElementNode> {
        let mut child = self.first_child()?;
        loop {
            match child {
                Node::Element(c) => return Some(c),
                _ => { child = child.next_sibling()? }
            }
        }
    }

    /// Last element child element of this DOM node.
    fn last_element_child(&self) -> Option<ElementNode> {
        let mut child = self.last_child()?;
        loop {
            if let Node::Element(c) = child {
                return Some(c);
            }
            child = child.previous_sibling()?;
        }
    }

    fn child_element_count(&self) -> usize {
        let mut child = self.first_element_child();
        let mut count = 0;
        while let Some(c) = child {
            child = c.next_element_sibling();
            count += 1;
        }
        count
    }

    fn prepend(&mut self, nodes: &[&Node]) {
        let fc = self.first_child();
        nodes.iter().rev().for_each(|&n| {
            self.insert_before(n, fc.as_ref());
        });
    }

    fn append(&mut self, nodes: &[&Node]) {
        nodes.iter().for_each(|&n| {
            self.append_child(n);
        });
    }

    fn replace_children(&mut self, nodes: &[&Node]) {
        while let Some(c) = self.first_child() {
            self.remove_child(&c);
        }
        self.append(nodes);
    }

    fn query_selector(&self, selectors: &str) -> Result<Option<ElementNode>, CSSParserError> {
        let sel_list = CSSSelectorList::parse_selectors(&self.upcast().tree, selectors)?;
        let mut result = Vec::<ElementNode>::with_capacity(1);
        sel_list.match_elements(self.as_noderef(), |e, _, ctx| {
            ctx.push(e);
            TraverseAction::Stop
        }, &mut result);
        Ok(result.pop())
    }

    fn query_selector_all(&self, selectors: &str) -> Result<ElementNodeList, CSSParserError> {
        let sel_list = CSSSelectorList::parse_selectors(&self.upcast().tree, selectors)?;
        let mut result = Vec::<ElementNode>::new();
        sel_list.match_elements(self.as_noderef(), |e, _, ctx| {
            ctx.push(e);
            TraverseAction::Ok
        }, &mut result);
        Ok(ElementNodeList::from(result))
    }
}

/// NonElementParentNode mixin trait.
pub trait NonElementParentNode: NodeInterface {
    fn element_by_id(&self, element_id: &str) -> Option<ElementNode> {
        unsafe { element_by_id(self.upcast(), element_id) }
    }
}

/// ChildNode mixin trait.
pub trait ChildNode: NodeInterface {
    fn before(&mut self, nodes: &[&mut Node]) {
        if let Some(p) = &mut self.parent_node() {
            let anchor = self.parent_node();
            nodes.iter().for_each(|n| {
                p.insert_before(n, anchor.as_ref());
            });
        }
    }

    fn after(&mut self, nodes: &[&mut Node]) {
        if let Some(p) = &mut self.parent_node() {
            let anchor = self.next_sibling();
            nodes.iter().for_each(|n| {
                p.insert_before(n, anchor.as_ref());
            });
        }
    }

    fn replace_with(&mut self, nodes: &[&mut Node]) {
        self.before(nodes);
        self.remove();
    }

    fn remove(&mut self) {
        let node = self.upcast_mut();
        check_node!(node);
        unsafe { lxb_dom_node_remove(node.node); }
        node.node = ptr::null_mut();
    }
}

/// NonDocumentTypeChildNode mixin trait.
pub trait NonDocumentTypeChildNode: NodeInterface {
    /// Previous sibling element node.
    fn previous_element_sibling(&self) -> Option<ElementNode> {
        let mut p = self.previous_sibling()?;
        loop {
            if let Node::Element(s) = p {
                return Some(s);
            }
            p = p.previous_sibling()?;
        }
    }

    /// Next sibling element node.
    fn next_element_sibling(&self) -> Option<ElementNode> {
        let mut p = self.next_sibling()?;
        loop {
            if let Node::Element(s) = p {
                return Some(s);
            }
            p = p.next_sibling()?;
        }
    }
}

/// HTML Element mixin trait.
pub trait Element: ParentNode + ChildNode + NonDocumentTypeChildNode {
    unsafe fn tag_name_unchecked(&self) -> Option<&str>;
    unsafe fn local_name_unchecked(&self) -> Option<&str>;
    unsafe fn id_unchecked(&self) -> Option<&str>;
    unsafe fn name_unchecked(&self) -> Option<&str>;
    unsafe fn class_name_unchecked(&self) -> Option<&str>;
    unsafe fn attribute_unchecked(&self, qualified_name: &str) -> Option<&str>;
    unsafe fn attribute_names_unchecked(&self) -> Vec<&str>;

    fn tag_name(&self) -> Option<String>;
    fn local_name(&self) -> Option<String>;
    fn id(&self) -> Option<String>;
    fn set_id(&mut self, id: &str);
    fn class_name(&self) -> Option<String>;
    fn set_class_name(&mut self, class_name: &str);
    fn class_list(&self) -> DOMTokenList;
    fn class_list_mut(&mut self) -> DOMTokenListMut;

    fn attribute(&self, qualified_name: &str) -> Option<String>;
    fn attribute_or(&self, qualified_name: &str, default: &str) -> String;
    fn attribute_or_default(&self, qualified_name: &str) -> String;
    fn attribute_node(&self, qualified_name: &str) -> Option<AttrNode>;
    fn attribute_names(&self) -> Vec<String>;
    fn attributes(&self) -> NamedNodeMap;
    fn set_attribute(&mut self, qualified_name: &str, value: &str);
    fn set_attribute_node(&mut self, attribute: &AttrNode);
    fn remove_attribute(&mut self, qualified_name: &str);
    fn toggle_attribute(&mut self, qualified_name: &str, force: Option<bool>) -> bool;
    fn has_attribute(&self, qualified_name: &str) -> bool;

    fn closest(&self, selectors: &str) -> Result<Option<ElementNode>, CSSParserError>;
    fn matches(&self, selectors: &str) -> Result<bool, CSSParserError>;
    fn elements_by_tag_name(&self, qualified_name: &str) -> HTMLCollection;
    fn elements_by_class_name(&self, class_names: &str) -> HTMLCollection;
    fn elements_by_attr(&self, qualified_name: &str, value: &str) -> HTMLCollection;

    fn inner_html(&self) -> String;
    fn set_inner_html(&mut self, html: &str);
    fn outer_html(&self) -> String;
    fn set_outer_html(&mut self, html: &str);
    fn inner_text(&self) -> String;
    fn set_inner_text(&mut self, text: &str);
    fn outer_text(&self) -> String;
    fn set_outer_text(&mut self, text: &str);
}

pub trait Attr: NodeInterface {
    unsafe fn name_unchecked(&self) -> Option<&str>;
    unsafe fn local_name_unchecked(&self) -> Option<&str>;
    unsafe fn value_unchecked(&self) -> Option<&str>;

    fn local_name(&self) -> Option<String>;
    fn name(&self) -> Option<String>;
    fn value(&self) -> Option<String>;
    fn set_value(&mut self, value: &str);

    fn owner_element(&self) -> Option<Node>;
}

pub trait CharacterData: NodeInterface + ChildNode + NonDocumentTypeChildNode {
    fn len(&self) -> usize {
        self.node_value().unwrap_or_default().len()
    }

    #[inline]
    fn data(&self) -> Option<String> {
        self.node_value()
    }

    #[inline]
    fn set_data(&mut self, data: &str) {
        self.set_node_value(data);
    }

    fn substring_data(&self, offset: usize, count: usize) -> Option<String> {
        Some(self.data()?.chars().into_iter().skip(offset).take(count).collect())
    }

    fn append_data(&mut self, data: &str) {
        if data.is_empty() {
            return;
        }
        self.set_data(&self.data().unwrap_or_default().add(data));
    }

    fn insert_data(&mut self, offset: usize, data: &str) {
        if data.is_empty() {
            return;
        }
        if let Some(s) = self.data() {
            let mut s_new = String::with_capacity(s.len() + data.len());
            s.chars().into_iter().enumerate().for_each(|(i, c)| {
                if i == offset {
                    s_new.push_str(data);
                }
                s_new.push(c);
            });
            self.set_data(&s_new);
        }
    }

    fn delete_data(&mut self, offset: usize, count: usize) {
        if let Some(s) = self.data() {
            let mut s_new = String::with_capacity(max(0, s.len() - count));
            s.chars().into_iter().enumerate().for_each(|(i, c)| {
                if i < offset || i >= offset + count {
                    s_new.push(c);
                }
            });
            self.set_data(&s_new);
        }
    }

    fn replace_data(&mut self, offset: usize, count: usize, data: &str) {
        if let Some(s) = self.data() {
            let mut s_new = String::with_capacity(max(0, s.len() - count + data.len()));
            s.chars().into_iter().enumerate().for_each(|(i, c)| {
                if i < offset {
                    s_new.push(c);
                }
                if i == offset {
                    s_new.push_str(data);
                }
                if i >= offset + count {
                    s_new.push(c);
                }
            });
            self.set_data(&s_new);
        }
    }
}

pub trait Text: CharacterData {}

pub trait CDataSection: CharacterData {}

pub trait ProcessingInstruction: CharacterData {
    fn target(&self) -> Option<String>;
}

pub trait Comment: CharacterData {}

macro_rules! define_node_type {
    ($Self: ident, $EnumType: ident) => {
        #[derive(Clone, PartialEq, Eq)]
        pub struct $Self {
            pub(super) node_base: NodeBase
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
            fn to_node(&self) -> Node { Node::from(self) }

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

        impl From<&$Self> for Node {
            fn from(value: &$Self) -> Node {
                Node::$EnumType((*value).clone())
            }
        }

        impl<'a> From<&'a $Self> for NodeRef<'a> {
            fn from(value: &'a $Self) -> Self {
                NodeRef::$EnumType(value)
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

// ------------------------------------------- Node impl -------------------------------------------


/// Base DOM node implementation.
#[derive(Clone)]
pub struct NodeBase {
    pub(super) tree: Rc<HTMLDocument>,
    pub(super) node: *mut lxb_dom_node_t,
}

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
    pub(super) fn wrap_node(tree: &Rc<HTMLDocument>, node: *mut lxb_dom_node_t) -> Option<Node> {
        if node.is_null() {
            return None;
        }
        let node_base = Self { tree: tree.clone(), node };
        use crate::third_party::lexbor::lxb_dom_node_type_t::*;
        match unsafe { (*node).type_ } {
            LXB_DOM_NODE_TYPE_ELEMENT => Some(Node::Element(ElementNode { node_base })),
            LXB_DOM_NODE_TYPE_ATTRIBUTE => Some(Node::Attr(AttrNode { node_base })),
            LXB_DOM_NODE_TYPE_TEXT => Some(Node::Text(TextNode { node_base })),
            LXB_DOM_NODE_TYPE_CDATA_SECTION => Some(Node::CDataSection(CDataSectionNode { node_base })),
            LXB_DOM_NODE_TYPE_PROCESSING_INSTRUCTION => Some(Node::ProcessingInstruction(
                ProcessingInstructionNode { node_base })),
            LXB_DOM_NODE_TYPE_COMMENT => Some(Node::Comment(CommentNode { node_base })),
            LXB_DOM_NODE_TYPE_DOCUMENT => Some(Node::Document(DocumentNode { node_base })),
            LXB_DOM_NODE_TYPE_DOCUMENT_TYPE => Some(Node::DocumentType(DocumentTypeNode { node_base })),
            LXB_DOM_NODE_TYPE_DOCUMENT_FRAGMENT => Some(Node::DocumentFragment(
                DocumentFragmentNode { node_base })),
            _ => None
        }
    }

    #[inline]
    pub(super) fn new_base(tree: &Rc<HTMLDocument>, node: *mut lxb_dom_node_t) -> Option<Self> {
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

    pub(super) unsafe fn create_cdata_section_unchecked(doc: &NodeBase, data: &str) -> Result<CDataSectionNode, DOMError> {
        debug_assert_eq!((*doc.node).type_, lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_DOCUMENT);
        let cdata = lxb_dom_document_create_cdata_section(
            doc.node.cast(), data.as_ptr(), data.len());
        if let Some(c) = Self::new_base(&doc.tree, cdata.cast()) {
            Ok(CDataSectionNode { node_base: c })
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

    #[inline]
    unsafe fn can_have_children(&self) -> bool {
        matches!((*self.node).type_,
            lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_ELEMENT
                | lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_DOCUMENT
                | lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_DOCUMENT_FRAGMENT
        )
    }

    unsafe fn insert_before_unchecked<'a>(&mut self, node: &'a Node, child: Option<&Node>) -> Option<&'a Node> {
        if let Some(c) = child {
            if c.parent_node()? != *self || !self.can_have_children() ||  node.contains(c) {
                return None;
            }
            if node == c {
                return Some(node);
            }
            lxb_dom_node_insert_before(c.node, node.node);
            Some(node)
        } else {
            self.append_child_unchecked(node)
        }
    }

    unsafe fn append_child_unchecked<'a>(&mut self, node: &'a Node) -> Option<&'a Node> {
        if self.can_have_children() {
            lxb_dom_node_insert_child(self.node, node.node);
            Some(node)
        } else {
            None
        }
    }

    unsafe fn replace_child_unchecked<'a>(&mut self, node: &'a Node, child: &'a Node) -> Option<&'a Node> {
        if child.parent_node()? != *self || !self.can_have_children() {
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
        if node.parent_node()? != *self || !self.can_have_children() {
            return None;
        }
        lxb_dom_node_remove(node.node);
        Some(node)
    }

    #[inline]
    unsafe fn owner_document_ptr(&self) -> Option<*mut lxb_dom_document_t> {
        let d = unsafe { self.node.as_ref()? }.owner_document;
        if !d.is_null() { Some(d) }
        else { None }
    }

    unsafe fn iter_raw(&self) -> NodeIteratorRaw {
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
        NodeRef::Undefined(self)
    }

    #[inline(always)]
    fn to_node(&self) -> Node {
        Node::from(self)
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
            lxb_dom_document_destroy_text_noi((*self.node).owner_document, t);
        }
        ret_value
    }

    #[inline]
    fn set_text_content(&mut self, content: &str) {
        self.set_node_value(content)
    }

    fn owner_document(&self) -> Option<DocumentNode> {
        check_node!(self);
        let d = unsafe { self.owner_document_ptr()? };
        Some(Self::wrap_node(&self.tree, d.cast())?.into())
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
        // TODO: Insert fragment itself once Lexbor bug is fixed: https://github.com/lexbor/lexbor/issues/180
        if let Node::DocumentFragment(d) = node {
            d.child_nodes().iter().for_each(|c| {
                unsafe { self.append_child_unchecked(&c); }
            });
            // Lexbor doesn't reset the child pointers upon moving elements from DocumentFragments
            // See: https://github.com/lexbor/lexbor/issues/180
            unsafe {
                (*d.node_base.node).first_child = ptr::null_mut();
                (*d.node_base.node).last_child = ptr::null_mut();
            }
            Some(node)
        } else {
            unsafe { self.append_child_unchecked(node) }
        }
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

struct NodeIteratorRaw {
    root: *mut lxb_dom_node_t,
    next_node: *mut lxb_dom_node_t,
}

impl NodeIteratorRaw {
    unsafe fn new(root: *mut lxb_dom_node_t) -> Self {
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
    fn new(root: &'a NodeBase) -> Self {
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
    fn new(root: &'a NodeBase) -> Self {
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
            let doctype = (*self.node_base.owner_document_ptr()?).doctype;
            let base = NodeBase::new_base(&self.node_base.tree, doctype.cast())?;
            Some(DocumentTypeNode { node_base: base })
        }
    }

    fn document_element(&self) -> Option<DocumentNode> {
        check_node!(self.node_base);
        let base = NodeBase::new_base(&self.node_base.tree, self.node_base.node)?;
        Some(DocumentNode { node_base: base })
    }

    fn elements_by_tag_name(&self, qualified_name: &str) -> HTMLCollection {
        check_node!(self.node_base);
        HTMLCollection::new_live(self.into(), Some(Box::new([qualified_name.to_owned()])), |n, qn| {
            unsafe { elements_by_tag_name(n, &qn.unwrap_unchecked()[0]) }
        })
    }

    fn elements_by_class_name(&self, qualified_name: &str) -> HTMLCollection {
        check_node!(self.node_base);
        HTMLCollection::new_live(self.into(), Some(Box::new([qualified_name.to_owned()])), |n, cls| {
            unsafe { elements_by_class_name(n, &cls.unwrap_unchecked()[0]) }
        })
    }

    fn elements_by_attr(&self, qualified_name: &str, value: &str) -> HTMLCollection {
        check_node!(self.node_base);
        HTMLCollection::new_live(self.into(), Some(Box::new([qualified_name.to_owned(), value.to_owned()])), |n, attr| {
            unsafe { elements_by_attr(n, &attr.unwrap_unchecked()[0], &attr.unwrap_unchecked()[1]) }
        })
    }

    fn create_element(&mut self, local_name: &str) -> Result<ElementNode, DOMError> {
        assert!(!self.node_base.node.is_null());
        unsafe { NodeBase::create_element_unchecked(&self.node_base, local_name) }
    }

    fn create_document_fragment(&mut self) -> Result<DocumentFragmentNode, DOMError> {
        assert!(!self.node_base.node.is_null());
        unsafe { NodeBase::create_document_fragment_unchecked(&self.node_base) }
    }

    fn create_text_node(&mut self, data: &str) -> Result<TextNode, DOMError> {
        assert!(!self.node_base.node.is_null());
        unsafe { NodeBase::create_text_node_unchecked(&self.node_base, data) }
    }

    fn create_cdata_section(&mut self, data: &str) -> Result<CDataSectionNode, DOMError> {
        assert!(!self.node_base.node.is_null());
        unsafe { NodeBase::create_cdata_section_unchecked(&self.node_base, data) }
    }

    fn create_comment(&mut self, data: &str) -> Result<CommentNode, DOMError> {
        assert!(!self.node_base.node.is_null());
        unsafe { NodeBase::create_comment_unchecked(&self.node_base, data) }
    }

    fn create_processing_instruction(&mut self, target: &str, data: &str) -> Result<ProcessingInstructionNode, DOMError> {
        assert!(!self.node_base.node.is_null());
        unsafe { NodeBase::create_processing_instruction_unchecked(&self.node_base, target, data) }
    }

    fn create_attribute(&mut self, local_name: &str) -> Result<AttrNode, DOMError> {
        assert!(!self.node_base.node.is_null());
        unsafe { NodeBase::create_attribute_unchecked(&self.node_base, local_name) }
    }
}

impl DocumentOrShadowRoot for DocumentNode {}

impl ParentNode for DocumentNode {
    fn children(&self) -> HTMLCollection {
        HTMLCollection::new_live(self.into(), None, |n, _| {
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
        HTMLCollection::new_live(self.into(), None, |n, _| {
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
        NamedNodeMap::new_live(self.into(), None, |n, _| {
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
        if self.parent_node().is_none() {
            return Ok(None);
        }
        let sel_list = CSSSelectorList::parse_selectors(
            &self.upcast().tree, selectors)?;
        let mut result = Vec::<ElementNode>::with_capacity(1);
        sel_list.match_elements_reverse(self.as_noderef(), |node, _, ctx| {
            ctx.push(node);
            TraverseAction::Stop
        }, &mut result);
        Ok(result.pop())
    }

    fn matches(&self, selectors: &str) -> Result<bool, CSSParserError> {
        let doc = self.owner_document();
        let root = if doc.is_some() { doc.as_ref().unwrap().as_noderef() } else { self.as_noderef() };

        let sel_list = CSSSelectorList::parse_selectors(&self.upcast().tree, selectors)?;
        let mut ctx = (false, self.as_noderef());
        sel_list.match_elements(root, |e, _, ctx| {
            if e.as_noderef() == ctx.1 {
                ctx.0 = true;
                TraverseAction::Stop
            } else {
                TraverseAction::Ok
            }
        }, &mut ctx);
        Ok(ctx.0)
    }

    fn elements_by_tag_name(&self, qualified_name: &str) -> HTMLCollection {
        HTMLCollection::new_live(self.into(), Some(Box::new([qualified_name.to_owned()])), |n, qn| {
            unsafe { elements_by_tag_name(&n, &qn.unwrap_unchecked()[0]) }
        })
    }

    fn elements_by_class_name(&self, class_names: &str) -> HTMLCollection {
        HTMLCollection::new_live(self.into(), Some(Box::new([class_names.to_owned()])), |n, cls| {
            unsafe { elements_by_class_name(&n, &cls.unwrap_unchecked()[0]) }
        })
    }

    fn elements_by_attr(&self, qualified_name: &str, value: &str) -> HTMLCollection {
        HTMLCollection::new_live(self.into(), Some(Box::new([qualified_name.to_owned(), value.to_owned()])), |n, attr| {
            unsafe { elements_by_attr(&n, &attr.unwrap_unchecked()[0], &attr.unwrap_unchecked()[1]) }
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
            lexbor_str_destroy(html_str, (*(*self.node_base.node).owner_document).text, true);
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
        HTMLCollection::new_live(self.into(), None, |n, _| {
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


define_node_type!(AttrNode, Attr);

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

    fn owner_element(&self) -> Option<Node> {
        check_node!(self.node_base);
        unsafe {
            let attr = self.node_base.node as *mut lxb_dom_attr_t;
            if attr.is_null() || (*attr).owner.is_null() {
                return None;
            }
            NodeBase::wrap_node(&self.node_base.tree, (*attr).owner.cast())
        }
    }
}


// -------------------------------------------- Text impl ------------------------------------------


define_node_type!(TextNode, Text);

impl Text for TextNode {}

impl CharacterData for TextNode {}

impl ChildNode for TextNode {}

impl NonDocumentTypeChildNode for TextNode {}


// ---------------------------------------- CDataSection impl --------------------------------------


define_node_type!(CDataSectionNode, CDataSection);

impl CDataSection for CDataSectionNode {}

impl CharacterData for CDataSectionNode {}

impl ChildNode for CDataSectionNode {}

impl NonDocumentTypeChildNode for CDataSectionNode {}


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


// --------------------------------- NodeList / HTMLCollection impl --------------------------------


#[derive(Clone)]
struct NodeListClosure<T> {
    n: Node,
    d: Option<Box<[String]>>,
    f: fn(&Node, Option<&Box<[String]>>) -> Vec<T>
}

pub struct NodeListGeneric<T> {
    live: Option<NodeListClosure<T>>,
    items: Vec<T>,
}

impl<T> Default for NodeListGeneric<T> {
    fn default() -> Self {
        NodeListGeneric { live: None, items: Vec::default() }
    }
}

impl<T: Clone> From<&[T]> for NodeListGeneric<T>  {
    fn from(items: &[T]) -> Self {
        Self { live: None, items: Vec::from(items) }
    }
}

impl<T: Clone> From<Vec<T>> for NodeListGeneric<T>  {
    fn from(items: Vec<T>) -> Self {
        Self { live: None, items }
    }
}

impl<'a, T: Clone> NodeListGeneric<T> {
    pub(super) fn new_live(node: Node, user_data: Option<Box<[String]>>,
                           f: fn(&Node, Option<&Box<[String]>>) -> Vec<T>) -> Self {
        Self { live: Some(NodeListClosure { n: node, d: user_data, f }),
            items: Vec::default() }
    }

    pub fn iter(&self) -> vec::IntoIter<T> {
        if let Some(closure) = &self.live {
            (closure.f)(&closure.n, closure.d.as_ref()).into_iter()
        } else {
            self.items.clone().into_iter()
        }
    }

    #[inline]
    pub fn item(&self, index: usize) -> Option<T> {
        Some(self.iter().skip(index).next()?)
    }

    #[inline]
    pub fn items(&self) -> Vec<T> {
        self.iter().collect()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.iter().count()
    }
}

impl<T: Clone> IntoIterator for &NodeListGeneric<T> {
    type Item = T;
    type IntoIter = vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T: Clone + Debug> Debug for NodeListGeneric<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for (i, n) in self.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            Debug::fmt(&n, f)?;
        };
        write!(f, "]")
    }
}

type NodeList = NodeListGeneric<Node>;
type ElementNodeList = NodeListGeneric<ElementNode>;
type HTMLCollection = NodeListGeneric<ElementNode>;
type NamedNodeMap = NodeListGeneric<AttrNode>;

impl HTMLCollection {
    pub fn named_item(&self, name: &str) -> Option<ElementNode> {
        self.iter()
            .find(|e| {
                e.id()
                    .filter(|i| i == name).is_some() || e.attribute("name")
                    .filter(|n| n == name).is_some()
            })
    }

    pub fn elements_by_tag_name(&self, qualified_name: &str) -> HTMLCollection {
        let mut coll = Vec::default();
        self.iter().for_each(|e| {
            coll.append(&mut e.elements_by_tag_name(qualified_name).iter().collect())
        });
        HTMLCollection::from(coll)
    }

    pub fn elements_by_class_name(&self, class_names: &str) -> HTMLCollection {
        let mut coll = Vec::default();
        self.iter().for_each(|e| {
            coll.append(&mut e.elements_by_class_name(class_names).iter().collect())
        });
        HTMLCollection::from(coll)
    }

    pub fn elements_by_attr(&self, qualified_name: &str, value: &str) -> HTMLCollection {
        let mut coll = Vec::default();
        self.iter().for_each(|e| {
            coll.append(&mut e.elements_by_attr(qualified_name, value).iter().collect())
        });
        HTMLCollection::from(coll)
    }
}

impl ElementNodeList {
     pub fn query_selector(&self, selectors: &str) -> Result<Option<ElementNode>, CSSParserError> {
        for item in self.iter() {
            match item.query_selector(selectors) {
                Ok(Some(e)) => return Ok(Some(e)),
                Ok(None) => continue,
                Err(e) => return Err(e)
            }
        }
        Ok(None)
    }

    pub fn query_selector_all(&self, selectors: &str) -> Result<ElementNodeList, CSSParserError> {
        let mut coll = Vec::default();
        for item in self.iter() {
            match item.query_selector_all(selectors) {
                Ok(e) => coll.append(&mut e.iter().collect()),
                Err(e) => return Err(e)
            }
        }
        Ok(ElementNodeList::from(coll))
    }

    pub fn matches(&self, selectors: &str) -> Result<bool, CSSParserError> {
        for item in self.iter() {
            match item.matches(selectors) {
                Ok(true) => return Ok(true),
                Ok(false) => continue,
                Err(e) => return Err(e)
            }
        }
        Ok(false)
    }
}

// ---------------------------------------- DOMTokenList impl --------------------------------------


pub trait DOMTokenListInterface: IntoIterator + PartialEq + Debug + Display + PartialEq {
    fn value(&self) -> String;

    fn values(&self) -> Vec<String> {
        let mut h = HashSet::new();
        self.value().split_ascii_whitespace()
            .filter(|&v| h.insert(v))
            .map(String::from)
            .collect()
    }

    fn item(&self, index: usize) -> Option<String> {
        Some(self.values().get(index)?.to_owned())
    }

    fn contains(&self, token: &str) -> bool {
        self.iter().find(|s: &String| s == token).is_some()
    }

    #[inline]
    fn iter(&self) -> vec::IntoIter<String> {
        self.values().into_iter()
    }

    #[inline]
    fn len(&self) -> usize {
        self.values().len()
    }
}

// Cannot derive default implementations for sub traits (yet)
macro_rules! dom_node_list_impl {
    ($Self: ident) => {
        impl DOMTokenListInterface for $Self<'_> {
            #[inline]
            fn value(&self) -> String {
                self.element.class_name().unwrap_or_default()
            }
        }

        impl IntoIterator for $Self<'_>  {
            type Item = String;
            type IntoIter = vec::IntoIter<String>;

            fn into_iter(self) -> Self::IntoIter {
                self.iter()
            }
        }

        impl PartialEq<&[&str]> for $Self<'_> {
            fn eq(&self, other: &&[&str]) -> bool {
                let val = self.values();
                if other.len() != val.len() {
                    return false;
                }
                val.iter().zip(other.iter()).find(|(a, b)| a != *b).is_none()
            }
        }

        impl Debug for $Self<'_> {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                write!(f, "[")?;
                for (i, s) in self.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    f.write_str(&format!("{:?}", s))?;
                };
                write!(f, "]")
            }
        }

        impl Display for $Self<'_> {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                Debug::fmt(self, f)
            }
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct DOMTokenList<'a> {
    element: &'a ElementNode,
}

impl<'a> DOMTokenList<'a> {
    fn new(element: &'a ElementNode) -> Self {
        Self { element }
    }
}

dom_node_list_impl!(DOMTokenList);

#[derive(PartialEq)]
pub struct DOMTokenListMut<'a> {
    element: &'a mut ElementNode
}

impl<'a> DOMTokenListMut<'a> {
    fn new(element: &'a mut ElementNode) -> Self {
        Self { element }
    }

    fn update_node(&mut self, values: &Vec<String>) {
        self.element.set_class_name(&values.join(" "));
    }

    pub fn add(&mut self, tokens: &[&str]) {
        let mut values = self.values();
        tokens.iter().for_each(|t: &&str| {
            let t_owned = (*t).to_owned();
            if !values.contains(&t_owned) {
                values.push(t_owned);
            }
        });
        self.update_node(&values);
    }

    pub fn remove(&mut self, tokens: &[&str]) {
        self.update_node(&self
            .iter()
            .filter(|t: &String| !tokens.contains(&t.as_str()))
            .collect()
        );
    }

    pub fn replace(&mut self, old_token: &str, new_token: &str) -> bool {
        let mut repl = false;
        self.update_node(&self
            .iter()
            .map(|t: String| {
                if t == old_token {
                    repl = true;
                    new_token.to_owned()
                } else { t }
            })
            .collect()
        );
        repl
    }

    pub fn toggle(&mut self, token: &str, force: Option<bool>) -> bool {
        if let Some(f) = force {
            if f {
                self.add(&[token]);
            } else {
                self.remove(&[token])
            }
            return f;
        }

        if self.contains(token) {
            self.remove(&[token]);
            false
        } else {
            self.add(&[token]);
            true
        }
    }
}

dom_node_list_impl!(DOMTokenListMut);


// --------------------------------------------- Helpers -------------------------------------------


unsafe fn element_by_id(node: &NodeBase, id: &str) -> Option<ElementNode> {
    let coll = lxb_dom_collection_create((*node.node).owner_document);
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
    let coll = lxb_dom_collection_create((*node.node).owner_document);
    if coll.is_null() {
        return Vec::default();
    }
    lxb_dom_elements_by_attr(node.node as *mut lxb_dom_element_t, coll, qualified_name.as_ptr(),
                             qualified_name.len(), value.as_ptr(), value.len(), false);
    dom_coll_to_vec(&node.tree, coll, true)
}

unsafe fn elements_by_tag_name(node: &NodeBase, qualified_name: &str) -> Vec<ElementNode> {
    let coll = lxb_dom_collection_create((*node.node).owner_document);
    if coll.is_null() {
        return Vec::default();
    }
    lxb_dom_node_by_tag_name(node.node, coll, qualified_name.as_ptr(), qualified_name.len());
    dom_coll_to_vec(&node.tree, coll, true)
}

unsafe fn elements_by_class_name(node: &NodeBase, class_name: &str) -> Vec<ElementNode> {
    let coll = lxb_dom_collection_create((*node.node).owner_document);
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

pub(super) unsafe fn str_from_lxb_char_t<'a>(cdata: *const lxb_char_t, size: usize) -> Option<&'a str> {
    if !cdata.is_null() {
        Some(std::str::from_utf8_unchecked(slice::from_raw_parts(cdata, size)))
    } else {
        None
    }
}

#[inline]
pub(super) unsafe fn str_from_lxb_str_t<'a>(s: *const lexbor_str_t) -> Option<&'a str> {
    str_from_lxb_char_t((*s).data, (*s).length)
}

#[inline]
pub(super) unsafe fn str_from_dom_node<'a>(node: *const lxb_dom_node_t) -> Option<&'a str> {
    let cdata = node as *const lxb_dom_character_data_t;
    str_from_lxb_str_t(addr_of!((*cdata).data))
}

pub(super) unsafe fn str_from_lxb_str_cb<'a, Node, Fn>(
    node: *mut Node, lxb_fn: unsafe extern "C" fn(*mut Fn, *mut usize) -> *const lxb_char_t) -> Option<&'a str> {
    if node.is_null() {
        return None;
    }
    let mut size = 0;
    let name = lxb_fn(node.cast(), addr_of_mut!(size));
    if name.is_null() {
        None
    } else {
        str_from_lxb_char_t(name, size)
    }
}
