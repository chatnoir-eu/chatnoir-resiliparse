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
use pyo3::exceptions::PyIndexError;
use pyo3::prelude::*;
use pyo3::types::*;
use resiliparse_common::parse::html::dom::node as node_impl;
use resiliparse_common::parse::html::dom::iter as iter_impl;
use resiliparse_common::parse::html::dom::traits::*;
use crate::coll::*;
use crate::exception::*;


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
}

#[pyclass(subclass, module = "resiliparse.parse._html_rs.node")]
#[derive(Clone, PartialEq, Eq)]
pub struct Node {
    pub(crate) node: node_impl::Node
}

impl From<node_impl::Node> for Node {
    fn from(value: node_impl::Node) -> Self {
        Node { node: value }
    }
}

macro_rules! define_node_type {
    ($Self: ident, $BaseEnum: ident, $Base: ty) => {
        #[pyclass(extends=Node, module = "resiliparse.parse._html_rs.node")]
        #[derive(Clone)]
        pub struct $Self;

        impl $Self {
            pub fn new_bound(py: Python, node: $Base) -> PyResult<Bound<$Self>> {
                Bound::new(py, (Self {}, node.into()))
            }

            #[allow(dead_code)]
            fn raw_node<'py, 'a>(slf: &'a PyRef<'py, Self>) -> &'a $Base {
                match &slf.as_super().node {
                    node_impl::Node::$BaseEnum(n) => n,
                    _ => unreachable!()
                }
            }

            #[allow(dead_code)]
            fn raw_node_mut<'py, 'a>(slf: &'a mut PyRefMut<'py, Self>) -> &'a mut $Base {
                match &mut slf.as_super().node {
                    node_impl::Node::$BaseEnum(n) => n,
                    _ => unreachable!()
                }
            }
        }

        impl From<$Base> for Node {
            fn from(value: $Base) -> Node {
                Node { node: value.as_node() }
            }
        }
    }
}

define_node_type!(ElementNode, Element, node_impl::ElementNode);
define_node_type!(AttrNode, Attribute, node_impl::AttrNode);
define_node_type!(TextNode, Text, node_impl::TextNode);
define_node_type!(CdataSectionNode, CdataSection, node_impl::CdataSectionNode);
define_node_type!(ProcessingInstructionNode, ProcessingInstruction, node_impl::ProcessingInstructionNode);
define_node_type!(CommentNode, Comment, node_impl::CommentNode);
define_node_type!(DocumentNode, Document, node_impl::DocumentNode);
define_node_type!(DocumentTypeNode, DocumentType, node_impl::DocumentTypeNode);
define_node_type!(DocumentFragmentNode, DocumentFragment, node_impl::DocumentFragmentNode);
define_node_type!(NotationNode, Notation, node_impl::NotationNode);


pub fn create_upcast_node(py: Python, node: node_impl::Node) -> PyResult<Bound<PyAny>> {
    Ok(match node {
        // TODO: Replace with Bound::into_super() in PyO3 0.23
        node_impl::Node::Element(e) => ElementNode::new_bound(py, e)?.into_any(),
        node_impl::Node::Attribute(e) => AttrNode::new_bound(py, e)?.into_any(),
        node_impl::Node::Text(e) => TextNode::new_bound(py, e)?.into_any(),
        node_impl::Node::CdataSection(e) => CdataSectionNode::new_bound(py, e)?.into_any(),
        node_impl::Node::ProcessingInstruction(e) => ProcessingInstructionNode::new_bound(py, e)?.into_any(),
        node_impl::Node::Comment(e) => CommentNode::new_bound(py, e)?.into_any(),
        node_impl::Node::Document(e) => DocumentNode::new_bound(py, e)?.into_any(),
        node_impl::Node::DocumentType(e) => DocumentTypeNode::new_bound(py, e)?.into_any(),
        node_impl::Node::DocumentFragment(e) => DocumentFragmentNode::new_bound(py, e)?.into_any(),
        node_impl::Node::Notation(e) => NotationNode::new_bound(py, e)?.into_any(),
    })
}


#[pymethods]
impl Node {
    #[getter]
    pub fn node_type(&self) -> Option<NodeType> {
        Some(match self.node.node_type()? {
            node_impl::NodeType::Element => NodeType::Element,
            node_impl::NodeType::Attribute => NodeType::Attribute,
            node_impl::NodeType::Text => NodeType::Text,
            node_impl::NodeType::CdataSection => NodeType::CdataSection,
            node_impl::NodeType::EntityReference => NodeType::EntityReference,
            node_impl::NodeType::Entity => NodeType::Entity,
            node_impl::NodeType::ProcessingInstruction => NodeType::ProcessingInstruction,
            node_impl::NodeType::Comment => NodeType::Comment,
            node_impl::NodeType::Document => NodeType::Document,
            node_impl::NodeType::DocumentType => NodeType::DocumentType,
            node_impl::NodeType::DocumentFragment => NodeType::DocumentFragment,
            node_impl::NodeType::Notation => NodeType::Notation,
        })
    }

    #[getter]
    #[pyo3(name = "type")]
    pub fn type_(&self) -> Option<NodeType> {
        self.node_type()
    }

    #[getter]
    pub fn name(&self) -> Option<String> {
        self.node_name()
    }

    #[getter]
    pub fn node_name(&self) -> Option<String> {
        self.node.node_name()
    }

    #[getter]
    pub fn value(&self) -> Option<String> {
        self.node_value()
    }

    #[getter]
    pub fn node_value(&self) -> Option<String> {
        self.node.node_value()
    }

    #[getter]
    pub fn text(&self) -> Option<String> {
        self.text_content()
    }

    #[getter]
    pub fn text_content(&self) -> Option<String> {
        self.node.text_content()
    }

    // This function was here before
    #[getter]
    pub fn owner_document<'py>(&self, py: Python<'py>) -> Option<Bound<'py, DocumentNode>> {
        DocumentNode::new_bound(py, self.node.owner_document()?).ok()
    }

    #[getter]
    pub fn parent<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyAny>> {
        self.parent_node(py)
    }

    #[getter]
    pub fn parent_node<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyAny>> {
        create_upcast_node(py, self.node.parent_node()?).ok()
    }

    #[getter]
    pub fn parent_element<'py>(&self, py: Python<'py>) -> Option<Bound<'py, ElementNode>> {
        if let node_impl::Node::Element(e) = self.node.parent_node()? {
            ElementNode::new_bound(py, e).ok()
        } else {
            None
        }
    }

    pub fn has_child_nodes(&self) -> bool {
        self.node.has_child_nodes()
    }

    pub fn contains<'py>(&self, node: &Bound<'py, PyAny>) -> bool {
        node.downcast::<Node>().map_or(false, |n| self.node.contains(&n.borrow().node))
    }

    #[getter]
    pub fn child_nodes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, NodeList>> {
        NodeList::new_bound(py, self.node.child_nodes())
    }

    #[getter]
    pub fn first_child<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.node.first_child().map_or(Ok(None), |n| Ok(Some(create_upcast_node(py, n)?)))
    }

    #[getter]
    pub fn last_child<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.node.last_child().map_or(Ok(None), |n| Ok(Some(create_upcast_node(py, n)?)))
    }

    #[getter]
    pub fn previous_sibling<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.node.previous_sibling().map_or(Ok(None), |n| Ok(Some(create_upcast_node(py, n)?)))
    }

    #[getter]
    pub fn prev<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.previous_sibling(py)
    }

    #[getter]
    pub fn next_sibling<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.node.next_sibling().map_or(Ok(None), |n| Ok(Some(create_upcast_node(py, n)?)))
    }

    #[getter]
    pub fn next<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.previous_sibling(py)
    }

    #[pyo3(signature = (deep=false))]
    pub fn clone_node<'py>(&self, py: Python<'py>, deep: bool) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.node.clone_node(deep).map_or(Ok(None), |n| Ok(Some(create_upcast_node(py, n)?)))
    }

    #[pyo3(signature = (node, reference=None))]
    pub fn insert_before<'py>(&mut self, node: Bound<'py, Node>, reference: Option<&Bound<'py, Node>>) -> Option<Bound<'py, Node>> {
        let rb = reference.map(|r| r.borrow().node.clone());
        self.node.insert_before(&node.borrow().node, rb.as_ref()).and(Some(node))
    }

    pub fn append_child<'a, 'py>(&mut self, node: Bound<'py, Node>) -> Option<Bound<'py, Node>> {
        self.node.append_child(&node.borrow().node).and(Some(node))
    }

    pub fn replace_child<'py>(&mut self, new_child: Bound<'py, Node>, old_child: Bound<'py, Node>) -> Option<Bound<'py, Node>> {
        self.node.replace_child(&new_child.borrow().node, &old_child.borrow().node).and(Some(old_child))
    }

    pub fn remove_child<'py>(&mut self, child: Bound<'py, Node>) -> Option<Bound<'py, Node>> {
        self.node.remove_child(&child.borrow().node).and(Some(child))
    }

    pub fn decompose(&mut self) {
        self.node.decompose()
    }

    pub fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Bound<'_, NodeIter>> {
        Bound::new(slf.py(), NodeIter { iter: (*slf.node).clone().into_iter() })
    }

    pub fn __contains__(&self, node: &Bound<'_, PyAny>) -> bool {
        self.contains(node)
    }
}


#[pymethods]
impl ElementNode {
    fn tag_name(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).tag_name()
    }

    fn tag(slf: PyRef<'_, Self>) -> Option<String> {
        Self::tag_name(slf)
    }

    fn local_name(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).local_name()
    }

    fn id(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).id()
    }

    fn class_name(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).class_name()
    }

    fn class_list(mut slf: PyRefMut<'_, Self>) -> PyResult<Bound<'_, DOMTokenList>> {
        Bound::new(slf.py(), DOMTokenList::from(Self::raw_node_mut(&mut slf).class_list_mut()))
    }

    fn attribute(slf: PyRef<'_, Self>, qualified_name: &str) -> Option<String> {
        Self::raw_node(&slf).attribute(qualified_name)
    }

    fn attribute_node<'py>(slf: PyRef<'py, Self>, qualified_name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        if let Some(a) = Self::raw_node(&slf).attribute_node(qualified_name) {
            Ok(Some(create_upcast_node(slf.py(), a.into_node())?))
        } else {
            Ok(None)
        }
    }

    fn attribute_names(slf: PyRef<'_, Self>) -> Bound<'_, PyTuple> {
        PyTuple::new_bound(slf.py(), Self::raw_node(&slf).attribute_names().into_iter())
    }

    fn has_attribute(slf: PyRef<'_, Self>, qualified_name: &str) -> bool {
        Self::raw_node(&slf).has_attribute(qualified_name)
    }

    fn set_attribute(mut slf: PyRefMut<'_, Self>, qualified_name: &str, value: &str) {
        Self::raw_node_mut(&mut slf).set_attribute(qualified_name, value)
    }

    fn remove_attribute(mut slf: PyRefMut<'_, Self>, qualified_name: &str) {
        Self::raw_node_mut(&mut slf).remove_attribute(qualified_name)
    }

    #[pyo3(signature = (qualified_name, force=None))]
    fn toggle_attribute(mut slf: PyRefMut<'_, Self>, qualified_name: &str, force: Option<bool>) -> bool {
        Self::raw_node_mut(&mut slf).toggle_attribute(qualified_name, force)
    }

    #[getter]
    fn attrs(slf: PyRef<'_, Self>) -> Bound<'_, PyTuple> {
        Self::attribute_names(slf)
    }

    fn hasattr(slf: PyRef<'_, Self>, qualified_name: &str) -> bool {
        Self::has_attribute(slf, qualified_name)
    }

    fn __contains__(slf: PyRef<'_, Self>, qualified_name: &str) -> bool {
        Self::has_attribute(slf, qualified_name)
    }

    #[pyo3(signature = (qualified_name, default_value=None))]
    fn getattr(slf: PyRef<'_, Self>, qualified_name: &str, default_value: Option<&str>) -> Option<String> {
        Self::attribute(slf, qualified_name).or(default_value.map(str::to_owned))
    }

    fn __getitem__(slf: PyRef<'_, Self>, qualified_name: &str) -> PyResult<String> {
        Self::attribute(slf, qualified_name).map_or_else(
            || Err(PyIndexError::new_err(format!("Attribute {} does not exist", qualified_name))),
            |a| Ok(a))
    }

    fn setattr(slf: PyRefMut<'_, Self>, qualified_name: &str, value: &str) {
        Self::set_attribute(slf, qualified_name, value)
    }

    fn __setitem__( slf: PyRefMut<'_, Self>, qualified_name: &str, value: &str) {
        Self::set_attribute(slf, qualified_name, value)
    }

    fn delattr(slf: PyRefMut<'_, Self>, qualified_name: &str) {
        Self::remove_attribute(slf, qualified_name)
    }

    fn __delitem__(mut slf: PyRefMut<'_, Self>, qualified_name: &str) -> PyResult<()> {
        if Self::raw_node_mut(&mut slf).has_attribute(qualified_name) {
            Ok(Self::remove_attribute(slf, qualified_name))
        } else {
            Err(PyIndexError::new_err(format!("Attribute {} does not exist", qualified_name)))
        }
    }

    fn closest<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
       Self::raw_node(&slf).closest(selectors).map_or_else(
            |e| Err(CSSParserException::new_err(e.to_string())),
            |n| n.map_or(
                Ok(None),
                |n_| Ok(Some(create_upcast_node(slf.py(), n_.into_node())?))
            )
       )
    }

    fn matches<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<bool> {
       Self::raw_node(&slf).matches(selectors).map_or_else(
            |e| Err(CSSParserException::new_err(e.to_string())),
            |n| Ok(n)
       )
    }

    //noinspection DuplicatedCode
    fn get_elements_by_tag_name<'py>(slf: PyRef<'py, Self>, qualified_name: &str) -> PyResult<Bound<'py, ElementNodeList>> {
       ElementNodeList::new_bound(slf.py(), Self::raw_node(&slf).get_elements_by_tag_name(qualified_name))
    }

    //noinspection DuplicatedCode
    fn get_elements_by_class_name<'py>(slf: PyRef<'py, Self>, qualified_name: &str) -> PyResult<Bound<'py, ElementNodeList>> {
       ElementNodeList::new_bound(slf.py(), Self::raw_node(&slf).get_elements_by_class_name(qualified_name))
    }

    //noinspection DuplicatedCode
    #[pyo3(signature = (attr, qualified_name, case_insensitive=false))]
    fn get_elements_by_attr<'py>(slf: PyRef<'py, Self>, attr: &str, qualified_name: &str, case_insensitive: bool) -> PyResult<Bound<'py, ElementNodeList>> {
       ElementNodeList::new_bound(slf.py(), Self::raw_node(&slf).get_elements_by_attr_case(attr, qualified_name, case_insensitive))
    }

    //noinspection DuplicatedCode
    #[pyo3(signature = (element_id, case_insensitive=false))]
    fn get_elements_by_id<'py>(slf: PyRef<'py, Self>, element_id: &str, case_insensitive: bool) -> PyResult<Bound<'py, ElementNodeList>> {
       ElementNodeList::new_bound(slf.py(), Self::raw_node(&slf).get_elements_by_attr_case("id", element_id, case_insensitive))
    }

    #[getter]
    fn html(slf: PyRef<'_, Self>) -> String {
        Self::outer_html(slf)
    }

    #[setter]
    fn set_html<'py>(slf: PyRefMut<'py, Self>, html: &str) {
        Self::set_inner_html(slf, html)
    }

    #[getter]
    fn inner_html(slf: PyRef<'_, Self>) -> String {
        Self::raw_node(&slf).inner_html()
    }

    #[setter]
    fn set_inner_html<'py>(mut slf: PyRefMut<'py, Self>, inner_html: &str) {
        Self::raw_node_mut(&mut slf).set_inner_html(inner_html)
    }

    #[getter]
    fn outer_html(slf: PyRef<'_, Self>) -> String {
        Self::raw_node(&slf).outer_html()
    }

    #[setter]
    fn set_outer_html<'py>(mut slf: PyRefMut<'py, Self>, outer_html: &str) {
        Self::raw_node_mut(&mut slf).set_outer_html(outer_html)
    }

    #[getter]
    fn inner_text(slf: PyRef<'_, Self>) -> String {
        Self::raw_node(&slf).inner_text()
    }

    #[setter]
    fn set_inner_text<'py>(mut slf: PyRefMut<'py, Self>, inner_text: &str) {
        Self::raw_node_mut(&mut slf).set_inner_text(inner_text)
    }

    #[getter]
    fn outer_text(slf: PyRef<'_, Self>) -> String {
        Self::raw_node(&slf).outer_text()
    }

    #[setter]
    fn set_outer_text<'py>(mut slf: PyRefMut<'py, Self>, outer_text: &str) {
        Self::raw_node_mut(&mut slf).set_outer_text(outer_text)
    }
}

#[pymethods]
impl DocumentTypeNode {
    fn name(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).name()
    }

    fn public_id(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).public_id()
    }

    fn system_id(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).system_id()
    }
}


macro_rules! doc_create_x_mixin {
    ($self_func_name: ident, $func_name: ident) => {
        #[inline]
        fn $self_func_name<'py>(mut slf: PyRefMut<'py, Self>, data: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
            Self::raw_node_mut(&mut slf).$func_name(data).map_or_else(
                |e| Err(DOMException::new_err(e.to_string())),
                |e| Ok(Some(create_upcast_node(slf.py(), e.into_node())?))
            )
        }
    };
}

impl DocumentNode {
    doc_create_x_mixin!(create_element_, create_element);
    doc_create_x_mixin!(create_text_node_, create_text_node);
    doc_create_x_mixin!(create_cdata_section_, create_cdata_section);
    doc_create_x_mixin!(create_comment_, create_comment);
    doc_create_x_mixin!(create_attribute_, create_attribute);
}

macro_rules! parent_node_mixin {
    () => {
        #[inline]
        fn children_(slf: PyRef<'_, Self>) -> PyResult<Bound<'_, ElementNodeList>> {
            ElementNodeList::new_bound(slf.py(), Self::raw_node(&slf).children().into())
        }

        #[inline]
        fn first_element_child_(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNode>>> {
            Self::raw_node(&slf).first_element_child().map_or(
                Ok(None),
                |e| Ok(Some(ElementNode::new_bound(slf.py(), e)?))
            )
        }

        #[inline]
        fn last_element_child_(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNode>>> {
            Self::raw_node(&slf).last_element_child().map_or(
                Ok(None),
                |e| Ok(Some(ElementNode::new_bound(slf.py(), e)?))
            )
        }

        #[inline]
        fn child_element_count_(slf: PyRef<'_, Self>) -> usize {
            Self::raw_node(&slf).child_element_count()
        }

        #[inline]
        fn prepend_(mut slf: PyRefMut<'_, Self>, nodes: &Bound<'_, PyTuple>) -> PyResult<()> {
            let n = nodes.extract::<Vec<Node>>()?;
            let s: Vec<&node_impl::Node> = n.iter().map(|n_| &n_.node).collect();
            Ok(Self::raw_node_mut(&mut slf).prepend(&s))
        }

        #[inline]
        fn append_(mut slf: PyRefMut<'_, Self>, nodes: &Bound<'_, PyTuple>) -> PyResult<()> {
            let n = nodes.extract::<Vec<Node>>()?;
            let s: Vec<&node_impl::Node> = n.iter().map(|n_| &n_.node).collect();
            Ok(Self::raw_node_mut(&mut slf).append(&s))
        }

        #[inline]
        fn replace_children_(mut slf: PyRefMut<'_, Self>, nodes: &Bound<'_, PyTuple>) -> PyResult<()> {
            let n = nodes.extract::<Vec<Node>>()?;
            let s: Vec<&node_impl::Node> = n.iter().map(|n_| &n_.node).collect();
            Ok(Self::raw_node_mut(&mut slf).replace_children(&s))
        }

        // TODO: query_selector, query_selector_all
    };
}


#[pymethods]
impl DocumentNode {
    fn doctype(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, PyAny>>> {
        Self::raw_node(&slf).doctype().map_or(
            Ok(None),
            |d| Ok(Some(create_upcast_node(slf.py(), d.into_node())?))
        )
    }

    fn document_element(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, PyAny>>> {
        Self::raw_node(&slf).document_element().map_or(
            Ok(None),
            |d| Ok(Some(create_upcast_node(slf.py(), d.into_node())?))
        )
    }

    //noinspection DuplicatedCode
    fn get_elements_by_tag_name<'py>(slf: PyRef<'py, Self>, qualified_name: &str) -> PyResult<Bound<'py, ElementNodeList>> {
       ElementNodeList::new_bound(slf.py(), Self::raw_node(&slf).get_elements_by_tag_name(qualified_name))
    }

    //noinspection DuplicatedCode
    fn get_elements_by_class_name<'py>(slf: PyRef<'py, Self>, qualified_name: &str) -> PyResult<Bound<'py, ElementNodeList>> {
       ElementNodeList::new_bound(slf.py(), Self::raw_node(&slf).get_elements_by_class_name(qualified_name))
    }

    //noinspection DuplicatedCode
    #[pyo3(signature = (attr, qualified_name, case_insensitive=false))]
    fn get_elements_by_attr<'py>(slf: PyRef<'py, Self>, attr: &str, qualified_name: &str, case_insensitive: bool) -> PyResult<Bound<'py, ElementNodeList>> {
       ElementNodeList::new_bound(slf.py(), Self::raw_node(&slf).get_elements_by_attr_case(attr, qualified_name, case_insensitive))
    }

    //noinspection DuplicatedCode
    #[pyo3(signature = (element_id, case_insensitive=false))]
    fn get_elements_by_id<'py>(slf: PyRef<'py, Self>, element_id: &str, case_insensitive: bool) -> PyResult<Bound<'py, ElementNodeList>> {
       ElementNodeList::new_bound(slf.py(), Self::raw_node(&slf).get_elements_by_attr_case("id", element_id, case_insensitive))
    }

    fn create_element<'py>(slf: PyRefMut<'py, Self>, local_name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        Self::create_element_(slf, local_name)
    }

    fn create_document_fragment(mut slf: PyRefMut<'_, Self>) -> PyResult<Option<Bound<'_, PyAny>>> {
        Self::raw_node_mut(&mut slf).create_document_fragment().map_or_else(
            |e| Err(DOMException::new_err(e.to_string())),
            |e| Ok(Some(create_upcast_node(slf.py(), e.into_node())?))
        )
    }

    fn create_text_node<'py>(slf: PyRefMut<'py, Self>, data: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        Self::create_text_node_(slf, data)
    }

    fn create_cdata_section<'py>(slf: PyRefMut<'py, Self>, data: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        Self::create_cdata_section_(slf, data)
    }

    fn create_comment<'py>(slf: PyRefMut<'py, Self>, data: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        Self::create_comment_(slf, data)
    }

    fn create_processing_instruction<'py>(mut slf: PyRefMut<'py, Self>, target: &str, local_name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        Self::raw_node_mut(&mut slf).create_processing_instruction(target, local_name).map_or_else(
            |e| Err(DOMException::new_err(e.to_string())),
            |e| Ok(Some(create_upcast_node(slf.py(), e.into_node())?))
        )
    }

    fn create_attribute<'py>(slf: PyRefMut<'py, Self>, local_name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        Self::create_attribute_(slf, local_name)
    }
}

#[pymethods]
impl AttrNode {
    fn local_name(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).local_name()
    }

    fn name(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).name()
    }

    fn value(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).value()
    }

    fn owner_element(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, PyAny>>> {
        Self::raw_node(&slf).owner_element().map_or(
            Ok(None),
            |e| Ok(Some(create_upcast_node(slf.py(), e.into_node())?))
        )
    }
}

#[pymethods]
impl ProcessingInstructionNode {
    fn target(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).target()
    }
}


#[pyclass]
pub struct NodeIter {
    iter: iter_impl::NodeIteratorOwned,
}

#[pymethods]
impl NodeIter {
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    pub fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<Bound<'_, PyAny>> {
        create_upcast_node(slf.py(), slf.iter.next()?).ok()
    }
}
