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
use pyo3::types::*;

use resiliparse_common::parse::html::dom::node as node_impl;
use resiliparse_common::parse::html::dom::iter as iter_impl;
use resiliparse_common::parse::html::dom::traits::*;


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

macro_rules! node_forward_opt_call {
    ($Self: expr, $NodeType: ident, $FuncName: ident) => {
        match &$Self {
            node_impl::Node::$NodeType(e) => Some(e.$FuncName()?.into()),
            _ => None
        }
    }
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
    ($Self: ident, $Base: path) => {
        #[pyclass(extends=Node, module = "resiliparse.parse._html_rs.node")]
        #[derive(Clone)]
        pub struct $Self;

        impl $Self {
            pub fn new_bound(py: Python, node: $Base) -> Bound<$Self> {
                Bound::new(py, (Self {}, node.into())).unwrap()
            }
        }

        impl From<$Base> for Node {
            fn from(value: $Base) -> Self {
                Node { node: value.as_node() }
            }
        }
    }
}

define_node_type!(ElementNode, node_impl::ElementNode);
define_node_type!(AttrNode, node_impl::AttrNode);
define_node_type!(TextNode, node_impl::TextNode);
define_node_type!(CdataSectionNode, node_impl::CdataSectionNode);
define_node_type!(ProcessingInstructionNode, node_impl::ProcessingInstructionNode);
define_node_type!(CommentNode, node_impl::CommentNode);
define_node_type!(DocumentNode, node_impl::DocumentNode);
define_node_type!(DocumentTypeNode, node_impl::DocumentTypeNode);
define_node_type!(DocumentFragmentNode, node_impl::DocumentFragmentNode);
define_node_type!(NotationNode, node_impl::NotationNode);


pub fn create_upcast_node(py: Python, node: node_impl::Node) -> Bound<PyAny> {
    match node {
        // TODO: Replace with Bound::into_super() in PyO3 0.23
        node_impl::Node::Element(e) => ElementNode::new_bound(py, e).into_any(),
        node_impl::Node::Attribute(e) => AttrNode::new_bound(py, e).into_any(),
        node_impl::Node::Text(e) => TextNode::new_bound(py, e).into_any(),
        node_impl::Node::CdataSection(e) => CdataSectionNode::new_bound(py, e).into_any(),
        node_impl::Node::ProcessingInstruction(e) => ProcessingInstructionNode::new_bound(py, e).into_any(),
        node_impl::Node::Comment(e) => CommentNode::new_bound(py, e).into_any(),
        node_impl::Node::Document(e) => DocumentNode::new_bound(py, e).into_any(),
        node_impl::Node::DocumentType(e) => DocumentTypeNode::new_bound(py, e).into_any(),
        node_impl::Node::DocumentFragment(e) => DocumentFragmentNode::new_bound(py, e).into_any(),
        node_impl::Node::Notation(e) => NotationNode::new_bound(py, e).into_any(),
    }
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

    #[getter]
    pub fn owner_document<'py>(&self, py: Python<'py>) -> Option<Bound<'py, DocumentNode>> {
        Some(DocumentNode::new_bound(py, self.node.owner_document()?))
    }

    #[getter]
    pub fn parent<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyAny>> {
        self.parent_node(py)
    }

    #[getter]
    pub fn parent_node<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyAny>> {
        Some(create_upcast_node(py, self.node.parent_node()?))
    }

    #[getter]
    pub fn parent_element<'py>(&self, py: Python<'py>) -> Option<Bound<'py, ElementNode>> {
        if let node_impl::Node::Element(e) = self.node.parent_node()? {
            Some(ElementNode::new_bound(py, e))
        } else {
            None
        }
    }

    pub fn has_child_nodes(&self) -> bool {
        self.node.has_child_nodes()
    }

    pub fn contains<'py>(&self, node: Bound<'py, PyAny>) -> bool {
        if let Ok(n) = node.downcast::<Node>() {
            self.node.contains(&n.borrow().node)
        } else {
            false
        }
    }
    //
    // pub fn child_nodes<'py>(&self, py: Python<'py>) -> Bound<'py, NodeList> {
    //
    // }

    pub fn __contains__(&self, node: Bound<'_, PyAny>) -> bool {
        self.contains(node)
    }

    pub fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Bound<'_, NodeIter>> {
        Bound::new(slf.py(), NodeIter { iter: (*slf.node).clone().into_iter() })
    }
}

#[pyclass]
struct NodeIter {
    iter: iter_impl::NodeIteratorOwned,
}

#[pymethods]
impl NodeIter {
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    pub fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<Bound<'_, PyAny>> {
        Some(create_upcast_node(slf.py(), slf.iter.next()?))
    }
}
