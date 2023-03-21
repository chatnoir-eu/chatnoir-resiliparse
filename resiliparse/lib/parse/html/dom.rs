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

#![allow(dead_code)]

use std::{ptr, slice, vec};
use std::collections::{HashSet};
use std::ops::{Deref, DerefMut};
use std::ptr::{addr_of, addr_of_mut};
use std::rc::{Rc, Weak};

use crate::third_party::lexbor::*;
use super::serialize::node_format_visible_text;
use super::tree::{HTMLTreeRc};

#[derive(Clone, PartialEq, Eq)]
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

impl PartialEq<NodeBase> for Node {
    fn eq(&self, other: &NodeBase) -> bool {
        **self == *other
    }
}

impl Deref for Node {
    type Target = NodeBase;

    fn deref(&self) -> &Self::Target {
        match &self {
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

macro_rules! check_node {
    ($node: expr) => {
        if $node.tree.upgrade().is_none() || $node.node.is_null() {
            return Default::default();
        }
    }
}

macro_rules! check_nodes {
    ($node1: expr, $node2: expr) => {
        {
            let t1 = $node1.tree.upgrade();
            let t2 = $node2.tree.upgrade();
            if t1.is_none() || t2.is_none() || !Rc::ptr_eq(&t1.unwrap(), &t2.unwrap())
                || $node1.node.is_null() || $node2.node.is_null() || $node1 == $node2 {
                return Default::default();
            }
        }
    }
}

/// Base DOM node.
pub trait NodeInterface {
    unsafe fn node_name_unchecked(&self) -> Option<&str>;
    unsafe fn node_value_unchecked(&self) -> Option<&str>;

    fn upcast(&self) -> &NodeBase;
    fn upcast_mut(&mut self) -> &mut NodeBase;
    fn as_node(&self) -> Node;

    fn node_name(&self) -> Option<String>;
    fn node_value(&self) -> Option<String>;
    fn text_content(&self) -> Option<String>;

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

    fn insert_before<'a>(&self, node: &'a Node, child: Option<&Node>) -> Option<&'a Node>;
    fn append_child<'a>(&self, node: &'a Node) -> Option<&'a Node>;
    fn replace_child<'a>(&self, node: &'a Node, child: &Node) -> Option<&'a Node>;
    fn remove_child<'a>(&self, node: &'a Node) -> Option<&'a Node>;

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

    fn elements_by_tag_name(&self) -> HTMLCollection;
    fn elements_by_class_name(&self) -> HTMLCollection;

    fn create_element(&mut self, local_name: &str) -> Option<ElementNode>;
    fn create_text_node(&mut self, data: &str) -> Option<TextNode>;
    fn create_cdata_section(&mut self, data: &str) -> Option<CDataSectionNode>;
    fn create_comment(&mut self, data: &str) -> Option<CommentNode>;
    fn create_attribute(&mut self, local_name: &str) -> Option<AttrNode>;
}

pub trait DocumentFragment: DocumentOrShadowRoot + ParentNode + NonElementParentNode {}

/// ParentNode mixin trait.
pub trait ParentNode: NodeInterface {
    fn children(&self) -> HTMLCollection;
    fn first_element_child(&self) -> Option<Node>;
    fn last_element_child(&self) -> Option<Node>;
    fn child_element_count(&self) -> usize;
    fn prepend(&mut self, nodes: &[&Node]);
    fn append(&mut self, nodes: &[&Node]);
    fn replace_children(&mut self, nodes: &[&Node]);

    fn query_selector(&self, selectors: &str) -> Option<Node>;
    fn query_selector_all(&self, selectors: &str) -> NodeList;
}

/// NonElementParentNode mixin trait.
pub trait NonElementParentNode: NodeInterface {
    fn get_element_by_id(element_id: &str) -> Option<Node>;
}

/// ChildNode mixin trait.
pub trait ChildNode: NodeInterface {
    fn before(&mut self, nodes: &[&Node]) {
        if let Some(p) = &self.parent_node() {
            let anchor = Some(self.as_node());
            nodes.iter().for_each(|&n| {
                p.insert_before(n, anchor.as_ref());
            });
        }
    }

    fn after(&mut self, nodes: &[&Node]) {
        if let Some(p) = &self.parent_node() {
            if let Some(next) = self.next_sibling() {
                let anchor = Some(next);
                nodes.iter().for_each(|&n| {
                    p.insert_before(n, anchor.as_ref());
                });
            } else {
                nodes.iter().for_each(|&n| {
                    p.append_child(n);
                });
            }
        }
    }

    fn replace_with(&mut self, nodes: &[&Node]) {
        self.before(nodes);
        self.remove();
    }

    fn remove(&mut self) {
        let node = self.upcast_mut();
        check_node!(node);
        unsafe { lxb_dom_node_remove(node.node); }
        node.node = ptr::null_mut();
        node.tree = Weak::default();
    }
}

/// NonDocumentTypeChildNode mixin trait.
pub trait NonDocumentTypeChildNode: NodeInterface {
    /// Previous sibling element node.
    fn previous_element_sibling(&self) -> Option<Node> {
        loop {
            if let Node::Element(s) = self.previous_sibling()? {
                return Some(s.into());
            }
        }
    }

    /// Next sibling element node.
    fn next_element_sibling(&self) -> Option<Node> {
        loop {
            if let Node::Element(s) = self.next_sibling()? {
                return Some(s.into());
            }
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
    fn class_name(&self) -> Option<String>;
    fn class_list(&mut self) -> DOMTokenList;

    fn attribute(&self, qualified_name: &str) -> Option<String>;
    fn attribute_node(&self, qualified_name: &str) -> Option<AttrNode>;
    fn attribute_names(&self) -> Vec<String>;
    fn set_attribute(&mut self, qualified_name: &str, value: &str);
    fn remove_attribute(&mut self, qualified_name: &str);
    fn toggle_attribute(&mut self, qualified_name: &str, force: Option<bool>);
    fn has_attribute(&self, qualified_name: &str) -> bool;

    fn closest(&self, selectors: &str) -> Option<ElementNode>;
    fn matches(&self, selectors: &str) -> bool;
    fn elements_by_tag_name(&self, qualified_name: &str) -> HTMLCollection;
    fn elements_by_class_name(&self, class_names: &str) -> HTMLCollection;

    fn inner_html(&self) -> String;
    fn outer_html(&self) -> String;
    fn inner_text(&self) -> String;
    fn outer_text(&self) -> String;
}

pub trait Attr: NodeInterface {
    unsafe fn name_unchecked(&self) -> Option<&str>;
    unsafe fn local_name_unchecked(&self) -> Option<&str>;
    unsafe fn value_unchecked(&self) -> Option<&str>;

    fn local_name(&self) -> Option<String>;
    fn name(&self) -> Option<String>;
    fn value(&self) -> Option<String>;

    fn owner_element(&self) -> Option<Node>;
}

pub trait CharacterData: NodeInterface + ChildNode + NonDocumentTypeChildNode {
    fn len(&self) -> usize {
        self.node_value().unwrap_or_else(|| String::new()).len()
    }
    #[inline]
    fn data(&self) -> Option<String> {
        self.node_value()
    }
    fn substring_data(&self, offset: usize, count: usize) -> Option<String> {
        Some(String::from(&self.data()?[offset..offset + count]))
    }
    fn append_data(&self, data: &str) {
        todo!()
    }
    fn insert_data(&self, offset: usize, data: &str) {
        todo!()
    }
    fn delete_data(&self, offset: usize, count: usize) {
        todo!()
    }
    fn replace_data(&self, offset: usize, count: usize, data:& str) {
        todo!()
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
            node_base: NodeBase
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
            fn as_node(&self) -> Node { Node::$EnumType(self.clone()) }

            #[inline(always)]
            fn node_name(&self) -> Option<String> { self.node_base.node_name() }
            #[inline(always)]
            fn node_value(&self) -> Option<String> { self.node_base.node_value() }
            #[inline(always)]
            fn text_content(&self) -> Option<String> { self.node_base.text_content() }

            #[inline(always)]
            fn owner_document(&self) -> Option<DocumentNode> { self.node_base.owner_document() }
            #[inline(always)]
            fn parent_node(&self) -> Option<Node> { self.node_base.parent_node() }
            #[inline(always)]
            fn parent_element(&self) -> Option<ElementNode> { self.node_base.parent_element() }

            #[inline(always)]
            fn has_child_nodes(&self) -> bool { self.node_base.has_child_nodes() }
            #[inline(always)]
            fn contains(&self, node: &Node) -> bool { self.node_base.contains(&node) }
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
            fn insert_before<'a>(&self, node: &'a Node, child: Option<&Node>) -> Option<&'a Node> {
                self.node_base.insert_before(&node, child) }
            #[inline(always)]
            fn append_child<'a>(&self, node: &'a Node) -> Option<&'a Node> {
                self.node_base.append_child(&node) }
            #[inline(always)]
            fn replace_child<'a>(&self, node: &'a Node, child: &Node) -> Option<&'a Node> {
                self.node_base.replace_child(&node, &child) }
            #[inline(always)]
            fn remove_child<'a>(&self, node: &'a Node) -> Option<&'a Node> {
                self.node_base.remove_child(&node) }

            #[inline(always)]
            fn iter(&self) -> NodeIterator { self.node_base.iter() }
            #[inline(always)]
            fn iter_elements(&self) -> ElementIterator { self.node_base.iter_elements() }
        }

        // impl Deref for $Self {
        //     type Target = NodeBase;
        //
        //     #[inline]
        //     fn deref(&self) -> &Self::Target {
        //         &self.node_base
        //     }
        // }
        //
        // impl DerefMut for $Self {
        //     #[inline]
        //     fn deref_mut(&mut self) -> &mut Self::Target {
        //         &mut self.node_base
        //     }
        // }

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
    }
}

// ------------------------------------------- Node impl -------------------------------------------


/// Base DOM node implementation.
#[derive(Clone)]
pub struct NodeBase {
    tree: Weak<HTMLTreeRc>,
    node: *mut lxb_dom_node_t,
}

impl PartialEq<NodeBase> for NodeBase {
    fn eq(&self, other: &Self) -> bool {
        self.node == other.node
    }
}

impl PartialEq<Node> for NodeBase {
    fn eq(&self, other: &Node) -> bool {
        self.node == (*other).node
    }
}

impl Eq for NodeBase {}

impl NodeBase {
    pub(super) fn create_node(tree: &Rc<HTMLTreeRc>, node: *mut lxb_dom_node_t) -> Option<Node> {
        if node.is_null() {
            return None;
        }
        let node_base = Self { tree: Rc::downgrade(tree), node };
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

    fn serialize_node(node: &Self) -> Option<String> {
        check_node!(node);

        let out_html;
        unsafe {
            let s = lexbor_str_create();
            lxb_html_serialize_tree_str(node.node, s);
            out_html = str_from_lxb_str_t(s).to_string();
            lexbor_str_destroy(s, node.node.as_ref()?.owner_document.as_ref()?.text, true);
        }
        Some(out_html)
    }

    unsafe fn insert_before_unchecked<'a>(&self, node: &'a Node, child: Option<&Node>) -> Option<&'a Node> {
        if let Some(c) = child {
            if c.parent_node()? != *self || node.contains(c) {
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

    unsafe fn append_child_unchecked<'a>(&self, node: &'a Node) -> Option<&'a Node> {
        lxb_dom_node_insert_child(self.node, node.node);
        Some(node)
    }

    unsafe fn replace_child_unchecked<'a>(&self, node: &'a Node, child: &Node) -> Option<&'a Node> {
        if child.parent_node()? != *self {
            return None;
        }
        if node == child {
            return Some(node);
        }
        self.insert_before_unchecked(node, Some(child))?;
        self.remove_child_unchecked(child)?;
        Some(node)
    }

    unsafe fn remove_child_unchecked<'a>(&self, node: &'a Node) -> Option<&'a Node> {
        if node.parent_node()? != *self {
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
        let cdata = self.node as *const lxb_dom_character_data_t;
        Some(str_from_lxb_str_t(addr_of!((*cdata).data)))
    }

    fn upcast(&self) -> &NodeBase {
        self
    }

    fn upcast_mut(&mut self) -> &mut NodeBase {
        self
    }

    fn as_node(&self) -> Node {
        NodeBase::create_node(&self.tree.upgrade().unwrap(), self.node).unwrap()
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

    /// Text contents of this DOM node and its children.
    fn text_content(&self) -> Option<String> {
        check_node!(self);

        let out_text;
        unsafe {
            let mut l = 0;
            let t = lxb_dom_node_text_content(self.node, &mut l);
            out_text = str_from_lxb_char_t(t, l).to_string();
            lxb_dom_document_destroy_text_noi(self.node.as_ref()?.owner_document, t);
        }
        Some(out_text)
    }

    fn owner_document(&self) -> Option<DocumentNode> {
        check_node!(self);
        let d = unsafe { self.owner_document_ptr()? };
        Some(Self::create_node(&self.tree.upgrade()?, d.cast())?.into())
    }

    /// Parent of this node.
    fn parent_node(&self) -> Option<Node> {
        Self::create_node(&self.tree.upgrade()?, unsafe { self.node.as_ref()?.parent })
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
        NodeListGeneric::new_live(&self, |s: &Self| {
            let mut nodes = Vec::new();
            let mut child = s.first_child();
            while let Some(c) = child {
                child = c.next_sibling();
                nodes.push(c);
            }
            nodes
        })
    }

    /// First child element of this DOM node.
    fn first_child(&self) -> Option<Node> {
        Self::create_node(&self.tree.upgrade()?, unsafe { self.node.as_ref()?.first_child })
    }

    /// Last child element of this DOM node.
    fn last_child(&self) -> Option<Node> {
        Self::create_node(&self.tree.upgrade()?, unsafe { self.node.as_ref()?.last_child })
    }

    /// Previous sibling node.
    fn previous_sibling(&self) -> Option<Node> {
        Self::create_node(&self.tree.upgrade()?, unsafe { self.node.as_ref()?.prev })
    }

    /// Next sibling node.
    fn next_sibling(&self) -> Option<Node> {
        Self::create_node(&self.tree.upgrade()?, unsafe { self.node.as_ref()?.next })
    }

    fn clone_node(&self, deep: bool) -> Option<Node> {
        check_node!(self);
        Self::create_node(&self.tree.upgrade()?, unsafe { lxb_dom_node_clone(self.node, deep) })
    }

    fn insert_before<'a>(&self, node: &'a Node, child: Option<&Node>) -> Option<&'a Node> {
        check_nodes!(self, node);
        if child.is_some() {
            check_nodes!(node, child.unwrap());
            if self == child.unwrap() {
                return None;
            }
        }
        unsafe { self.insert_before_unchecked(&node, child) }
    }

    fn append_child<'a>(&self, node: &'a Node) -> Option<&'a Node> {
        check_nodes!(self, node);
        unsafe { self.append_child_unchecked(&node) }
    }

    fn replace_child<'a>(&self, node: &'a Node, child: &Node) -> Option<&'a Node> {
        check_nodes!(self, node);
        check_nodes!(node, child);
        unsafe { self.replace_child_unchecked(&node, &child) }
    }

    fn remove_child<'a>(&self, node: &'a Node) -> Option<&'a Node> {
        check_nodes!(self, node);
        unsafe { self.remove_child_unchecked(&node) }
    }

    fn iter(&self) -> NodeIterator {
        NodeIterator::new(&self)
    }

    fn iter_elements(&self) -> ElementIterator {
        ElementIterator::new(&self)
    }

    // /// Visible text contents of this DOM node and its children.
    // #[inline]
    // fn outer_text(&self) -> Option<String> {
    //     self.inner_text()
    // }
    //
    // /// Visible text contents of this DOM node and its children.
    // #[inline]
    // fn inner_text(&self) -> Option<String> {
    //     self.tree.upgrade()?;
    //     match self.node_type() {
    //         NodeType::Element => Some(node_format_visible_text(self.node)),
    //         _ => None
    //     }
    // }
    //
    // /// Outer HTML of this DOM node and its children.
    // #[inline]
    // fn outer_html(&self) -> Option<String> {
    //     Self::serialize_node(self)
    // }
    //
    // /// Inner HTML of this DOM node's children.
    // fn inner_html(&self) -> Option<String> {
    //     self.child_nodes()
    //         .into_iter()
    //         .flat_map(|c| Self::serialize_node(&c))
    //         .reduce(|a, b| a + &b)
    // }
}

struct NodeIteratorRaw {
    root: *mut lxb_dom_node_t,
    node: *mut lxb_dom_node_t,
}

impl NodeIteratorRaw {
    unsafe fn new(root: *mut lxb_dom_node_t) -> Self {
        if root.is_null() || unsafe { (*root).first_child }.is_null() {
            Self { root: ptr::null_mut(), node: ptr::null_mut() }
        } else {
            Self { root, node: unsafe { (*root).first_child } }
        }
    }
}

impl Iterator for NodeIteratorRaw {
    type Item = *mut lxb_dom_node_t;

    fn next(&mut self) -> Option<Self::Item> {
        if self.node.is_null() {
            return None;
        }

        let return_node = self.node;
        unsafe {
            if !(*self.node).first_child.is_null() {
                self.node = (*self.node).first_child;
            } else {
                while self.node != self.root && !(*self.node).next.is_null() {
                    self.node = (*self.node).parent;
                }
                if self.node == self.root {
                    return None;
                }
                self.node = (*self.node).next;
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
        NodeBase::create_node(&self.root.tree.upgrade()?, self.iterator_raw.next()?)
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
        let tree = &self.root.tree.upgrade()?;
        while let next = unsafe { self.iterator_raw.next()?.as_ref()? } {
            if next.type_ != lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_ELEMENT {
                continue
            }
            if let Some(Node::Element(e)) = NodeBase::create_node(tree, self.iterator_raw.next()?) {
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
            let tree = self.node_base.tree.upgrade()?;
            let doctype = (*self.node_base.owner_document_ptr()?).doctype;
            Some(NodeBase::create_node(&tree, doctype.cast())?.into())
        }
    }

    fn document_element(&self) -> Option<DocumentNode> {
        check_node!(self.node_base);
        unsafe {
            Some(NodeBase::create_node(&self.node_base.tree.upgrade()?, self.node_base.owner_document_ptr()?.cast())?.into())
        }
    }

    fn elements_by_tag_name(&self) -> HTMLCollection {
        todo!()
    }

    fn elements_by_class_name(&self) -> HTMLCollection {
        todo!()
    }

    fn create_element(&mut self, local_name: &str) -> Option<ElementNode> {
        check_node!(self.node_base);
        let element = unsafe {
            lxb_dom_document_create_element(
                self.node_base.owner_document_ptr()?, local_name.as_ptr(), local_name.len(), ptr::null_mut())
        };
        Some(NodeBase::create_node(&self.node_base.tree.upgrade()?, element.cast())?.into())
    }

    fn create_text_node(&mut self, data: &str) -> Option<TextNode> {
        check_node!(self.node_base);
        let text = unsafe {
            lxb_dom_document_create_text_node(self.node_base.owner_document_ptr()?, data.as_ptr(), data.len())
        };
        Some(NodeBase::create_node(&self.node_base.tree.upgrade()?, text.cast())?.into())
    }

    fn create_cdata_section(&mut self, data: &str) -> Option<CDataSectionNode> {
        check_node!(self.node_base);
        let cdata = unsafe {
            lxb_dom_document_create_cdata_section(self.node_base.owner_document_ptr()?, data.as_ptr(), data.len())
        };
        Some(NodeBase::create_node(&self.node_base.tree.upgrade()?, cdata.cast())?.into())
    }

    fn create_comment(&mut self, data: &str) -> Option<CommentNode> {
        check_node!(self.node_base);
        let comment = unsafe {
            lxb_dom_document_create_comment(self.node_base.owner_document_ptr()?, data.as_ptr(), data.len())
        };
        Some(NodeBase::create_node(&self.node_base.tree.upgrade()?, comment.cast())?.into())
    }

    fn create_attribute(&mut self, local_name: &str) -> Option<AttrNode> {
        check_node!(self.node_base);
        let attr = unsafe { lxb_dom_attr_interface_create(self.node_base.owner_document_ptr()?) };
        if attr.is_null() {
            return None;
        }
        let status = unsafe {
            lxb_dom_attr_set_name(attr, local_name.as_ptr(), local_name.len(), true)
        };
        if status != lexbor_status_t::LXB_STATUS_OK {
            unsafe { lxb_dom_attr_interface_destroy(attr); }
            return None;
        }
        Some(NodeBase::create_node(&self.node_base.tree.upgrade()?, attr.cast())?.into())
    }
}

impl DocumentOrShadowRoot for DocumentNode {}

impl ParentNode for DocumentNode {
    fn children(&self) -> HTMLCollection {
        // if let Some(d) = self.document_element() {
        //     d.children()
        // } else {
        //     HTMLCollection::default()
        // }
        HTMLCollection::default()
    }

    #[inline]
    fn first_element_child(&self) -> Option<Node> {
        self.document_element()?.first_element_child()
    }

    #[inline]
    fn last_element_child(&self) -> Option<Node> {
        self.document_element()?.last_element_child()
    }

    #[inline]
    fn child_element_count(&self) -> usize {
        if let Some(d) = self.document_element() {
            d.child_element_count()
        } else {
            0
        }
    }

    #[inline]
    fn prepend(&mut self, nodes: &[&Node]) {
        if let Some(mut d) = self.document_element() {
            d.prepend(nodes)
        }
    }

    #[inline]
    fn append(&mut self, nodes: &[&Node]) {
        if let Some(mut d) = self.document_element() {
            d.append(nodes)
        }
    }

    #[inline]
    fn replace_children(&mut self, nodes: &[&Node]) {
        if let Some(mut d) = self.document_element() {
            d.replace_children(nodes)
        }
    }

    #[inline]
    fn query_selector(&self, selectors: &str) -> Option<Node> {
        self.document_element()?.query_selector(selectors)
    }

    #[inline]
    fn query_selector_all(&self, selectors: &str) -> NodeList {
        // if let Some(d) = self.document_element() {
        //     d.query_selector_all(selectors)
        // } else {
        //     NodeListGeneric::default()
        // }
        NodeListGeneric::default()
    }
}

impl NonElementParentNode for DocumentNode {
    fn get_element_by_id(element_id: &str) -> Option<Node> {
        todo!()
    }
}



// ------------------------------------- DocumentFragment impl -------------------------------------


define_node_type!(DocumentFragmentNode, DocumentFragment);

impl DocumentFragment for DocumentFragmentNode {}

impl DocumentOrShadowRoot for DocumentFragmentNode {}

impl ParentNode for DocumentFragmentNode {
    fn children(&self) -> HTMLCollection {
        todo!()
    }

    fn first_element_child(&self) -> Option<Node> {
        todo!()
    }

    fn last_element_child(&self) -> Option<Node> {
        todo!()
    }

    fn child_element_count(&self) -> usize {
        todo!()
    }

    fn prepend(&mut self, nodes: &[&Node]) {
        todo!()
    }

    fn append(&mut self, nodes: &[&Node]) {
        todo!()
    }

    fn replace_children(&mut self, nodes: &[&Node]) {
        todo!()
    }

    fn query_selector(&self, selectors: &str) -> Option<Node> {
        todo!()
    }

    fn query_selector_all(&self, selectors: &str) -> NodeList {
        todo!()
    }
}

impl NonElementParentNode for DocumentFragmentNode {
    fn get_element_by_id(element_id: &str) -> Option<Node> {
        todo!()
    }
}


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
        match size {
            0 => None,
            _ => Some(str_from_lxb_char_t(name, size))
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
    fn class_name(&self) -> Option<String> {
        check_node!(self.node_base);
        unsafe { Some(self.class_name_unchecked()?.to_owned()) }
    }

    fn class_list(&mut self) -> DOMTokenList {
        DOMTokenList::new(self)
    }

    fn attribute(&self, qualified_name: &str) -> Option<String> {
        check_node!(self.node_base);
        unsafe { Some(self.attribute_unchecked(qualified_name)?.to_owned()) }
    }

    fn attribute_node(&self, qualified_name: &str) -> Option<AttrNode> {
        check_node!(self.node_base);
        Some(NodeBase::create_node(&self.node_base.tree.upgrade()?, unsafe { lxb_dom_element_attr_by_name(
            self.node_base.node.cast(), qualified_name.as_ptr(), qualified_name.len()) }.cast())?.into())
    }

    fn attribute_names(&self) -> Vec<String> {
        check_node!(self.node_base);
        unsafe { self.attribute_names_unchecked().into_iter().map(|s| s.to_owned()).collect() }
    }

    fn set_attribute(&mut self, qualified_name: &str, value: &str) {
        check_node!(self.node_base);
        unsafe {
             lxb_dom_element_set_attribute(self.node_base.node.cast(),
                                           qualified_name.as_ptr(), qualified_name.len(),
                                           value.as_ptr(), value.len());
        }
    }

    fn remove_attribute(&mut self, qualified_name: &str) {
        check_node!(self.node_base);
        unsafe {
             lxb_dom_element_remove_attribute(
                 self.node_base.node.cast(), qualified_name.as_ptr(), qualified_name.len());
        }
    }

    fn toggle_attribute(&mut self, qualified_name: &str, force: Option<bool>) {
        todo!()
    }

    fn has_attribute(&self, qualified_name: &str) -> bool {
        check_node!(self.node_base);
        unsafe { lxb_dom_element_has_attribute(self.node_base.node.cast(), qualified_name.as_ptr(), qualified_name.len()) }
    }

    fn closest(&self, selectors: &str) -> Option<ElementNode> {
        todo!()
    }

    fn matches(&self, selectors: &str) -> bool {
        todo!()
    }

    fn elements_by_tag_name(&self, qualified_name: &str) -> HTMLCollection {
        todo!()
    }

    fn elements_by_class_name(&self, class_names: &str) -> HTMLCollection {
        todo!()
    }

    fn inner_html(&self) -> String {
        todo!()
    }

    fn outer_html(&self) -> String {
        todo!()
    }

    fn inner_text(&self) -> String {
        todo!()
    }

    fn outer_text(&self) -> String {
        todo!()
    }
}

impl ParentNode for ElementNode {
    /// List of child element nodes.
    fn children(&self) -> HTMLCollection {
        HTMLCollection::new_live(&self, |s: &Self| {
            let mut nodes : Vec<Node> = Vec::new();
            let mut child = s.first_element_child();
            while let Some(Node::Element(c)) = child {
                child = c.next_element_sibling();
                nodes.push(c.into());
            }
            nodes
        })
    }

    /// First element child of this DOM node.
    fn first_element_child(&self) -> Option<Node> {
        let mut child = self.first_child()?;
        loop {
            match child {
                Node::Element(c) => return Some(c.into()),
                _ => { child = child.next_sibling()? }
            }
        }
    }

    /// Last element child element of this DOM node.
    fn last_element_child(&self) -> Option<Node> {
        let mut child = self.last_child()?;
        loop {
            if let Node::Element(c) = child {
                return Some(c.into());
            }
            child = child.previous_sibling()?;
        }
    }

    fn child_element_count(&self) -> usize {
        let mut child = self.first_element_child();
        let mut count = 0;
        while let Some(Node::Element(c)) = child {
            child = c.next_element_sibling();
            count += 1;
        }
        count
    }

    fn prepend(&mut self, nodes: &[&Node]) {
        nodes.iter().rev().for_each(|&n| {
            self.insert_before(n, self.first_child().as_ref());
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

    fn query_selector(&self, selectors: &str) -> Option<Node> {
        todo!()
    }

    fn query_selector_all(&self, selectors: &str) -> NodeList {
        todo!()
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

    fn owner_element(&self) -> Option<Node> {
        check_node!(self.node_base);
        unsafe {
            let attr = self.node_base.node as *mut lxb_dom_attr_t;
            if attr.is_null() || (*attr).owner.is_null() {
                return None;
            }
            NodeBase::create_node(&self.node_base.tree.upgrade()?, (*attr).owner.cast())
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
        todo!()
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
struct NodeListClosure<'a, T> {
    ctx: &'a T,
    f: fn(&'a T) -> Vec<Node>
}

#[derive(Clone)]
pub struct NodeListGeneric<'a, T> {
    live: Option<NodeListClosure<'a, T>>,
    items: Vec<Node>,
}

impl<T> Default for NodeListGeneric<'_, T> {
    fn default() -> Self {
        NodeListGeneric { live: None, items: Vec::default() }
    }
}

impl<'a, T> NodeListGeneric<'a, T> {
    unsafe fn new_unchecked(tree: &Rc<HTMLTreeRc>, coll: *mut lxb_dom_collection_t) -> Self {
        if coll.is_null() {
            return Self::default();
        }
        let mut v = Vec::new();
        v.reserve(lxb_dom_collection_length_noi(coll));
        for i in 0..lxb_dom_collection_length_noi(coll) {
            v.push(NodeBase::create_node(tree, lxb_dom_collection_node_noi(coll, i)).unwrap())
        }
        Self { live: None, items: v }
    }

    fn new(items: &[Node]) -> NodeListGeneric<'a, T> {
        Self { live: None, items: Vec::from(items) }
    }

    fn new_live(ctx: &'a T, f: fn(&'a T) -> Vec<Node>) -> NodeListGeneric<'a, T> {
        Self { live: Some(NodeListClosure{ ctx: &ctx, f }), items: Vec::default() }
    }

    pub fn iter(&self) -> vec::IntoIter<Node> {
        if let Some(closure) = &self.live {
            (closure.f)(closure.ctx).into_iter()
        } else {
            self.items.clone().into_iter()
        }
    }

    #[inline]
    pub fn item(&self, index: usize) -> Option<Node> {
        Some(self.iter().take(index).next()?)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    pub fn named_item(&self, name: &str) -> Option<Node> {
        self.iter()
            .find(|n: &Node| {
                match n {
                    Node::Element(e) =>
                        e.id().filter(|i| i == name).is_some() || e.attribute("name").filter(|n| n == name).is_some(),
                    _ => false
                }
            })
    }
}

impl<T> IntoIterator for &NodeListGeneric<'_, T> {
    type Item = Node;
    type IntoIter = vec::IntoIter<Node>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

type NodeList<'a> = NodeListGeneric<'a, NodeBase>;
type HTMLCollection<'a> = NodeListGeneric<'a, ElementNode>;


// ---------------------------------------- DOMTokenList impl --------------------------------------


pub struct DOMTokenList<'a> {
    element: &'a ElementNode,
}

impl<'a> DOMTokenList<'a> {
    fn new(element: &'a ElementNode) -> Self {
        Self { element }
    }

    fn item(&self, index: usize) -> Option<String> {
        Some(self.values().get(index)?.to_owned())
    }

    fn update_node(&mut self, values: &Vec<String>) {
        todo!()
        // self.element.set_class_name(values.join(" ").as_str());
    }

    pub fn contains(&self, token: &str) -> bool {
        self.iter().find(|s: &String| s.as_str() == token).is_some()
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

    pub fn replace(&mut self, old_token: &str, new_token: &str) {
        self.update_node(&self
            .iter()
            .map(|t: String| {
                if t == old_token { new_token.to_owned() }
                else { t }
            })
            .collect()
        );
    }

    pub fn toggle(&mut self, token: &str, force: Option<bool>) {
        if let Some(f) = force {
            if f {
                self.add(&[token]);
            } else {
                self.remove(&[token])
            }
            return;
        }

        if self.contains(token) {
            self.remove(&[token]);
        } else {
            self.add(&[token]);
        }
    }

    #[inline]
    pub fn value(&self) -> String {
        self.element.class_name().unwrap_or_else(|| String::new())
    }

    pub fn values(&self) -> Vec<String> {
        let mut h = HashSet::new();
        self.value().split_ascii_whitespace()
            .filter(|&v| h.insert(v))
            .map(String::from)
            .collect()
    }

    #[inline]
    pub fn iter(&self) -> vec::IntoIter<String> {
        self.values().into_iter()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.values().len()
    }
}

impl IntoIterator for DOMTokenList<'_> {
    type Item = String;
    type IntoIter = vec::IntoIter<String>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl IntoIterator for &DOMTokenList<'_> {
    type Item = String;
    type IntoIter = vec::IntoIter<String>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}


// --------------------------------------------- Helpers -------------------------------------------


#[inline]
pub(super) unsafe fn str_from_lxb_char_t<'a>(cdata: *const lxb_char_t, size: usize) -> &'a str {
    if size > 0 {
        std::str::from_utf8_unchecked(slice::from_raw_parts(cdata, size))
    } else {
        ""
    }
}

#[inline]
pub(super) unsafe fn str_from_lxb_str_t<'a>(s: *const lexbor_str_t) -> &'a str {
    str_from_lxb_char_t((*s).data, (*s).length)
}

#[inline]
pub(super) unsafe fn str_from_dom_node<'a>(node: *const lxb_dom_node_t) -> &'a str {
    let cdata = node as *const lxb_dom_character_data_t;
    str_from_lxb_str_t(addr_of!((*cdata).data))
}

#[inline]
pub(super) unsafe fn str_from_lxb_str_cb<'a, Node, Fn>(
    node: *mut Node, lxb_fn: unsafe extern "C" fn(*mut Fn, *mut usize) -> *const lxb_char_t) -> Option<&'a str> {
    if node.is_null() {
        return None;
    }
    let mut size = 0;
    let name = lxb_fn(node.cast(), addr_of_mut!(size));
    match size {
        0 => None,
        _ => Some(str_from_lxb_char_t(name, size))
    }
}


// ---------------------------------------------- Tests --------------------------------------------


#[cfg(test)]
mod tests {
    use crate::parse::html::tree::HTMLTree;

    const HTML: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <title>Example page</title>
  </head>
  <body>
    <main id="foo">
      <p id="a">Hello <span class="bar">world</span>!</p>
      <p id="b" class="dom">Hello <a href="https://example.com" class="bar baz">DOM</a>!</p>
     </main>
     <!-- A comment -->
  </body>
</html>"#;

    #[test]
    fn parse_from_str() {
        let _tree1 = HTMLTree::from(HTML);
        let _tree2 = HTMLTree::from("<html></html>");
    }

    #[test]
    fn parse_from_string() {
        let _tree1 = HTMLTree::from(HTML.to_owned());
        let _tree2 = HTMLTree::from(&HTML.to_owned());
    }

    #[test]
    fn parse_from_bytes() {
        let _tree1 = HTMLTree::from(HTML.to_owned().into_bytes());
        let _tree2 = HTMLTree::from(&HTML.to_owned().into_bytes());
        let _tree3 = HTMLTree::from(HTML.as_bytes());
    }
}
