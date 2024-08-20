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

use std::ops::Deref;
use pyo3::prelude::*;
use pyo3::exceptions::*;
use pyo3::types::*;
use resiliparse_common::parse::html::dom::coll as coll_impl;
use resiliparse_common::parse::html::dom::traits::NodeInterface;
use super::node::*;

pub enum NL {
    NodeList(coll_impl::NodeList),
    ElementNodeList(coll_impl::ElementNodeList),
    NamedNodeMap(coll_impl::NamedNodeMap),
}

impl From<coll_impl::NodeList> for NL {
    fn from(value: coll_impl::NodeList) -> Self {
        NL::NodeList(value)
    }
}

impl From<coll_impl::ElementNodeList> for NL {
    fn from(value: coll_impl::ElementNodeList) -> Self {
        NL::ElementNodeList(value)
    }
}

impl From<coll_impl::NamedNodeMap> for NL {
    fn from(value: coll_impl::NamedNodeMap) -> Self {
        NL::NamedNodeMap(value)
    }
}

#[pyclass(subclass, sequence, frozen, module = "resiliparse.parse._html_rs.coll")]
pub struct NodeList {
    list: NL
}


impl NodeList {
    fn new_bound(py: Python, list: coll_impl::NodeList) -> Bound<Self> {
        Bound::new(py, Self { list: list.into() }).unwrap()
    }
}


#[pymethods]
impl NodeList {
    #[new]
    fn new(py: Python) -> Bound<Self> {
        Self::new_bound(py, coll_impl::NodeList::default())
    }
    fn item<'py>(&self, index: usize, py: Python<'py>) -> Option<Bound<'py, PyAny>> {
        Some(create_upcast_node(py, match &self.list {
            NL::NodeList(l) => l.item(index)?,
            NL::ElementNodeList(l) => l.item(index)?.as_node(),
            NL::NamedNodeMap(l) => l.item(index)?.as_node(),
        }))
    }

    // fn values<'py>(&self, py: Python<'py>) -> Bound<'py, PyTuple> {
    //     // PyTuple::new_bound::<PyAny>(py, match &self.list {
    //     //     NL::NodeList(l) => l.values().into_iter().map(|n| create_upcast_node(py, n)).collect(),
    //     //     NL::ElementNodeList(l) => l.values().into_iter().map(|n| create_upcast_node(py, n.as_node())).collect(),
    //     //     NL::NamedNodeMap(l) => l.values().into_iter().map(|n| create_upcast_node(py, n.as_node())).collect()
    //     // })
    //     let items = match &self.list {
    //         NL::NodeList(l) => l.values().into_iter().map(|n| create_upcast_node(py, n)).collect(),
    //         NL::ElementNodeList(l) => l.values().into_iter().map(|n| create_upcast_node(py, n.as_node())).collect(),
    //         NL::NamedNodeMap(l) => l.values().into_iter().map(|n| create_upcast_node(py, n.as_node())).collect()
    //     };
    //     PyTuple::new_bound(py, items)
    // }

    fn __len__(&self) -> usize {
        match &self.list {
            NL::NodeList(l) => l.len(),
            NL::ElementNodeList(l) => l.len(),
            NL::NamedNodeMap(l) => l.len(),
        }
    }

    fn __contains__<'py>(&self, node: Bound<'py, PyAny>) -> bool {
        if let Ok(n) = node.downcast::<Node>() {
            match &self.list {
                NL::NodeList(l) => l.iter().any(|i| i == n.borrow().node),
                NL::ElementNodeList(l) => l.iter().any(|i| i.as_node() == n.borrow().node),
                NL::NamedNodeMap(l) => l.iter().any(|i| i.as_node() == n.borrow().node),
            }
        } else {
            false
        }
    }
    //
    // fn __getitem__<'py>(&self, py: Python<'py>, index: usize) -> PyResult<Bound<'py, PyAny>> {
    //     match &self.list {
    //         NL::NodeList(l) |
    //         NL::ElementNodeList(l) |
    //         NL::NamedNodeMap(l) => {
    //             if let Some(n) = l.item(index) {
    //                 Ok(create_upcast_node(py, n))
    //             } else {
    //                 Err(PyIndexError::new_err("List index out of range."))
    //             }
    //         },
    //         _ => unreachable!()
    //     }
    // }
}


#[pyclass(subclass, sequence, frozen, extends = NodeList, module = "resiliparse.parse._html_rs.coll")]
#[derive(Clone)]
pub struct ElementNodeList {}

impl ElementNodeList {
    fn new_bound(py: Python, list: coll_impl::NodeList) -> Bound<Self> {
        Bound::new(py, (Self {}, NodeList { list: list.into() })).unwrap()
    }
}

#[pymethods]
impl ElementNodeList {
    #[new]
    fn new(py: Python) -> Bound<Self> {
        Self::new_bound(py, coll_impl::NodeList::default())
    }

    // #[pyo3(signature = (element_id, case_insensitive=false))]
    // pub fn get_element_by_id(slf: PyRef<Self>, element_id: &str, case_insensitive: bool) -> PyResult<Option<Node>> {
    //     match &slf.as_super().list {
    //         NodeListType::ElementNodeList(l) => {
    //             match l.elements_by_attr_case("id", element_id, case_insensitive).item(0) {
    //                 Some(e) => Ok(Some(e.as_node().into())),
    //                 _ => Ok(None)
    //             }
    //         },
    //         _ => Err(PyValueError::new_err("Invalid DOM collection type"))
    //     }
    // }
//
//     #[pyo3(signature = (attr_name, attr_value, case_insensitive=false))]
//     pub fn get_elements_by_attr(&self, attr_name: &str, attr_value: &str, case_insensitive: bool) -> PyResult<NodeList> {
//         match &self.list {
//             NodeListType::ElementNodeList(l) => {
//                 Ok(Self { list: l.elements_by_attr_case(attr_name, attr_value, case_insensitive).into() })
//             },
//             _ => Err(PyValueError::new_err("Invalid DOM collection type"))
//         }
//     }
//
//     pub fn get_elements_by_class_name(&self, class_name: &str) -> PyResult<NodeList> {
//         match &self.list {
//             NodeListType::ElementNodeList(l) => {
//                 Ok(Self { list: l.elements_by_class_name(class_name).into() })
//             },
//             _ => Err(PyValueError::new_err("Invalid DOM collection type"))
//         }
//     }
//
//     pub fn get_elements_by_tag_name(&self, tag_name: &str) -> PyResult<NodeList> {
//         match &self.list {
//             NodeListType::ElementNodeList(l) => {
//                 Ok(Self { list:l.elements_by_tag_name(tag_name).into() })
//             },
//             _ => Err(PyValueError::new_err("Invalid DOM collection type"))
//         }
//     }
//
//     pub fn query_selector(&self, selector: &str) -> PyResult<Option<Node>> {
//         match &self.list {
//             NodeListType::ElementNodeList(l) => {
//                 match l.query_selector(selector) {
//                     Ok(e) => {
//                         if let Some(n) = e {
//                             Ok(Some(n.as_node().into()))
//                         } else { Ok(None) }
//                     },
//                     Err(e) => Err(PyValueError::new_err(e.to_string()))
//                 }
//             },
//             _ => Err(PyValueError::new_err("Invalid DOM collection type"))
//         }
//     }
//
//     pub fn query_selector_all(&self, selector: &str) -> PyResult<NodeList> {
//         match &self.list {
//             NodeListType::ElementNodeList(l) => {
//                 Ok(Self { list: l.elements_by_tag_name(selector).into() })
//             },
//             _ => Err(PyNotImplementedError::new_err("Invalid DOM collection type"))
//         }
//     }
//
//     pub fn matches(&self, selector: &str) -> PyResult<bool> {
//         Ok(false)
//     }
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
