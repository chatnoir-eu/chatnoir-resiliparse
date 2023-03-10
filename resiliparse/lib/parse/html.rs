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
use std::ptr::addr_of_mut;
use std::rc::{Rc, Weak};

use crate::third_party::lexbor::*;

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
    html_document: Rc<HTMLTreeRc>
}

impl From<&[u8]> for HTMLTree {
    fn from(value: &[u8]) -> Self {
        let doc_ptr;
        unsafe {
            doc_ptr = lxb_html_document_create();
            lxb_html_document_parse(doc_ptr, value.as_ptr(), value.len());
        }

        HTMLTree { html_document: Rc::new(HTMLTreeRc { html_document: doc_ptr }) }
    }
}

impl From<&str> for HTMLTree {
    #[inline]
    fn from(value: &str) -> Self {
        value.as_bytes().into()
    }
}

impl From<&String> for HTMLTree {
    #[inline]
    fn from(value: &String) -> Self {
        value.as_bytes().into()
    }
}

impl HTMLTree {
    #[inline]
    pub fn from_bytes(html: &[u8]) -> HTMLTree {
        html.into()
    }

    #[inline]
    pub fn from_str(html: &str) -> HTMLTree {
        html.into()
    }

    #[inline]
    pub fn from_string(html: &String) -> HTMLTree {
        html.into()
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

/// DOM node.
pub struct DOMNode {
    tree: Weak<HTMLTreeRc>,
    node: *mut lxb_dom_node_t
}

impl DOMNode {
    #[inline]
    fn new(tree: &Rc<HTMLTreeRc>, node: *mut lxb_dom_node_t) -> Self {
        Self { tree: Rc::downgrade(tree), node }
    }

    /// DOM node type.
    pub fn node_type(&self) -> NodeType {
        match self.tree.upgrade() {
            Some(_) => unsafe { (*self.node).type_.into() },
            _ => NodeType::Undefined
        }
    }

    /// DOM element tag or node name.
    pub fn tag(&self) -> Option<String> {
        unsafe { Some(self.tag_unsafe()?.to_owned()) }
    }

    /// DOM element tag or node name.
    pub unsafe fn tag_unsafe(&self) -> Option<&str> {
        self.tree.upgrade()?;
        unsafe {
            let mut size = 0;
            let name= lxb_dom_node_name(self.node, addr_of_mut!(size));
            match size {
                0 => None,
                _ => Some(std::str::from_utf8_unchecked(slice::from_raw_parts(name.cast(), size)))
            }
        }
    }

    /// First child element of this DOM node.
    pub fn first_child(&self) -> Option<Self> {
        let t = self.tree.upgrade()?;
        unsafe {
            if (*self.node).first_child.is_null() {
                None
            } else {
                Some(Self::new(&t, (*self.node).first_child))
            }
        }
    }

    /// Last child element of this DOM node.
    pub fn last_child(&self) -> Option<Self> {
        let t = self.tree.upgrade()?;
        unsafe {
            if (*self.node).last_child.is_null() {
                None
            } else {
                Some(Self::new(&t, (*self.node).last_child))
            }
        }
    }

    /// First element child of this DOM node.
    pub fn first_element_child(&self) -> Option<Self> {
        let mut child = self.first_child()?;
        loop {
            if child.node_type() == NodeType::Element {
                return Some(child);
            }
            child = child.next_sibling()?;
        }
    }

    /// Last element child element of this DOM node.
    pub fn last_element_child(&self) -> Option<Self> {
        let mut child = self.last_child()?;
        loop {
            if child.node_type() == NodeType::Element {
                return Some(child);
            }
            child = child.prev_sibling()?;
        }
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

    /// List of child element nodes.
    pub fn child_element_nodes(&self) -> Vec<Self> {
        let mut nodes = Vec::new();
        let mut child = self.first_element_child();
        while let Some(c) = child {
            child = c.next_element_sibling();
            nodes.push(c);
        }
        nodes
    }

    /// Parent of this node.
    pub fn parent(&self) -> Option<Self> {
        let t = self.tree.upgrade()?;
        unsafe {
            if !(*self.node).parent.is_null() {
                Some(Self::new(&t, (*self.node).parent))
            } else {
                None
            }
        }
    }

    /// Next sibling node.
    pub fn next_sibling(&self) -> Option<Self> {
        let t = self.tree.upgrade()?;
        unsafe {
            if !(*self.node).next.is_null() {
                Some(Self::new(&t, (*self.node).next))
            } else {
                None
            }
        }
    }

    /// Previous sibling node.
    pub fn prev_sibling(&self) -> Option<Self> {
        let t = self.tree.upgrade()?;
        unsafe {
            if !(*self.node).prev.is_null() {
                Some(Self::new(&t, (*self.node).prev))
            } else {
                None
            }
        }
    }

    /// Next sibling element node.
    pub fn next_element_sibling(&self) -> Option<Self> {
        loop {
            let s = self.next_sibling()?;
            if s.node_type() == NodeType::Element {
                return Some(s);
            }
        }
    }

    /// Previous sibling element node.
    pub fn prev_element_sibling(&self) -> Option<Self> {
        loop {
            let s = self.prev_sibling()?;
            if s.node_type() == NodeType::Element {
                return Some(s);
            }
        }
    }

    /// Node text value.
    #[inline]
    pub fn value(&self) -> Option<String> {
        unsafe { Some(self.value_unsafe()?.to_owned()) }
    }

    /// Node text value.
    pub unsafe fn value_unsafe(&self) -> Option<&str> {
        self.tree.upgrade()?;
        unsafe {
            let cdata = self.node as *const lxb_dom_character_data_t;
            Some(std::str::from_utf8_unchecked(slice::from_raw_parts(
                (*cdata).data.data.cast(), (*cdata).data.length)))
        }
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
            lxb_dom_document_destroy_text_noi((*self.node).owner_document, t);
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
            lexbor_str_destroy(h, (*(*node.node).owner_document).text, true);
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

// Collection of DOM nodes that are the result set of an element matching operation.
//
// A node collection is only valid for as long as the owning :class:`HTMLTree` is alive
// and the DOM tree hasn't been modified. Do not access :class:`DOMCollection` instances
// after any sort of DOM tree manipulation.
// pub struct DOMCollection<'a> {
//     tree: &'a HTMLTree,
//     coll: *mut lxb_dom_collection_t
// }
//
// impl<'a> DOMCollection<'_> {
//     fn new(tree: &HTMLTree, coll: *mut lxb_dom_collection_t) -> DOMCollection {
//         DOMCollection { tree, coll }
//     }
// }

// cdef inline DOMCollection _create_dom_collection(HTMLTree tree, lxb_dom_collection_t* coll):
//     cdef DOMCollection return_coll = DOMCollection.__new__(DOMCollection, tree)
//     return_coll.coll = coll
//     return return_coll
//
//
// cdef bint init_css_parser(lxb_css_parser_t** parser) nogil except 0:
//     parser[0] = lxb_css_parser_create()
//     if lxb_css_parser_init(parser[0], NULL) != LXB_STATUS_OK:
//         with gil:
//             raise RuntimeError('Failed to initialize CSS parser.')
//     return True
//
//
// cdef void destroy_css_parser(lxb_css_parser_t* parser) nogil:
//     if parser:
//         lxb_css_parser_destroy(parser, True)
//
//
// cdef bint init_css_selectors(lxb_css_parser_t* parser, lxb_css_selectors_t** css_selectors,
//                              lxb_selectors_t** selectors) nogil except 0:
//     css_selectors[0] = lxb_css_selectors_create()
//     if lxb_css_selectors_init(css_selectors[0]) != LXB_STATUS_OK:
//         with gil:
//             raise RuntimeError('Failed to initialize CSS selectors.')
//
//     lxb_css_parser_selectors_set(parser, css_selectors[0])
//
//     selectors[0] = lxb_selectors_create()
//     if lxb_selectors_init(selectors[0]) != LXB_STATUS_OK:
//         with gil:
//             raise RuntimeError('Failed to initialize element selectors.')
//
//     return True
//
//
// cdef void destroy_css_selectors(lxb_css_selectors_t* css_selectors, lxb_selectors_t* selectors) nogil:
//     if selectors:
//         lxb_selectors_destroy(selectors, True)
//     if css_selectors:
//         lxb_css_selectors_destroy(css_selectors, True)
//
//
// cdef inline void _log_serialize_cb(const lxb_char_t *data, size_t len, void *ctx) nogil:
//     (<string*>ctx).append(<const char*>data, len)
//
//
// cdef lxb_css_selector_list_t* parse_css_selectors(lxb_css_parser_t* css_parser, const lxb_char_t* selector,
//                                                   size_t selector_len) nogil except NULL:
//     cdef lxb_css_selector_list_t* sel_list = lxb_css_selectors_parse(css_parser, selector, selector_len)
//     cdef string err
//     if css_parser.status != LXB_STATUS_OK:
//         lxb_css_log_serialize(css_parser.log, <lexbor_serialize_cb_f>_log_serialize_cb, &err, <const lxb_char_t*>b'', 0)
//         with gil:
//             raise ValueError(f'CSS parser error: {err.decode().strip()}')
//     return sel_list
