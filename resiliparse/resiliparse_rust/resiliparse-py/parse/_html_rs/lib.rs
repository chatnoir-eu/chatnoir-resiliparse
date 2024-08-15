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
use pyo3::exceptions::*;

use resiliparse_common::parse::html;
use resiliparse_common::parse::html::dom::coll::*;
use resiliparse_common::parse::html::dom::node::*;
use resiliparse_common::parse::html::dom::traits::*;


#[pymodule]
#[allow(unused_variables)]
pub mod _html_rs {
    use super::*;

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

    pub enum NodeListType {
        NodeList(NodeList),
        ElementNodeList(ElementNodeList),
        NamedNodeMap(NamedNodeMap),
    }

    impl From<NodeList> for NodeListType {
        fn from(value: NodeList) -> Self {
            NodeListType::NodeList(value)
        }
    }

    impl From<ElementNodeList> for NodeListType {
        fn from(value: ElementNodeList) -> Self {
            NodeListType::ElementNodeList(value)
        }
    }

    impl From<NamedNodeMap> for NodeListType {
        fn from(value: NamedNodeMap) -> Self {
            NodeListType::NamedNodeMap(value)
        }
    }

    #[pyclass(sequence, frozen)]
    pub struct DOMCollection {
        list: NodeListType
    }


    #[pymethods]
    //noinspection DuplicatedCode
    impl DOMCollection {
        #[pyo3(signature = (element_id, case_insensitive=false))]
        pub fn get_element_by_id(&self, element_id: &str, case_insensitive: Option<bool>) -> Option<DOMNode> {
            None
        }

        #[pyo3(signature = (attr_name, attr_value, case_insensitive=false))]
        pub fn get_elements_by_attr(&self, attr_name: &str, attr_value: &str, case_insensitive: bool) -> PyResult<DOMCollection> {
            match &self.list {
                NodeListType::ElementNodeList(l) => {
                    Ok(Self { list: l.elements_by_attr_case(attr_name, attr_value, case_insensitive).into() })
                },
                _ => Err(PyValueError::new_err("Invalid DOM collection type"))
            }
        }

        pub fn get_elements_by_class_name(&self, class_name: &str) -> PyResult<DOMCollection> {
            match &self.list {
                NodeListType::ElementNodeList(l) => {
                    Ok(Self { list: l.elements_by_class_name(class_name).into() })
                },
                _ => Err(PyValueError::new_err("Invalid DOM collection type"))
            }
        }

        pub fn get_elements_by_tag_name(&self, tag_name: &str) -> PyResult<DOMCollection> {
            match &self.list {
                NodeListType::ElementNodeList(l) => {
                    Ok(Self { list:l.elements_by_tag_name(tag_name).into() })
                },
                _ => Err(PyValueError::new_err("Invalid DOM collection type"))
            }
        }

        pub fn query_selector(&self, selector: &str) -> PyResult<Option<DOMNode>> {
            match &self.list {
                NodeListType::ElementNodeList(l) => {
                    match l.query_selector(selector) {
                        Ok(e) => {
                            if let Some(n) = e {
                                Ok(Some(n.into()))
                            } else { Ok(None) }
                        },
                        Err(e) => Err(PyValueError::new_err(e.to_string()))
                    }
                },
                _ => Err(PyValueError::new_err("Invalid DOM collection type"))
            }
        }

        pub fn query_selector_all(&self, selector: &str) -> PyResult<DOMCollection> {
            match &self.list {
                NodeListType::ElementNodeList(l) => {
                    Ok(Self { list: l.elements_by_tag_name(selector).into() })
                },
                _ => Err(PyValueError::new_err("Invalid DOM collection type"))
            }
        }

        pub fn matches(&self, selector: &str) -> PyResult<bool> {
            Ok(false)
        }
    }

    #[pyclass(eq, sequence)]
    #[derive(PartialEq, Eq)]
    pub struct DOMElementClassList {}

    #[pymethods]
    impl DOMElementClassList {
        #[new]
        pub fn __new__() -> Self {
            DOMElementClassList {}
        }

        pub fn add(&self, class_name: &str) -> PyResult<()> {
            Ok(())
        }

        pub fn remove(&mut self, class_name: &str) -> PyResult<()> {
            Ok(())
        }
    }

    #[pyclass]
    pub struct DOMNode {
        node: Node
    }

    impl From<Node> for DOMNode {
        fn from(value: Node) -> Self {
            Self { node: value }
        }
    }

    impl From<ElementNode> for DOMNode {
        fn from(value: ElementNode) -> Self {
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
            if let Node::Element(e) = &self.node {
                Some(e.first_element_child()?.to_node().into())
            } else { None }
        }

        #[getter]
        pub fn last_element_child(&self) -> Option<DOMNode> {
            if let Node::Element(e) = &self.node {
                Some(e.last_element_child()?.to_node().into())
            } else { None }
        }

        #[getter]
        pub fn child_nodes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
            Ok(PyList::empty_bound(py))
        }

        #[getter]
        pub fn parent(&self) -> Option<DOMNode> {
            if let Node::Element(e) = &self.node {
                Some(e.parent_node()?.to_node().into())
            } else { None }
        }

        #[getter]
        pub fn next(&self) -> Option<DOMNode> {
            if let Node::Element(e) = &self.node {
                Some(e.next_sibling()?.to_node().into())
            } else { None }
        }

        #[getter]
        pub fn prev(&self) -> Option<DOMNode> {
            if let Node::Element(e) = &self.node {
                Some(e.previous_sibling()?.to_node().into())
            } else { None }
        }

        #[getter]
        pub fn next_element(&self) -> Option<DOMNode> {
            if let Node::Element(e) = &self.node {
                Some(e.next_element_sibling()?.to_node().into())
            } else { None }
        }

        #[getter]
        pub fn prev_element(&self) -> Option<DOMNode> {
            if let Node::Element(e) = &self.node {
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

    #[pyclass]
    pub struct HTMLTree {
        tree: html::tree::HTMLTree
    }

    #[pymethods]
    impl HTMLTree {
        #[staticmethod]
        pub fn parse(document: &str) -> PyResult<Self> {
            match html::tree::HTMLTree::parse(document) {
                Ok(t) => Ok(Self { tree: t }),
                _ => Err(PyValueError::new_err("Failed to parse HTML document."))
            }
        }

        #[staticmethod]
        #[pyo3(signature = (document, encoding="utf-8", errors="ignore"))]
        pub fn parse_from_bytes(document: &[u8], encoding: &str, errors: &str) -> PyResult<Self> {
            match html::tree::HTMLTree::try_from(document) {
                Ok(t) => Ok(Self { tree: t }),
                _ => Err(PyValueError::new_err("Failed to parse HTML document."))
            }
        }

        pub fn create_element(&mut self, tag_name: &str) -> PyResult<Option<DOMNode>> {
            if let Some(mut d) = self.tree.document() {
                match d.create_element(tag_name) {
                    Ok(e) => Ok(Some(e.to_node().into())),
                    Err(e) => Err(PyValueError::new_err(e.to_string()))
                }
            } else {
                Err(PyValueError::new_err("No document node."))
            }
        }

        pub fn create_text_node(&mut self, text: &str) -> PyResult<Option<DOMNode>> {
            if let Some(mut d) = self.tree.document() {
                match d.create_text_node(text) {
                    Ok(t) => Ok(Some(t.to_node().into())),
                    Err(e) => Err(PyValueError::new_err(e.to_string()))
                }
            } else {
                Err(PyValueError::new_err("No document node."))
            }
        }

        #[getter]
        pub fn document(&self) -> Option<DOMNode> {
            Some(self.tree.document()?.to_node().into())
        }

        #[getter]
        pub fn head(&self) -> Option<DOMNode> {
            Some(self.tree.head()?.to_node().into())
        }

        #[getter]
        pub fn body(&self) -> Option<DOMNode> {
            Some(self.tree.body()?.to_node().into())
        }

        #[getter]
        pub fn title(&self) -> Option<String> {
            self.tree.title()
        }
    }
}
