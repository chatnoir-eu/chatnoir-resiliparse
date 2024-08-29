// Copyright 2024 Janek Bevendorff
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

use pyo3::prelude::*;
use pyo3::types::*;
use resiliparse_common::parse::html::tree as tree_impl;
use crate::exception::*;
use crate::node::*;


#[pyclass(module = "resiliparse.parse._html_rs")]
pub struct HTMLTree {
    tree: tree_impl::HTMLTree
}

#[pymethods]
impl HTMLTree {
    #[staticmethod]
    pub fn parse(document: &str) -> PyResult<Self> {
        match tree_impl::HTMLTree::parse(document) {
            Ok(t) => Ok(Self { tree: t }),
            _ => Err(HTMLParserException::new_err("Failed to parse HTML document."))
        }
    }

    #[allow(unused_variables)]
    #[staticmethod]
    #[pyo3(signature = (document, encoding="utf-8", errors="ignore"))]
    pub fn parse_from_bytes<'py>(document: &Bound<'py, PyBytes>, encoding: &str, errors: &str) -> PyResult<Self> {
        match tree_impl::HTMLTree::try_from(document.as_bytes()) {
            Ok(t) => Ok(Self { tree: t }),
            _ => Err(HTMLParserException::new_err("Failed to parse HTML document."))
        }
    }

    pub fn create_element<'py>(slf: PyRef<'py, Self>, tag_name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        Self::document(slf).map_or_else(
            || Err(DOMException::new_err("No document node.")),
            |d| DocumentNode::create_element(d.borrow_mut(), tag_name)
        )
    }

    pub fn create_text_node<'py>(slf: PyRef<'py, Self>, text: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        Self::document(slf).map_or_else(
            || Err(DOMException::new_err("No document node.")),
            |d| DocumentNode::create_text_node(d.borrow_mut(), text)
        )
    }

    #[getter]
    pub fn document(slf: PyRef<'_, Self>) -> Option<Bound<'_, DocumentNode>> {
        DocumentNode::new_bound(slf.py(), slf.tree.document()?).ok()
    }

    #[getter]
    pub fn head(slf: PyRef<'_, Self>) -> Option<Bound<'_, ElementNode>> {
        ElementNode::new_bound(slf.py(), slf.tree.head()?).ok()
    }

    #[getter]
    pub fn body(slf: PyRef<'_, Self>) -> Option<Bound<'_, ElementNode>> {
        ElementNode::new_bound(slf.py(), slf.tree.body()?).ok()
    }

    #[getter]
    pub fn title(slf: PyRef<'_, Self>) -> Option<String> {
        slf.tree.title()
    }

    pub fn __str__(slf: PyRef<'_, Self>) -> String {
        slf.tree.document().map_or_else(
            || "<Invalid Document>".to_owned(),
            |d| format!("{}", d)
        )
    }
}
