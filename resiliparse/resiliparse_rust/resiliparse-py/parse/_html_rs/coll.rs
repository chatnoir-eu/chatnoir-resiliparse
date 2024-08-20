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
use resiliparse_common::parse::html::dom::coll as coll_impl;
use resiliparse_common::parse::html::dom::coll::{DOMTokenListInterface, DOMTokenListMutInterface};
use resiliparse_common::parse::html::dom::traits::NodeInterface;
use crate::exception::CSSParserException;
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
    fn new_bound(py: Python, list: coll_impl::NodeList) -> PyResult<Bound<Self>> {
        Bound::new(py, Self { list: list.into() })
    }
}


#[pymethods]
impl NodeList {
    fn item<'py>(&self, index: usize, py: Python<'py>) -> Option<Bound<'py, PyAny>> {
        create_upcast_node(py, match &self.list {
            NL::NodeList(l) => l.item(index)?,
            NL::ElementNodeList(l) => l.item(index)?.into_node(),
            NL::NamedNodeMap(l) => l.item(index)?.into_node(),
        }).ok()
    }

    fn values<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let items: Vec<Bound<'py, PyAny>> = match &self.list {
            NL::NodeList(l) => l.values().into_iter()
                .map(|n| create_upcast_node(py, n).unwrap()).collect(),
            NL::ElementNodeList(l) => l.values().into_iter()
                .map(|n| create_upcast_node(py, n.into_node()).unwrap()).collect(),
            NL::NamedNodeMap(l) => l.values().into_iter()
                .map(|n| create_upcast_node(py, n.into_node()).unwrap()).collect()
        };
        Ok(PyTuple::new_bound(py, items))
    }

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

    fn __getitem__<'py>(&self, py: Python<'py>, index_or_slice: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let val = self.values(py)?;
        if let Ok(s) = index_or_slice.downcast::<PySlice>() {
            let i = s.indices(val.len() as isize)?;
            let e = val.get_slice(i.start as usize, i.stop as usize)
                .iter()
                .step_by(i.step.abs() as usize);
            Ok(PyTuple::new_bound(py, e).into_any())
        } else if let Ok(mut i) = index_or_slice.downcast::<PyInt>() {
            if i.lt(i)? {
                i.add(val.len())?;
            }
            val.get_item(i.extract()?)
        } else {
            Err(PyTypeError::new_err("NodeList indices must be integers or slices"))
        }
    }
}


#[pyclass(subclass, sequence, frozen, extends = NodeList, module = "resiliparse.parse._html_rs.coll")]
pub struct ElementNodeList {}

impl ElementNodeList {
    fn new_bound(py: Python, list: coll_impl::ElementNodeList) -> PyResult<Bound<Self>> {
        Bound::new(py, (Self {}, NodeList { list: list.into() }))
    }
}

#[pymethods]
impl ElementNodeList {
    #[pyo3(signature = (element_id, case_insensitive=false))]
    pub fn get_element_by_id<'py>(slf: PyRef<'py, Self>, element_id: &str,
                                  case_insensitive: bool) -> PyResult<Option<Bound<'py, ElementNode>>> {
        if let NL::ElementNodeList(l) = &slf.as_super().list {
            let res = l.elements_by_attr_case("id", element_id, case_insensitive)
                .item(0).map(|n| ElementNode::new_bound(slf.py(), n).unwrap());
            return Ok(res)
        }
        Err(PyValueError::new_err("Invalid DOM collection type"))
    }

    #[pyo3(signature = (attr_name, attr_value, case_insensitive=false))]
    pub fn get_elements_by_attr<'py>(slf: PyRef<'py, Self>, attr_name: &str,
                                     attr_value: &str, case_insensitive: bool) -> PyResult<Bound<'py, ElementNodeList>> {
        if let NL::ElementNodeList(l) = &slf.as_super().list {
            return Ok(Self::new_bound(slf.py(), l.elements_by_attr_case(attr_name, attr_value, case_insensitive).into())?)
        }
        Err(PyValueError::new_err("Invalid DOM collection type"))
    }

    pub fn get_elements_by_class_name<'py>(slf: PyRef<'py, Self>, class_name: &str) -> PyResult<Bound<'py, ElementNodeList>> {
        if let NL::ElementNodeList(l) = &slf.as_super().list {
            return Ok(Self::new_bound(slf.py(), l.elements_by_class_name(class_name).into())?)
        }
        Err(PyValueError::new_err("Invalid DOM collection type"))
    }

    pub fn get_elements_by_tag_name<'py>(slf: PyRef<'py, Self>, tag_name: &str) -> PyResult<Bound<'py, ElementNodeList>> {
        if let NL::ElementNodeList(l) = &slf.as_super().list {
            return Ok(Self::new_bound(slf.py(), l.elements_by_tag_name(tag_name).into())?)
        }
        Err(PyValueError::new_err("Invalid DOM collection type"))
    }

    pub fn query_selector<'py>(slf: PyRef<'py, Self>, selector: &str) -> PyResult<Option<Bound<'py, ElementNode>>> {
        if let NL::ElementNodeList(l) = &slf.as_super().list {
            let res = l.query_selector(selector)
                .or_else(|e| Err(CSSParserException::new_err(e.to_string())))?;
            return Ok(res.map(|e| ElementNode::new_bound(slf.py(), e).unwrap()))
        }
        Err(PyValueError::new_err("Invalid DOM collection type"))
    }

    pub fn query_selector_all<'py>(slf: PyRef<'py, Self>, selector: &str) -> PyResult<Bound<'py, ElementNodeList>> {
        if let NL::ElementNodeList(l) = &slf.as_super().list {
            let res = l.query_selector_all(selector)
                .or_else(|e| Err(CSSParserException::new_err(e.to_string())))?;
            return Ok(Self::new_bound(slf.py(), res)?)
        }
        Err(PyValueError::new_err("Invalid DOM collection type"))
    }

    pub fn matches(slf: PyRef<Self>, selector: &str) -> PyResult<bool> {
        if let NL::ElementNodeList(l) = &slf.as_super().list {
            return l.matches(selector)
                .or_else(|e| Err(CSSParserException::new_err(e.to_string())))
        }
        Err(PyValueError::new_err("Invalid DOM collection type"))
    }
}


#[pyclass(subclass, sequence, frozen, extends = NodeList, module = "resiliparse.parse._html_rs.coll")]
pub struct NamedNodeMap {}

impl NamedNodeMap {
    fn new_bound(py: Python, list: coll_impl::NamedNodeMap) -> PyResult<Bound<Self>> {
        Bound::new(py, (Self {}, NodeList { list: list.into() }))
    }
}


#[pyclass(eq, sequence)]
#[derive(PartialEq, Eq)]
pub struct DOMTokenList {
    list: coll_impl::DOMTokenListOwned,
}

#[pymethods]
impl DOMTokenList {
    pub fn value(&self) -> String {
        self.list.value()
    }

    pub fn values<'py>(&self, py: Python<'py>) -> Bound<'py, PyTuple> {
        PyTuple::new_bound(py, self.list.iter())
    }

    pub fn item(&self, index: usize) -> Option<String> {
        self.list.item(index)
    }

    pub fn contains(&self, token: &str) -> bool {
        self.list.contains(token)
    }

    #[pyo3(signature = (*tokens))]
    pub fn add<'py>(&mut self, tokens: &Bound<'py, PyTuple>) -> PyResult<()> {
        if tokens.is_empty() {
            return Err(PyValueError::new_err("At least one class required."))
        }
        let c = tokens.extract::<Vec<String>>()?;
        Ok(self.list.add(&c.iter().map(String::as_str).collect::<Vec<&str>>()))
    }

    pub fn remove(&mut self, class_name: &str) -> PyResult<()> {
        Ok(())
    }
}
