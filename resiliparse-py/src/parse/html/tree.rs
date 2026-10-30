// Copyright 2026 Janek Bevendorff
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

use super::exception::*;
use super::node::*;
use pyo3::prelude::*;
use pyo3::types::*;
use resiliparse::parse::html::tree;

/// HTML DOM tree parser.
#[pyclass(name = "HTMLTree", unsendable, module = "resiliparse.parse.html")]
pub struct HTMLTreePy {
    pub(crate) tree: tree::HtmlTree,
}

#[pymethods]
impl HTMLTreePy {
    /// Parse HTML from a Unicode string into a DOM tree.
    ///
    /// :param document: Unicode HTML document to parse.
    /// :return: Parsed HTML document tree.
    /// :raise HTMLParserException: If parsing fails
    #[staticmethod]
    pub fn parse(document: &str) -> PyResult<Self> {
        match tree::HtmlTree::parse(document) {
            Ok(t) => Ok(Self { tree: t }),
            _ => Err(HTMLParserExceptionPy::new_err("Failed to parse HTML document.")),
        }
    }

    /// Decode a raw HTML byte string and parse it into a DOM tree.
    ///
    /// The encoding name is remapped according to the WHATWG specification before decoding. The ``errors`` policy follows ``bytes.decode()``.
    ///
    /// :param document: Raw HTML byte string to decode and parse.
    /// :param encoding: Character encoding used to decode the input.
    /// :param errors: Decoding error policy, as accepted by ``bytes.decode()``.
    /// :return: Parsed HTML document tree.
    /// :raise HTMLParserException: If parsing fails
    /// :raise LookupError: If a decoding error handler is unknown and is needed
    /// :raise UnicodeError: If decoding or re-encoding fails under the selected error policy
    #[allow(unused_variables)]
    #[staticmethod]
    #[pyo3(signature = (document, encoding="utf-8", errors="ignore"))]
    pub fn parse_from_bytes<'py>(
        py: Python<'py>,
        document: &Bound<'py, PyBytes>,
        encoding: &str,
        errors: &str,
    ) -> PyResult<Self> {
        let encoding_module = py.import("resiliparse.parse.encoding")?;
        let encoding: String = encoding_module
            .call_method1("map_encoding_to_html5", (encoding,))?
            .extract()?;
        let decoded: String = encoding_module
            .call_method1("bytes_to_str", (document, encoding, errors))?
            .extract()?;
        match tree::HtmlTree::parse(&decoded) {
            Ok(t) => Ok(Self { tree: t }),
            _ => Err(HTMLParserExceptionPy::new_err("Failed to parse HTML document.")),
        }
    }

    /// Create a new DOM element node.
    ///
    /// :param tag_name: Tag name for the new element.
    /// :raise DOMException: If the document is unavailable or the node cannot be created
    pub fn create_element<'py>(slf: PyRef<'py, Self>, tag_name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        Self::document(slf).map_or_else(
            || Err(DOMExceptionPy::new_err("No document node.")),
            |d| DocumentNodePy::create_element(d.borrow(), tag_name),
        )
    }

    /// Create a new text node with the given string contents.
    ///
    /// :param text: Text to store in the node.
    /// :raise DOMException: If the document is unavailable or the node cannot be created
    pub fn create_text_node<'py>(slf: PyRef<'py, Self>, text: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        Self::document(slf).map_or_else(
            || Err(DOMExceptionPy::new_err("No document node.")),
            |d| DocumentNodePy::create_text_node(d.borrow(), text),
        )
    }

    /// The document node.
    #[getter]
    pub fn document(slf: PyRef<'_, Self>) -> Option<Bound<'_, DocumentNodePy>> {
        DocumentNodePy::new_bound(slf.py(), slf.tree.document()?).ok()
    }

    /// HTML head element or ``None`` if the document has no head.
    #[getter]
    pub fn head(slf: PyRef<'_, Self>) -> Option<Bound<'_, ElementNodePy>> {
        ElementNodePy::new_bound(slf.py(), slf.tree.head()?).ok()
    }

    /// HTML body element or ``None`` if the document has no body.
    #[getter]
    pub fn body(slf: PyRef<'_, Self>) -> Option<Bound<'_, ElementNodePy>> {
        ElementNodePy::new_bound(slf.py(), slf.tree.body()?).ok()
    }

    /// The HTML document title.
    ///
    /// A missing or empty HTML title element returns ``None``. Whitespace-only
    /// title text is normalized to an empty string by the parser.
    ///
    /// .. versionchanged:: 2.0.0
    ///    Returns ``None`` instead of an empty string for missing or empty title elements.
    #[getter]
    pub fn title(slf: PyRef<'_, Self>) -> Option<String> {
        slf.tree.title()
    }

    /// Return the serialized HTML document.
    pub fn __str__(slf: PyRef<'_, Self>) -> String {
        slf.tree
            .document()
            .map_or_else(|| "<Invalid Document>".to_owned(), |d| format!("{}", d))
    }
}
