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

use std::ptr;
use std::ptr::addr_of_mut;
use std::rc::Rc;

use crate::parse::html::dom::{Node, NodeBase, str_from_lxb_char_t};
use crate::third_party::lexbor::*;


/// Internal heap-allocated and reference-counted HTMLTree.
pub(super) struct HTMLTreeRc {
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
        NodeBase::new(
            &self.tree_rc,
            addr_of_mut!(self.get_html_document_raw()?.dom_document) as *mut lxb_dom_node_t)
    }

    pub fn head(&self) -> Option<Node> {
        NodeBase::new(&self.tree_rc, self.get_html_document_raw()?.head as *mut lxb_dom_node_t)
    }

    pub fn body(&self) -> Option<Node> {
        NodeBase::new(&self.tree_rc, self.get_html_document_raw()?.body as *mut lxb_dom_node_t)
    }

    pub unsafe fn title_unchecked(&self) -> Option<&str> {
        let mut size = 0;
        let t = lxb_html_document_title(self.get_html_document_raw()?, addr_of_mut!(size));
        match size {
            0 => None,
            _ => Some(str_from_lxb_char_t(t, size))
        }
    }

    #[inline]
    pub fn title(&self) -> Option<String> {
        unsafe { Some(self.title_unchecked()?.to_owned()) }
    }
}
