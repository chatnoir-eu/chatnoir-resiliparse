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
use pyo3::create_exception;

use resiliparse_common::parse::html::tree as tree_impl;
use resiliparse_common::parse::html::dom::traits::*;
use super::DOMException;
use super::node::DOMNode;

create_exception!(_html_rs, HTMLParserException, DOMException);


#[pyclass]
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
    pub fn parse_from_bytes(document: &[u8], encoding: &str, errors: &str) -> PyResult<Self> {
        match tree_impl::HTMLTree::try_from(document) {
            Ok(t) => Ok(Self { tree: t }),
            _ => Err(HTMLParserException::new_err("Failed to parse HTML document."))
        }
    }

    pub fn create_element(&mut self, tag_name: &str) -> PyResult<Option<DOMNode>> {
        if let Some(mut d) = self.tree.document() {
            match d.create_element(tag_name) {
                Ok(e) => Ok(Some(e.into())),
                Err(e) => Err(DOMException::new_err(e.to_string()))
            }
        } else {
            Err(DOMException::new_err("No document node."))
        }
    }

    pub fn create_text_node(&mut self, text: &str) -> PyResult<Option<DOMNode>> {
        if let Some(mut d) = self.tree.document() {
            match d.create_text_node(text) {
                Ok(t) => Ok(Some(t.into())),
                Err(e) => Err(DOMException::new_err(e.to_string()))
            }
        } else {
            Err(DOMException::new_err("No document node."))
        }
    }

    #[getter]
    pub fn document(&self) -> Option<DOMNode> {
        Some(self.tree.document()?.to_node().into())
    }

    #[getter]
    pub fn head(&self) -> Option<DOMNode> {
        Some(self.tree.head()?.into())
    }

    #[getter]
    pub fn body(&self) -> Option<DOMNode> {
        Some(self.tree.body()?.into())
    }

    #[getter]
    pub fn title(&self) -> Option<String> {
        self.tree.title()
    }
}
