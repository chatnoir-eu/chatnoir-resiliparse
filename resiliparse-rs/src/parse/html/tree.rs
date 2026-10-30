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

//! Parse HTML and access the resulting document tree.
//!
//! [`HtmlTree::parse`] constructs a document from a string of markup. The tree
//! handle provides access to the document, its head and body elements, and its
//! title.

use crate::parse::html::dom::node::*;
use crate::parse::html::lexbor::*;
use crate::third_party::lexbor::lexbor_status_t::*;
use crate::third_party::lexbor::*;
use parking_lot::{ReentrantMutex, ReentrantMutexGuard};
use std::cell::Cell;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::marker::PhantomData;
use std::ptr;
use std::ptr::addr_of_mut;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// An HTML parser error.
#[derive(Debug)]
pub struct HtmlParserError {
    msg: String,
}

impl Display for HtmlParserError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTML parser error: {}.", self.msg)
    }
}

impl Error for HtmlParserError {}

/// HTML DOM tree handle.
pub struct HtmlTree {
    doc: Arc<HtmlDocument>,
    _not_sync: PhantomData<Cell<()>>,
}

impl HtmlTree {
    /// Parse HTML from a Unicode `str` into a DOM tree.
    ///
    /// Constructs a DOM tree from the given HTML input string.
    /// Returns an [HtmlParserError] if the input could not be processed.
    ///
    /// # Arguments
    ///
    /// * `html` - HTML markup to parse into a DOM tree.
    ///
    /// # Returns
    ///
    /// The parsed HTML document tree.
    pub fn parse(html: &str) -> Result<Self, HtmlParserError> {
        let doc_ptr;
        unsafe {
            doc_ptr = lxb_html_document_create();
            if doc_ptr.is_null() {
                return Err(HtmlParserError {
                    msg: "Failed to allocate memory.".to_owned(),
                });
            }
            let status = lxb_html_document_parse(doc_ptr, html.as_ptr(), html.len());
            if status != LXB_STATUS_OK as lxb_status_t {
                lxb_html_document_destroy(doc_ptr);
                return Err(HtmlParserError {
                    msg: format!("Failed to parse document (Error {}).", status),
                });
            }
        }

        Ok(HtmlTree {
            doc: Arc::new(HtmlDocument::new(doc_ptr)),
            _not_sync: PhantomData,
        })
    }
}

impl FromStr for HtmlTree {
    type Err = HtmlParserError;

    /// Parse HTML from a Unicode `str` into a DOM tree.
    ///
    /// This is an alias for [HtmlTree::parse].
    #[inline]
    fn from_str(html: &str) -> Result<Self, Self::Err> {
        Self::parse(html)
    }
}

impl TryFrom<&str> for HtmlTree {
    type Error = HtmlParserError;

    /// Parse HTML from a Unicode `str` into a DOM tree.
    ///
    /// This is an alias for [HtmlTree::parse].
    #[inline]
    fn try_from(html: &str) -> Result<Self, Self::Error> {
        Self::parse(html)
    }
}

impl TryFrom<String> for HtmlTree {
    type Error = HtmlParserError;

    /// Parse HTML from a Unicode `String` into a DOM tree.
    ///
    /// This is an alias for [HtmlTree::parse], but accepts an owned `String`.
    #[inline]
    fn try_from(html: String) -> Result<Self, Self::Error> {
        Self::parse(html.as_str())
    }
}

impl TryFrom<&[u8]> for HtmlTree {
    type Error = HtmlParserError;

    /// Decode a raw HTML byte string and parse it into a DOM tree.
    /// The bytes must be a valid UTF-8 encoding. Invalid characters will be replaced with `U+FFFD`.
    ///
    /// This is an alias for [HtmlTree::parse], but accepts raw bytes.
    #[inline]
    fn try_from(html: &[u8]) -> Result<Self, Self::Error> {
        Self::parse(String::from_utf8_lossy(html).to_mut())
    }
}

impl HtmlTree {
    /// Get Lexbor raw pointer to HTML document.
    fn get_html_document_ptr(&self) -> *mut lxb_html_document_t {
        *self.doc.doc_ptr()
    }

    /// Get DOM document root node.
    ///
    /// # Returns
    ///
    /// The document node, if available.
    #[inline]
    pub fn document(&self) -> Option<DocumentNode> {
        let _guard = self.doc.doc_ptr();
        let ptr = self.get_html_document_ptr();
        if !ptr.is_null() {
            Some(DocumentNode::new(&self.doc, ptr.cast()))
        } else {
            None
        }
    }

    /// Get HTML `<head>` element node.
    ///
    /// # Returns
    ///
    /// The document’s head element, if present.
    pub fn head(&self) -> Option<ElementNode> {
        let _guard = self.doc.doc_ptr();
        let ptr = self.get_html_document_ptr();
        if !ptr.is_null() {
            Some(ElementNode::new(&self.doc, unsafe { *ptr }.head.cast()))
        } else {
            None
        }
    }

    /// Get HTML `<body>` element node.
    ///
    /// # Returns
    ///
    /// The document’s body element, if present.
    pub fn body(&self) -> Option<ElementNode> {
        let _guard = self.doc.doc_ptr();
        let ptr = self.get_html_document_ptr();
        if !ptr.is_null() {
            Some(ElementNode::new(&self.doc, unsafe { *ptr }.body.cast()))
        } else {
            None
        }
    }

    /// Get HTML `<title>` contents as string.
    ///
    /// # Returns
    ///
    /// The document title, if present.
    #[inline]
    pub fn title(&self) -> Option<String> {
        let _guard = self.doc.doc_ptr();
        unsafe {
            let mut size = 0;
            let t = lxb_html_document_title(self.get_html_document_ptr(), addr_of_mut!(size));
            Some(str_from_lxb_char_t(t, size)?.to_owned())
        }
    }
}

/// Internal heap-allocated and reference-counted HTMLTree.
#[derive(Debug)]
pub(crate) struct HtmlDocument {
    html_document: ReentrantMutex<*mut lxb_html_document_t>,
    mutation_epoch: AtomicU64,
}

// SAFETY: this internal owner is always accessed through the document lock.
// Cloneable public handles can be sent separately, so they still need this one
// shared lock even though the public tree and node handles are deliberately !Sync.
unsafe impl Send for HtmlDocument {}
unsafe impl Sync for HtmlDocument {}

impl HtmlDocument {
    pub(crate) fn doc_ptr(&self) -> ReentrantMutexGuard<'_, *mut lxb_html_document_t> {
        self.html_document.lock()
    }

    /// The document lock must cover checking this counter AND using native
    /// pointers. Atomicity alone cannot prevent destruction between those steps.
    pub(crate) fn mutation_epoch(&self) -> u64 {
        self.mutation_epoch.load(Ordering::Relaxed)
    }

    /// Call under the document lock, before changing links or destroying nodes.
    pub(crate) fn mark_mutated(&self) {
        self.mutation_epoch
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |epoch| epoch.checked_add(1))
            .expect("DOM mutation counter exhausted");
    }

    pub(crate) fn new(doc: *mut lxb_html_document_t) -> Self {
        HtmlDocument {
            html_document: ReentrantMutex::new(doc),
            mutation_epoch: AtomicU64::new(0),
        }
    }
}

impl Drop for HtmlDocument {
    fn drop(&mut self) {
        let doc = self.html_document.get_mut();
        if !doc.is_null() {
            unsafe {
                lxb_html_document_destroy(*doc);
                *doc = ptr::null_mut();
            }
        }
    }
}

// ===========================================================
// Tests
// ===========================================================

#[cfg(test)]
#[path = "tree_test.rs"]
mod tree_test;
