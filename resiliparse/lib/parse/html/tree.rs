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

use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::ptr;
use std::ptr::addr_of_mut;
use std::rc::Rc;
use std::str::FromStr;

use crate::parse::html::dom::node::*;
use crate::parse::html::dom::node_base::NodeBase;
use crate::parse::html::lexbor::*;
use crate::third_party::lexbor::lexbor_status_t::*;
use crate::third_party::lexbor::*;


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


/// HTML tree handler.
pub struct HTMLTree {
    doc_rc: Rc<HTMLDocument>
}

impl HTMLTree {
    /// Parse HTML from a Unicode str into a DOM tree.
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

        Ok(HTMLTree { doc_rc: Rc::new(HTMLDocument { html_document: doc_ptr }) })
    }
}

impl FromStr for HTMLTree {
    type Err = HTMLParserError;

    /// Parse HTML from a Unicode str into a DOM tree.
    #[inline]
    fn from_str(html: &str) -> Result<Self, Self::Err> {
        Self::parse(html)
    }
}

impl TryFrom<&str> for HTMLTree {
    type Error = HTMLParserError;

    /// Parse HTML from a Unicode str into a DOM tree.
    #[inline]
    fn try_from(html: &str) -> Result<Self, Self::Error> {
        Self::parse(html)
    }
}

impl TryFrom<String> for HTMLTree {
    type Error = HTMLParserError;

    /// Parse HTML from a Unicode str into a DOM tree.
    #[inline]
    fn try_from(html: String) -> Result<Self, Self::Error> {
        Self::parse(html.as_str())
    }
}

impl TryFrom<&[u8]> for HTMLTree {
    type Error = HTMLParserError;

    /// Decode a raw HTML byte string and parse it into a DOM tree.
    /// The bytes must be a valid UTF-8 encoding. Invalid characters will be replaced with U+FFFD.
    #[inline]
    fn try_from(html: &[u8]) -> Result<Self, Self::Error> {
        Self::parse(String::from_utf8_lossy(html).to_mut())
    }
}

impl HTMLTree {
    fn get_html_document_raw(&self) -> Option<&mut lxb_html_document_t> {
        unsafe { self.doc_rc.html_document.as_mut() }
    }

    #[inline]
    pub fn document(&self) -> Option<DocumentNode> {
        Some(NodeBase::wrap_node(
            &self.doc_rc, addr_of_mut!(self.get_html_document_raw()?.dom_document.node))?.into())
    }

    pub fn head(&self) -> Option<ElementNode> {
        Some(ElementNode {
            node_base: NodeBase::new_base(&self.doc_rc, self.get_html_document_raw()?.head as *mut lxb_dom_node_t)?
        })
    }

    pub fn body(&self) -> Option<ElementNode> {
        Some(ElementNode {
            node_base: NodeBase::new_base(&self.doc_rc, self.get_html_document_raw()?.body as *mut lxb_dom_node_t)?
        })
    }

    pub unsafe fn title_unchecked(&self) -> Option<&str> {
        let mut size = 0;
        let t = lxb_html_document_title(self.get_html_document_raw()?, addr_of_mut!(size));
        str_from_lxb_char_t(t, size)
    }

    #[inline]
    pub fn title(&self) -> Option<String> {
        unsafe { Some(self.title_unchecked()?.to_owned()) }
    }
}


/// Internal heap-allocated and reference-counted HTMLTree.
#[derive(Debug)]
pub(super) struct HTMLDocument {
    pub(super) html_document: *mut lxb_html_document_t
}

impl Drop for HTMLDocument {
    fn drop(&mut self) {
        if !self.html_document.is_null() {
            unsafe { lxb_html_document_destroy(self.html_document); }
            self.html_document = ptr::null_mut();
        }
    }
}
