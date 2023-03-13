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
use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::ptr::addr_of_mut;
use std::rc::{Rc, Weak};
use std::vec::IntoIter;

use crate::third_party::lexbor::*;


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

pub struct DOMTokenList<'a> {
    node: &'a Node,
    values: Vec<String>
}

impl<'a> DOMTokenList<'_> {
    fn item(&mut self, index: isize) -> Option<String> {
        let l = self.len();
        Some(self.values().get(wrap_index(l, index))?.clone())
    }

    fn update_node(&mut self) {
        todo!()
        // self.node.set_value(self.values.join(" "))
    }

    fn sync(&mut self) {
        self.values = self.value().split_ascii_whitespace().map(|s| String::from(s)).collect();
    }

    fn contains(&mut self, token: &str) -> bool {
        self.values().iter().find(|s| s.as_str() == token).is_some()
    }

    fn add(&mut self, tokens: &[&str]) {
        self.sync();
        for t in tokens {
            if !self.contains(t) {
                self.values.push((*t).to_owned());
            }
        }
        self.update_node();
    }

    fn remove(&mut self, tokens: &[&str]) {
        self.sync();
        let mut v = self.values.clone();
        for t in tokens {
            v = v.into_iter().filter(|s: &String| s != t.to_owned()).collect();
        }
        self.values = v;
        self.update_node();
    }

    fn replace(&mut self, old_token: &str, new_token: &str) {
        self.sync();
        self.values
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
    fn value(&mut self) -> String {
        self.node.value().unwrap_or(String::default())
    }

    #[inline]
    fn values(&mut self) -> &[String] {
        self.sync();
        self.values.as_slice()
    }

    #[inline]
    fn len(&mut self) -> usize {
        self.values().len()
    }
}

/// HTML Element mixin trait.
pub trait Element {
    unsafe fn tag_name_unchecked(&self) -> Option<&str>;
    unsafe fn local_name_unchecked(&self) -> Option<&str>;
    unsafe fn id_unchecked(&self) -> Option<&str>;
    unsafe fn name_unchecked(&self) -> Option<&str>;
    unsafe fn class_name_unchecked(&self) -> Option<&str>;
    unsafe fn attribute_unchecked(&self, qualified_name: &str) -> Option<&str>;

    fn tag_name(&self) -> Option<String>;
    fn local_name(&self) -> Option<String>;
    fn id(&self) -> Option<String>;
    fn name(&self) -> Option<String>;
    fn class_name(&self) -> Option<String>;
    fn class_list(&self) -> DOMTokenList;

    fn attribute(&self, qualified_name: &str) -> Option<String>;
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

/// Internal heap-allocated and reference-counted HTMLTree.
struct HTMLTreeRc {
    html_document: *mut lxb_html_document_t
}

impl Drop for HTMLTreeRc {
    fn drop(&mut self) {
        if !self.html_document.is_null() {
            unsafe { lxb_html_document_destroy(self.html_document); }
            self.html_document = ptr::null_mut();
        }
    }
}

/// HTML DOM tree.
pub struct HTMLTree {
    tree_rc: Rc<HTMLTreeRc>
}

impl From<&[u8]> for HTMLTree {
    /// Decode a raw HTML byte string and parse it into a DOM tree.
    /// The bytes must be a valid UTF-8 encoding.
    fn from(value: &[u8]) -> Self {
        let doc_ptr;
        unsafe {
            doc_ptr = lxb_html_document_create();
            lxb_html_document_parse(doc_ptr, value.as_ptr(), value.len());
        }

        HTMLTree { tree_rc: Rc::new(HTMLTreeRc { html_document: doc_ptr }) }
    }
}

impl From<Vec<u8>> for HTMLTree {
    /// Decode a raw HTML byte string and parse it into a DOM tree.
    /// The bytes must be a valid UTF-8 encoding.
    #[inline]
    fn from(value: Vec<u8>) -> Self {
        value.as_slice().into()
    }
}

impl From<&Vec<u8>> for HTMLTree {
    /// Decode a raw HTML byte string and parse it into a DOM tree.
    /// The bytes must be a valid UTF-8 encoding.
    #[inline]
    fn from(value: &Vec<u8>) -> Self {
        value.as_slice().into()
    }
}

impl From<&str> for HTMLTree {
    /// Parse HTML from a Unicode string slice into a DOM tree.
    #[inline]
    fn from(value: &str) -> Self {
        value.as_bytes().into()
    }
}

impl From<String> for HTMLTree {
    /// Parse HTML from a Unicode String into a DOM tree.
    #[inline]
    fn from(value: String) -> Self {
        value.as_bytes().into()
    }
}

impl From<&String> for HTMLTree {
    /// Parse HTML from a Unicode String into a DOM tree.
    #[inline]
    fn from(value: &String) -> Self {
        value.as_bytes().into()
    }
}

impl HTMLTree {
    fn get_html_document_raw(&self) -> Option<&mut lxb_html_document_t> {
        unsafe { self.tree_rc.html_document.as_mut() }
    }

    #[inline]
    pub fn document(&self) -> Option<Node> {
        Node::new(
            &self.tree_rc,
            addr_of_mut!(self.get_html_document_raw()?.dom_document) as *mut lxb_dom_node_t)
    }

    pub fn head(&self) -> Option<Node> {
        Node::new(&self.tree_rc, self.get_html_document_raw()?.head as *mut lxb_dom_node_t)
    }

    pub fn body(&self) -> Option<Node> {
        Node::new(&self.tree_rc, self.get_html_document_raw()?.body as *mut lxb_dom_node_t)
    }

    #[inline]
    pub fn title(&self) -> Option<String> {
        unsafe { Some(self.title_unchecked()?.to_owned()) }
    }

    pub unsafe fn title_unchecked(&self) -> Option<&str> {
        let mut title_len = 0;
        let cdata = lxb_html_document_title(self.get_html_document_raw()?, addr_of_mut!(title_len));
        match title_len {
            0 => None,
            _ => Some(std::str::from_utf8_unchecked(slice::from_raw_parts(cdata, title_len)))
        }
    }
}

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

impl From<lxb_dom_node_type_t> for NodeType {
    fn from(value: lxb_dom_node_type_t) -> Self {
        match value {
            lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_ELEMENT => NodeType::Element,
            lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_ATTRIBUTE => NodeType::Attribute,
            lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_TEXT => NodeType::Text,
            lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_CDATA_SECTION => NodeType::CDataSection,
            lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_ENTITY_REFERENCE => NodeType::EntityReference,
            lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_ENTITY => NodeType::Entity,
            lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_PROCESSING_INSTRUCTION => NodeType::ProcessingInstruction,
            lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_COMMENT => NodeType::Comment,
            lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_DOCUMENT => NodeType::Document,
            lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_DOCUMENT_TYPE => NodeType::DocumentType,
            lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_DOCUMENT_FRAGMENT => NodeType::DocumentFragment,
            lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_NOTATION => NodeType::Notation,
            lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_LAST_ENTRY => NodeType::LastEntry,
            _ => NodeType::Undefined,
        }
    }
}

pub struct Document {
}

pub struct DocumentType {
    name: String,
    public_id: String,
    system_id: String
}

impl Document {
    fn doctype(&self) -> Option<DocumentType> {
        todo!()
    }

    fn document_element(&self) -> Option<Node> {
        todo!()
    }

    fn elements_by_tag_name(&self) -> Collection {
        todo!()
    }

    fn elements_by_class_name(&self) -> Collection {
        todo!()
    }

    fn create_element(&self, local_name: &str) -> Option<Node> {
        todo!()
    }

    fn create_text_node(&self, data: &str) -> Option<Node> {
        todo!()
    }

    fn create_cdata_section(&self, data: &str) -> Option<Node> {
        todo!()
    }

    fn create_comment(&self, data: &str) -> Option<Node> {
        todo!()
    }

    fn create_attribute(&self, data: &str) -> Option<Node> {
        todo!()
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


/// DOM node.
pub struct Node {
    tree: Weak<HTMLTreeRc>,
    node: *mut lxb_dom_node_t,
}

impl Node {
    #[inline]
    fn new(tree: &Rc<HTMLTreeRc>, node: *mut lxb_dom_node_t) -> Option<Self> {
        if node.is_null() {
            return None;
        }
        Some(Self { tree: Rc::downgrade(tree), node })
    }

    #[inline]
    unsafe fn lxb_str_to_slice<T>(&self, lxb_fn: unsafe extern "C" fn(*mut T, *mut usize) -> *const lxb_char_t) -> Option<&str> {
        let mut size = 0;
        let name = lxb_fn(self.node as *mut T, addr_of_mut!(size));
        match size {
            0 => None,
            _ => Some(std::str::from_utf8_unchecked(slice::from_raw_parts(name.cast(), size)))
        }
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
        self.lxb_str_to_slice(lxb_dom_node_name)
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
    pub fn value(&self) -> Option<String> {
        unsafe { Some(self.value_unchecked()?.to_owned()) }
    }

    /// Node text value.
    pub unsafe fn value_unchecked(&self) -> Option<&str> {
        self.tree.upgrade()?;
        let cdata = self.node as *const lxb_dom_character_data_t;
        Some(std::str::from_utf8_unchecked(slice::from_raw_parts(
            (*cdata).data.data.cast(), (*cdata).data.length)))
    }

    /// Text contents of this DOM node and its children.
    pub fn outer_text(&self) -> Option<String> {
        self.tree.upgrade()?;

        if self.node_type() == NodeType::Text {
            return self.value();
        }

        let out_text;
        unsafe {
            let mut l = 0;
            let t = lxb_dom_node_text_content(self.node, &mut l);
            out_text = std::str::from_utf8_unchecked(slice::from_raw_parts(t.cast(), l)).to_string();
            lxb_dom_document_destroy_text_noi(self.node.as_ref()?.owner_document, t);
        }
        Some(out_text)
    }

    /// Text contents of this DOM node and its children.
    #[inline]
    pub fn inner_text(&self) -> Option<String> {
        self.outer_text()
    }

    fn serialize_node(node: &Self) -> Option<String> {
        node.tree.upgrade()?;

        let out_html;
        unsafe {
            let h = lexbor_str_create();
            lxb_html_serialize_tree_str(node.node, h);
            out_html = std::str::from_utf8_unchecked(slice::from_raw_parts((*h).data.cast(), (*h).length)).to_string();
            lexbor_str_destroy(h, node.node.as_ref()?.owner_document.as_ref()?.text, true);
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

macro_rules! check_element {
    ($self:ident) => {
        if !$self.tree.upgrade().is_some() || $self.node_type() != NodeType::Element {
            return None;
        }
    }
}

impl Element for Node {
    /// DOM element tag or node name.
    unsafe fn tag_name_unchecked(&self) -> Option<&str> {
        check_element!(self);
        self.lxb_str_to_slice(lxb_dom_element_tag_name)
    }

    unsafe fn local_name_unchecked(&self) -> Option<&str> {
        check_element!(self);
        self.lxb_str_to_slice(lxb_dom_element_local_name)
    }

    unsafe fn id_unchecked(&self) -> Option<&str> {
        check_element!(self);
        self.lxb_str_to_slice(lxb_dom_element_id_noi)
    }

    #[inline]
    unsafe fn name_unchecked(&self) -> Option<&str> {
        self.attribute_unchecked("name")
    }

    unsafe fn class_name_unchecked(&self) -> Option<&str> {
        check_element!(self);
        self.lxb_str_to_slice(lxb_dom_element_class_noi)
    }

    unsafe fn attribute_unchecked(&self, qualified_name: &str) -> Option<&str> {
        check_element!(self);
        let mut size = 0;
        let name = lxb_dom_element_get_attribute(
            self.node as *mut lxb_dom_element_t,
            qualified_name.as_ptr() as *const lxb_char_t,
            qualified_name.len(),
            addr_of_mut!(size));
        match size {
            0 => None,
            _ => Some(std::str::from_utf8_unchecked(slice::from_raw_parts(name.cast(), size)))
        }
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
        todo!()
    }

    fn attribute_names(&self) -> Vec<String> {
        todo!()
    }

    fn set_attribute(&mut self, qualified_name: &str, value: &str) {
        todo!()
    }

    fn remove_attribute(&mut self, qualified_name: &str) {
        todo!()
    }

    fn has_attribute(&self, qualified_name: &str) -> bool {
        todo!()
    }

    fn elements_by_tag_name(&self, qualified_name: &str) -> Collection {
        todo!()
    }

    fn elements_by_class_name(&self, class_names: &str) -> Collection {
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
        if self.tree.upgrade().is_none() | self.node.is_null() || node.node.is_null() {
            return;
        }
        if let Some(mut fc) = self.first_child() {
            fc.before(node);
        } else {
            self.replace_children(&[node]);
        }
    }

    fn append(&mut self, node: &Node) {
        if self.tree.upgrade().is_none() | self.node.is_null() || node.node.is_null() {
            return;
        }
        if let Some(mut lc) = self.last_child() {
            lc.after(node);
        } else {
            self.replace_children(&[node]);
        }
    }

    fn replace_children(&mut self, nodes: &[&Node]) {
        if self.tree.upgrade().is_none() || self.node.is_null() || nodes.len() == 0 {
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
        if self.tree.upgrade().is_none() || self.node.is_null() || node.node.is_null() {
            return;
        }
        unsafe { lxb_dom_node_insert_before(self.node, node.node) }
    }

    fn after(&mut self, node: &Node) {
        if self.tree.upgrade().is_none() || self.node.is_null() || node.node.is_null() {
            return;
        }
        unsafe { lxb_dom_node_insert_after(self.node, node.node) }
    }

    fn replace_with(&mut self, node: &Node) {
        if self.tree.upgrade().is_none() || self.node.is_null() || node.node.is_null() {
            return;
        }
        unsafe { lxb_dom_node_insert_before(self.node, node.node) }
        self.remove();
    }

    fn remove(&mut self) {
        if self.tree.upgrade().is_none() || self.node.is_null() {
            return;
        }
        unsafe { lxb_dom_node_remove(self.node); }
        self.node = ptr::null_mut();
        self.tree = Weak::default();
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
    use crate::parse::html::HTMLTree;

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
