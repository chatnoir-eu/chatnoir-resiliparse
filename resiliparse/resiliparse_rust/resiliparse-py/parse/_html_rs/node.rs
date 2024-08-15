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
use pyo3::exceptions::*;
use pyo3::types::*;

use resiliparse_common::parse::html::dom::node as node_impl;
use resiliparse_common::parse::html::dom::traits::*;
use super::coll::*;


#[pyclass(eq, eq_int, rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(PartialEq, Eq)]
pub enum NodeType {
    Element = 0x01,
    Attribute = 0x02,
    Text = 0x03,
    CdataSection = 0x04,
    EntityReference = 0x05,
    Entity = 0x06,
    ProcessingInstruction = 0x07,
    Comment = 0x08,
    Document = 0x09,
    DocumentType = 0x0A,
    DocumentFragment = 0x0B,
    Notation = 0x0C,
    LastEntry = 0x0D
}


#[pyclass]
pub struct DOMNode {
    node: node_impl::Node
}

impl From<node_impl::Node> for DOMNode {
    fn from(value: node_impl::Node) -> Self {
        Self { node: value }
    }
}

impl From<node_impl::ElementNode> for DOMNode {
    fn from(value: node_impl::ElementNode) -> Self {
        Self { node: value.to_node() }
    }
}

impl From<node_impl::TextNode> for DOMNode {
    fn from(value: node_impl::TextNode) -> Self {
        Self { node: value.to_node() }
    }
}


#[pymethods]
impl DOMNode {
    #[getter]
    #[pyo3(name = "type")]
    pub fn type_(&self) -> PyResult<NodeType> {
        Ok(NodeType::Element)
    }

    #[getter]
    pub fn first_child(&self) -> Option<DOMNode> {
        Some(self.node.first_child()?.into())
    }

    #[getter]
    pub fn first_element_child(&self) -> Option<DOMNode> {
        if let node_impl::Node::Element(e) = &self.node {
            Some(e.first_element_child()?.to_node().into())
        } else { None }
    }

    #[getter]
    pub fn last_element_child(&self) -> Option<DOMNode> {
        if let node_impl::Node::Element(e) = &self.node {
            Some(e.last_element_child()?.to_node().into())
        } else { None }
    }

    #[getter]
    pub fn child_nodes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        Ok(PyList::empty_bound(py))
    }

    #[getter]
    pub fn parent(&self) -> Option<DOMNode> {
        if let node_impl::Node::Element(e) = &self.node {
            Some(e.parent_node()?.to_node().into())
        } else { None }
    }

    #[getter]
    pub fn next(&self) -> Option<DOMNode> {
        if let node_impl::Node::Element(e) = &self.node {
            Some(e.next_sibling()?.to_node().into())
        } else { None }
    }

    #[getter]
    pub fn prev(&self) -> Option<DOMNode> {
        if let node_impl::Node::Element(e) = &self.node {
            Some(e.previous_sibling()?.to_node().into())
        } else { None }
    }

    #[getter]
    pub fn next_element(&self) -> Option<DOMNode> {
        if let node_impl::Node::Element(e) = &self.node {
            Some(e.next_element_sibling()?.to_node().into())
        } else { None }
    }

    #[getter]
    pub fn prev_element(&self) -> Option<DOMNode> {
        if let node_impl::Node::Element(e) = &self.node {
            Some(e.previous_element_sibling()?.to_node().into())
        } else { None }
    }

    #[getter]
    pub fn tag(&self) -> PyResult<&str> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    #[getter]
    pub fn value(&self) -> PyResult<&str> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    #[getter]
    pub fn get_text(&self) -> PyResult<&str> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    #[setter]
    pub fn set_text(&mut self, text: &str) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    #[getter]
    pub fn get_html(&self) -> PyResult<&str> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    #[setter]
    pub fn set_html(&mut self, html: &str) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    #[getter]
    pub fn get_id(&self) -> PyResult<&str> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    #[setter]
    pub fn set_id(&mut self, id: &str) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    #[getter]
    pub fn get_class_name(&self) -> PyResult<&str> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    #[setter]
    pub fn set_class_name(&mut self, class_name: &str) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    #[getter]
    pub fn get_class_list(&self) -> PyResult<DOMElementClassList> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    #[getter]
    pub fn attrs<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    pub fn hasattr(&self, attr_name: &str) -> PyResult<bool> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    #[pyo3(signature = (attr_name, default_value=None))]
    pub fn getattr(&self, attr_name: &str, default_value: Option<&str>) -> PyResult<&str> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    pub fn setattr(&mut self, attr_name: &str, attr_value: &str) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    pub fn delattr(&mut self, attr_name: &str) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    #[pyo3(signature = (element_id, case_insensitive=false))]
    pub fn get_element_by_id(&self, element_id: &str, case_insensitive: Option<bool>) -> PyResult<DOMNode> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    #[pyo3(signature = (attr_name, attr_value, case_insensitive=false))]
    pub fn get_elements_by_attr(&self, attr_name: &str, attr_value: &str, case_insensitive: Option<bool>) -> PyResult<DOMCollection> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    #[pyo3(signature = (class_name, case_insensitive=false))]
    pub fn get_elements_by_class_name(&self, class_name: &str, case_insensitive: Option<bool>) -> PyResult<DOMCollection> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    pub fn get_elements_by_tag_name(&self, tag_name: &str) -> PyResult<DOMCollection> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    pub fn query_selector(&self, selector: &str) -> PyResult<DOMNode> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    pub fn query_selector_all(&self, selector: &str) -> PyResult<DOMCollection> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    pub fn matches(&self, selector: &str) -> PyResult<bool> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    pub fn append_child(&mut self, node: &DOMNode) -> PyResult<DOMNode> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    pub fn insert_before(&mut self, node: &DOMNode, reference: &DOMNode) -> PyResult<DOMNode> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    pub fn replace_child(&mut self, new_child: &DOMNode, old_child: &DOMNode) -> PyResult<DOMNode> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    pub fn remove_child(&mut self, node: &DOMNode) -> PyResult<DOMNode> {
        Err(PyNotImplementedError::new_err("TODO"))
    }

    pub fn decompose(&mut self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err("TODO"))
    }
}
