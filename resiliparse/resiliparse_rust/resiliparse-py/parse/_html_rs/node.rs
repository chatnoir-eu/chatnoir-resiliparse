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

use pyo3::exceptions::PyIndexError;
use pyo3::prelude::*;
use pyo3::types::*;
use resiliparse_common::parse::html::dom::node as node_impl;
use resiliparse_common::parse::html::dom::iter as iter_impl;
use resiliparse_common::parse::html::dom::traits::*;
use crate::coll::*;
use crate::exception::*;

#[pyclass(eq, eq_int, rename_all = "SCREAMING_SNAKE_CASE", module = "resiliparse.parse._html_rs.node")]
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
#[derive(Clone, PartialEq)]
pub struct Node {
    pub(crate) node: node_impl::Node
}

impl From<node_impl::Node> for Node {
    fn from(value: node_impl::Node) -> Self {
        Node { node: value }
    }
}

pub(crate) fn create_upcast_node(py: Python, node: node_impl::Node) -> PyResult<Bound<PyAny>> {
    Ok(match node {
        // TODO: Replace with Bound::into_super() in PyO3 0.23
        node_impl::Node::Element(e) => ElementNode::new_bound(py, e)?.into_any(),
        node_impl::Node::Attribute(e) => AttrNode::new_bound(py, e)?.into_any(),
        node_impl::Node::Text(e) => TextNode::new_bound(py, e)?.into_any(),
        node_impl::Node::CdataSection(e) => CDATASectionNode::new_bound(py, e)?.into_any(),
        node_impl::Node::ProcessingInstruction(e) => ProcessingInstructionNode::new_bound(py, e)?.into_any(),
        node_impl::Node::Comment(e) => CommentNode::new_bound(py, e)?.into_any(),
        node_impl::Node::Document(e) => DocumentNode::new_bound(py, e)?.into_any(),
        node_impl::Node::DocumentType(e) => DocumentTypeNode::new_bound(py, e)?.into_any(),
        node_impl::Node::DocumentFragment(e) => DocumentFragmentNode::new_bound(py, e)?.into_any(),
        node_impl::Node::Notation(e) => NotationNode::new_bound(py, e)?.into_any(),
    })
}


// ------------------------------------ Node Interface Mixins --------------------------------------


macro_rules! get_elements_by_x {
    ($slf: ident, $func_name: ident $(, $args: ident)*) => {
        ElementNodeList::new_bound($slf.py(), Self::raw_node(&$slf).$func_name($($args),*))
    };
}

trait _NodeAccessorMixin<T: NodeInterface>: pyo3::PyClass<Frozen=pyo3::pyclass::boolean_struct::False> {
    fn raw_node<'py, 'a>(slf: &'a PyRef<'py, Self>) -> &'a T;
    fn raw_node_mut<'py, 'a>(slf: &'a mut PyRefMut<'py, Self>) -> &'a mut T;
}

trait _ParentNodeMixin<T: ParentNode>: _NodeAccessorMixin<T> {
    fn children_(slf: PyRef<'_, Self>) -> PyResult<Bound<'_, ElementNodeList>> {
        ElementNodeList::new_bound(slf.py(), Self::raw_node(&slf).children().into())
    }

    fn first_element_child_(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNode>>> {
        Self::raw_node(&slf).first_element_child().map_or(
            Ok(None),
            |e| Ok(Some(ElementNode::new_bound(slf.py(), e)?))
        )
    }

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

    fn prepend_(mut slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        let n = node.extract::<Vec<Node>>()?;
        Ok(Self::raw_node_mut(&mut slf).prepend(&n.iter()
            .map(|n_| n_.node.as_noderef())
            .collect::<Vec<_>>()
        ))
    }

    fn append_(mut slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        let n = node.extract::<Vec<Node>>()?;
        Ok(Self::raw_node_mut(&mut slf).append(&n.iter()
            .map(|n_| n_.node.as_noderef())
            .collect::<Vec<_>>()
        ))
    }

    fn replace_children_(mut slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        let n = node.extract::<Vec<Node>>()?;
        Ok(Self::raw_node_mut(&mut slf).replace_children(&n.iter()
            .map(|n_| n_.node.as_noderef())
            .collect::<Vec<_>>()
        ))
    }

    //noinspection DuplicatedCode
    fn query_selector_<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Option<Bound<'py, ElementNode>>> {
        Self::raw_node(&slf).query_selector(selectors).map_or_else(
            |e| Err(CSSParserException::new_err(e.to_string())),
            |e| e.map_or(
                Ok(None),
                |e_| Ok(Some(ElementNode::new_bound(slf.py(), e_)?))
            )
        )
    }

    //noinspection DuplicatedCode
    fn query_selector_all_<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Bound<'py, ElementNodeList>> {
        Self::raw_node(&slf).query_selector_all(selectors).map_or_else(
            |e| Err(CSSParserException::new_err(e.to_string())),
            |e| Ok(ElementNodeList::new_bound(slf.py(), e)?)
        )
    }
}


trait _NonElementParentNodeMixin<T: NonElementParentNode>: _NodeAccessorMixin<T> {
    fn get_element_by_id_<'py>(slf: PyRef<'py, Self>, element_id: &str, case_insensitive: bool) -> PyResult<Option<Bound<'py, ElementNode>>> {
        Self::raw_node(&slf).get_element_by_id_case(element_id, case_insensitive).map_or(
            Ok(None),
            |e| Ok(Some(ElementNode::new_bound(slf.py(), e)?))
        )
    }
}


macro_rules! child_node_funcs {
    ($func_name: ident, $call_func_name: ident) => {
        fn $func_name(mut slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
            let mut n = node.extract::<Vec<Node>>()?;
            let r: Vec<_> = n.iter_mut().map(|n_| &mut n_.node).collect();
            Self::raw_node_mut(&mut slf).$call_func_name(&r);
            Ok(())
        }
    };
}

trait _ChildNodeMixin<T: ChildNode>: _NodeAccessorMixin<T> {
    child_node_funcs!(before_, before);
    child_node_funcs!(after_, after);
    child_node_funcs!(replace_with_, replace_with);

    #[inline(always)]
    fn remove_(mut slf: PyRefMut<'_, Self>) {
        Self::raw_node_mut(&mut slf).remove();
    }
}



trait _NonDocumentTypeChildNodeMixin<T: NonDocumentTypeChildNode>: _NodeAccessorMixin<T> {
    fn next_element_sibling_(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNode>>> {
        Self::raw_node(&slf).next_element_sibling().map_or(
            Ok(None),
            |e| Ok(Some(ElementNode::new_bound(slf.py(), e)?))
        )
    }

    fn previous_element_sibling_(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNode>>> {
        Self::raw_node(&slf).previous_element_sibling().map_or(
            Ok(None),
            |e| Ok(Some(ElementNode::new_bound(slf.py(), e)?))
        )
    }
}


macro_rules! character_data_node {
    ($Self: ident, $Base: ty $(, $func_name: ident, $out_type: ty)*) => {
        #[pymethods]
        impl $Self {
            #[getter]
            #[inline(always)]
            pub fn len(slf: PyRef<'_, Self>) -> usize {
                Self::raw_node(&slf).len()
            }

            #[getter]
            #[inline(always)]
            pub fn data(slf: PyRef<'_, Self>) -> Option<String> {
                Self::raw_node(&slf).data()
            }

            #[setter]
            #[inline(always)]
            pub fn set_data<'py>(mut slf: PyRefMut<'py, Self>, data: &str) {
                Self::raw_node_mut(&mut slf).set_data(data)
            }

            #[inline(always)]
            pub fn substring_data(slf: PyRef<'_, Self>, offset: usize, count: usize) -> Option<String> {
                Self::raw_node(&slf).substring_data(offset, count)
            }

            #[inline(always)]
            pub fn append_data(mut slf: PyRefMut<'_, Self>, data: &str) {
                Self::raw_node_mut(&mut slf).append_data(data)
            }

            #[inline(always)]
            pub fn insert_data(mut slf: PyRefMut<'_, Self>, offset: usize, data: &str) {
                Self::raw_node_mut(&mut slf).insert_data(offset, data)
            }

            #[inline(always)]
            pub fn delete_data(mut slf: PyRefMut<'_, Self>, offset: usize, count: usize) {
                Self::raw_node_mut(&mut slf).delete_data(offset, count)
            }

            #[inline(always)]
            pub fn replace_data(mut slf: PyRefMut<'_, Self>, offset: usize, count: usize, data: &str) {
                Self::raw_node_mut(&mut slf).replace_data(offset, count, data)
            }

            $(#[getter]
            #[inline(always)]
            pub fn target(slf: PyRef<'_, Self>) -> $out_type {
                Self::raw_node(&slf).$func_name()
            })*

            // _ChildNodeMixin
            #[pyo3(signature = (*node))]
            #[inline(always)]
            pub fn before(slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
                Self::before_(slf, node)
            }

            #[pyo3(signature = (*node))]
            #[inline(always)]
            pub fn after(slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
                Self::after_(slf, node)
            }

            #[pyo3(signature = (*node))]
            #[inline(always)]
            pub fn replace_with(slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
                Self::replace_with_(slf, node)
            }

            #[inline(always)]
            pub fn remove(slf: PyRefMut<'_, Self>) {
                Self::remove_(slf);
            }

            // _NonDocumentTypeChildNodeMixin
            #[inline(always)]
            #[getter]
            pub fn next_element_sibling(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNode>>> {
                Self::next_element_sibling_(slf)
            }

            #[getter]
            #[inline(always)]
            pub fn next_element(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNode>>> {
                Self::next_element_sibling_(slf)
            }

            #[getter]
            #[inline(always)]
            pub fn previous_element_sibling(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNode>>> {
                Self::previous_element_sibling_(slf)
            }

            #[getter]
            #[inline(always)]
            pub fn prev_element(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNode>>> {
                Self::previous_element_sibling_(slf)
            }
        }

        impl _ChildNodeMixin<$Base> for $Self {}
        impl _NonDocumentTypeChildNodeMixin<$Base> for $Self {}
    }
}


// ------------------------------------------ Node Types -------------------------------------------

macro_rules! define_node_type {
    ($Self: ident, $BaseEnum: ident, $Base: ty) => {
        #[pyclass(extends=Node, module = "resiliparse.parse._html_rs.node")]
        #[derive(Clone)]
        pub struct $Self;

        impl $Self {
            pub fn new_bound(py: Python, node: $Base) -> PyResult<Bound<$Self>> {
                Bound::new(py, (Self {}, node.into()))
            }
        }

        impl _NodeAccessorMixin<$Base> for $Self {
            fn raw_node<'py, 'a>(slf: &'a PyRef<'py, Self>) -> &'a $Base {
                match &slf.as_super().node {
                    node_impl::Node::$BaseEnum(n) => n,
                    _ => unreachable!()
                }
            }

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
define_node_type!(CDATASectionNode, CdataSection, node_impl::CdataSectionNode);
define_node_type!(ProcessingInstructionNode, ProcessingInstruction, node_impl::ProcessingInstructionNode);
define_node_type!(CommentNode, Comment, node_impl::CommentNode);
define_node_type!(DocumentNode, Document, node_impl::DocumentNode);
define_node_type!(DocumentTypeNode, DocumentType, node_impl::DocumentTypeNode);
define_node_type!(DocumentFragmentNode, DocumentFragment, node_impl::DocumentFragmentNode);
define_node_type!(NotationNode, Notation, node_impl::NotationNode);


//noinspection DuplicatedCode
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

    #[setter]
    pub fn set_value(&mut self, value: &str) {
        self.set_node_value(value)
    }

    #[getter]
    pub fn node_value(&self) -> Option<String> {
        self.node.node_value()
    }

    #[setter]
    pub fn set_node_value(&mut self, value: &str) {
        self.node.set_node_value(value)
    }

    #[getter]
    pub fn text(&self) -> Option<String> {
        self.text_content()
    }

    #[setter]
    pub fn set_text(&mut self, text: &str) {
        self.set_text_content(text)
    }

    #[getter]
    pub fn text_content(&self) -> Option<String> {
        self.node.text_content()
    }

    #[setter]
    pub fn set_text_content(&mut self, text: &str) {
        self.node.set_text_content(text)
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

    pub fn contains<'py>(&self, node: &Bound<'py, Node>) -> bool {
        self.node.contains(&node.borrow().node.as_noderef())
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
    pub fn insert_before<'py>(&mut self, node: Bound<'py, Node>, reference: Option<Bound<'py, Node>>) -> Option<Bound<'py, Node>> {
        let b;
        let refnode = if let Some(r) = &reference {
            b = r.borrow();
            Some(b.node.as_noderef())
        } else {
            None
        };
        self.node.insert_before(&node.borrow().node.as_noderef(), refnode).and(Some(node))
    }

    pub fn append_child<'a, 'py>(&mut self, node: Bound<'py, Node>) -> Option<Bound<'py, Node>> {
        self.node.append_child(&node.borrow().node.as_noderef()).and(Some(node))
    }

    pub fn replace_child<'py>(&mut self, new_child: Bound<'py, Node>, old_child: Bound<'py, Node>) -> Option<Bound<'py, Node>> {
        self.node.replace_child(&new_child.borrow().node.as_noderef(), &old_child.borrow().node.as_noderef()).and(Some(old_child))
    }

    pub fn remove_child<'py>(&mut self, child: Bound<'py, Node>) -> Option<Bound<'py, Node>> {
        self.node.remove_child(&child.borrow().node.as_noderef()).and(Some(child))
    }

    pub fn decompose(&mut self) {
        self.node.decompose()
    }

    pub fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Bound<'_, NodeIter>> {
        Bound::new(slf.py(), NodeIter { iter: slf.node.clone().into_iter() })
    }

    pub fn __contains__(&self, node: &Bound<'_, PyAny>) -> bool {
        node.downcast::<Node>().map_or(false, |n| self.contains(n))
    }

    pub fn __str__(&self) -> String {
        format!("{}", self.node)
    }

    pub fn __repr__(&self) -> String {
        format!("{:?}", self.node)
    }
}


//noinspection DuplicatedCode
#[pymethods]
impl ElementNode {
    #[getter]
    pub fn tag_name(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).tag_name()
    }

    #[getter]
    pub fn tag(slf: PyRef<'_, Self>) -> Option<String> {
        Self::tag_name(slf)
    }

    #[getter]
    pub fn local_name(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).local_name()
    }

    #[getter]
    pub fn id(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).id()
    }

    #[setter]
    pub fn set_id<'py>(mut slf: PyRefMut<'py, Self>, id: &str) {
        Self::raw_node_mut(&mut slf).set_id(id)
    }

    #[getter]
    pub fn class_name(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).class_name()
    }

    #[setter]
    pub fn set_class_name<'py>(mut slf: PyRefMut<'py, Self>, class_names: &str) {
        Self::raw_node_mut(&mut slf).set_class_name(class_names)
    }

    #[getter]
    pub fn class_list(mut slf: PyRefMut<'_, Self>) -> PyResult<Bound<'_, DOMTokenList>> {
        Bound::new(slf.py(), DOMTokenList::from(Self::raw_node_mut(&mut slf).class_list_mut()))
    }

    pub fn get_attribute(slf: PyRef<'_, Self>, qualified_name: &str) -> Option<String> {
        Self::raw_node(&slf).attribute(qualified_name)
    }

    pub fn get_attribute_node<'py>(slf: PyRef<'py, Self>, qualified_name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        if let Some(a) = Self::raw_node(&slf).attribute_node(qualified_name) {
            Ok(Some(create_upcast_node(slf.py(), a.into_node())?))
        } else {
            Ok(None)
        }
    }

    #[getter]
    pub fn get_attribute_names(slf: PyRef<'_, Self>) -> Bound<'_, PyTuple> {
        PyTuple::new_bound(slf.py(), Self::raw_node(&slf).attribute_names().into_iter())
    }

    pub fn has_attribute(slf: PyRef<'_, Self>, qualified_name: &str) -> bool {
        Self::raw_node(&slf).has_attribute(qualified_name)
    }

    pub fn set_attribute(mut slf: PyRefMut<'_, Self>, qualified_name: &str, value: &str) {
        Self::raw_node_mut(&mut slf).set_attribute(qualified_name, value)
    }

    pub fn remove_attribute(mut slf: PyRefMut<'_, Self>, qualified_name: &str) {
        Self::raw_node_mut(&mut slf).remove_attribute(qualified_name)
    }

    #[pyo3(signature = (qualified_name, force=None))]
    pub fn toggle_attribute(mut slf: PyRefMut<'_, Self>, qualified_name: &str, force: Option<bool>) -> bool {
        Self::raw_node_mut(&mut slf).toggle_attribute(qualified_name, force)
    }

    #[getter]
    pub fn attrs(slf: PyRef<'_, Self>) -> Bound<'_, PyTuple> {
        Self::get_attribute_names(slf)
    }

    pub fn hasattr(slf: PyRef<'_, Self>, qualified_name: &str) -> bool {
        Self::has_attribute(slf, qualified_name)
    }

    pub fn __contains__(slf: PyRef<'_, Self>, qualified_name: &str) -> bool {
        Self::has_attribute(slf, qualified_name)
    }

    #[pyo3(signature = (qualified_name, default_value=None))]
    pub fn getattr(slf: PyRef<'_, Self>, qualified_name: &str, default_value: Option<&str>) -> Option<String> {
        Self::get_attribute(slf, qualified_name).or(default_value.map(str::to_owned))
    }

    pub fn __getitem__(slf: PyRef<'_, Self>, qualified_name: &str) -> PyResult<String> {
        Self::get_attribute(slf, qualified_name).map_or_else(
            || Err(PyIndexError::new_err(format!("Attribute {} does not exist", qualified_name))),
            |a| Ok(a))
    }

    pub fn setattr(slf: PyRefMut<'_, Self>, qualified_name: &str, value: &str) {
        Self::set_attribute(slf, qualified_name, value)
    }

    pub fn __setitem__( slf: PyRefMut<'_, Self>, qualified_name: &str, value: &str) {
        Self::set_attribute(slf, qualified_name, value)
    }

    pub fn delattr(slf: PyRefMut<'_, Self>, qualified_name: &str) {
        Self::remove_attribute(slf, qualified_name)
    }

    pub fn __delitem__(mut slf: PyRefMut<'_, Self>, qualified_name: &str) -> PyResult<()> {
        if Self::raw_node_mut(&mut slf).has_attribute(qualified_name) {
            Ok(Self::remove_attribute(slf, qualified_name))
        } else {
            Err(PyIndexError::new_err(format!("Attribute {} does not exist", qualified_name)))
        }
    }

    pub fn closest<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
       Self::raw_node(&slf).closest(selectors).map_or_else(
            |e| Err(CSSParserException::new_err(e.to_string())),
            |n| n.map_or(
                Ok(None),
                |n_| Ok(Some(create_upcast_node(slf.py(), n_.into_node())?))
            )
       )
    }

    pub fn matches<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<bool> {
       Self::raw_node(&slf).matches(selectors).map_or_else(
            |e| Err(CSSParserException::new_err(e.to_string())),
            |n| Ok(n)
       )
    }

    pub fn get_elements_by_tag_name<'py>(slf: PyRef<'py, Self>, name: &str) -> PyResult<Bound<'py, ElementNodeList>> {
        get_elements_by_x!(slf, get_elements_by_tag_name, name)
    }

    pub fn get_elements_by_class_name<'py>(slf: PyRef<'py, Self>, class_names: &str) -> PyResult<Bound<'py, ElementNodeList>> {
        get_elements_by_x!(slf, get_elements_by_class_name, class_names)
    }

    //noinspection DuplicatedCode
    #[pyo3(signature = (name, value, case_insensitive=false))]
    pub fn get_elements_by_attr<'py>(slf: PyRef<'py, Self>, name: &str, value: &str, case_insensitive: bool) -> PyResult<Bound<'py, ElementNodeList>> {
        get_elements_by_x!(slf, get_elements_by_attr_case, name, value, case_insensitive)
    }

    //noinspection DuplicatedCode
    #[pyo3(signature = (element_id, case_insensitive=false))]
    pub fn get_element_by_id<'py>(slf: PyRef<'py, Self>, element_id: &str, case_insensitive: bool) -> PyResult<Option<Bound<'py, ElementNode>>> {
        Self::raw_node(&slf).get_elements_by_attr_case("id", element_id, case_insensitive).into_iter().next().map_or(
            Ok(None),
            |e| Ok(Some(ElementNode::new_bound(slf.py(), e)?))
        )
    }

    #[getter]
    pub fn html(slf: PyRef<'_, Self>) -> String {
        Self::outer_html(slf)
    }

    #[setter]
    pub fn set_html<'py>(slf: PyRefMut<'py, Self>, html: &str) {
        Self::set_inner_html(slf, html)
    }

    #[getter]
    pub fn inner_html(slf: PyRef<'_, Self>) -> String {
        Self::raw_node(&slf).inner_html()
    }

    #[setter]
    pub fn set_inner_html<'py>(mut slf: PyRefMut<'py, Self>, inner_html: &str) {
        Self::raw_node_mut(&mut slf).set_inner_html(inner_html)
    }

    #[getter]
    pub fn outer_html(slf: PyRef<'_, Self>) -> String {
        Self::raw_node(&slf).outer_html()
    }

    #[setter]
    pub fn set_outer_html<'py>(mut slf: PyRefMut<'py, Self>, outer_html: &str) {
        Self::raw_node_mut(&mut slf).set_outer_html(outer_html)
    }

    #[getter]
    pub fn inner_text(slf: PyRef<'_, Self>) -> String {
        Self::raw_node(&slf).inner_text()
    }

    #[setter]
    pub fn set_inner_text<'py>(mut slf: PyRefMut<'py, Self>, inner_text: &str) {
        Self::raw_node_mut(&mut slf).set_inner_text(inner_text)
    }

    #[getter]
    pub fn outer_text(slf: PyRef<'_, Self>) -> String {
        Self::raw_node(&slf).outer_text()
    }

    #[setter]
    pub fn set_outer_text<'py>(mut slf: PyRefMut<'py, Self>, outer_text: &str) {
        Self::raw_node_mut(&mut slf).set_outer_text(outer_text)
    }

    // _ChildNodeMixin
    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn before(slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::before_(slf, node)
    }

    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn after(slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::after_(slf, node)
    }

    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn replace_with(slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::replace_with_(slf, node)
    }

    #[inline(always)]
    pub fn remove(slf: PyRefMut<'_, Self>) {
        Self::remove_(slf);
    }

    // _NonDocumentTypeChildNodeMixin
    #[getter]
    #[inline(always)]
    pub fn next_element_sibling(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNode>>> {
        Self::next_element_sibling_(slf)
    }

    #[getter]
    #[inline(always)]
    pub fn next_element(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNode>>> {
        Self::next_element_sibling_(slf)
    }

    #[getter]
    #[inline(always)]
    pub fn previous_element_sibling(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNode>>> {
        Self::previous_element_sibling_(slf)
    }

    #[getter]
    #[inline(always)]
    pub fn prev_element(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNode>>> {
        Self::previous_element_sibling_(slf)
    }

    // _ParentNodeMixin
    #[getter]
    #[inline(always)]
    pub fn children(slf: PyRef<'_, Self>) -> PyResult<Bound<'_, ElementNodeList>> {
        Self::children_(slf)
    }

    #[getter]
    #[inline(always)]
    pub fn first_element_child(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNode>>> {
        Self::first_element_child_(slf)
    }

    #[getter]
    #[inline(always)]
    pub fn last_element_child(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNode>>> {
        Self::last_element_child_(slf)
    }

    #[getter]
    #[inline(always)]
    fn child_element_count(slf: PyRef<'_, Self>) -> usize {
        Self::child_element_count_(slf)
    }

    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn prepend(slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::prepend_(slf, node)
    }

    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn append(slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::append_(slf, node)
    }

    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn replace_children(slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::replace_children_(slf, node)
    }

    #[inline(always)]
    pub fn query_selector<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Option<Bound<'py, ElementNode>>> {
        Self::query_selector_(slf, selectors)
    }

    #[inline(always)]
    pub fn query_selector_all<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Bound<'py, ElementNodeList>> {
        Self::query_selector_all_(slf, selectors)
    }
}

impl _ChildNodeMixin<node_impl::ElementNode> for ElementNode {}
impl _NonDocumentTypeChildNodeMixin<node_impl::ElementNode> for ElementNode {}
impl _ParentNodeMixin<node_impl::ElementNode> for ElementNode {}


//noinspection DuplicatedCode
#[pymethods]
impl AttrNode {
    #[getter]
    pub fn local_name(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).local_name()
    }

    #[getter]
    pub fn name(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).name()
    }

    #[getter]
    pub fn value(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).value()
    }

    #[setter]
    pub fn set_value<'py>(mut slf: PyRefMut<'py, Self>, value: &str) {
        Self::raw_node_mut(&mut slf).set_value(value)
    }

    #[getter]
    pub fn owner_element(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNode>>> {
        Self::raw_node(&slf).owner_element().map_or(
            Ok(None),
            |e| Ok(Some(ElementNode::new_bound(slf.py(), e)?))
        )
    }
}


impl _ChildNodeMixin<node_impl::DocumentTypeNode> for DocumentTypeNode {}

//noinspection DuplicatedCode
#[pymethods]
impl DocumentTypeNode {
    #[getter]
    pub fn name(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).name()
    }

    #[getter]
    pub fn public_id(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).public_id()
    }

    #[getter]
    pub fn system_id(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).system_id()
    }

    // _ChildNodeMixin
    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn before(slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::before_(slf, node)
    }

    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn after(slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::after_(slf, node)
    }

    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn replace_with(slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::replace_with_(slf, node)
    }

    #[inline(always)]
    pub fn remove(slf: PyRefMut<'_, Self>) {
        Self::remove_(slf);
    }
}


macro_rules! doc_create_x {
    ($slf: ident, $func_name: ident $(, $args: ident)*) => {
        Self::raw_node_mut(&mut $slf).$func_name($($args),*).map_or_else(
            |e| Err(DOMException::new_err(e.to_string())),
            |e| Ok(Some(create_upcast_node($slf.py(), e.into_node())?))
        )
    };
}

//noinspection DuplicatedCode
#[pymethods]
impl DocumentNode {
    #[getter]
    pub fn doctype(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, PyAny>>> {
        Self::raw_node(&slf).doctype().map_or_else(
            || Err(DOMException::new_err("No Document node")),
            |d| Ok(Some(create_upcast_node(slf.py(), d.into_node())?))
        )
    }

    #[getter]
    pub fn document_element(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNode>>> {
        Self::raw_node(&slf).document_element().map_or_else(
            || Err(DOMException::new_err("No Document node")),
            |d| d.first_element_child().map_or(
                Ok(None),
                |e| Ok(Some(ElementNode::new_bound(slf.py(), e)?))
            )
        )
    }

    pub fn get_elements_by_tag_name<'py>(slf: PyRef<'py, Self>, name: &str) -> PyResult<Bound<'py, ElementNodeList>> {
        get_elements_by_x!(slf, get_elements_by_tag_name, name)
    }

    //noinspection DuplicatedCode
    pub fn get_elements_by_class_name<'py>(slf: PyRef<'py, Self>, class_names: &str) -> PyResult<Bound<'py, ElementNodeList>> {
        get_elements_by_x!(slf, get_elements_by_class_name, class_names)
    }

    //noinspection DuplicatedCode
    #[pyo3(signature = (name, value, case_insensitive=false))]
    pub fn get_elements_by_attr<'py>(slf: PyRef<'py, Self>, name: &str, value: &str, case_insensitive: bool) -> PyResult<Bound<'py, ElementNodeList>> {
        get_elements_by_x!(slf, get_elements_by_attr_case, name, value, case_insensitive)
    }

    pub fn create_element<'py>(mut slf: PyRefMut<'py, Self>, local_name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        doc_create_x!(slf, create_element, local_name)
    }

    pub fn create_document_fragment(mut slf: PyRefMut<'_, Self>) -> PyResult<Option<Bound<'_, PyAny>>> {
        doc_create_x!(slf, create_document_fragment)
    }

    pub fn create_text_node<'py>(mut slf: PyRefMut<'py, Self>, data: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        doc_create_x!(slf, create_text_node, data)
    }

    pub fn create_cdata_section<'py>(mut slf: PyRefMut<'py, Self>, data: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        doc_create_x!(slf, create_cdata_section, data)
    }

    pub fn create_comment<'py>(mut slf: PyRefMut<'py, Self>, data: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        doc_create_x!(slf, create_comment, data)
    }

    pub fn create_processing_instruction<'py>(mut slf: PyRefMut<'py, Self>, target: &str, local_name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        doc_create_x!(slf, create_processing_instruction, target, local_name)
    }

    pub fn create_attribute<'py>(mut slf: PyRefMut<'py, Self>, local_name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        doc_create_x!(slf, create_attribute, local_name)
    }

    // _ParentNodeMixin
    #[getter]
    #[inline(always)]
    pub fn children(slf: PyRef<'_, Self>) -> PyResult<Bound<'_, ElementNodeList>> {
        Self::children_(slf)
    }

    #[getter]
    #[inline(always)]
    pub fn first_element_child(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNode>>> {
        Self::first_element_child_(slf)
    }

    #[getter]
    #[inline(always)]
    pub fn last_element_child(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNode>>> {
        Self::last_element_child_(slf)
    }

    #[getter]
    #[inline(always)]
    pub fn child_element_count(slf: PyRef<'_, Self>) -> usize {
        Self::child_element_count_(slf)
    }

    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn prepend(slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::prepend_(slf, node)
    }

    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn append(slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::append_(slf, node)
    }

    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn replace_children(slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::replace_children_(slf, node)
    }

    #[inline(always)]
    pub fn query_selector<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Option<Bound<'py, ElementNode>>> {
        Self::query_selector_(slf, selectors)
    }

    #[inline(always)]
    pub fn query_selector_all<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Bound<'py, ElementNodeList>> {
        Self::query_selector_all_(slf, selectors)
    }

    // _NonElementParentNodeMixin
    #[inline(always)]
    #[pyo3(signature = (element_id, case_insensitive=false))]
    pub fn get_element_by_id<'py>(slf: PyRef<'py, Self>, element_id: &str, case_insensitive: bool) -> PyResult<Option<Bound<'py, ElementNode>>> {
        Self::get_element_by_id_(slf, element_id, case_insensitive)
    }
}

impl _ParentNodeMixin<node_impl::DocumentNode> for DocumentNode {}
impl _NonElementParentNodeMixin<node_impl::DocumentNode> for DocumentNode {}


//noinspection DuplicatedCode
#[pymethods]
impl DocumentFragmentNode {
    // _ParentNodeMixin
    #[getter]
    #[inline(always)]
    pub fn children(slf: PyRef<'_, Self>) -> PyResult<Bound<'_, ElementNodeList>> {
        Self::children_(slf)
    }

    #[getter]
    #[inline(always)]
    pub fn first_element_child(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNode>>> {
        Self::first_element_child_(slf)
    }

    #[getter]
    #[inline(always)]
    pub fn last_element_child(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNode>>> {
        Self::last_element_child_(slf)
    }

    #[getter]
    #[inline(always)]
    pub fn child_element_count(slf: PyRef<'_, Self>) -> usize {
        Self::child_element_count_(slf)
    }

    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn prepend(slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::prepend_(slf, node)
    }

    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn append(slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::append_(slf, node)
    }

    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn replace_children(slf: PyRefMut<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::replace_children_(slf, node)
    }

    #[inline(always)]
    pub fn query_selector<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Option<Bound<'py, ElementNode>>> {
        Self::query_selector_(slf, selectors)
    }

    #[inline(always)]
    pub fn query_selector_all<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Bound<'py, ElementNodeList>> {
        Self::query_selector_all_(slf, selectors)
    }

    // _NonElementParentNodeMixin
    #[inline(always)]
    #[pyo3(signature = (element_id, case_insensitive=false))]
    pub fn get_element_by_id<'py>(slf: PyRef<'py, Self>, element_id: &str, case_insensitive: bool) -> PyResult<Option<Bound<'py, ElementNode>>> {
        Self::get_element_by_id_(slf, element_id, case_insensitive)
    }
}

impl _ParentNodeMixin<node_impl::DocumentFragmentNode> for DocumentFragmentNode {}
impl _NonElementParentNodeMixin<node_impl::DocumentFragmentNode> for DocumentFragmentNode {}


character_data_node!(TextNode, node_impl::TextNode);
character_data_node!(CDATASectionNode, node_impl::CdataSectionNode);
character_data_node!(ProcessingInstructionNode, node_impl::ProcessingInstructionNode, target, Option<String>);
character_data_node!(CommentNode, node_impl::CommentNode);


#[pyclass(module = "resiliparse.parse._html_rs.node")]
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
