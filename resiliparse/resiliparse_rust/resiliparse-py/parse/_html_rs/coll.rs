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

use resiliparse_common::parse::html::dom::coll as coll_impl;
use resiliparse_common::parse::html::dom::traits::NodeInterface;
use super::node::*;

pub enum NodeListType {
    NodeList(coll_impl::NodeList),
    ElementNodeList(coll_impl::ElementNodeList),
    NamedNodeMap(coll_impl::NamedNodeMap),
}

impl From<coll_impl::NodeList> for NodeListType {
    fn from(value: coll_impl::NodeList) -> Self {
        NodeListType::NodeList(value)
    }
}

impl From<coll_impl::ElementNodeList> for NodeListType {
    fn from(value: coll_impl::ElementNodeList) -> Self {
        NodeListType::ElementNodeList(value)
    }
}

impl From<coll_impl::NamedNodeMap> for NodeListType {
    fn from(value: coll_impl::NamedNodeMap) -> Self {
        NodeListType::NamedNodeMap(value)
    }
}

#[pyclass(sequence, frozen)]
pub struct DOMCollection {
    list: NodeListType
}


#[pymethods]
impl DOMCollection {
    // #[pyo3(signature = (element_id, case_insensitive=false))]
    // pub fn get_element_by_id(&self, element_id: &str, case_insensitive: bool) -> PyResult<Option<Node>> {
    //     match &self.list {
    //         NodeListType::ElementNodeList(l) => {
    //             match l.elements_by_attr_case("id", element_id, case_insensitive).item(0) {
    //                 Some(e) => Ok(Some(e.to_node().into())),
    //                 _ => Ok(None)
    //             }
    //         },
    //         _ => Err(PyValueError::new_err("Invalid DOM collection type"))
    //     }
    // }
    //
    // #[pyo3(signature = (attr_name, attr_value, case_insensitive=false))]
    // pub fn get_elements_by_attr(&self, attr_name: &str, attr_value: &str, case_insensitive: bool) -> PyResult<DOMCollection> {
    //     match &self.list {
    //         NodeListType::ElementNodeList(l) => {
    //             Ok(Self { list: l.elements_by_attr_case(attr_name, attr_value, case_insensitive).into() })
    //         },
    //         _ => Err(PyValueError::new_err("Invalid DOM collection type"))
    //     }
    // }
    //
    // pub fn get_elements_by_class_name(&self, class_name: &str) -> PyResult<DOMCollection> {
    //     match &self.list {
    //         NodeListType::ElementNodeList(l) => {
    //             Ok(Self { list: l.elements_by_class_name(class_name).into() })
    //         },
    //         _ => Err(PyValueError::new_err("Invalid DOM collection type"))
    //     }
    // }
    //
    // pub fn get_elements_by_tag_name(&self, tag_name: &str) -> PyResult<DOMCollection> {
    //     match &self.list {
    //         NodeListType::ElementNodeList(l) => {
    //             Ok(Self { list:l.elements_by_tag_name(tag_name).into() })
    //         },
    //         _ => Err(PyValueError::new_err("Invalid DOM collection type"))
    //     }
    // }
    //
    // pub fn query_selector(&self, selector: &str) -> PyResult<Option<Node>> {
    //     match &self.list {
    //         NodeListType::ElementNodeList(l) => {
    //             match l.query_selector(selector) {
    //                 Ok(e) => {
    //                     if let Some(n) = e {
    //                         Ok(Some(n.to_node().into()))
    //                     } else { Ok(None) }
    //                 },
    //                 Err(e) => Err(PyValueError::new_err(e.to_string()))
    //             }
    //         },
    //         _ => Err(PyValueError::new_err("Invalid DOM collection type"))
    //     }
    // }
    //
    // pub fn query_selector_all(&self, selector: &str) -> PyResult<DOMCollection> {
    //     match &self.list {
    //         NodeListType::ElementNodeList(l) => {
    //             Ok(Self { list: l.elements_by_tag_name(selector).into() })
    //         },
    //         _ => Err(PyNotImplementedError::new_err("Invalid DOM collection type"))
    //     }
    // }
    //
    // pub fn matches(&self, selector: &str) -> PyResult<bool> {
    //     Ok(false)
    // }
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
