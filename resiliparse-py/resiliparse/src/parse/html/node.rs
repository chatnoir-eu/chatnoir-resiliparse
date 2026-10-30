// Copyright 2026 Janek Bevendorff
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

use super::coll::*;
use super::exception::*;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::*;
use resiliparse::parse::html::dom::traits::*;
use resiliparse::parse::html::dom::{iter, node};

/// The type of DOM node.
#[pyclass(
    name = "NodeType",
    eq,
    eq_int,
    rename_all = "SCREAMING_SNAKE_CASE",
    module = "resiliparse.parse.html.node"
)]
#[derive(PartialEq, Eq)]
pub enum NodeTypePy {
    /// An element node.
    Element = 0x01,
    /// An attribute node.
    Attribute = 0x02,
    /// A text node.
    Text = 0x03,
    /// A CDATA section node.
    CdataSection = 0x04,
    /// An entity reference node.
    EntityReference = 0x05,
    /// An entity node.
    Entity = 0x06,
    /// A processing instruction node.
    ProcessingInstruction = 0x07,
    /// A comment node.
    Comment = 0x08,
    /// A document node.
    Document = 0x09,
    /// A document type node.
    DocumentType = 0x0A,
    /// A document fragment node.
    DocumentFragment = 0x0B,
    /// A notation node.
    Notation = 0x0C,
}

/// A DOM node.
///
/// A node exposes DOM navigation, content access, and tree-editing operations.
#[pyclass(
    name = "Node",
    eq,
    weakref,
    unsendable,
    subclass,
    module = "resiliparse.parse.html.node",
    from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct NodePy {
    pub(crate) node: node::Node,
    identity: Option<(usize, usize)>,
}

impl From<node::Node> for NodePy {
    fn from(value: node::Node) -> Self {
        NodePy {
            identity: value.identity(),
            node: value,
        }
    }
}
pub(crate) fn create_upcast_node(py: Python, node: node::Node) -> PyResult<Bound<PyAny>> {
    Ok(match node {
        // TODO: Replace with Bound::into_super() in PyO3 0.23
        node::Node::Element(e) => ElementNodePy::new_bound(py, e)?.into_any(),
        node::Node::Attribute(e) => AttrNodePy::new_bound(py, e)?.into_any(),
        node::Node::Text(e) => TextNodePy::new_bound(py, e)?.into_any(),
        node::Node::CdataSection(e) => CDATASectionNodePy::new_bound(py, e)?.into_any(),
        node::Node::ProcessingInstruction(e) => ProcessingInstructionNodePy::new_bound(py, e)?.into_any(),
        node::Node::Comment(e) => CommentNodePy::new_bound(py, e)?.into_any(),
        node::Node::Document(e) => DocumentNodePy::new_bound(py, e)?.into_any(),
        node::Node::DocumentType(e) => DocumentTypeNodePy::new_bound(py, e)?.into_any(),
        node::Node::DocumentFragment(e) => DocumentFragmentNodePy::new_bound(py, e)?.into_any(),
        node::Node::Notation(e) => NotationNodePy::new_bound(py, e)?.into_any(),
    })
}

// ------------------------------------ Node Interface Mixins --------------------------------------

macro_rules! get_elements_by_x {
    ($slf: ident, $func_name: ident $(, $args: ident)*) => {
        ElementNodeListPy::new_bound($slf.py(), Self::raw_node(&$slf).$func_name($($args),*))
    };
}

trait _NodeAccessorMixin<T: NodeInterface>: pyo3::PyClass<Frozen = pyo3::pyclass::boolean_struct::False> {
    fn raw_node<'py, 'a>(slf: &'a PyRef<'py, Self>) -> &'a T;
}

trait _ParentNodeMixin<T: ParentNode>: _NodeAccessorMixin<T> {
    fn children_(slf: PyRef<'_, Self>) -> PyResult<Bound<'_, ElementNodeListPy>> {
        ElementNodeListPy::new_bound(slf.py(), Self::raw_node(&slf).children())
    }

    fn first_element_child_(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNodePy>>> {
        Self::raw_node(&slf)
            .first_element_child()
            .map_or(Ok(None), |e| Ok(Some(ElementNodePy::new_bound(slf.py(), e)?)))
    }

    fn last_element_child_(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNodePy>>> {
        Self::raw_node(&slf)
            .last_element_child()
            .map_or(Ok(None), |e| Ok(Some(ElementNodePy::new_bound(slf.py(), e)?)))
    }

    #[inline]
    fn child_element_count_(slf: PyRef<'_, Self>) -> usize {
        Self::raw_node(&slf).child_element_count()
    }

    fn prepend_(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        let n = node.extract::<Vec<NodePy>>()?;
        Ok(Self::raw_node(&slf).prepend(&n.iter().map(|n_| n_.node.as_noderef()).collect::<Vec<_>>()))
    }

    fn append_(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        let n = node.extract::<Vec<NodePy>>()?;
        Ok(Self::raw_node(&slf).append(&n.iter().map(|n_| n_.node.as_noderef()).collect::<Vec<_>>()))
    }

    fn replace_children_(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        let n = node.extract::<Vec<NodePy>>()?;
        Ok(Self::raw_node(&slf).replace_children(&n.iter().map(|n_| n_.node.as_noderef()).collect::<Vec<_>>()))
    }

    //noinspection DuplicatedCode
    fn query_selector_<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Option<Bound<'py, ElementNodePy>>> {
        Self::raw_node(&slf).query_selector(selectors).map_or_else(
            |e| Err(CSSParserExceptionPy::new_err(e.to_string())),
            |e| e.map_or(Ok(None), |e_| Ok(Some(ElementNodePy::new_bound(slf.py(), e_)?))),
        )
    }

    //noinspection DuplicatedCode
    fn query_selector_all_<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Bound<'py, ElementNodeListPy>> {
        Self::raw_node(&slf).query_selector_all(selectors).map_or_else(
            |e| Err(CSSParserExceptionPy::new_err(e.to_string())),
            |e| Ok(ElementNodeListPy::new_bound(slf.py(), e)?),
        )
    }
}

trait _NonElementParentNodeMixin<T: NonElementParentNode>: _NodeAccessorMixin<T> {
    fn get_element_by_id_<'py>(
        slf: PyRef<'py, Self>,
        element_id: &str,
        case_insensitive: bool,
    ) -> PyResult<Option<Bound<'py, ElementNodePy>>> {
        Self::raw_node(&slf)
            .get_element_by_id_case(element_id, case_insensitive)
            .map_or(Ok(None), |e| Ok(Some(ElementNodePy::new_bound(slf.py(), e)?)))
    }
}

macro_rules! child_node_funcs {
    ($func_name: ident, $call_func_name: ident) => {
        fn $func_name(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
            let n = node.extract::<Vec<NodePy>>()?;
            let r: Vec<_> = n.iter().map(|n_| n_.node.as_noderef()).collect();
            Self::raw_node(&slf).$call_func_name(&r);
            Ok(())
        }
    };
}

trait _ChildNodeMixin<T: ChildNode>: _NodeAccessorMixin<T> {
    child_node_funcs!(before_, before);
    child_node_funcs!(after_, after);
    child_node_funcs!(replace_with_, replace_with);

    #[inline(always)]
    fn remove_(slf: PyRef<'_, Self>) {
        Self::raw_node(&slf).remove();
    }
}

trait _NonDocumentTypeChildNodeMixin<T: NonDocumentTypeChildNode>: _NodeAccessorMixin<T> {
    fn next_element_sibling_(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNodePy>>> {
        Self::raw_node(&slf)
            .next_element_sibling()
            .map_or(Ok(None), |e| Ok(Some(ElementNodePy::new_bound(slf.py(), e)?)))
    }

    fn previous_element_sibling_(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNodePy>>> {
        Self::raw_node(&slf)
            .previous_element_sibling()
            .map_or(Ok(None), |e| Ok(Some(ElementNodePy::new_bound(slf.py(), e)?)))
    }
}

macro_rules! character_data_node {
    ($Self: ident, $Base: ty $(, $func_name: ident, $out_type: ty)*) => {
        #[pymethods]
        impl $Self {
            /// Return the number of items.
            #[inline(always)]
            pub fn __len__(slf: PyRef<'_, Self>) -> usize {
                Self::raw_node(&slf).len()
            }

            /// Return whether the character data is empty.
            #[inline(always)]
            pub fn is_empty(slf: PyRef<'_, Self>) -> bool {
                Self::raw_node(&slf).is_empty()
            }

            /// The character data.
            #[getter]
            #[inline(always)]
            pub fn data(slf: PyRef<'_, Self>) -> Option<String> {
                Self::raw_node(&slf).data()
            }

            /// Replace the character data.
            ///
            /// :param data: Character data to store in the node.
            #[setter]
            #[inline(always)]
            pub fn set_data<'py>(slf: PyRef<'py, Self>, data: &str) {
                Self::raw_node(&slf).set_data(data)
            }

            /// Return a portion of the character data.
            ///
            /// :param offset: Character offset at which to edit or read data.
            /// :param count: Number of characters to read or remove.
            #[inline(always)]
            pub fn substring_data(slf: PyRef<'_, Self>, offset: usize, count: usize) -> PyResult<String> {
                Self::raw_node(&slf).substring_data(offset, count).map_err(|error| DOMExceptionPy::new_err(error.to_string()))
            }

            /// Append text to the character data.
            ///
            /// :param data: Character data to store in the node.
            #[inline(always)]
            pub fn append_data(slf: PyRef<'_, Self>, data: &str) {
                Self::raw_node(&slf).append_data(data)
            }

            /// Insert text into the character data.
            ///
            /// :param offset: Character offset at which to edit or read data.
            /// :param data: Character data to store in the node.
            #[inline(always)]
            pub fn insert_data(slf: PyRef<'_, Self>, offset: usize, data: &str) -> PyResult<()> {
                Self::raw_node(&slf).insert_data(offset, data).map_err(|error| DOMExceptionPy::new_err(error.to_string()))
            }

            /// Delete a portion of the character data.
            ///
            /// :param offset: Character offset at which to edit or read data.
            /// :param count: Number of characters to read or remove.
            #[inline(always)]
            pub fn delete_data(slf: PyRef<'_, Self>, offset: usize, count: usize) -> PyResult<()> {
                Self::raw_node(&slf).delete_data(offset, count).map_err(|error| DOMExceptionPy::new_err(error.to_string()))
            }

            /// Replace a portion of the character data.
            ///
            /// :param offset: Character offset at which to edit or read data.
            /// :param count: Number of characters to read or remove.
            /// :param data: Character data to store in the node.
            #[inline(always)]
            pub fn replace_data(slf: PyRef<'_, Self>, offset: usize, count: usize, data: &str) -> PyResult<()> {
                Self::raw_node(&slf).replace_data(offset, count, data).map_err(|error| DOMExceptionPy::new_err(error.to_string()))
            }

            $(/// The target of this node.
            #[getter]
            #[inline(always)]
            pub fn target(slf: PyRef<'_, Self>) -> $out_type {
                Self::raw_node(&slf).$func_name()
            })*

            /// Insert nodes immediately before this node.
            ///
            /// :param node: Node to append as a child.
            #[pyo3(signature = (*node))]
            #[inline(always)]
            pub fn before(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
                Self::before_(slf, node)
            }

            /// Insert nodes immediately after this node.
            ///
            /// :param node: Node to append as a child.
            #[pyo3(signature = (*node))]
            #[inline(always)]
            pub fn after(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
                Self::after_(slf, node)
            }

            /// Replace this node with the given nodes.
            ///
            /// :param node: Node to append as a child.
            #[pyo3(signature = (*node))]
            #[inline(always)]
            pub fn replace_with(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
                Self::replace_with_(slf, node)
            }

            /// Remove this node from its parent.
            #[inline(always)]
            pub fn remove(slf: PyRef<'_, Self>) {
                Self::remove_(slf);
            }

            /// The next sibling element, if one exists.
            #[inline(always)]
            #[getter]
            pub fn next_element_sibling(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNodePy>>> {
                Self::next_element_sibling_(slf)
            }

            /// The previous sibling element, if one exists.
            #[getter]
            #[inline(always)]
            pub fn previous_element_sibling(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNodePy>>> {
                Self::previous_element_sibling_(slf)
            }
        }

        impl _ChildNodeMixin<$Base> for $Self {}
        impl _NonDocumentTypeChildNodeMixin<$Base> for $Self {}
    }
}

// ------------------------------------------ Node Types -------------------------------------------

macro_rules! define_node_type {
    ($Self: ident, $py_name: literal, $BaseEnum: ident, $Base: ty) => {
        #[pyclass(name = $py_name, unsendable, extends=NodePy, module = "resiliparse.parse.html.node", from_py_object)]
        #[derive(Clone)]
        pub struct $Self;

        impl $Self {
            pub fn new_bound(py: Python, node: $Base) -> PyResult<Bound<$Self>> {
                let cache = py.import("resiliparse.parse.html.node")?.getattr("_node_cache")?;
                let key = node.identity();
                if let Some(key) = key {
                    let cached = cache.call_method1("get", (key,))?;
                    if let Ok(cached) = cached.cast::<$Self>() {
                        if cached.borrow().as_super().node == node.as_node() {
                            return Ok(cached.clone());
                        }
                    }
                }
                let result = Bound::new(py, (Self {}, NodePy::from(node)))?;
                if let Some(key) = key {
                    cache.set_item(key, &result)?;
                }
                Ok(result)
            }
        }

        impl _NodeAccessorMixin<$Base> for $Self {
            fn raw_node<'py, 'a>(slf: &'a PyRef<'py, Self>) -> &'a $Base {
                match &slf.as_super().node {
                    node::Node::$BaseEnum(n) => n,
                    _ => unreachable!(),
                }
            }
        }

        impl From<$Base> for NodePy {
            fn from(value: $Base) -> NodePy {
                NodePy {
                    identity: value.identity(),
                    node: value.as_node(),
                }
            }
        }
    };
}

define_node_type!(ElementNodePy, "ElementNode", Element, node::ElementNode);
define_node_type!(AttrNodePy, "AttrNode", Attribute, node::AttrNode);
define_node_type!(TextNodePy, "TextNode", Text, node::TextNode);
define_node_type!(CDATASectionNodePy, "CDATASectionNode", CdataSection, node::CdataSectionNode);
define_node_type!(
    ProcessingInstructionNodePy,
    "ProcessingInstructionNode",
    ProcessingInstruction,
    node::ProcessingInstructionNode
);
define_node_type!(CommentNodePy, "CommentNode", Comment, node::CommentNode);
define_node_type!(DocumentNodePy, "DocumentNode", Document, node::DocumentNode);
define_node_type!(DocumentTypeNodePy, "DocumentTypeNode", DocumentType, node::DocumentTypeNode);
define_node_type!(DocumentFragmentNodePy, "DocumentFragmentNode", DocumentFragment, node::DocumentFragmentNode);
define_node_type!(NotationNodePy, "NotationNode", Notation, node::NotationNode);

//noinspection DuplicatedCode
#[pymethods]
impl NodePy {
    pub fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.identity.hash(&mut hasher);
        hasher.finish()
    }

    /// The type of this DOM node.
    #[getter]
    pub fn node_type(&self) -> Option<NodeTypePy> {
        Some(match self.node.node_type()? {
            node::NodeType::Element => NodeTypePy::Element,
            node::NodeType::Attribute => NodeTypePy::Attribute,
            node::NodeType::Text => NodeTypePy::Text,
            node::NodeType::CdataSection => NodeTypePy::CdataSection,
            node::NodeType::EntityReference => NodeTypePy::EntityReference,
            node::NodeType::Entity => NodeTypePy::Entity,
            node::NodeType::ProcessingInstruction => NodeTypePy::ProcessingInstruction,
            node::NodeType::Comment => NodeTypePy::Comment,
            node::NodeType::Document => NodeTypePy::Document,
            node::NodeType::DocumentType => NodeTypePy::DocumentType,
            node::NodeType::DocumentFragment => NodeTypePy::DocumentFragment,
            node::NodeType::Notation => NodeTypePy::Notation,
        })
    }

    /// The node type, using the legacy DOM API name.
    #[getter]
    #[pyo3(name = "type")]
    pub fn type_(&self) -> Option<NodeTypePy> {
        self.node_type()
    }

    /// The DOM name of this node.
    #[getter]
    pub fn node_name(&self) -> Option<String> {
        self.node.node_name()
    }

    /// The DOM element tag or node name. This is the legacy ``tag`` property.
    #[getter]
    pub fn tag(&self) -> Option<String> {
        self.node_name().map(|name| name.to_lowercase())
    }

    /// Node text value.
    #[getter]
    pub fn value(&self) -> Option<String> {
        self.node_value()
    }

    /// Set the value associated with this node or token list.
    ///
    /// :param value: Attribute value to match.
    #[setter]
    pub fn set_value(&self, value: &str) {
        self.set_node_value(value)
    }

    /// The node value.
    #[getter]
    pub fn node_value(&self) -> Option<String> {
        self.node.node_value()
    }

    /// Set the node value.
    ///
    /// :param value: Attribute value to match.
    #[setter]
    pub fn set_node_value(&self, value: &str) {
        self.node.set_node_value(value)
    }

    /// Text contents of this DOM node and its children, or ``None`` if unavailable.
    ///
    /// The text can be changed by assigning to this property.
    ///
    /// .. versionchanged:: 1.2.0
    ///    Return ``None`` instead of an empty string when text content is unavailable.
    ///
    /// :type: str or None
    #[getter]
    pub fn text(&self) -> Option<String> {
        self.text_content()
    }

    /// Set the text content, using the legacy DOM API name.
    ///
    /// :param text: Text to store in the node.
    #[setter]
    pub fn set_text(&self, text: &str) {
        self.set_text_content(text)
    }

    /// Text contents of this DOM node and its children, or ``None`` if unavailable.
    ///
    /// The text can be changed by assigning to this property.
    ///
    /// .. versionchanged:: 1.2.0
    ///    Return ``None`` instead of an empty string when text content is unavailable.
    ///
    /// :type: str or None
    #[getter]
    pub fn text_content(&self) -> Option<String> {
        self.node.text_content()
    }

    /// Replace the descendant text content of this node.
    ///
    /// :param text: Text to store in the node.

    #[setter]
    pub fn set_text_content(&self, text: &str) {
        self.node.set_text_content(text)
    }

    /// HTML contents of this DOM node and its children.
    ///
    /// The inner HTML can be changed by assigning to this property.
    #[getter]
    pub fn html(&self) -> String {
        self.node.to_string()
    }

    /// Set the element's inner HTML, matching the Cython DOM API.
    /// Replace this element’s contents from an HTML string.
    ///
    /// :param html: HTML string used to replace this element’s contents.
    #[setter]
    pub fn set_html(&self, html: &str) -> PyResult<()> {
        match &self.node {
            node::Node::Element(element) => {
                element.set_inner_html(html);
                Ok(())
            }
            _ => Err(pyo3::exceptions::PyTypeError::new_err("The html property can only be set on an element node")),
        }
    }

    /// The document that owns this node.
    #[getter]
    pub fn owner_document<'py>(&self, py: Python<'py>) -> Option<Bound<'py, DocumentNodePy>> {
        DocumentNodePy::new_bound(py, self.node.owner_document()?).ok()
    }

    /// Parent of this node.
    #[getter]
    pub fn parent<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyAny>> {
        self.parent_node(py)
    }

    /// Parent of this node.
    #[getter]
    pub fn parent_node<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyAny>> {
        create_upcast_node(py, self.node.parent_node()?).ok()
    }

    /// The parent element, if one exists.
    #[getter]
    pub fn parent_element<'py>(&self, py: Python<'py>) -> Option<Bound<'py, ElementNodePy>> {
        if let node::Node::Element(e) = self.node.parent_node()? {
            ElementNodePy::new_bound(py, e).ok()
        } else {
            None
        }
    }

    /// Return whether this node has child nodes.
    pub fn has_child_nodes(&self) -> bool {
        self.node.has_child_nodes()
    }

    /// Check whether this node contains the given node.
    ///
    /// :param node: Node to append as a child.
    pub fn contains<'py>(&self, node: &Bound<'py, NodePy>) -> bool {
        self.node.contains(&node.borrow().node.as_noderef())
    }

    /// List of child nodes.
    #[getter]
    pub fn child_nodes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, NodeListPy>> {
        NodeListPy::new_bound(py, self.node.child_nodes())
    }

    /// First child node of this DOM node.
    #[getter]
    pub fn first_child<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.node
            .first_child()
            .map_or(Ok(None), |n| Ok(Some(create_upcast_node(py, n)?)))
    }

    /// Last child node of this DOM node.
    #[getter]
    pub fn last_child<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.node
            .last_child()
            .map_or(Ok(None), |n| Ok(Some(create_upcast_node(py, n)?)))
    }

    /// The previous sibling node, if one exists.
    #[getter]
    pub fn previous_sibling<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.node
            .previous_sibling()
            .map_or(Ok(None), |n| Ok(Some(create_upcast_node(py, n)?)))
    }

    /// Previous sibling node.
    #[getter]
    pub fn prev<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.previous_sibling(py)
    }

    /// The next sibling node, if one exists.
    #[getter]
    pub fn next_sibling<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.node
            .next_sibling()
            .map_or(Ok(None), |n| Ok(Some(create_upcast_node(py, n)?)))
    }

    /// Next sibling node.
    #[getter]
    pub fn next<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.next_sibling(py)
    }

    /// Next sibling element node.
    #[getter]
    pub fn next_element<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, ElementNodePy>>> {
        let mut sibling = self.node.next_sibling();
        while let Some(node) = sibling {
            match node {
                node::Node::Element(element) => return ElementNodePy::new_bound(py, element).map(Some),
                node => sibling = node.next_sibling(),
            }
        }
        Ok(None)
    }

    /// Previous sibling element node.
    #[getter]
    pub fn prev_element<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, ElementNodePy>>> {
        let mut sibling = self.node.previous_sibling();
        while let Some(node) = sibling {
            match node {
                node::Node::Element(element) => return ElementNodePy::new_bound(py, element).map(Some),
                node => sibling = node.previous_sibling(),
            }
        }
        Ok(None)
    }

    /// Create a copy of this node.
    ///
    /// :param deep: Whether to clone descendant nodes as well.
    #[pyo3(signature = (deep=false))]
    pub fn clone_node<'py>(&self, py: Python<'py>, deep: bool) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.node
            .clone_node(deep)
            .map_or(Ok(None), |n| Ok(Some(create_upcast_node(py, n)?)))
    }

    /// Insert ``node`` before ``reference`` as a child. The reference must be a child of this node; ``None`` appends the new node after the last child.
    ///
    /// :param node: Node to append as a child.
    /// :param reference: Child node before which the new node is inserted.
    #[pyo3(signature = (node, reference=None))]
    pub fn insert_before<'py>(
        &self,
        node: Bound<'py, NodePy>,
        reference: Option<Bound<'py, NodePy>>,
    ) -> Option<Bound<'py, NodePy>> {
        let b;
        let refnode = if let Some(r) = &reference {
            b = r.borrow();
            Some(b.node.as_noderef())
        } else {
            None
        };
        self.node
            .insert_before(&node.borrow().node.as_noderef(), refnode)
            .and(Some(node))
    }

    /// Append a new child node and return the appended node.
    ///
    /// :param node: Node to append as a child.
    pub fn append_child<'a, 'py>(&self, node: Bound<'py, NodePy>) -> Option<Bound<'py, NodePy>> {
        self.node.append_child(&node.borrow().node.as_noderef()).and(Some(node))
    }

    /// Replace the child node ``old_child`` with ``new_child`` and return the removed child.
    ///
    /// :param new_child: New child node to insert.
    /// :param old_child: Existing child node to replace.
    pub fn replace_child<'py>(
        &self,
        new_child: Bound<'py, NodePy>,
        old_child: Bound<'py, NodePy>,
    ) -> Option<Bound<'py, NodePy>> {
        self.node
            .replace_child(&new_child.borrow().node.as_noderef(), &old_child.borrow().node.as_noderef())
            .and(Some(old_child))
    }

    /// Remove the child node from the DOM tree and return it.
    ///
    /// :param child: Child node to remove.
    pub fn remove_child<'py>(&self, child: Bound<'py, NodePy>) -> Option<Bound<'py, NodePy>> {
        self.node
            .remove_child(&child.borrow().node.as_noderef())
            .and(Some(child))
    }

    /// Delete the current node and all its children.
    pub fn decompose(&self) {
        self.node.decompose()
    }

    /// Traverse this subtree in pre-order, starting at this node.
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Bound<'_, NodeIterPy>> {
        Bound::new(
            slf.py(),
            NodeIterPy {
                iter: slf.node.clone().into_iter(),
            },
        )
    }

    /// Return whether the requested node, token, or value is present.
    ///
    /// :param node: Node whose containment is checked.
    pub fn __contains__(&self, node: &Bound<'_, PyAny>) -> bool {
        node.cast::<NodePy>().map_or(false, |n| self.contains(n))
    }

    /// Return the serialized HTML document.
    pub fn __str__(&self) -> String {
        format!("{}", self.node)
    }

    /// Return a readable representation of this object.
    pub fn __repr__(&self) -> String {
        format!("{:?}", self.node)
    }
}

// noinspection DuplicatedCode
#[pymethods]
impl ElementNodePy {
    /// The qualified tag name of this element.
    #[getter]
    pub fn tag_name(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).tag_name()
    }

    /// The local name of this node.
    #[getter]
    pub fn local_name(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).local_name()
    }

    /// ID attribute of this element, or ``None`` if the attribute is absent.
    /// An existing attribute with an empty value returns an empty string.
    ///
    /// .. versionchanged:: 1.2.0
    ///    Return ``None`` instead of an empty string when the attribute is absent.
    ///
    /// :type: str or None
    #[getter]
    pub fn id(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).id()
    }

    /// Set the element ID.
    ///
    /// :param id: ID value to assign to the element.
    #[setter]
    pub fn set_id<'py>(slf: PyRef<'py, Self>, id: &str) {
        Self::raw_node(&slf).set_id(id)
    }

    /// Class name attribute of this element, or ``None`` if the attribute is absent.
    /// An existing attribute with an empty value returns an empty string.
    ///
    /// .. versionchanged:: 1.2.0
    ///    Return ``None`` instead of an empty string when the attribute is absent.
    ///
    /// :type: str or None
    #[getter]
    pub fn class_name(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).class_name()
    }

    /// Set the element’s class names.
    ///
    /// :param class_names: Whitespace-separated class names to match.
    #[setter]
    pub fn set_class_name<'py>(slf: PyRef<'py, Self>, class_names: &str) {
        Self::raw_node(&slf).set_class_name(class_names)
    }

    /// List of class names set on this element.
    #[getter]
    pub fn class_list(slf: PyRef<'_, Self>) -> PyResult<Bound<'_, DOMTokenListPy>> {
        Bound::new(slf.py(), DOMTokenListPy::from(Self::raw_node(&slf).class_list()))
    }

    /// Get the value of an attribute, or ``default_value`` if it is unset.
    ///
    /// Attributes without a value (e.g., ``<input checked>``) return an empty string.
    ///
    /// :param qualified_name: Qualified attribute name.
    /// :param default_value: Value returned when the attribute is absent.
    #[pyo3(signature = (qualified_name, default_value=None))]
    pub fn get_attribute(slf: PyRef<'_, Self>, qualified_name: &str, default_value: Option<&str>) -> Option<String> {
        Self::raw_node(&slf)
            .attribute(qualified_name)
            .or_else(|| default_value.map(str::to_owned))
    }

    /// Return the named attribute node, if present.
    ///
    /// :param qualified_name: Qualified attribute name.
    pub fn attribute_node<'py>(slf: PyRef<'py, Self>, qualified_name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        if let Some(a) = Self::raw_node(&slf).attribute_node(qualified_name) {
            Ok(Some(create_upcast_node(slf.py(), a.into_node())?))
        } else {
            Ok(None)
        }
    }

    /// The names of this element’s attributes.
    #[getter]
    pub fn attribute_names(slf: PyRef<'_, Self>) -> PyResult<Bound<'_, PyTuple>> {
        PyTuple::new(slf.py(), Self::raw_node(&slf).attribute_names().into_iter())
    }

    /// The attributes of this element as a named map.
    #[getter]
    pub fn attributes(slf: PyRef<'_, Self>) -> PyResult<Bound<'_, NamedNodeMapPy>> {
        NamedNodeMapPy::new_bound(slf.py(), Self::raw_node(&slf).attributes())
    }

    /// Check whether this element has an attribute with the given name.
    ///
    /// :param qualified_name: Qualified attribute name.
    pub fn has_attribute(slf: PyRef<'_, Self>, qualified_name: &str) -> bool {
        Self::raw_node(&slf).has_attribute(qualified_name)
    }

    /// Set an attribute value.
    ///
    /// :param qualified_name: Qualified attribute name.
    /// :param value: Attribute value to match.
    pub fn set_attribute(slf: PyRef<'_, Self>, qualified_name: &str, value: &str) {
        Self::raw_node(&slf).set_attribute(qualified_name, value)
    }

    /// Remove an attribute.
    ///
    /// :param qualified_name: Qualified attribute name.
    pub fn remove_attribute(slf: PyRef<'_, Self>, qualified_name: &str) {
        Self::raw_node(&slf).remove_attribute(qualified_name)
    }

    /// Add or remove an attribute and return whether it is present.
    ///
    /// :param qualified_name: Qualified attribute name.
    /// :param force: Desired token presence, or ``None`` to toggle it.
    #[pyo3(signature = (qualified_name, force=None))]
    pub fn toggle_attribute(slf: PyRef<'_, Self>, qualified_name: &str, force: Option<bool>) -> bool {
        Self::raw_node(&slf).toggle_attribute(qualified_name, force)
    }

    /// List of attribute names if this node is an element.
    #[getter]
    pub fn attrs(slf: PyRef<'_, Self>) -> PyResult<Bound<'_, PyTuple>> {
        Self::attribute_names(slf)
    }

    /// Check whether this element has an attribute with the given name.
    ///
    /// :param qualified_name: Qualified attribute name.
    pub fn hasattr(slf: PyRef<'_, Self>, qualified_name: &str) -> bool {
        Self::has_attribute(slf, qualified_name)
    }

    /// Return whether the requested node, token, or value is present.
    ///
    /// :param qualified_name: Qualified attribute name.
    pub fn __contains__(slf: PyRef<'_, Self>, qualified_name: &str) -> bool {
        Self::has_attribute(slf, qualified_name)
    }

    /// Get the value of ``attr_name`` or ``default_value`` if the element has no such attribute.
    ///
    /// :param qualified_name: Qualified attribute name.
    /// :param default_value: Value returned when the attribute is absent.
    #[pyo3(signature = (qualified_name, default_value=None))]
    pub fn getattr(slf: PyRef<'_, Self>, qualified_name: &str, default_value: Option<&str>) -> Option<String> {
        Self::get_attribute(slf, qualified_name, default_value)
    }

    /// Get the value of an attribute.
    ///
    /// :param qualified_name: Qualified attribute name.
    pub fn __getitem__(slf: PyRef<'_, Self>, qualified_name: &str) -> PyResult<String> {
        Self::get_attribute(slf, qualified_name, None)
            .ok_or_else(|| PyKeyError::new_err(format!("No such attribute: {qualified_name}")))
    }

    /// Insert or update an attribute with the given name and value.
    ///
    /// :param qualified_name: Qualified attribute name.
    /// :param value: Attribute value to match.
    pub fn setattr(slf: PyRef<'_, Self>, qualified_name: &str, value: &str) {
        Self::set_attribute(slf, qualified_name, value)
    }

    /// Insert or update an attribute with the given name and value.
    ///
    /// :param qualified_name: Qualified attribute name.
    /// :param value: Attribute value to match.
    pub fn __setitem__(slf: PyRef<'_, Self>, qualified_name: &str, value: &str) {
        Self::set_attribute(slf, qualified_name, value)
    }

    /// Remove the given attribute if it exists.
    ///
    /// :param qualified_name: Qualified attribute name.
    pub fn delattr(slf: PyRef<'_, Self>, qualified_name: &str) {
        Self::remove_attribute(slf, qualified_name)
    }

    /// Remove the given attribute.
    ///
    /// :param qualified_name: Qualified attribute name.
    pub fn __delitem__(slf: PyRef<'_, Self>, qualified_name: &str) -> PyResult<()> {
        if Self::raw_node(&slf).has_attribute(qualified_name) {
            Ok(Self::remove_attribute(slf, qualified_name))
        } else {
            Err(PyKeyError::new_err(format!("No such attribute: {qualified_name}")))
        }
    }

    /// Find the nearest matching element, including this element.
    ///
    /// :param selectors: CSS selector expression.
    pub fn closest<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        Self::raw_node(&slf).closest(selectors).map_or_else(
            |e| Err(CSSParserExceptionPy::new_err(e.to_string())),
            |n| n.map_or(Ok(None), |n_| Ok(Some(create_upcast_node(slf.py(), n_.into_node())?))),
        )
    }

    /// Return whether this element matches the selector.
    ///
    /// .. versionchanged:: 1.2.0
    ///    Follow DOM ``Element.matches()`` semantics by testing the element itself.
    ///
    /// .. warning::
    ///    Before 1.2.0, this method searched descendants. To preserve that behavior,
    ///    use ``node.query_selector(selectors) is not None``.
    ///
    /// :param selectors: CSS selector expression.
    /// :return: Whether this element matches the selector.
    pub fn matches<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<bool> {
        Self::raw_node(&slf)
            .matches(selectors)
            .map_or_else(|e| Err(CSSParserExceptionPy::new_err(e.to_string())), |n| Ok(n))
    }

    /// Find all descendant elements with the given tag name.
    ///
    /// :param name: Attribute name to match.
    pub fn get_elements_by_tag_name<'py>(slf: PyRef<'py, Self>, name: &str) -> PyResult<Bound<'py, ElementNodeListPy>> {
        get_elements_by_x!(slf, get_elements_by_tag_name, name)
    }

    /// Find all descendant elements matching the given class name.
    ///
    /// :param class_names: Whitespace-separated class names to match.
    /// :param case_insensitive: Legacy option; class-name matching remains case-sensitive.
    #[pyo3(signature = (class_names, case_insensitive=false))]
    pub fn get_elements_by_class_name<'py>(
        slf: PyRef<'py, Self>,
        class_names: &str,
        case_insensitive: bool,
    ) -> PyResult<Bound<'py, ElementNodeListPy>> {
        let _ = case_insensitive;
        get_elements_by_x!(slf, get_elements_by_class_name, class_names)
    }

    // noinspection DuplicatedCode
    /// Find all descendant elements matching the given attribute name and value.
    ///
    /// :param name: Attribute name to match.
    /// :param value: Attribute value to match.
    /// :param case_insensitive: Whether attribute value matching ignores case.
    #[pyo3(signature = (name, value, case_insensitive=false))]
    pub fn get_elements_by_attr<'py>(
        slf: PyRef<'py, Self>,
        name: &str,
        value: &str,
        case_insensitive: bool,
    ) -> PyResult<Bound<'py, ElementNodeListPy>> {
        get_elements_by_x!(slf, get_elements_by_attr_case, name, value, case_insensitive)
    }

    // noinspection DuplicatedCode
    /// Find the descendant element whose ID attribute matches ``element_id``.
    ///
    /// :param element_id: ID value to search for.
    /// :param case_insensitive: Whether ID matching ignores case.
    #[pyo3(signature = (element_id, case_insensitive=false))]
    pub fn get_element_by_id<'py>(
        slf: PyRef<'py, Self>,
        element_id: &str,
        case_insensitive: bool,
    ) -> PyResult<Option<Bound<'py, ElementNodePy>>> {
        Self::raw_node(&slf)
            .get_elements_by_attr_case("id", element_id, case_insensitive)
            .into_iter()
            .next()
            .map_or(Ok(None), |e| Ok(Some(ElementNodePy::new_bound(slf.py(), e)?)))
    }

    /// The serialized HTML inside this element.
    #[getter]
    pub fn inner_html(slf: PyRef<'_, Self>) -> String {
        Self::raw_node(&slf).inner_html()
    }

    /// Replace this element’s contents from an HTML string.
    ///
    /// :param inner_html: HTML string used to replace the element contents.
    #[setter]
    pub fn set_inner_html<'py>(slf: PyRef<'py, Self>, inner_html: &str) {
        Self::raw_node(&slf).set_inner_html(inner_html)
    }

    /// The serialized HTML for this element and its contents.
    #[getter]
    pub fn outer_html(slf: PyRef<'_, Self>) -> String {
        Self::raw_node(&slf).outer_html()
    }

    /// Replace this element with parsed HTML.
    ///
    /// :param outer_html: HTML string used to replace this element.
    #[setter]
    pub fn set_outer_html<'py>(slf: PyRef<'py, Self>, outer_html: &str) {
        Self::raw_node(&slf).set_outer_html(outer_html)
    }

    /// The text inside this element.
    #[getter]
    pub fn inner_text(slf: PyRef<'_, Self>) -> String {
        Self::raw_node(&slf).inner_text()
    }

    /// Replace this element’s contents with text.
    ///
    /// :param inner_text: Text used to replace the element contents.
    #[setter]
    pub fn set_inner_text<'py>(slf: PyRef<'py, Self>, inner_text: &str) {
        Self::raw_node(&slf).set_inner_text(inner_text)
    }

    /// The text represented by this element and its descendants.
    #[getter]
    pub fn outer_text(slf: PyRef<'_, Self>) -> String {
        Self::raw_node(&slf).outer_text()
    }

    /// Replace this element with a text node.
    ///
    /// :param outer_text: Text used to replace this element.
    #[setter]
    pub fn set_outer_text<'py>(slf: PyRef<'py, Self>, outer_text: &str) {
        Self::raw_node(&slf).set_outer_text(outer_text)
    }

    /// Insert nodes immediately before this node.
    ///
    /// :param node: Nodes to insert in place of this node.
    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn before(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::before_(slf, node)
    }
    /// Insert nodes immediately after this node.
    ///
    /// :param node: Nodes to insert in place of this node.
    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn after(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::after_(slf, node)
    }

    /// Replace this node with the given nodes.
    ///
    /// :param node: Nodes to insert in place of this node.
    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn replace_with(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::replace_with_(slf, node)
    }

    /// Remove this node or the specified tokens.
    #[inline(always)]
    pub fn remove(slf: PyRef<'_, Self>) {
        Self::remove_(slf);
    }

    /// The next sibling element, if one exists.
    #[getter]
    #[inline(always)]
    pub fn next_element_sibling(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNodePy>>> {
        Self::next_element_sibling_(slf)
    }

    /// The previous sibling element, if one exists.
    #[getter]
    #[inline(always)]
    pub fn previous_element_sibling(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNodePy>>> {
        Self::previous_element_sibling_(slf)
    }

    /// The child elements of this node.
    #[getter]
    #[inline(always)]
    pub fn children(slf: PyRef<'_, Self>) -> PyResult<Bound<'_, ElementNodeListPy>> {
        Self::children_(slf)
    }

    /// First element child of this DOM node.
    #[getter]
    #[inline(always)]
    pub fn first_element_child(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNodePy>>> {
        Self::first_element_child_(slf)
    }

    /// Last element child of this DOM node.
    #[getter]
    #[inline(always)]
    pub fn last_element_child(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNodePy>>> {
        Self::last_element_child_(slf)
    }

    /// Number of child elements.
    #[getter]
    #[inline(always)]
    fn child_element_count(slf: PyRef<'_, Self>) -> usize {
        Self::child_element_count_(slf)
    }

    /// Insert nodes before the existing children.
    ///
    /// :param node: Nodes to insert in place of this node.
    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn prepend(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::prepend_(slf, node)
    }

    /// Append nodes after the existing children.
    ///
    /// :param node: Nodes to insert in place of this node.
    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn append(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::append_(slf, node)
    }

    /// Replace all children with the given nodes.
    ///
    /// :param node: Nodes to insert in place of this node.
    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn replace_children(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::replace_children_(slf, node)
    }

    /// Find and return the first element matching the CSS selector. This is more efficient than calling :meth:`query_selector_all` and discarding additional results.
    ///
    /// :param selectors: CSS selector expression.
    /// :return: The first matching element, or ``None`` if no element matches.
    #[inline(always)]
    pub fn query_selector<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Option<Bound<'py, ElementNodePy>>> {
        Self::query_selector_(slf, selectors)
    }

    /// Find all elements matching the CSS selector and return them as an :class:`ElementNodeList`.
    ///
    /// :param selectors: CSS selector expression.
    /// :return: All matching elements.
    #[inline(always)]
    pub fn query_selector_all<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Bound<'py, ElementNodeListPy>> {
        Self::query_selector_all_(slf, selectors)
    }
}

impl _ChildNodeMixin<node::ElementNode> for ElementNodePy {}
impl _NonDocumentTypeChildNodeMixin<node::ElementNode> for ElementNodePy {}
impl _ParentNodeMixin<node::ElementNode> for ElementNodePy {}

// noinspection DuplicatedCode
#[pymethods]
impl AttrNodePy {
    /// The local name of this node.
    #[getter]
    pub fn local_name(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).local_name()
    }

    /// The name of this node.
    #[getter]
    pub fn name(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).name()
    }

    /// Node text value.
    #[getter]
    pub fn value(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).value()
    }

    /// Set the value associated with this node or token list.
    ///
    /// :param value: Attribute value to match.
    #[setter]
    pub fn set_value<'py>(slf: PyRef<'py, Self>, value: &str) {
        Self::raw_node(&slf).set_value(value)
    }

    /// The element that owns this attribute, if present.
    #[getter]
    pub fn owner_element(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNodePy>>> {
        Self::raw_node(&slf)
            .owner_element()
            .map_or(Ok(None), |e| Ok(Some(ElementNodePy::new_bound(slf.py(), e)?)))
    }
}

impl _ChildNodeMixin<node::DocumentTypeNode> for DocumentTypeNodePy {}

// noinspection DuplicatedCode
#[pymethods]
impl DocumentTypeNodePy {
    /// The name of this node.
    #[getter]
    pub fn name(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).name()
    }

    /// The public identifier of this document type.
    #[getter]
    pub fn public_id(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).public_id()
    }

    /// The system identifier of this document type.
    #[getter]
    pub fn system_id(slf: PyRef<'_, Self>) -> Option<String> {
        Self::raw_node(&slf).system_id()
    }

    /// Insert nodes immediately before this node.
    ///
    /// :param node: Nodes to insert in place of this node.
    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn before(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::before_(slf, node)
    }

    /// Insert nodes immediately after this node.
    ///
    /// :param node: Nodes to insert in place of this node.
    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn after(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::after_(slf, node)
    }

    /// Replace this node with the given nodes.
    ///
    /// :param node: Nodes to insert in place of this node.
    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn replace_with(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::replace_with_(slf, node)
    }

    /// Remove this node or the specified tokens.
    #[inline(always)]
    pub fn remove(slf: PyRef<'_, Self>) {
        Self::remove_(slf);
    }
}

macro_rules! doc_create_x {
    ($slf: ident, $func_name: ident $(, $args: ident)*) => {
        Self::raw_node(&$slf).$func_name($($args),*).map_or_else(
            |e| Err(DOMExceptionPy::new_err(e.to_string())),
            |e| Ok(Some(create_upcast_node($slf.py(), e.into_node())?))
        )
    };
}

// noinspection DuplicatedCode
#[pymethods]
impl DocumentNodePy {
    /// The document type node, if present.
    #[getter]
    pub fn doctype(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, PyAny>>> {
        Self::raw_node(&slf).doctype().map_or_else(
            || Err(DOMExceptionPy::new_err("No Document node")),
            |d| Ok(Some(create_upcast_node(slf.py(), d.into_node())?)),
        )
    }

    /// The root element of this document, if present.
    #[getter]
    pub fn document_element(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNodePy>>> {
        Self::raw_node(&slf).document_element().map_or_else(
            || Err(DOMExceptionPy::new_err("No Document node")),
            |element| Ok(Some(ElementNodePy::new_bound(slf.py(), element)?)),
        )
    }

    /// Find all descendant elements with the given tag name.
    ///
    /// :param name: Attribute name to match.
    pub fn get_elements_by_tag_name<'py>(slf: PyRef<'py, Self>, name: &str) -> PyResult<Bound<'py, ElementNodeListPy>> {
        get_elements_by_x!(slf, get_elements_by_tag_name, name)
    }

    // noinspection DuplicatedCode
    /// Find all descendant elements matching the given class name.
    ///
    /// :param class_names: Whitespace-separated class names to match.
    /// :param case_insensitive: Legacy option; class-name matching remains case-sensitive.
    #[pyo3(signature = (class_names, case_insensitive=false))]
    pub fn get_elements_by_class_name<'py>(
        slf: PyRef<'py, Self>,
        class_names: &str,
        case_insensitive: bool,
    ) -> PyResult<Bound<'py, ElementNodeListPy>> {
        let _ = case_insensitive;
        get_elements_by_x!(slf, get_elements_by_class_name, class_names)
    }

    // noinspection DuplicatedCode
    /// Find all descendant elements matching the given attribute name and value.
    ///
    /// :param name: Attribute name to match.
    /// :param value: Attribute value to match.
    /// :param case_insensitive: Whether attribute value matching ignores case.
    #[pyo3(signature = (name, value, case_insensitive=false))]
    pub fn get_elements_by_attr<'py>(
        slf: PyRef<'py, Self>,
        name: &str,
        value: &str,
        case_insensitive: bool,
    ) -> PyResult<Bound<'py, ElementNodeListPy>> {
        get_elements_by_x!(slf, get_elements_by_attr_case, name, value, case_insensitive)
    }

    /// Create a new DOM element node.
    ///
    /// :param local_name: Data for the processing instruction.
    pub fn create_element<'py>(slf: PyRef<'py, Self>, local_name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        doc_create_x!(slf, create_element, local_name)
    }

    /// Create an empty document fragment.
    pub fn create_document_fragment(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, PyAny>>> {
        doc_create_x!(slf, create_document_fragment)
    }

    /// Create a new text node with the given string contents.
    ///
    /// :param data: String contents of the new text node.
    pub fn create_text_node<'py>(slf: PyRef<'py, Self>, data: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        doc_create_x!(slf, create_text_node, data)
    }

    /// Create a CDATA section containing the given data.
    ///
    /// :param data: Character data to store in the node.
    pub fn create_cdata_section<'py>(slf: PyRef<'py, Self>, data: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        doc_create_x!(slf, create_cdata_section, data)
    }

    /// Create a comment containing the given data.
    ///
    /// :param data: Character data to store in the node.
    pub fn create_comment<'py>(slf: PyRef<'py, Self>, data: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        doc_create_x!(slf, create_comment, data)
    }

    /// Create a processing instruction with the given target and data.
    ///
    /// :param target: The target of this processing instruction.
    /// :param local_name: Data for the processing instruction.
    pub fn create_processing_instruction<'py>(
        slf: PyRef<'py, Self>,
        target: &str,
        local_name: &str,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        doc_create_x!(slf, create_processing_instruction, target, local_name)
    }

    /// Create an attribute with the given local name.
    ///
    /// :param local_name: Local name for the new attribute.
    pub fn create_attribute<'py>(slf: PyRef<'py, Self>, local_name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        doc_create_x!(slf, create_attribute, local_name)
    }

    /// The child elements of this node.
    #[getter]
    #[inline(always)]
    pub fn children(slf: PyRef<'_, Self>) -> PyResult<Bound<'_, ElementNodeListPy>> {
        Self::children_(slf)
    }

    /// First element child of this DOM node.
    #[getter]
    #[inline(always)]
    pub fn first_element_child(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNodePy>>> {
        Self::first_element_child_(slf)
    }

    /// Last element child of this DOM node.
    #[getter]
    #[inline(always)]
    pub fn last_element_child(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNodePy>>> {
        Self::last_element_child_(slf)
    }

    /// Return the number of child elements.
    #[getter]
    #[inline(always)]
    pub fn child_element_count(slf: PyRef<'_, Self>) -> usize {
        Self::child_element_count_(slf)
    }

    /// Insert nodes before the existing children.
    ///
    /// :param node: Node or nodes to insert, replace, or remove.
    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn prepend(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::prepend_(slf, node)
    }

    /// Append nodes after the existing children.
    ///
    /// :param node: Node or nodes to insert, replace, or remove.
    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn append(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::append_(slf, node)
    }

    /// Replace all children with the given nodes.
    ///
    /// :param node: Node or nodes to insert, replace, or remove.
    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn replace_children(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::replace_children_(slf, node)
    }

    /// Find and return the first element matching the CSS selector. This is more efficient than calling :meth:`query_selector_all` and discarding additional results.
    ///
    /// :param selectors: CSS selector expression.
    #[inline(always)]
    pub fn query_selector<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Option<Bound<'py, ElementNodePy>>> {
        Self::query_selector_(slf, selectors)
    }

    /// Find all elements matching the CSS selector and return them as an :class:`ElementNodeList`.
    ///
    /// :param selectors: CSS selector expression.
    #[inline(always)]
    pub fn query_selector_all<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Bound<'py, ElementNodeListPy>> {
        Self::query_selector_all_(slf, selectors)
    }

    /// Find the descendant element whose ID attribute matches ``element_id``.
    ///
    /// :param element_id: ID value to search for.
    /// :param case_insensitive: Whether ID matching ignores case.
    #[inline(always)]
    #[pyo3(signature = (element_id, case_insensitive=false))]
    pub fn get_element_by_id<'py>(
        slf: PyRef<'py, Self>,
        element_id: &str,
        case_insensitive: bool,
    ) -> PyResult<Option<Bound<'py, ElementNodePy>>> {
        Self::get_element_by_id_(slf, element_id, case_insensitive)
    }
}

impl _ParentNodeMixin<node::DocumentNode> for DocumentNodePy {}
impl _NonElementParentNodeMixin<node::DocumentNode> for DocumentNodePy {}

// noinspection DuplicatedCode
#[pymethods]
impl DocumentFragmentNodePy {
    /// The child elements of this node.
    #[getter]
    #[inline(always)]
    pub fn children(slf: PyRef<'_, Self>) -> PyResult<Bound<'_, ElementNodeListPy>> {
        Self::children_(slf)
    }

    /// First element child of this DOM node.
    #[getter]
    #[inline(always)]
    pub fn first_element_child(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNodePy>>> {
        Self::first_element_child_(slf)
    }

    /// Last element child of this DOM node.
    #[getter]
    #[inline(always)]
    pub fn last_element_child(slf: PyRef<'_, Self>) -> PyResult<Option<Bound<'_, ElementNodePy>>> {
        Self::last_element_child_(slf)
    }

    /// The number of child elements.
    #[getter]
    #[inline(always)]
    pub fn child_element_count(slf: PyRef<'_, Self>) -> usize {
        Self::child_element_count_(slf)
    }

    /// Insert nodes before the existing children.
    ///
    /// :param node: Node or nodes to insert, replace, or remove.
    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn prepend(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::prepend_(slf, node)
    }

    /// Append nodes after the existing children.
    ///
    /// :param node: Node or nodes to insert, replace, or remove.
    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn append(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::append_(slf, node)
    }

    /// Replace all children with the given nodes.
    ///
    /// :param node: Node or nodes to insert, replace, or remove.
    #[pyo3(signature = (*node))]
    #[inline(always)]
    pub fn replace_children(slf: PyRef<'_, Self>, node: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::replace_children_(slf, node)
    }

    /// Find and return the first element matching the CSS selector. This is more efficient than calling :meth:`query_selector_all` and discarding additional results.
    ///
    /// :param selectors: CSS selector expression.
    #[inline(always)]
    pub fn query_selector<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Option<Bound<'py, ElementNodePy>>> {
        Self::query_selector_(slf, selectors)
    }

    /// Find all elements matching the CSS selector and return them as an :class:`ElementNodeList`.
    ///
    /// :param selectors: CSS selector expression.
    #[inline(always)]
    pub fn query_selector_all<'py>(slf: PyRef<'py, Self>, selectors: &str) -> PyResult<Bound<'py, ElementNodeListPy>> {
        Self::query_selector_all_(slf, selectors)
    }

    /// Find the descendant element whose ID attribute matches ``element_id``.
    ///
    /// :param element_id: ID value to search for.
    /// :param case_insensitive: Whether ID matching ignores case.
    #[inline(always)]
    #[pyo3(signature = (element_id, case_insensitive=false))]
    pub fn get_element_by_id<'py>(
        slf: PyRef<'py, Self>,
        element_id: &str,
        case_insensitive: bool,
    ) -> PyResult<Option<Bound<'py, ElementNodePy>>> {
        Self::get_element_by_id_(slf, element_id, case_insensitive)
    }
}

impl _ParentNodeMixin<node::DocumentFragmentNode> for DocumentFragmentNodePy {}
impl _NonElementParentNodeMixin<node::DocumentFragmentNode> for DocumentFragmentNodePy {}

character_data_node!(TextNodePy, node::TextNode);
character_data_node!(CDATASectionNodePy, node::CdataSectionNode);
character_data_node!(ProcessingInstructionNodePy, node::ProcessingInstructionNode, target, Option<String>);
character_data_node!(CommentNodePy, node::CommentNode);

/// An iterator over a node’s descendants.
#[pyclass(name = "NodeIter", unsendable, module = "resiliparse.parse.html.node")]
pub struct NodeIterPy {
    iter: iter::NodeIteratorOwned,
}

#[pymethods]
impl NodeIterPy {
    /// Return this iterator.
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Return the next node in the traversal, or ``None`` when complete.
    pub fn __next__(mut slf: PyRefMut<'_, Self>) -> PyResult<Option<Bound<'_, PyAny>>> {
        slf.iter
            .next()
            .map(|node| {
                let node = node.map_err(|error| DOMExceptionPy::new_err(error.to_string()))?;
                create_upcast_node(slf.py(), node)
            })
            .transpose()
    }
}

/// Mutable state shared with DOM traversal callbacks.
/// State shared with DOM traversal callbacks.
#[pyclass(name = "DOMContext", unsendable, dict, module = "resiliparse.parse.html")]
pub struct DOMContextPy {
    /// The node currently being visited, or ``None`` outside a callback.
    #[pyo3(get, set)]
    node: Option<Py<PyAny>>,

    /// The depth of the current node relative to the traversal root.
    #[pyo3(get, set)]
    depth: usize,
}

#[pymethods]
impl DOMContextPy {
    /// Create a traversal context with no current node and depth zero.
    #[new]
    fn new() -> Self {
        Self { node: None, depth: 0 }
    }
}

/// Traverse the subtree rooted at ``base_node`` in pre-order and call ``start_callback`` for each visited node.
///
/// If ``end_callback`` is provided, it is called when traversal leaves a node. Callbacks receive the shared :class:`DOMContext`, which tracks the current node and depth and can hold custom state.
///
/// :param base_node: Root node whose subtree is traversed.
/// :param start_callback: Callable invoked when a node is entered.
/// :param end_callback: Optional callable invoked when a node is exited.
/// :param context: Optional mutable state shared with callbacks.
/// :param elements_only: whether to invoke callbacks only for elements
/// :return: ``None``
#[pyfunction]
#[pyo3(signature = (base_node, start_callback, end_callback=None, context=None, elements_only=false))]
pub fn traverse_dom(
    py: Python<'_>,
    base_node: PyRef<'_, NodePy>,
    start_callback: &Bound<'_, PyAny>,
    end_callback: Option<&Bound<'_, PyAny>>,
    context: Option<Py<DOMContextPy>>,
    elements_only: bool,
) -> PyResult<()> {
    if elements_only && base_node.node.node_type() != Some(node::NodeType::Element) {
        return Ok(());
    }

    let context = match context {
        Some(context) => context,
        None => Py::new(py, DOMContextPy::new())?,
    };
    let root = base_node.node.clone();
    let mut iter = root.clone().into_iter();

    let invoke = |node: &node::Node, depth: usize, callback: &Bound<'_, PyAny>| -> PyResult<()> {
        let wrapped = create_upcast_node(py, node.clone())?.unbind();
        {
            let mut state = context.bind(py).borrow_mut();
            state.node = Some(wrapped);
            state.depth = depth;
        }
        callback.call1((context.bind(py),))?;
        Ok(())
    };

    let mut open_nodes: Vec<(node::Node, usize)> = Vec::new();
    while let Some(node) = iter.next() {
        let node = node.map_err(|error| DOMExceptionPy::new_err(error.to_string()))?;
        let parent = node.parent_node();
        while open_nodes
            .last()
            .is_some_and(|(open_node, _)| Some(open_node) != parent.as_ref())
        {
            let (closing_node, closing_depth) = open_nodes.pop().unwrap();
            if let Some(callback) = end_callback {
                if closing_node != root && (!elements_only || closing_node.node_type() == Some(node::NodeType::Element))
                {
                    invoke(&closing_node, closing_depth, callback)?;
                }
            }
        }

        let depth = open_nodes.len();
        if !elements_only || node.node_type() == Some(node::NodeType::Element) {
            invoke(&node, depth, start_callback)?;
        }
        if node.has_child_nodes() {
            open_nodes.push((node, depth));
        }
    }

    while open_nodes.len() > 1 {
        let (closing_node, closing_depth) = open_nodes.pop().unwrap();
        if let Some(callback) = end_callback {
            if !elements_only || closing_node.node_type() == Some(node::NodeType::Element) {
                invoke(&closing_node, closing_depth, callback)?;
            }
        }
    }
    Ok(())
}
