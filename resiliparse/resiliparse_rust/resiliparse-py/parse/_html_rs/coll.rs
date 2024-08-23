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
    pub fn new_bound(py: Python, list: coll_impl::NodeList) -> PyResult<Bound<Self>> {
        Bound::new(py, Self { list: list.into() })
    }
}

fn get_tuple_slice<'py>(tup: &Bound<'py, PyTuple>, index_or_slice: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if let Ok(s) = index_or_slice.downcast::<PySlice>() {
        let i = s.indices(tup.len() as isize)?;
        let e = tup.get_slice(i.start as usize, i.stop as usize)
            .iter()
            .step_by(i.step.abs() as usize);
        Ok(PyTuple::new_bound(index_or_slice.py(), e).into_any())
    } else if let Ok(i) = index_or_slice.downcast::<PyInt>() {
        if i.lt(i)? {
            i.add(tup.len())?;
        }
        tup.get_item(i.extract()?)
    } else {
        Err(PyTypeError::new_err("Indices must be integers or slices"))
    }
}


#[pymethods]
impl NodeList {
    pub fn item<'py>(&self, index: usize, py: Python<'py>) -> Option<Bound<'py, PyAny>> {
        create_upcast_node(py, match &self.list {
            NL::NodeList(l) => l.item(index)?,
            NL::ElementNodeList(l) => l.item(index)?.into_node(),
            NL::NamedNodeMap(l) => l.item(index)?.into_node(),
        }).ok()
    }

    pub fn values<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
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

    pub fn __len__(&self) -> usize {
        match &self.list {
            NL::NodeList(l) => l.len(),
            NL::ElementNodeList(l) => l.len(),
            NL::NamedNodeMap(l) => l.len(),
        }
    }

    pub fn __contains__<'py>(&self, node: &Bound<'py, PyAny>) -> bool {
        node.downcast::<Node>().map_or(false, |n| {
            match &self.list {
                NL::NodeList(l) => l.iter().any(|i| i == n.borrow().node),
                NL::ElementNodeList(l) => l.iter().any(|i| i.as_node() == n.borrow().node),
                NL::NamedNodeMap(l) => l.iter().any(|i| i.as_node() == n.borrow().node),
            }
        })
    }

    #[inline(always)]
    pub fn __getitem__<'py>(&self, index_or_slice: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        get_tuple_slice(&self.values(index_or_slice.py())?, index_or_slice)
    }
}


#[pyclass(subclass, sequence, frozen, extends = NodeList, module = "resiliparse.parse._html_rs.coll")]
pub struct ElementNodeList {}

impl ElementNodeList {
    pub fn new_bound(py: Python, list: coll_impl::ElementNodeList) -> PyResult<Bound<Self>> {
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

    //noinspection DuplicatedCode
    pub fn query_selector<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Option<Bound<'py, ElementNode>>> {
        if let NL::ElementNodeList(l) = &slf.as_super().list {
            return l.query_selector(selectors).map_or_else(
                |e| Err(CSSParserException::new_err(e.to_string())),
                |e| e.map_or(
                    Ok(None),
                    |e_| Ok(Some(ElementNode::new_bound(slf.py(), e_)?))
                )
            );
        }
        Err(PyValueError::new_err("Invalid DOM collection type"))
    }

    //noinspection DuplicatedCode
    pub fn query_selector_all<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Bound<'py, ElementNodeList>> {
        if let NL::ElementNodeList(l) = &slf.as_super().list {
            return l.query_selector_all(selectors).map_or_else(
                |e| Err(CSSParserException::new_err(e.to_string())),
                |e| Ok(ElementNodeList::new_bound(slf.py(), e)?)
            );
        }
        Err(PyValueError::new_err("Invalid DOM collection type"))
    }

    pub fn matches(slf: PyRef<Self>, selectors: &str) -> PyResult<bool> {
        if let NL::ElementNodeList(l) = &slf.as_super().list {
            return l.matches(selectors)
                .or_else(|e| Err(CSSParserException::new_err(e.to_string())))
        }
        Err(PyValueError::new_err("Invalid DOM collection type"))
    }
}


#[pyclass(subclass, sequence, frozen, extends = NodeList, module = "resiliparse.parse._html_rs.coll")]
pub struct NamedNodeMap {}

impl NamedNodeMap {
    pub fn new_bound(py: Python, list: coll_impl::NamedNodeMap) -> PyResult<Bound<Self>> {
        Bound::new(py, (Self {}, NodeList { list: list.into() }))
    }
}


#[pyclass(subclass, eq, sequence, module = "resiliparse.parse._html_rs.coll")]
#[derive(PartialEq, Eq)]
pub struct DOMTokenList {
    list: coll_impl::DOMTokenListOwned,
}

impl From<coll_impl::DOMTokenListOwned> for DOMTokenList {
    fn from(list: coll_impl::DOMTokenListOwned) -> Self {
        Self { list }
    }
}

impl From<coll_impl::DOMTokenList<'_>> for DOMTokenList {
    fn from(list: coll_impl::DOMTokenList<'_>) -> Self {
        Self { list: list.into() }
    }
}

impl From<coll_impl::DOMTokenListMut<'_>> for DOMTokenList {
    fn from(list: coll_impl::DOMTokenListMut<'_>) -> Self {
        Self { list: list.into() }
    }
}

#[pymethods]
impl DOMTokenList {
    #[getter]
    pub fn value(&self) -> String {
        self.list.value()
    }

    #[setter]
    pub fn set_value(&mut self, value: &str) {
        self.list.set_value(value)
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

    //noinspection DuplicatedCode
    #[pyo3(signature = (*token))]
    pub fn add<'py>(&mut self, token: &Bound<'py, PyTuple>) -> PyResult<()> {
        self.list.add(token.extract::<Vec<String>>()?
            .iter()
            .map(String::as_str)
            .collect::<Vec<&str>>()
            .as_slice());
        Ok(())
    }

    //noinspection DuplicatedCode
    #[pyo3(signature = (*token))]
    pub fn remove<'py>(&mut self, token: &Bound<'py, PyTuple>) -> PyResult<()> {
        self.list.remove(token.extract::<Vec<String>>()?
            .iter()
            .map(String::as_str)
            .collect::<Vec<&str>>()
            .as_slice());
        Ok(())
    }

    pub fn replace(&mut self, old_token: &str, new_token: &str) -> bool {
        self.list.replace(old_token, new_token)
    }

    #[pyo3(signature = (token, force=None))]
    pub fn toggle(&mut self, token: &str, force: Option<bool>) -> bool {
        self.list.toggle(token, force)
    }

    pub fn __getitem__<'py>(&self, index_or_slice: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        get_tuple_slice(&PyTuple::new_bound(index_or_slice.py(), self.list.values().into_iter()), index_or_slice)
    }

    pub fn __len__(&self) -> usize {
        self.list.len()
    }

    pub fn __contains__<'py>(&self, token: &Bound<'py, PyAny>) -> bool {
        token.extract::<String>().map_or(false, |s| self.list.contains(&s))
    }
}
