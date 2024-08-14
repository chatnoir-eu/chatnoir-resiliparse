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

//! HTML parser and DOM tree handle.

use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::ptr;
use std::ptr::addr_of_mut;
use std::sync::{Arc, Mutex};
use std::str::FromStr;

use crate::parse::html::dom::node::*;
use crate::parse::html::dom::node_base::NodeBase;
use crate::parse::html::lexbor::*;
use crate::third_party::lexbor::lexbor_status_t::*;
use crate::third_party::lexbor::*;


/// HTML parser error.
#[derive(Debug)]
pub struct HTMLParserError {
    msg: String
}

impl Display for HTMLParserError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTML parser error: {}.", self.msg)
    }
}

impl Error for HTMLParserError {}


/// CSS parsing error.
#[derive(Debug)]
pub struct CSSParserError {
    msg: String
}

impl Display for CSSParserError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "CSS parser error: {}", self.msg)
    }
}

impl Error for CSSParserError {}


/// HTML DOM tree handle.
pub struct HTMLTree {
    doc: Arc<HTMLDocument>
}

impl HTMLTree {
    /// Parse HTML from a Unicode `str` into a DOM tree.
    ///
    /// Constructs a DOM tree from the given HTML input string.
    /// Returns an [HTMLParserError] if the input could not be processed.
    pub fn parse(html: &str) -> Result<Self, HTMLParserError> {
        let doc_ptr;
        unsafe {
            doc_ptr = lxb_html_document_create();
            if doc_ptr.is_null() {
                return Err(HTMLParserError { msg: "Failed to allocate memory.".to_owned() });
            }
            let status = lxb_html_document_parse(doc_ptr, html.as_ptr(), html.len());
            if status != LXB_STATUS_OK {
                lxb_html_document_destroy(doc_ptr);
                return Err(HTMLParserError { msg: format!("Failed to parse document (Error {}).", status) });
            }
        }

        Ok(HTMLTree { doc: Arc::new(HTMLDocument { html_document: Mutex::new(doc_ptr) }) })
    }
}

impl FromStr for HTMLTree {
    type Err = HTMLParserError;

    /// Parse HTML from a Unicode `str` into a DOM tree.
    ///
    /// This is an alias for [HTMLTree::parse].
    #[inline]
    fn from_str(html: &str) -> Result<Self, Self::Err> {
        Self::parse(html)
    }
}

impl TryFrom<&str> for HTMLTree {
    type Error = HTMLParserError;

    /// Parse HTML from a Unicode `str` into a DOM tree.
    ///
    /// This is an alias for [HTMLTree::parse].
    #[inline]
    fn try_from(html: &str) -> Result<Self, Self::Error> {
        Self::parse(html)
    }
}

impl TryFrom<String> for HTMLTree {
    type Error = HTMLParserError;

    /// Parse HTML from a Unicode `String` into a DOM tree.
    ///
    /// This is an alias for [HTMLTree::parse], but accepts an owned `String`.
    #[inline]
    fn try_from(html: String) -> Result<Self, Self::Error> {
        Self::parse(html.as_str())
    }
}

impl TryFrom<&[u8]> for HTMLTree {
    type Error = HTMLParserError;

    /// Decode a raw HTML byte string and parse it into a DOM tree.
    /// The bytes must be a valid UTF-8 encoding. Invalid characters will be replaced with `U+FFFD`.
    ///
    /// This is an alias for [HTMLTree::parse], but accepts raw bytes.
    #[inline]
    fn try_from(html: &[u8]) -> Result<Self, Self::Error> {
        Self::parse(String::from_utf8_lossy(html).to_mut())
    }
}

impl HTMLTree {
    /// Get Lexbor raw pointer to HTML document.
    fn get_html_document_raw(&self) -> Option<&mut lxb_html_document_t> {
        unsafe { self.doc.doc_ptr()?.as_mut() }
    }

    #[inline]
    /// Get DOM document root node.
    pub fn document(&self) -> Option<DocumentNode> {
        Some(NodeBase::wrap_node(
            &self.doc, addr_of_mut!(self.get_html_document_raw()?.dom_document.node))?.into())
    }

    /// Get HTML `<head>` element node.
    pub fn head(&self) -> Option<ElementNode> {
        Some(ElementNode {
            node_base: NodeBase::new_base(&self.doc, self.get_html_document_raw()?.head as *mut lxb_dom_node_t)?
        })
    }

    /// Get HTML `<body>` element node.
    pub fn body(&self) -> Option<ElementNode> {
        Some(ElementNode {
            node_base: NodeBase::new_base(&self.doc, self.get_html_document_raw()?.body as *mut lxb_dom_node_t)?
        })
    }

    /// Get HTML `<title>` contents as string.
    #[inline]
    pub fn title(&self) -> Option<String> {
        unsafe {
            let mut size = 0;
            let t = lxb_html_document_title(self.get_html_document_raw()?, addr_of_mut!(size));
            Some(str_from_lxb_char_t(t, size)?.to_owned())
        }
    }
}


/// Internal heap-allocated and reference-counted HTMLTree.
#[derive(Debug)]
pub(super) struct HTMLDocument {
    html_document: Mutex<*mut lxb_html_document_t>
}

impl HTMLDocument {
    pub(super) unsafe fn doc_ptr(&self) -> Option<*mut lxb_html_document_t> {
        match self.html_document.lock() {
            Ok(p) => Some(*p),
            _ => None
        }
    }
}

impl Drop for HTMLDocument {
    fn drop(&mut self) {
        if let Ok(mut p) = self.html_document.lock() {
            if !p.is_null() {
                unsafe { lxb_html_document_destroy(*p); };
                *p = ptr::null_mut();
            }
        }
    }
}

unsafe impl Send for HTMLDocument {}
unsafe impl Sync for HTMLDocument {}