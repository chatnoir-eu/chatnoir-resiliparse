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

use super::node::*;
use crate::exception::CSSParserException;
use pyo3::exceptions::*;
use pyo3::prelude::*;
use pyo3::types::*;
use resiliparse::parse::html::dom::coll as coll_impl;
use resiliparse::parse::html::dom::coll::{DomTokenListInterface, DomTokenListMutInterface};
use resiliparse::parse::html::dom::traits::NodeInterface;

/// NL exposed by the HTML DOM API.
pub enum NL {
    NodeList(coll_impl::NodeList),
    LiveNodes(coll_impl::LiveCollection<resiliparse::parse::html::dom::node::Node>),
    LiveElements(coll_impl::HtmlCollection),
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

impl From<coll_impl::LiveCollection<resiliparse::parse::html::dom::node::Node>> for NL {
    fn from(value: coll_impl::LiveCollection<resiliparse::parse::html::dom::node::Node>) -> Self {
        Self::LiveNodes(value)
    }
}

impl From<coll_impl::HtmlCollection> for NL {
    fn from(value: coll_impl::HtmlCollection) -> Self {
        Self::LiveElements(value)
    }
}

impl NL {
    fn elements(&self) -> Option<std::borrow::Cow<'_, coll_impl::ElementNodeList>> {
        match self {
            Self::ElementNodeList(l) => Some(std::borrow::Cow::Borrowed(l)),
            Self::LiveElements(l) => Some(std::borrow::Cow::Owned(l.snapshot())),
            _ => None,
        }
    }
}

/// An ordered collection of DOM nodes.
#[pyclass(unsendable, subclass, sequence, frozen, module = "resiliparse.parse._html_rs.coll")]
pub struct NodeList {
    list: NL,
}

impl NodeList {
    pub fn new_bound(py: Python, list: impl Into<NL>) -> PyResult<Bound<Self>> {
        Bound::new(py, Self { list: list.into() })
    }
}
fn get_tuple_slice<'py>(tup: &Bound<'py, PyTuple>, index_or_slice: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    tup.as_any().get_item(index_or_slice)
}

#[pymethods]
impl NodeList {
    /// Return the item at the given index, or ``None`` if it is out of range.
    ///
    /// :param index: Zero-based item index.
    pub fn item<'py>(&self, index: usize, py: Python<'py>) -> Option<Bound<'py, PyAny>> {
        create_upcast_node(
            py,
            match &self.list {
                NL::NodeList(l) => l.item(index)?.clone(),
                NL::LiveNodes(l) => l.item(index)?,
                NL::LiveElements(l) => l.item(index)?.into_node(),
                NL::ElementNodeList(l) => l.item(index)?.clone().into_node(),
                NL::NamedNodeMap(l) => l.item(index)?.into_node(),
            },
        )
        .ok()
    }

    /// Return the collection values as a list.
    pub fn values<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let items: Vec<Bound<'py, PyAny>> = match &self.list {
            NL::NodeList(l) => l
                .values()
                .into_iter()
                .map(|n| create_upcast_node(py, n).unwrap())
                .collect(),
            NL::LiveNodes(l) => l
                .values()
                .into_iter()
                .map(|n| create_upcast_node(py, n).unwrap())
                .collect(),
            NL::ElementNodeList(l) => l
                .values()
                .into_iter()
                .map(|n| create_upcast_node(py, n.into_node()).unwrap())
                .collect(),
            NL::LiveElements(l) => l
                .values()
                .into_iter()
                .map(|n| create_upcast_node(py, n.into_node()).unwrap())
                .collect(),
            NL::NamedNodeMap(l) => l
                .values()
                .into_iter()
                .map(|n| create_upcast_node(py, n.into_node()).unwrap())
                .collect(),
        };
        PyList::new(py, items)
    }

    /// Return the number of items in this collection.
    pub fn __len__(&self) -> usize {
        match &self.list {
            NL::NodeList(l) => l.len(),
            NL::LiveNodes(l) => l.len(),
            NL::LiveElements(l) => l.len(),
            NL::ElementNodeList(l) => l.len(),
            NL::NamedNodeMap(l) => l.len(),
        }
    }

    /// Return whether the requested node, token, or value is present.
    ///
    /// :param node: Node to check for containment.
    pub fn __contains__<'py>(&self, node: &Bound<'py, PyAny>) -> bool {
        node.cast::<Node>().map_or(false, |n| match &self.list {
            NL::NodeList(l) => l.iter().any(|i| *i == n.borrow().node),
            NL::LiveNodes(l) => l.iter().any(|i| i == n.borrow().node),
            NL::LiveElements(l) => l.iter().any(|i| i.as_node() == n.borrow().node),
            NL::ElementNodeList(l) => l.iter().any(|i| i.as_node() == n.borrow().node),
            NL::NamedNodeMap(l) => l.iter().any(|i| i.as_node() == n.borrow().node),
        })
    }

    /// Return a readable representation of this object.
    pub fn __repr__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Ok(self.values(py)?.call_method0("__repr__")?)
    }

    /// Return the node at the given index, or a collection containing the nodes in a slice. Negative indexes are supported.
    ///
    /// :param index_or_slice: Integer index or slice.
    #[inline(always)]
    pub fn __getitem__<'py>(&self, index_or_slice: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        get_tuple_slice(&self.values(index_or_slice.py())?.to_tuple(), index_or_slice)
    }
}

/// An ordered collection of element nodes.
#[pyclass(unsendable, subclass, sequence, frozen, extends = NodeList, module = "resiliparse.parse._html_rs.coll")]
pub struct ElementNodeList {}

impl ElementNodeList {
    pub fn new_bound(py: Python, list: impl Into<NL>) -> PyResult<Bound<Self>> {
        Bound::new(py, (Self {}, NodeList { list: list.into() }))
    }
}

#[pymethods]
impl ElementNodeList {
    /// Find the element in this collection whose ID attribute matches ``element_id``.
    ///
    /// :param element_id: ID value to search for.
    /// :param case_insensitive: Whether ID matching ignores case.
    #[pyo3(signature = (element_id, case_insensitive=false))]
    pub fn get_element_by_id<'py>(
        slf: PyRef<'py, Self>,
        element_id: &str,
        case_insensitive: bool,
    ) -> PyResult<Option<Bound<'py, ElementNode>>> {
        if let Some(l) = slf.as_super().list.elements() {
            let res = l
                .elements_by_attr_case("id", element_id, case_insensitive)
                .item(0)
                .cloned()
                .map(|n| ElementNode::new_bound(slf.py(), n).unwrap());
            return Ok(res);
        }
        Err(PyValueError::new_err("Invalid DOM collection type"))
    }

    /// Find elements in this collection matching the given attribute name and value.
    ///
    /// :param attr_name: Attribute name to match.
    /// :param attr_value: Attribute value to match.
    /// :param case_insensitive: Whether attribute value matching ignores case.
    #[pyo3(signature = (attr_name, attr_value, case_insensitive=false))]
    pub fn get_elements_by_attr<'py>(
        slf: PyRef<'py, Self>,
        attr_name: &str,
        attr_value: &str,
        case_insensitive: bool,
    ) -> PyResult<Bound<'py, ElementNodeList>> {
        if let Some(l) = slf.as_super().list.elements() {
            return Ok(Self::new_bound(slf.py(), l.elements_by_attr_case(attr_name, attr_value, case_insensitive))?);
        }
        Err(PyValueError::new_err("Invalid DOM collection type"))
    }

    /// Find elements in this collection matching the given class name.
    ///
    /// :param class_name: Class name token to match.
    /// :param case_insensitive: Legacy option; class-name matching remains case-sensitive.
    #[pyo3(signature = (class_name, case_insensitive=false))]
    pub fn get_elements_by_class_name<'py>(
        slf: PyRef<'py, Self>,
        class_name: &str,
        case_insensitive: bool,
    ) -> PyResult<Bound<'py, ElementNodeList>> {
        let _ = case_insensitive;
        if let Some(l) = slf.as_super().list.elements() {
            return Ok(Self::new_bound(slf.py(), l.elements_by_class_name(class_name))?);
        }
        Err(PyValueError::new_err("Invalid DOM collection type"))
    }

    /// Find elements in this collection with the given tag name.
    ///
    /// :param tag_name: Tag name to match.
    pub fn get_elements_by_tag_name<'py>(
        slf: PyRef<'py, Self>,
        tag_name: &str,
    ) -> PyResult<Bound<'py, ElementNodeList>> {
        if let Some(l) = slf.as_super().list.elements() {
            return Ok(Self::new_bound(slf.py(), l.elements_by_tag_name(tag_name))?);
        }
        Err(PyValueError::new_err("Invalid DOM collection type"))
    }

    // noinspection DuplicatedCode
    /// Find the first matching element in this collection. This is more efficient than calling :meth:`query_selector_all` and discarding additional results.
    ///
    /// :param selectors: CSS selector expression.
    pub fn query_selector<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Option<Bound<'py, ElementNode>>> {
        if let Some(l) = slf.as_super().list.elements() {
            return l.query_selector(selectors).map_or_else(
                |e| Err(CSSParserException::new_err(e.to_string())),
                |e| e.map_or(Ok(None), |e_| Ok(Some(ElementNode::new_bound(slf.py(), e_)?))),
            );
        }
        Err(PyValueError::new_err("Invalid DOM collection type"))
    }

    // noinspection DuplicatedCode
    /// Find all matching elements in this collection and return an :class:`ElementNodeList`.
    ///
    /// :param selectors: CSS selector expression.
    pub fn query_selector_all<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Bound<'py, ElementNodeList>> {
        if let Some(l) = slf.as_super().list.elements() {
            return l.query_selector_all(selectors).map_or_else(
                |e| Err(CSSParserException::new_err(e.to_string())),
                |e| Ok(ElementNodeList::new_bound(slf.py(), e)?),
            );
        }
        Err(PyValueError::new_err("Invalid DOM collection type"))
    }

    /// Check whether any element in this collection matches the given CSS selector.
    ///
    /// :param selectors: CSS selector expression.
    pub fn matches(slf: PyRef<Self>, selectors: &str) -> PyResult<bool> {
        if let Some(l) = slf.as_super().list.elements() {
            return l
                .matches(selectors)
                .or_else(|e| Err(CSSParserException::new_err(e.to_string())));
        }
        Err(PyValueError::new_err("Invalid DOM collection type"))
    }
}

/// A collection of named attributes.
#[pyclass(unsendable, subclass, sequence, frozen, extends = NodeList, module = "resiliparse.parse._html_rs.coll")]
pub struct NamedNodeMap {}

impl NamedNodeMap {
    pub fn new_bound(py: Python, list: coll_impl::NamedNodeMap) -> PyResult<Bound<Self>> {
        Bound::new(py, (Self {}, NodeList { list: list.into() }))
    }
}

/// Class name list of an element DOM node.
#[pyclass(unsendable, subclass, eq, sequence, module = "resiliparse.parse._html_rs.coll")]
#[derive(PartialEq)]
pub struct DOMTokenList {
    list: coll_impl::DomTokenListOwned,
}

impl From<coll_impl::DomTokenListOwned> for DOMTokenList {
    fn from(list: coll_impl::DomTokenListOwned) -> Self {
        Self { list }
    }
}

impl From<coll_impl::DomTokenList<'_>> for DOMTokenList {
    fn from(list: coll_impl::DomTokenList<'_>) -> Self {
        Self { list: list.into() }
    }
}

#[pymethods]
impl DOMTokenList {
    /// The node value.
    #[getter]
    pub fn value(&self) -> String {
        self.list.value()
    }

    /// Set the value associated with this node or token list.
    ///
    /// :param value: Value to assign or match.
    #[setter]
    pub fn set_value(&self, value: &str) {
        self.list.set_value(value)
    }

    /// Return the collection values as a list.
    pub fn values<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        PyList::new(py, self.list.iter())
    }

    /// Return the item at the given index, or ``None`` if it is out of range.
    ///
    /// :param index: Zero-based item index.
    pub fn item(&self, index: usize) -> Option<String> {
        self.list.item(index)
    }

    /// Return whether the token is present.
    ///
    /// :param token: One or more class-name tokens.
    pub fn contains(&self, token: &str) -> bool {
        self.list.contains(token)
    }

    // noinspection DuplicatedCode
    /// Add a new class name to the element if it is not already present.
    ///
    /// :param token: One or more class-name tokens.
    #[pyo3(signature = (*token))]
    pub fn add<'py>(&self, token: &Bound<'py, PyTuple>) -> PyResult<()> {
        self.list.add(
            token
                .extract::<Vec<String>>()?
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .as_slice(),
        );
        Ok(())
    }

    // noinspection DuplicatedCode
    /// Remove a class name from this element.
    ///
    /// :param token: One or more class-name tokens.
    #[pyo3(signature = (*token))]
    pub fn remove<'py>(&self, token: &Bound<'py, PyTuple>) -> PyResult<()> {
        self.list.remove(
            token
                .extract::<Vec<String>>()?
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .as_slice(),
        );
        Ok(())
    }

    /// Replace one token with another.
    ///
    /// :param old_token: Token to replace.
    /// :param new_token: Replacement token.
    pub fn replace(&self, old_token: &str, new_token: &str) -> bool {
        self.list.replace(old_token, new_token)
    }

    /// Add or remove a token, optionally forcing its presence.
    ///
    /// :param token: One or more class-name tokens.
    /// :param force: Desired token presence, or ``None`` to toggle it.
    #[pyo3(signature = (token, force=None))]
    pub fn toggle(&self, token: &str, force: Option<bool>) -> bool {
        self.list.toggle(token, force)
    }

    /// Return the class name at the given index or the class names in a slice.
    ///
    /// :param index_or_slice: Integer index or slice.
    pub fn __getitem__<'py>(&self, index_or_slice: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        get_tuple_slice(&PyTuple::new(index_or_slice.py(), self.list.values().into_iter())?, index_or_slice)
    }

    /// Return the number of class names in this token list.
    pub fn __len__(&self) -> usize {
        self.list.len()
    }

    /// Check whether this token list contains the given class name.
    ///
    /// :param token: One or more class-name tokens.
    pub fn __contains__<'py>(&self, token: &Bound<'py, PyAny>) -> bool {
        token.extract::<String>().map_or(false, |s| self.list.contains(&s))
    }

    // noinspection DuplicatedCode
    /// Return the representation of this token list.
    pub fn __repr__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Ok(self.values(py)?.call_method0("__repr__")?)
    }
}
