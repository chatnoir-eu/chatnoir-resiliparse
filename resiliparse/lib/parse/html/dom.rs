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

use std::{ptr, slice};
use std::cell::{Ref, RefCell};
use std::ops::{Deref, DerefMut};
use std::ptr::{addr_of, addr_of_mut};
use std::rc::{Rc, Weak};

use crate::third_party::lexbor::*;
use super::serialize::node_format_visible_text;
use super::tree::{HTMLTree, HTMLTreeRc};


#[derive(PartialEq, Eq)]
pub enum NodeType {
    Element,
    Attribute,
    Text,
    CDataSection,
    EntityReference,
    Entity,
    ProcessingInstruction,
    Comment,
    Document,
    DocumentType,
    DocumentFragment,
    Notation,
    LastEntry,
    Undefined
}

/// ParentNode mixin trait.
pub trait ParentNode {
    fn first_element_child(&self) -> Option<Node>;
    fn last_element_child(&self) -> Option<Node>;
    fn child_element_nodes(&self) -> Vec<Node>;
    fn child_element_count(&self) -> usize;
    fn prepend(&mut self, node: &Node);
    fn append(&mut self, node: &Node);
    fn replace_children(&mut self, node: &[&Node]);

    fn query_selector(&self, selectors: &str) -> Option<Node>;
    fn query_selector_all(&self, selectors: &str) -> Collection;
}

/// NonElementParentNode mixin trait.
pub trait NonElementParentNode {
    fn get_element_by_id(element_id: &str) -> Option<Node>;
}

/// ChildNode mixin trait.
pub trait ChildNode {
    fn before(&mut self, node: &Node);
    fn after(&mut self, node: &Node);
    fn replace_with(&mut self, node: &Node);
    fn remove(&mut self);
}

/// NonDocumentTypeChildNode mixin trait.
pub trait NonDocumentTypeChildNode {
    fn previous_element_sibling(&self) -> Option<Node>;
    fn next_element_sibling(&self) -> Option<Node>;
}

/// HTML Element mixin trait.
pub trait Element {
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
    fn name(&self) -> Option<String>;
    fn class_name(&self) -> Option<String>;
    fn class_list(&self) -> DOMTokenList;

    fn attribute(&self, qualified_name: &str) -> Option<String>;
    fn attributeNode(&self, qualified_name: &str) -> Option<Attr>;
    fn attribute_names(&self) -> Vec<String>;
    fn set_attribute(&mut self, qualified_name: &str, value: &str);
    fn remove_attribute(&mut self, qualified_name: &str);
    fn has_attribute(&self, qualified_name: &str) -> bool;

    fn elements_by_tag_name(&self, qualified_name: &str) -> Collection;
    fn elements_by_class_name(&self, class_names: &str) -> Collection;
}

/// NodeList mixin trait.
pub trait NodeList {
    fn item(&self, index: isize) -> Option<&Node>;
    fn item_mut(&mut self, index: isize) -> Option<&mut Node>;
    fn len(&self) -> usize;
}

/// HTMLCollection mixin trait.
pub trait HTMLCollection {
    fn named_item(&self, name: &str) -> Option<&Node>;
    fn named_item_mut(&mut self, name: &str) -> Option<&mut Node>;
}

impl From<lxb_dom_node_type_t::Type> for NodeType {
    fn from(value: lxb_dom_node_type_t::Type) -> Self {
        use lxb_dom_node_type_t::*;
        match value {
            LXB_DOM_NODE_TYPE_ELEMENT => NodeType::Element,
            LXB_DOM_NODE_TYPE_ATTRIBUTE => NodeType::Attribute,
            LXB_DOM_NODE_TYPE_TEXT => NodeType::Text,
            LXB_DOM_NODE_TYPE_CDATA_SECTION => NodeType::CDataSection,
            LXB_DOM_NODE_TYPE_ENTITY_REFERENCE => NodeType::EntityReference,
            LXB_DOM_NODE_TYPE_ENTITY => NodeType::Entity,
            LXB_DOM_NODE_TYPE_PROCESSING_INSTRUCTION => NodeType::ProcessingInstruction,
            LXB_DOM_NODE_TYPE_COMMENT => NodeType::Comment,
            LXB_DOM_NODE_TYPE_DOCUMENT => NodeType::Document,
            LXB_DOM_NODE_TYPE_DOCUMENT_TYPE => NodeType::DocumentType,
            LXB_DOM_NODE_TYPE_DOCUMENT_FRAGMENT => NodeType::DocumentFragment,
            LXB_DOM_NODE_TYPE_NOTATION => NodeType::Notation,
            LXB_DOM_NODE_TYPE_LAST_ENTRY => NodeType::LastEntry,
            _ => NodeType::Undefined,
        }
    }
}

pub struct DocumentType {
    name: String,
    public_id: String,
    system_id: String
}

pub struct Document {
    tree: Weak<HTMLTreeRc>,
    doc_type: DocumentType,
    document_element: *mut lxb_dom_document_t,
}

impl Document {
    fn doctype(&self) -> Option<&DocumentType> {
        self.tree.upgrade()?;
        Some(&self.doc_type)
    }

    fn document_element(&self) -> Option<Node> {
        Node::new(&self.tree.upgrade()?, self.document_element.cast())
    }

    fn elements_by_tag_name(&self) -> Collection {
        todo!()
    }

    fn elements_by_class_name(&self) -> Collection {
        todo!()
    }

    fn create_element(&mut self, local_name: &str) -> Option<Node> {
        let element = unsafe {
            lxb_dom_document_create_element(
                self.document_element, local_name.as_ptr(), local_name.len(), ptr::null_mut())
        };
        if !element.is_null() {
            Node::new(&self.tree.upgrade()?, element.cast())
        } else {
            None
        }
    }

    fn create_text_node(&mut self, data: &str) -> Option<Node> {
        let text = unsafe {
            lxb_dom_document_create_text_node(self.document_element, data.as_ptr(), data.len())
        };
        if !text.is_null() {
            Node::new(&self.tree.upgrade()?, text.cast())
        } else {
            None
        }
    }

    fn create_cdata_section(&mut self, data: &str) -> Option<Node> {
        let cdata = unsafe {
            lxb_dom_document_create_cdata_section(self.document_element, data.as_ptr(), data.len())
        };
        if !cdata.is_null() {
            Node::new(&self.tree.upgrade()?, cdata.cast())
        } else {
            None
        }
    }

    fn create_comment(&mut self, data: &str) -> Option<Node> {
        let comment = unsafe {
            lxb_dom_document_create_comment(self.document_element, data.as_ptr(), data.len())
        };
        if !comment.is_null() {
            Node::new(&self.tree.upgrade()?, comment.cast())
        } else {
            None
        }
    }

    fn create_attribute(&mut self, local_name: &str) -> Option<Node> {
        let attr = unsafe { lxb_dom_attr_interface_create(self.document_element) };
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
        Node::new(&self.tree.upgrade()?, attr.cast())
    }
}

impl ParentNode for Document {
    #[inline]
    fn first_element_child(&self) -> Option<Node> {
        self.document_element()?.first_element_child()
    }

    #[inline]
    fn last_element_child(&self) -> Option<Node> {
        self.document_element()?.last_element_child()
    }

    fn child_element_nodes(&self) -> Vec<Node> {
        if let Some(d) = self.document_element() {
            d.child_element_nodes()
        } else {
            Vec::default()
        }
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
    fn prepend(&mut self, node: &Node) {
        if let Some(mut d) = self.document_element() {
            d.prepend(node)
        }
    }

    #[inline]
    fn append(&mut self, node: &Node) {
        if let Some(mut d) = self.document_element() {
            d.append(node)
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
    fn query_selector_all(&self, selectors: &str) -> Collection {
        if let Some(d) = self.document_element() {
            d.query_selector_all(selectors)
        } else {
            Collection::default()
        }
    }
}

pub struct DOMTokenList<'a> {
    node: &'a mut Node,
    values: RefCell<Vec<String>>,
}

impl<'a> DOMTokenList<'a> {
    fn new(node: &'a mut Node) -> Self {
        Self { node, values: RefCell::new(Vec::new()) }
    }

    fn item(&self, index: isize) -> Option<String> {
        let l = self.len();
        Some(self.values().get(wrap_index(l, index))?.clone())
    }

    fn update_node(&mut self) {
        let v = self.value();
        unsafe { lxb_dom_node_text_content_set(self.node.node, v.as_ptr(), v.len()); }
    }

    fn sync(&self) {
        self.values.replace(self.value().split_ascii_whitespace().map(|s| String::from(s)).collect());
    }

    fn contains(&self, token: &str) -> bool {
        self.values().iter().find(|s: &&String| s.as_str() == token).is_some()
    }

    fn add(&mut self, tokens: &[&str]) {
        self.sync();
        tokens.iter().for_each(|t: &&str| {
            if !self.contains(t) {
                self.values.borrow_mut().push((*t).to_owned());
            }
        });
        self.update_node();
    }

    fn remove(&mut self, tokens: &[&str]) {
        self.sync();
        let mut v = self.values.borrow().clone();
        for t in tokens {
            v = v.into_iter().filter(|s: &String| s != t.to_owned()).collect();
        }
        self.values.replace(v);
        self.update_node();
    }

    fn replace(&mut self, old_token: &str, new_token: &str) {
        self.sync();
        self.values.borrow_mut()
            .iter_mut()
            .for_each(|s: &mut String| {
                if s.as_str() == old_token {
                    *s = new_token.to_owned();
                }
            });
        self.update_node();
    }

    fn toggle(&mut self, token: &str, force: Option<bool>) {
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
        self.update_node();
    }

    #[inline]
    fn value(&self) -> String {
        self.node.node_value().unwrap_or(String::default())
    }

    #[inline]
    fn values(&self) -> Ref<Vec<String>> {
        self.sync();
        self.values.borrow()
    }

    #[inline]
    fn len(&self) -> usize {
        self.values().len()
    }
}

/// DOM node.
pub struct Node {
    tree: Weak<HTMLTreeRc>,
    node: *mut lxb_dom_node_t,
}

impl Node {
    #[inline]
    pub(super) fn new(tree: &Rc<HTMLTreeRc>, node: *mut lxb_dom_node_t) -> Option<Self> {
        if node.is_null() {
            return None;
        }
        Some(Self { tree: Rc::downgrade(tree), node })
    }

    /// DOM node type.
    pub fn node_type(&self) -> NodeType {
        match self.tree.upgrade() {
            Some(_) => unsafe { (*self.node).type_.into() },
            _ => NodeType::Undefined
        }
    }

    pub unsafe fn node_name_unchecked(&self) -> Option<&str> {
        self.tree.upgrade()?;
        str_from_lxb_str_cb(self.node, lxb_dom_node_name)
    }

    /// DOM element tag or node name.
    pub fn node_name(&self) -> Option<String> {
        unsafe { Some(self.node_name_unchecked()?.to_owned()) }
    }

    /// First child element of this DOM node.
    pub fn first_child(&self) -> Option<Self> {
        unsafe { Self::new(&self.tree.upgrade()?, self.node.as_ref()?.first_child) }
    }

    /// Last child element of this DOM node.
    pub fn last_child(&self) -> Option<Self> {
        self.tree.upgrade()?;
        unsafe { Self::new(&self.tree.upgrade()?, self.node.as_ref()?.last_child) }
    }

    /// List of child nodes.
    pub fn child_nodes(&self) -> Vec<Self> {
        let mut nodes = Vec::new();
        let mut child = self.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            nodes.push(c);
        }
        nodes
    }

    /// Parent of this node.
    pub fn parent(&self) -> Option<Self> {
        unsafe { Self::new(&self.tree.upgrade()?, self.node.as_ref()?.parent) }
    }

    /// Previous sibling node.
    pub fn previous_sibling(&self) -> Option<Self> {
        unsafe { Self::new(&self.tree.upgrade()?, self.node.as_ref()?.prev) }
    }

    /// Next sibling node.
    pub fn next_sibling(&self) -> Option<Self> {
        unsafe { Self::new(&self.tree.upgrade()?, self.node.as_ref()?.next) }
    }

    /// Node text value.
    #[inline]
    pub fn node_value(&self) -> Option<String> {
        unsafe { Some(self.node_value_unchecked()?.to_owned()) }
    }

    /// Node text value.
    pub unsafe fn node_value_unchecked(&self) -> Option<&str> {
        self.tree.upgrade()?;
        let cdata = self.node as *const lxb_dom_character_data_t;
        Some(str_from_lxb_str_t(addr_of!((*cdata).data)))
    }

    /// Text contents of this DOM node and its children.
    pub fn text_content(&self) -> Option<String> {
        self.tree.upgrade()?;

        if self.node_type() == NodeType::Text {
            return self.node_value();
        }

        let out_text;
        unsafe {
            let mut l = 0;
            let t = lxb_dom_node_text_content(self.node, &mut l);
            out_text = str_from_lxb_char_t(t, l).to_string();
            lxb_dom_document_destroy_text_noi(self.node.as_ref()?.owner_document, t);
        }
        Some(out_text)
    }

    /// Visible text contents of this DOM node and its children.
    #[inline]
    pub fn outer_text(&self) -> Option<String> {
        self.inner_text()
    }

    /// Visible text contents of this DOM node and its children.
    #[inline]
    pub fn inner_text(&self) -> Option<String> {
        self.tree.upgrade()?;
        match self.node_type() {
            NodeType::Element => Some(node_format_visible_text(self.node)),
            _ => None
        }
    }

    fn serialize_node(node: &Self) -> Option<String> {
        node.tree.upgrade()?;

        let out_html;
        unsafe {
            let s = lexbor_str_create();
            lxb_html_serialize_tree_str(node.node, s);
            out_html = str_from_lxb_str_t(s).to_string();
            lexbor_str_destroy(s, node.node.as_ref()?.owner_document.as_ref()?.text, true);
        }
        Some(out_html)
    }

    /// Outer HTML of this DOM node and its children.
    #[inline]
    pub fn outer_html(&self) -> Option<String> {
        Self::serialize_node(self)
    }

    /// Inner HTML of this DOM node's children.
    pub fn inner_html(&self) -> Option<String> {
        self.child_nodes()
            .into_iter()
            .flat_map(|c| Self::serialize_node(&c))
            .reduce(|a, b| a + &b)
    }
}

macro_rules! check_node {
    ($self:ident) => {
        if $self.tree.upgrade().is_none() || $self.node.is_null() {
            return Default::default();
        }
    }
}

macro_rules! check_element {
    ($self:ident) => {
        if !$self.tree.upgrade().is_some() || $self.node_type() != NodeType::Element {
            return Default::default();
        }
    }
}

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

impl Element for Node {
    /// DOM element tag or node name.
    unsafe fn tag_name_unchecked(&self) -> Option<&str> {
        check_element!(self);
        str_from_lxb_str_cb(self.node, lxb_dom_element_tag_name)
    }

    unsafe fn local_name_unchecked(&self) -> Option<&str> {
        check_element!(self);
        str_from_lxb_str_cb(self.node, lxb_dom_element_local_name)
    }

    unsafe fn id_unchecked(&self) -> Option<&str> {
        check_element!(self);
        str_from_lxb_str_cb(self.node, lxb_dom_element_id_noi)
    }

    #[inline]
    unsafe fn name_unchecked(&self) -> Option<&str> {
        self.attribute_unchecked("name")
    }

    unsafe fn class_name_unchecked(&self) -> Option<&str> {
        check_element!(self);
        str_from_lxb_str_cb(self.node, lxb_dom_element_class_noi)
    }

    unsafe fn attribute_unchecked(&self, qualified_name: &str) -> Option<&str> {
        check_element!(self);
        let mut size = 0;
        let name = lxb_dom_element_get_attribute(
            self.node.cast(),
            qualified_name.as_ptr().cast(),
            qualified_name.len(),
            addr_of_mut!(size));
        match size {
            0 => None,
            _ => Some(str_from_lxb_char_t(name, size))
        }
    }

    unsafe fn attribute_names_unchecked(&self) -> Vec<&str> {
        check_element!(self);
        let mut attr =  lxb_dom_element_first_attribute_noi(self.node.cast());
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
        unsafe { Some(self.tag_name_unchecked()?.to_owned()) }
    }

    #[inline]
    fn local_name(&self) -> Option<String> {
        unsafe { Some(self.local_name_unchecked()?.to_owned()) }
    }

    #[inline]
    fn id(&self) -> Option<String> {
        unsafe { Some(self.id_unchecked()?.to_owned()) }
    }

    #[inline]
    fn name(&self) -> Option<String> {
        unsafe { Some(self.name_unchecked()?.to_owned()) }
    }

    #[inline]
    fn class_name(&self) -> Option<String> {
        unsafe { Some(self.class_name_unchecked()?.to_owned()) }
    }

    fn class_list(&self) -> DOMTokenList {
        todo!()
        // check_element!(self);
        // let Some(cls) = {
        //     unsafe { self.class_name_unchecked() }
        // } else {
        //     return Vec::default();
        // };
        // let mut v= Vec::new();
        // cls.split_ascii_whitespace().flat_map(|c| v.push(c.to_owned())).collect();
        // v
    }

    fn attribute(&self, qualified_name: &str) -> Option<String> {
        unsafe { Some(self.attribute_unchecked(qualified_name)?.to_owned()) }
    }

    fn attributeNode(&self, qualified_name: &str) -> Option<Attr> {
        check_element!(self);
        Attr::new(&self.tree.upgrade()?, unsafe { lxb_dom_element_attr_by_name(
            self.node.cast(), qualified_name.as_ptr(), qualified_name.len()) })
    }

    fn attribute_names(&self) -> Vec<String> {
        unsafe { self.attribute_names_unchecked().into_iter().map(|s| s.to_owned()).collect() }
    }

    fn set_attribute(&mut self, qualified_name: &str, value: &str) {
        check_element!(self);
        unsafe {
             lxb_dom_element_set_attribute(self.node.cast(),
                                           qualified_name.as_ptr(), qualified_name.len(),
                                           value.as_ptr(), value.len());
        }
    }

    fn remove_attribute(&mut self, qualified_name: &str) {
        check_element!(self);
        unsafe {
             lxb_dom_element_remove_attribute(
                 self.node.cast(), qualified_name.as_ptr(), qualified_name.len());
        }
    }

    fn has_attribute(&self, qualified_name: &str) -> bool {
        check_element!(self);
        unsafe { lxb_dom_element_has_attribute(self.node.cast(), qualified_name.as_ptr(), qualified_name.len()) }
    }

    fn elements_by_tag_name(&self, qualified_name: &str) -> Collection {
        check_element!(self);
        todo!()
    }

    fn elements_by_class_name(&self, class_names: &str) -> Collection {
        check_element!(self);
        todo!()
    }
}

impl NonDocumentTypeChildNode for Node {
    /// Previous sibling element node.
    fn previous_element_sibling(&self) -> Option<Self> {
        loop {
            let s = self.previous_sibling()?;
            if s.node_type() == NodeType::Element {
                return Some(s);
            }
        }
    }

    /// Next sibling element node.
    fn next_element_sibling(&self) -> Option<Self> {
        loop {
            let s = self.next_sibling()?;
            if s.node_type() == NodeType::Element {
                return Some(s);
            }
        }
    }
}

impl ParentNode for Node {
    /// First element child of this DOM node.
    fn first_element_child(&self) -> Option<Self> {
        let mut child = self.first_child()?;
        loop {
            if child.node_type() == NodeType::Element {
                return Some(child);
            }
            child = child.next_sibling()?;
        }
    }

    /// Last element child element of this DOM node.
    fn last_element_child(&self) -> Option<Self> {
        let mut child = self.last_child()?;
        loop {
            if child.node_type() == NodeType::Element {
                return Some(child);
            }
            child = child.previous_sibling()?;
        }
    }

    /// List of child element nodes.
    fn child_element_nodes(&self) -> Vec<Node> {
        let mut nodes = Vec::new();
        let mut child = self.first_element_child();
        while let Some(c) = child {
            child = c.next_element_sibling();
            nodes.push(c);
        }
        nodes
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

    fn prepend(&mut self, node: &Node) {
        check_node!(self);
        if node.node.is_null() {
            return;
        }
        if let Some(mut fc) = self.first_child() {
            fc.before(node);
        } else {
            self.replace_children(&[node]);
        }
    }

    fn append(&mut self, node: &Node) {
        check_node!(self);
        if node.node.is_null() {
            return;
        }
        if let Some(mut lc) = self.last_child() {
            lc.after(node);
        } else {
            self.replace_children(&[node]);
        }
    }

    fn replace_children(&mut self, nodes: &[&Node]) {
        check_node!(self);
        if nodes.is_empty() {
            return;
        }
        unsafe { lxb_dom_node_replace_all(self.node, nodes[0].node); }
        nodes.iter()
            .skip(1)
            .filter(|n: &&&Node| !n.node.is_null())
            .for_each(|n: &&Node| self.after(n));
    }

    fn query_selector(&self, selectors: &str) -> Option<Node> {
        todo!()
    }

    fn query_selector_all(&self, selectors: &str) -> Collection {
        todo!()
    }
}

impl ChildNode for Node {
    fn before(&mut self, node: &Node) {
        check_node!(self);
        if node.node.is_null() {
            return;
        }
        unsafe { lxb_dom_node_insert_before(self.node, node.node) }
    }

    fn after(&mut self, node: &Node) {
        check_node!(self);
        if node.node.is_null() {
            return;
        }
        unsafe { lxb_dom_node_insert_after(self.node, node.node) }
    }

    fn replace_with(&mut self, node: &Node) {
        check_node!(self);
        if node.node.is_null() {
            return;
        }
        unsafe { lxb_dom_node_insert_before(self.node, node.node) }
        self.remove();
    }

    fn remove(&mut self) {
        check_node!(self);
        unsafe { lxb_dom_node_remove(self.node); }
        self.node = ptr::null_mut();
        self.tree = Weak::default();
    }
}

pub struct Attr {
    tree: Weak<HTMLTreeRc>,
    attr: *mut lxb_dom_attr_t
}

impl Attr {
    fn new(tree: &Rc<HTMLTreeRc>, attr: *mut lxb_dom_attr_t) -> Option<Attr> {
        if !attr.is_null() {
            Some(Attr { tree: Rc::downgrade(&tree), attr })
        } else {
            None
        }
    }

    pub unsafe fn local_name_unchecked(&self) -> Option<&str> {
        self.tree.upgrade()?;
        str_from_lxb_str_cb(self.attr, lxb_dom_attr_local_name_noi)
    }

    pub unsafe fn name_unchecked(&self) -> Option<&str> {
        self.tree.upgrade()?;
        str_from_lxb_str_cb(self.attr, lxb_dom_attr_qualified_name)
    }

    pub unsafe fn value_unchecked(&self) -> Option<&str> {
        self.tree.upgrade()?;
        str_from_lxb_str_cb(self.attr, lxb_dom_attr_value_noi)
    }

    #[inline]
    pub fn local_name(&self) -> Option<String> {
        unsafe { Some(self.local_name_unchecked()?.to_owned()) }
    }

    #[inline]
    pub fn name(&self) -> Option<String> {
        unsafe { Some(self.name_unchecked()?.to_owned()) }
    }

    #[inline]
    pub fn value(&self) -> Option<String> {
        unsafe { Some(self.value_unchecked()?.to_owned()) }
    }

    pub fn owner_element(&self) -> Option<Node> {
        let tree = self.tree.upgrade()?;
        unsafe {
            if self.attr.is_null() || (*self.attr).owner.is_null() {
                return None;
            }
            Node::new(&tree, (*self.attr).owner.cast())
        }
    }
}


pub struct Collection {
    items: Vec<Node>,
}

impl Default for Collection {
    fn default() -> Self {
        Collection { items: Vec::new() }
    }
}

impl Collection {
    unsafe fn new_unchecked(tree: &Rc<HTMLTreeRc>, coll: *mut lxb_dom_collection_t) -> Self {
        if coll.is_null() {
            return Self::default();
        }
        let mut v = Vec::new();
        v.reserve(lxb_dom_collection_length_noi(coll));
        for i in 0..lxb_dom_collection_length_noi(coll) {
            v.push(Node::new(tree, lxb_dom_collection_node_noi(coll, i)).unwrap())
        }
        Self { items: v }
    }
}

impl Deref for Collection {
    type Target = [Node];

    fn deref(&self) -> &Self::Target {
        self.items.as_slice()
    }
}

impl DerefMut for Collection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.items.as_mut_slice()
    }
}

#[inline]
fn wrap_index(max_idx: usize, index: isize) -> usize {
    if index >= 0 {
        index as usize
    } else {
        (index % max_idx as isize) as usize
    }
}

impl NodeList for Collection {
    #[inline]
    fn item(&self, index: isize) -> Option<&Node> {
        self.items.get(wrap_index(self.len(), index))
    }

    #[inline]
    fn item_mut(&mut self, index: isize) -> Option<&mut Node> {
        let idx_wrapped = wrap_index(self.len(), index);
        self.items.get_mut(idx_wrapped)
    }

    #[inline]
    fn len(&self) -> usize {
        self.items.len()
    }
}

impl HTMLCollection for Collection {
    fn named_item(&self, name: &str) -> Option<&Node> {
        self.iter()
            .find(|e: &&Node| e.id().filter(|i| i == name).is_some() || e.name().filter(|n| n == name).is_some())
    }

    fn named_item_mut(&mut self, name: &str) -> Option<&mut Node> {
        self.iter_mut()
            .find(|e: &&mut Node| e.id().filter(|i| i == name).is_some() || e.name().filter(|n| n == name).is_some())
    }
}

#[cfg(test)]
mod tests {
    use crate::parse::html::dom::HTMLTree;

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