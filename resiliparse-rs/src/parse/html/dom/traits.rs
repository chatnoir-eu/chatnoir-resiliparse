// Copyright 2023 Janek Bevendorff
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

//! Node type traits (abstract).
//!
//! Abstract DOM node type interfaces.

use crate::parse::html::dom::coll::*;
use crate::parse::html::dom::iter::*;
use crate::parse::html::dom::*;
use crate::parse::html::lexbor::{str_from_lxb_char_t, str_from_lxb_str_cb, str_from_lxb_str_t};
use crate::parse::html::selector::{CssParserError, CssSelector};
use std::fmt::{Debug, Display};
use std::ops::Range;
use std::ptr::{self, addr_of};

/// The type of DOM node.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum NodeType {
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
    Notation = 0x0C, // legacy
}

/// Shared implementation for before, after, and replace_with. Caller holds
/// the document lock so the anchor remains valid throughout the operation.
unsafe fn splice_siblings_unchecked(
    receiver: NodeRef,
    nodes: &[NodeRef],
    mut anchor: Option<Node>,
    removed: &[*mut lxb_dom_node_t],
) {
    if let Some(parent) = receiver.parent_node() {
        // Keep an anchor that will not itself be moved by this operation.
        while let Some(node) = &anchor {
            if !nodes.iter().any(|input| *input == node.as_noderef()) {
                break;
            }
            anchor = node.next_sibling();
        }
        unsafe {
            parent.splice_children_unchecked(
                nodes,
                anchor.as_ref().map_or(ptr::null_mut(), |node| node.node_ptr_()),
                removed,
            );
        }
    }
}

pub(crate) trait NodeInterfaceBaseImpl {
    fn tree_(&self) -> Arc<HtmlDocument>;
    fn node_ptr_(&self) -> *mut lxb_dom_node_t;
    fn reset_node_ptr_(&self);

    #[inline(always)]
    unsafe fn doc_ptr_unchecked(&self) -> *mut lxb_dom_document_t {
        unsafe { (*self.node_ptr_()).owner_document }
    }

    unsafe fn node_name_unchecked(&self) -> Option<&str> {
        unsafe { str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_node_name) }
    }

    /// Node text value.
    unsafe fn node_value_unchecked(&self) -> Option<&str> {
        unsafe {
            use crate::third_party::lexbor::lxb_dom_node_type_t::*;
            match (*self.node_ptr_()).type_ {
                LXB_DOM_NODE_TYPE_ATTRIBUTE => str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_attr_value_noi),
                LXB_DOM_NODE_TYPE_TEXT
                | LXB_DOM_NODE_TYPE_CDATA_SECTION
                | LXB_DOM_NODE_TYPE_COMMENT
                | LXB_DOM_NODE_TYPE_PROCESSING_INSTRUCTION => {
                    str_from_lxb_str_t(addr_of!((*(self.node_ptr_() as *const lxb_dom_character_data_t)).data))
                }
                _ => None,
            }
        }
    }

    unsafe fn can_have_children_unchecked(&self) -> bool {
        unsafe {
            matches!(
                (*self.node_ptr_()).type_,
                lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_ELEMENT
                    | lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_DOCUMENT
                    | lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_DOCUMENT_FRAGMENT
            )
        }
    }

    /// Validate the whole operation before changing links. The document lock
    /// covers both validation and mutation; reference/removal pointers are valid.
    unsafe fn splice_children_unchecked(
        &self,
        nodes: &[NodeRef],
        reference: *mut lxb_dom_node_t,
        removed: &[*mut lxb_dom_node_t],
    ) -> bool {
        use lxb_dom_node_type_t::*;
        let document = self.tree_();
        let _guard = document.doc_ptr();
        unsafe {
            let parent = self.node_ptr_();
            if parent.is_null()
                || !self.can_have_children_unchecked()
                || (!reference.is_null() && (*reference).parent != parent)
            {
                return false;
            }
            let mut inserted = Vec::new();
            for node in nodes {
                if !Arc::ptr_eq(&document, &node.tree_()) || node.node_ptr_().is_null() {
                    return false;
                }
                let node = node.node_ptr_();
                let mut ancestor = parent;
                while !ancestor.is_null() {
                    if ancestor == node {
                        return false;
                    }
                    ancestor = (*ancestor).parent;
                }
                if (*node).type_ == LXB_DOM_NODE_TYPE_DOCUMENT_FRAGMENT {
                    let mut child = (*node).first_child;
                    while !child.is_null() {
                        // Earlier arguments may already have moved this child.
                        if !inserted.contains(&child) {
                            inserted.push(child);
                        }
                        child = (*child).next;
                    }
                } else {
                    inserted.push(node);
                }
            }
            let is_document = (*parent).type_ == LXB_DOM_NODE_TYPE_DOCUMENT;
            for &node in &inserted {
                match (*node).type_ {
                    LXB_DOM_NODE_TYPE_ELEMENT
                    | LXB_DOM_NODE_TYPE_TEXT
                    | LXB_DOM_NODE_TYPE_CDATA_SECTION
                    | LXB_DOM_NODE_TYPE_COMMENT
                    | LXB_DOM_NODE_TYPE_PROCESSING_INSTRUCTION => {}
                    LXB_DOM_NODE_TYPE_DOCUMENT_TYPE if is_document => {}
                    _ => return false,
                }
            }
            if is_document {
                let (mut element, mut doctype) = (false, false);
                let mut valid_child = |node: *mut lxb_dom_node_t| match (*node).type_ {
                    LXB_DOM_NODE_TYPE_ELEMENT if !element => {
                        element = true;
                        true
                    }
                    LXB_DOM_NODE_TYPE_DOCUMENT_TYPE if !doctype && !element => {
                        doctype = true;
                        true
                    }
                    LXB_DOM_NODE_TYPE_COMMENT | LXB_DOM_NODE_TYPE_PROCESSING_INSTRUCTION => true,
                    _ => false,
                };
                let mut child = (*parent).first_child;
                loop {
                    if child == reference {
                        for (i, &node) in inserted.iter().enumerate() {
                            if node != reference && !inserted[i + 1..].contains(&node) && !valid_child(node) {
                                return false;
                            }
                        }
                    }
                    if child.is_null() {
                        break;
                    }
                    let moved = inserted.contains(&child);
                    if (!moved || child == reference) && (!removed.contains(&child) || moved) && !valid_child(child) {
                        return false;
                    }
                    child = (*child).next;
                }
            }
            for &node in &inserted {
                if node != reference {
                    document.mark_mutated();
                    move_node_before_unchecked(parent, node, reference);
                }
            }
            for &node in removed {
                if !inserted.contains(&node) {
                    document.mark_mutated();
                    lxb_dom_node_remove(node);
                }
            }
            true
        }
    }

    unsafe fn insert_before_unchecked<'a>(&self, node: &NodeRef<'a>, child: Option<NodeRef>) -> Option<NodeRef<'a>> {
        let reference = child.as_ref().map_or(ptr::null_mut(), |child| child.node_ptr_());
        unsafe { self.splice_children_unchecked(&[node.clone()], reference, &[]) }.then(|| node.clone())
    }

    unsafe fn append_child_unchecked<'a>(&self, node: &NodeRef<'a>) -> Option<NodeRef<'a>> {
        unsafe { self.insert_before_unchecked(node, None) }
    }

    unsafe fn replace_child_unchecked<'a>(
        &self,
        new_child: &NodeRef<'a>,
        old_child: &NodeRef<'a>,
    ) -> Option<NodeRef<'a>> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        unsafe {
            let old = old_child.node_ptr_();
            if old_child.parent_node()?.node_ptr_() != self.node_ptr_() {
                return None;
            }
            self.splice_children_unchecked(&[new_child.clone()], old, &[old])
                .then(|| old_child.clone())
        }
    }

    unsafe fn remove_child_unchecked<'a>(&self, node: &NodeRef<'a>) -> Option<NodeRef<'a>> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        unsafe {
            if node.parent_node()?.node_ptr_() != self.node_ptr_() || !self.can_have_children_unchecked() {
                return None;
            }
            document.mark_mutated();
            lxb_dom_node_remove(node.node_ptr_());
            Some(node.clone())
        }
    }
}

/// Shared DOM node interface. Clones are aliases, not deep copies. Mutating
/// methods take `&self` and serialize native access through the document owner;
/// an exclusive Rust borrow of a wrapper does not imply exclusive node access.
#[allow(private_bounds)]
pub trait NodeInterface: NodeInterfaceBaseImpl + Debug + Display {
    /// Return this node’s DOM node type, if the handle is valid.
    ///
    /// # Returns
    ///
    /// The DOM node type, or `None` if the handle is invalid or the type is unsupported.
    fn node_type(&self) -> Option<NodeType> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        use lxb_dom_node_type_t::*;
        match unsafe { self.node_ptr_().as_ref()? }.type_ {
            LXB_DOM_NODE_TYPE_ELEMENT => Some(NodeType::Element),
            LXB_DOM_NODE_TYPE_ATTRIBUTE => Some(NodeType::Attribute),
            LXB_DOM_NODE_TYPE_TEXT => Some(NodeType::Text),
            LXB_DOM_NODE_TYPE_CDATA_SECTION => Some(NodeType::CdataSection),
            LXB_DOM_NODE_TYPE_ENTITY_REFERENCE => Some(NodeType::EntityReference),
            LXB_DOM_NODE_TYPE_ENTITY => Some(NodeType::Entity),
            LXB_DOM_NODE_TYPE_PROCESSING_INSTRUCTION => Some(NodeType::ProcessingInstruction),
            LXB_DOM_NODE_TYPE_COMMENT => Some(NodeType::Comment),
            LXB_DOM_NODE_TYPE_DOCUMENT => Some(NodeType::Document),
            LXB_DOM_NODE_TYPE_DOCUMENT_TYPE => Some(NodeType::DocumentType),
            LXB_DOM_NODE_TYPE_DOCUMENT_FRAGMENT => Some(NodeType::DocumentFragment),
            LXB_DOM_NODE_TYPE_NOTATION => Some(NodeType::Notation),
            _ => None,
        }
    }

    /// Borrow this node as a generic node reference.
    ///
    /// # Returns
    ///
    /// A borrowed view of this node.
    fn as_noderef(&self) -> NodeRef<'_>;

    /// Borrow this wrapper mutably; aliases still share the underlying DOM node.
    ///
    /// # Arguments
    ///
    ///
    /// # Returns
    ///
    /// A mutable borrowed view of this node.
    fn as_noderef_mut(&mut self) -> NodeRefMut<'_>;

    /// Clone this handle into the generic owned node enum.
    ///
    /// # Returns
    ///
    /// An owned generic node handle that refers to the same DOM node.
    fn as_node(&self) -> Node;

    /// Convert this handle into the generic owned node enum.
    ///
    /// # Returns
    ///
    /// The owned generic node handle.
    fn into_node(self) -> Node;

    /// DOM element tag or node name.
    ///
    /// # Returns
    ///
    /// The node’s name, or `None` if no name is defined.
    fn node_name(&self) -> Option<String> {
        check_node!(self);
        unsafe { Some(self.node_name_unchecked()?.to_owned()) }
    }

    /// Node text value.
    /// Return this node’s value when its kind has one.
    ///
    /// # Returns
    ///
    /// The node’s value, or `None` for node kinds without a value.
    #[inline]
    fn node_value(&self) -> Option<String> {
        check_node!(self);
        unsafe { Some(self.node_value_unchecked()?.to_owned()) }
    }

    /// Set attribute or character data. Other node kinds are unchanged.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to assign or compare.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn set_node_value(&self, value: &str) {
        check_node!(self);
        unsafe {
            use lxb_dom_node_type_t::*;
            match (*self.node_ptr_()).type_ {
                LXB_DOM_NODE_TYPE_ATTRIBUTE => {
                    self.tree_().mark_mutated();
                    lxb_dom_attr_set_existing_value(self.node_ptr_().cast(), value.as_ptr(), value.len());
                }
                LXB_DOM_NODE_TYPE_TEXT
                | LXB_DOM_NODE_TYPE_CDATA_SECTION
                | LXB_DOM_NODE_TYPE_COMMENT
                | LXB_DOM_NODE_TYPE_PROCESSING_INSTRUCTION => {
                    self.tree_().mark_mutated();
                    lxb_dom_character_data_replace(self.node_ptr_().cast(), value.as_ptr(), value.len(), 0, 0);
                }
                _ => {}
            }
        }
    }

    /// Text contents of this DOM node and its children.
    ///
    /// # Returns
    ///
    /// The text content of this node and its descendants, or `None` if unavailable.
    fn text_content(&self) -> Option<String> {
        check_node!(self);
        let ret_value;
        unsafe {
            let mut l = 0;
            let t = lxb_dom_node_text_content(self.node_ptr_(), &mut l);
            ret_value = str_from_lxb_char_t(t, l).map(String::from);
            lxb_dom_document_destroy_text_noi(self.doc_ptr_unchecked(), t);
        }
        ret_value
    }

    /// Replace children of an element or fragment, or set attribute/character data.
    ///
    /// # Arguments
    ///
    /// * `content` - The replacement text content.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn set_text_content(&self, content: &str) {
        check_node!(self);
        unsafe {
            use lxb_dom_node_type_t::*;
            if matches!((*self.node_ptr_()).type_, LXB_DOM_NODE_TYPE_ELEMENT | LXB_DOM_NODE_TYPE_DOCUMENT_FRAGMENT) {
                self.tree_().mark_mutated();
                lxb_dom_node_text_content_set(self.node_ptr_(), content.as_ptr(), content.len());
            } else {
                self.set_node_value(content);
            }
        }
    }

    /// Return the document that owns this node.
    ///
    /// # Returns
    ///
    /// The owning document, or `None` if it is unavailable.
    fn owner_document(&self) -> Option<DocumentNode> {
        check_node!(self);
        let d = unsafe { self.doc_ptr_unchecked() };
        if !d.is_null() {
            Some(DocumentNode::new(&self.tree_(), d.cast()))
        } else {
            None
        }
    }

    /// Parent of this node.
    ///
    /// # Returns
    ///
    /// The parent node, or `None` if this node has no parent.
    fn parent_node(&self) -> Option<Node> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        wrap_any_raw_node(&self.tree_(), unsafe { self.node_ptr_().as_ref()?.parent })
    }

    /// Return this node’s parent if it is an element.
    ///
    /// # Returns
    ///
    /// The parent element, or `None` if the parent is not an element.
    #[inline]
    fn parent_element(&self) -> Option<ElementNode> {
        match self.parent_node()? {
            Node::Element(n) => Some(n),
            _ => None,
        }
    }

    /// Return whether this node has any children.
    ///
    /// # Returns
    ///
    /// True if this node has at least one child.
    #[inline]
    fn has_child_nodes(&self) -> bool {
        self.first_child().is_some()
    }

    /// Return whether the supplied node is this node or one of its descendants.
    ///
    /// # Arguments
    ///
    /// * `node` - The node to insert or inspect.
    ///
    /// # Returns
    ///
    /// True if `node` is this node or one of its descendants.
    fn contains(&self, node: &NodeRef) -> bool {
        check_nodes!(self, node);
        if self.node_ptr_() == node.node_ptr_() {
            return true;
        }
        unsafe {
            let mut ancestor = (*node.node_ptr_()).parent;
            while !ancestor.is_null() {
                if ancestor == self.node_ptr_() {
                    return true;
                }
                ancestor = (*ancestor).parent;
            }
        }
        false
    }

    /// List of child nodes.
    ///
    /// # Returns
    ///
    /// A live collection of this node’s child nodes.
    fn child_nodes(&self) -> LiveCollection<Node> {
        LiveCollection::new_live(&self.as_noderef(), None, |n, _| {
            let mut nodes = Vec::new();
            let mut child = n.first_child();
            while let Some(c) = child {
                child = c.next_sibling();
                nodes.push(c);
            }
            nodes
        })
    }

    /// First child node, regardless of its kind.
    ///
    /// # Returns
    ///
    /// The first child node, or `None` if there are no children.
    fn first_child(&self) -> Option<Node> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        wrap_any_raw_node(&self.tree_(), unsafe { self.node_ptr_().as_ref()?.first_child })
    }

    /// Last child node, regardless of its kind.
    ///
    /// # Returns
    ///
    /// The last child node, or `None` if there are no children.
    fn last_child(&self) -> Option<Node> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        wrap_any_raw_node(&self.tree_(), unsafe { self.node_ptr_().as_ref()?.last_child })
    }

    /// Previous sibling node.
    ///
    /// # Returns
    ///
    /// The previous sibling node, or `None` if there is no previous sibling.
    fn previous_sibling(&self) -> Option<Node> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        wrap_any_raw_node(&self.tree_(), unsafe { self.node_ptr_().as_ref()?.prev })
    }

    /// Next sibling node.
    ///
    /// # Returns
    ///
    /// The next sibling node, or `None` if there is no next sibling.
    fn next_sibling(&self) -> Option<Node> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        wrap_any_raw_node(&self.tree_(), unsafe { self.node_ptr_().as_ref()?.next })
    }

    /// Clone this node, optionally including its descendants.
    ///
    /// # Arguments
    ///
    /// * `deep` - Whether to include descendants in the clone.
    ///
    /// # Returns
    ///
    /// A clone of this node, or `None` if cloning is unavailable.
    fn clone_node(&self, deep: bool) -> Option<Node> {
        check_node!(self);
        wrap_any_raw_node(&self.tree_(), unsafe { lxb_dom_node_clone(self.node_ptr_(), deep) })
    }

    /// Insert a child before the optional reference child.
    ///
    /// # Arguments
    ///
    /// * `node` - The node to insert or inspect.
    /// * `child` - The reference child, or the existing child to replace or remove.
    ///
    /// # Returns
    ///
    /// A reference to the inserted node, or `None` if insertion is invalid.
    fn insert_before<'a>(&self, node: &NodeRef<'a>, child: Option<NodeRef<'a>>) -> Option<NodeRef<'a>> {
        check_nodes!(self, node);
        if self.node_ptr_() == node.node_ptr_() {
            return None;
        }
        if let Some(c) = &child {
            check_nodes!(node, c);
            if self.node_ptr_() == c.node_ptr_() {
                return None;
            }
        }
        unsafe { self.insert_before_unchecked(node, child) }
    }

    /// Append a child as the last child.
    ///
    /// # Arguments
    ///
    /// * `node` - The node to insert, remove, compare, or query, as indicated by the method.
    ///
    /// # Returns
    ///
    /// A reference to the appended node, or `None` if insertion is invalid.
    fn append_child<'a>(&self, node: &NodeRef<'a>) -> Option<NodeRef<'a>> {
        check_nodes!(self, node);
        if self.node_ptr_() == node.node_ptr_() {
            return None;
        }
        unsafe { self.append_child_unchecked(node) }
    }

    /// Replace a child and return the removed node.
    ///
    /// # Arguments
    ///
    /// * `node` - The node to insert or inspect.
    /// * `child` - The reference child, or the existing child to replace or remove.
    ///
    /// # Returns
    ///
    /// The removed child, or `None` if replacement is invalid.
    fn replace_child<'a>(&self, node: &NodeRef<'a>, child: &NodeRef<'a>) -> Option<NodeRef<'a>> {
        check_nodes!(self, node);
        check_nodes!(node, child);
        if self.node_ptr_() == node.node_ptr_() {
            return None;
        }
        unsafe { self.replace_child_unchecked(node, child) }
    }

    /// Remove a child and return it.
    ///
    /// # Arguments
    ///
    /// * `node` - The node to insert, remove, compare, or query, as indicated by the method.
    ///
    /// # Returns
    ///
    /// The removed child, or `None` if it is not a child of this node.
    fn remove_child<'a>(&self, node: &NodeRef<'a>) -> Option<NodeRef<'a>> {
        check_nodes!(self, node);
        unsafe { self.remove_child_unchecked(node) }
    }

    /// Lazy pre-order traversal including this node. See [`NodeIterator`] for
    /// mutation invalidation and error semantics.
    ///
    /// ```
    /// use resiliparse::parse::html::{
    ///     dom::{node::Node, traits::NodeInterface}, tree::HtmlTree,
    /// };
    ///
    /// let tree = HtmlTree::parse("<p>Hello <b>DOM</b></p>").unwrap();
    /// let node = Node::from(tree.body().unwrap());
    /// assert_eq!(node.iter().collect::<Result<Vec<_>, _>>().unwrap().len(), 5);
    /// assert_eq!(node.iter_elements().collect::<Result<Vec<_>, _>>().unwrap().len(), 3);
    /// ```
    ///
    /// # Returns
    ///
    /// A lazy pre-order iterator over this subtree.
    fn iter(&self) -> NodeIterator<'_>
    where
        Self: Sized,
    {
        NodeIterator::new(self.as_noderef())
    }

    /// Lazy element-only pre-order traversal, including an element root.
    ///
    /// # Returns
    ///
    /// A lazy pre-order iterator over elements in this subtree.
    fn iter_elements(&self) -> ElementIterator<'_>
    where
        Self: Sized,
    {
        ElementIterator::new(self)
    }

    /// Remove and destroy this node and its descendants.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn decompose(&self) {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        if self.node_ptr_().is_null() {
            return;
        }
        unsafe {
            document.mark_mutated();
            lxb_dom_node_destroy_deep(self.node_ptr_());
        }
        self.reset_node_ptr_();
    }
}

/// DocumentType node.
pub trait DocumentType: ChildNode {
    /// Return the attribute or node name.
    ///
    /// # Returns
    ///
    /// The document type name, or `None` if unavailable.
    fn name(&self) -> Option<String>;

    /// Return the public identifier, if present.
    ///
    /// # Returns
    ///
    /// The public identifier, or `None` if unavailable.
    fn public_id(&self) -> Option<String>;

    /// Return the system identifier, if present.
    ///
    /// # Returns
    ///
    /// The system identifier, or `None` if unavailable.
    fn system_id(&self) -> Option<String>;
}

/// Common operations available on document and shadow-root nodes.
pub trait DocumentOrShadowRoot: NodeInterface {}

/// Operations associated with a shadow root.
pub trait ShadowRoot: DocumentOrShadowRoot {}

/// Operations available on a document node.
pub trait Document: DocumentOrShadowRoot + ParentNode + NonElementParentNode {
    /// Return the document type declaration, if present.
    ///
    /// # Returns
    ///
    /// The document type declaration, or `None` if the document has none.
    fn doctype(&self) -> Option<DocumentTypeNode>;

    /// Return the document element, if present.
    ///
    /// # Returns
    ///
    /// The document element, or `None` if the document has none.
    fn document_element(&self) -> Option<ElementNode>;

    /// Return a live collection of descendant elements with the given tag name.
    ///
    /// # Arguments
    ///
    /// * `qualified_name` - The tag name to match.
    ///
    /// # Returns
    ///
    /// A live collection of descendant elements with the requested tag name.
    fn get_elements_by_tag_name(&self, qualified_name: &str) -> HtmlCollection;

    /// Return a live collection of descendant elements containing the requested classes.
    ///
    /// # Arguments
    ///
    /// * `qualified_name` - The whitespace-separated class names to find.
    ///
    /// # Returns
    ///
    /// A live collection of descendant elements containing all requested classes.
    fn get_elements_by_class_name(&self, qualified_name: &str) -> HtmlCollection;

    /// Return a live collection of descendant elements with the given attribute value.
    ///
    /// # Arguments
    ///
    /// * `qualified_name` - The qualified attribute name to match.
    /// * `value` - The value to assign or compare.
    ///
    /// # Returns
    ///
    /// A live collection of descendant elements with the requested attribute value.
    fn get_elements_by_attr(&self, qualified_name: &str, value: &str) -> HtmlCollection;

    /// Return a live collection of descendants with the given attribute value and comparison mode.
    ///
    /// # Arguments
    ///
    /// * `qualified_name` - The qualified tag name to match.
    /// * `value` - The attribute value to match or set.
    /// * `case_insensitive` - Whether matching ignores character case.
    ///
    /// # Returns
    ///
    /// A live collection of descendant elements with the requested attribute value.
    fn get_elements_by_attr_case(&self, qualified_name: &str, value: &str, case_insensitive: bool) -> HtmlCollection;

    /// Create a detached element with the given local name.
    ///
    /// # Arguments
    ///
    /// * `local_name` - The local name for the new node.
    ///
    /// # Returns
    ///
    /// The new detached element, or a `DOMError` if creation fails.
    fn create_element(&self, local_name: &str) -> Result<ElementNode, DomError>;

    /// Create an empty document fragment.
    ///
    /// # Returns
    ///
    /// The new empty document fragment, or a `DOMError` if creation fails.
    fn create_document_fragment(&self) -> Result<DocumentFragmentNode, DomError>;

    /// Create a detached text node.
    ///
    /// # Arguments
    ///
    /// * `data` - The initial text content.
    ///
    /// # Returns
    ///
    /// The new detached text node, or a `DOMError` if creation fails.
    fn create_text_node(&self, data: &str) -> Result<TextNode, DomError>;

    /// Create a detached CDATA section.
    ///
    /// # Arguments
    ///
    /// * `data` - The initial CDATA content.
    ///
    /// # Returns
    ///
    /// The new detached CDATA section, or a `DOMError` if creation fails.
    fn create_cdata_section(&self, data: &str) -> Result<CdataSectionNode, DomError>;
    /// Create a detached comment node.
    ///
    /// # Arguments
    ///
    /// * `data` - The initial comment content.
    ///
    /// # Returns
    ///
    /// The new detached comment node, or a `DOMError` if creation fails.
    fn create_comment(&self, data: &str) -> Result<CommentNode, DomError>;

    /// Create a detached processing instruction.
    ///
    /// # Arguments
    ///
    /// * `target` - The processing-instruction target.
    /// * `data` - The processing instruction data.
    ///
    /// # Returns
    ///
    /// The new detached processing instruction, or a `DOMError` if creation fails.
    fn create_processing_instruction(&self, target: &str, data: &str) -> Result<ProcessingInstructionNode, DomError>;

    /// Create a detached attribute node.
    ///
    /// # Arguments
    ///
    /// * `local_name` - The local name for the new node.
    ///
    /// # Returns
    ///
    /// The new detached attribute node, or a `DOMError` if creation fails.
    fn create_attribute(&self, local_name: &str) -> Result<AttrNode, DomError>;
}

/// Operations available on a document fragment node.
pub trait DocumentFragment: DocumentOrShadowRoot + ParentNode + NonElementParentNode {}

/// ParentNode mixin trait.
pub trait ParentNode: NodeInterface {
    /// List of child element nodes.
    ///
    /// # Returns
    ///
    /// A live collection of child elements.
    fn children(&self) -> HtmlCollection;

    /// First element child of this DOM node.
    ///
    /// # Returns
    ///
    /// The first child element, or `None` if there is no child element.
    fn first_element_child(&self) -> Option<ElementNode> {
        check_node!(self);
        unsafe { next_element_unchecked(&self.tree_(), (*self.node_ptr_()).first_child) }
    }

    /// Last element child element of this DOM node.
    ///
    /// # Returns
    ///
    /// The last child element, or `None` if there is no child element.
    fn last_element_child(&self) -> Option<ElementNode> {
        check_node!(self);
        unsafe { previous_element_unchecked(&self.tree_(), (*self.node_ptr_()).last_child) }
    }

    /// Return the number of child elements.
    ///
    /// # Returns
    ///
    /// The number of child elements.
    fn child_element_count(&self) -> usize {
        let mut child = self.first_element_child();
        let mut count = 0;
        while let Some(c) = child {
            child = c.next_element_sibling();
            count += 1;
        }
        count
    }

    /// Insert nodes before the first child.
    ///
    /// # Arguments
    ///
    /// * `nodes` - The nodes to insert or use as replacements.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn prepend(&self, nodes: &[NodeRef]) {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        let mut fc = self.first_child();
        while let Some(child) = &fc {
            if !nodes.iter().any(|n| *n == child.as_noderef()) {
                break;
            }
            fc = child.next_sibling();
        }
        unsafe {
            self.splice_children_unchecked(nodes, fc.as_ref().map_or(ptr::null_mut(), |node| node.node_ptr_()), &[]);
        }
    }

    /// Insert nodes after the last child.
    ///
    /// # Arguments
    ///
    /// * `nodes` - The nodes to insert or use as replacements.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn append(&self, nodes: &[NodeRef]) {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        unsafe {
            self.splice_children_unchecked(nodes, ptr::null_mut(), &[]);
        }
    }

    /// Replace all children with the supplied nodes.
    ///
    /// # Arguments
    ///
    /// * `nodes` - The nodes to insert or use as replacements.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn replace_children(&self, nodes: &[NodeRef]) {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        let children = self.child_nodes().values();
        let removed: Vec<_> = children.iter().map(|node| node.node_ptr_()).collect();
        unsafe {
            self.splice_children_unchecked(nodes, ptr::null_mut(), &removed);
        }
    }

    /// Return the first matching descendant element, if any.
    ///
    /// # Arguments
    ///
    /// * `selectors` - A CSS selector list.
    ///
    /// # Returns
    ///
    /// The first matching descendant element, or `None` if no element matches.
    fn query_selector(&self, selectors: &str) -> Result<Option<ElementNode>, CssParserError> {
        CssSelector::new(selectors)?.query_selector(&self.as_noderef())
    }

    /// Return all matching descendant elements in tree order.
    ///
    /// # Arguments
    ///
    /// * `selectors` - The CSS selector list to parse or apply.
    ///
    /// # Returns
    ///
    /// A snapshot of matching descendant elements in tree order.
    fn query_selector_all(&self, selectors: &str) -> Result<ElementNodeList, CssParserError> {
        CssSelector::new(selectors)?.query_selector_all(&self.as_noderef())
    }
}

/// NonElementParentNode mixin trait.
pub trait NonElementParentNode: NodeInterface {
    /// Return the first descendant element with this ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The element ID to read, set, or search for.
    ///
    /// # Returns
    ///
    /// The first descendant element with this ID, or `None` if none matches.
    fn get_element_by_id(&self, id: &str) -> Option<ElementNode> {
        unsafe { get_element_by_id(&self.as_noderef(), id, false) }
    }

    /// Return the first descendant whose ID matches using the requested case sensitivity.
    ///
    /// # Arguments
    ///
    /// * `id` - The element ID to read, set, or find.
    /// * `case_insensitive` - Whether name or value comparison ignores character case.
    ///
    /// # Returns
    ///
    /// The first matching descendant element, or `None` if none matches.
    fn get_element_by_id_case(&self, id: &str, case_insensitive: bool) -> Option<ElementNode> {
        unsafe { get_element_by_id(&self.as_noderef(), id, case_insensitive) }
    }
}

/// ChildNode mixin trait.
pub trait ChildNode: NodeInterface {
    /// Insert nodes immediately before this node among its siblings.
    ///
    /// # Arguments
    ///
    /// * `nodes` - The nodes to insert or use as replacements.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn before(&self, nodes: &[NodeRef]) {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        unsafe {
            splice_siblings_unchecked(self.as_noderef(), nodes, Some(self.as_node()), &[]);
        }
    }

    /// Insert nodes immediately after this node among its siblings.
    ///
    /// # Arguments
    ///
    /// * `nodes` - The nodes to insert or use as replacements.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn after(&self, nodes: &[NodeRef]) {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        unsafe {
            splice_siblings_unchecked(self.as_noderef(), nodes, self.next_sibling(), &[]);
        }
    }

    /// Replace this node among its siblings with the supplied nodes.
    ///
    /// # Arguments
    ///
    /// * `nodes` - The nodes to insert or use as replacements.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn replace_with(&self, nodes: &[NodeRef]) {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        unsafe {
            splice_siblings_unchecked(self.as_noderef(), nodes, Some(self.as_node()), &[self.node_ptr_()]);
        }
    }

    /// Remove the named attribute if it exists.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn remove(&self) {
        check_node!(self);
        unsafe {
            self.tree_().mark_mutated();
            lxb_dom_node_remove(self.node_ptr_());
        }
    }
}

/// NonDocumentTypeChildNode mixin trait.
pub trait NonDocumentTypeChildNode: NodeInterface {
    /// Previous sibling element node.
    ///
    /// # Returns
    ///
    /// The previous sibling element, or `None` if there is no previous element sibling.
    fn previous_element_sibling(&self) -> Option<ElementNode> {
        check_node!(self);
        unsafe { previous_element_unchecked(&self.tree_(), (*self.node_ptr_()).prev) }
    }

    /// Next sibling element node.
    ///
    /// # Returns
    ///
    /// The next sibling element, or `None` if there is no next element sibling.
    fn next_element_sibling(&self) -> Option<ElementNode> {
        check_node!(self);
        unsafe { next_element_unchecked(&self.tree_(), (*self.node_ptr_()).next) }
    }
}

/// HTML Element mixin trait.
pub trait Element: ParentNode + ChildNode + NonDocumentTypeChildNode {
    /// Return the element tag name.
    ///
    /// # Returns
    ///
    /// The element’s tag name, or `None` if unavailable.
    fn tag_name(&self) -> Option<String>;

    /// Return the local name.
    ///
    /// # Returns
    ///
    /// The local name, or `None` if unavailable.
    fn local_name(&self) -> Option<String>;

    /// Return the element ID.
    ///
    /// # Returns
    ///
    /// The element ID, or `None` if the ID is absent.
    fn id(&self) -> Option<String>;

    /// Set the element ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The element ID to read, set, or find.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn set_id(&self, id: &str);

    /// Return the element’s class attribute.
    ///
    /// # Returns
    ///
    /// The class attribute value, or `None` if it is absent.
    fn class_name(&self) -> Option<String>;

    /// Set the element’s class attribute.
    ///
    /// # Arguments
    ///
    /// * `class_name` - The replacement value for the class attribute.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn set_class_name(&self, class_name: &str);

    /// Return a live token-list view of the class attribute.
    ///
    /// # Returns
    ///
    /// A live token-list view of the class attribute.
    fn class_list(&self) -> DomTokenList<'_>;

    /// Return a mutable token-list view of the class attribute.
    ///
    /// # Returns
    ///
    /// A mutable token-list view of the class attribute.
    fn class_list_mut(&self) -> DomTokenListMut<'_> {
        self.class_list()
    }

    /// Return the named attribute value, if present.
    ///
    /// # Arguments
    ///
    /// * `qualified_name` - The qualified attribute name to read.
    ///
    /// # Returns
    ///
    /// The attribute value, or `None` if the attribute is absent.
    fn attribute(&self, qualified_name: &str) -> Option<String>;

    /// Return the named attribute value or the supplied default.
    ///
    /// # Arguments
    ///
    /// * `qualified_name` - The qualified attribute name to read.
    /// * `default` - The string to return when the attribute is absent.
    ///
    /// # Returns
    ///
    /// The attribute value, or `default` if the attribute is absent.
    fn attribute_or(&self, qualified_name: &str, default: &str) -> String;

    /// Return the named attribute value or an empty string.
    ///
    /// # Arguments
    ///
    /// * `qualified_name` - The qualified attribute name to look up.
    ///
    /// # Returns
    ///
    /// The attribute value, or an empty string if the attribute is absent.
    fn attribute_or_default(&self, qualified_name: &str) -> String;

    /// Return the named attribute node, if present.
    ///
    /// # Arguments
    ///
    /// * `qualified_name` - The qualified tag name to match.
    ///
    /// # Returns
    ///
    /// The named attribute node, or `None` if it is absent.
    fn attribute_node(&self, qualified_name: &str) -> Option<AttrNode>;

    /// Return the qualified names of this element’s attributes.
    ///
    /// # Returns
    ///
    /// The qualified names of the element’s attributes.
    fn attribute_names(&self) -> Vec<String>;

    /// Return a live map of this element’s attributes.
    ///
    /// # Returns
    ///
    /// A live map of the element’s attributes.
    fn attributes(&self) -> NamedNodeMap;

    /// Set an attribute, creating it if needed.
    ///
    /// # Arguments
    ///
    /// * `qualified_name` - The qualified attribute name to set.
    /// * `value` - The value to assign or compare.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn set_attribute(&self, qualified_name: &str, value: &str);

    /// Set or replace an attribute using an attribute node.
    ///
    /// # Arguments
    ///
    /// * `attribute` - The attribute node to assign.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn set_attribute_node(&self, attribute: &AttrNode);

    /// Remove the named attribute if it exists.
    ///
    /// # Arguments
    ///
    /// * `qualified_name` - The qualified attribute name to toggle.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn remove_attribute(&self, qualified_name: &str);

    /// Add or remove an attribute according to the optional force value.
    ///
    /// # Arguments
    ///
    /// * `qualified_name` - The qualified attribute name to check.
    /// * `force` - Whether to force presence (`Some(true)`), force absence (`Some(false)`), or toggle (`None`).
    ///
    /// # Returns
    ///
    /// True if the attribute is present after this call.
    fn toggle_attribute(&self, qualified_name: &str, force: Option<bool>) -> bool;

    /// Return whether the named attribute exists.
    ///
    /// # Arguments
    ///
    /// * `qualified_name` - The qualified tag name to match.
    ///
    /// # Returns
    ///
    /// True if the element has the named attribute.
    fn has_attribute(&self, qualified_name: &str) -> bool;

    /// Return the nearest matching element at or above the supplied element.
    ///
    /// # Arguments
    ///
    /// * `selectors` - The CSS selector list to parse or apply.
    ///
    /// # Returns
    ///
    /// The nearest matching element at or above this element, or `None` if none matches.
    fn closest(&self, selectors: &str) -> Result<Option<ElementNode>, CssParserError>;

    /// Return whether the element matches this selector list.
    ///
    /// # Arguments
    ///
    /// * `selectors` - The CSS selector list to parse or apply.
    ///
    /// # Returns
    ///
    /// True if this element matches the selector list.
    fn matches(&self, selectors: &str) -> Result<bool, CssParserError>;

    /// Return a live collection of descendant elements with the given tag name.
    ///
    /// # Arguments
    ///
    /// * `name` - The tag name to match.
    ///
    /// # Returns
    ///
    /// A live collection of descendant elements with the requested tag name.
    fn get_elements_by_tag_name(&self, name: &str) -> HtmlCollection;

    /// Return a live collection of descendant elements containing the requested classes.
    ///
    /// # Arguments
    ///
    /// * `class_names` - The whitespace-separated class names to find.
    ///
    /// # Returns
    ///
    /// A live collection of descendant elements containing all requested classes.
    fn get_elements_by_class_name(&self, class_names: &str) -> HtmlCollection;

    /// Return a live collection of descendant elements with the given attribute value.
    ///
    /// # Arguments
    ///
    /// * `name` - The attribute name to match.
    /// * `value` - The value to assign or compare.
    ///
    /// # Returns
    ///
    /// A live collection of descendant elements with the requested attribute value.
    fn get_elements_by_attr(&self, name: &str, value: &str) -> HtmlCollection;

    /// Return a live collection of descendants with the given attribute value and comparison mode.
    ///
    /// # Arguments
    ///
    /// * `name` - The attribute name to match.
    /// * `value` - The value to assign or compare.
    /// * `case_insensitive` - Whether name or value comparison ignores character case.
    ///
    /// # Returns
    ///
    /// A live collection of descendant elements with the requested attribute value.
    fn get_elements_by_attr_case(&self, name: &str, value: &str, case_insensitive: bool) -> HtmlCollection;

    /// Serialize this element’s child nodes as markup.
    ///
    /// # Returns
    ///
    /// The serialized markup of this element’s child nodes.
    fn inner_html(&self) -> String;

    /// Replace this element’s children by parsing the supplied markup.
    ///
    /// # Arguments
    ///
    /// * `html` - HTML markup to parse or serialize as text.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn set_inner_html(&self, html: &str);

    /// Serialize this element, including its children, as markup.
    ///
    /// # Returns
    ///
    /// The serialized markup of this element and its descendants.
    fn outer_html(&self) -> String;

    /// Replace this element with markup parsed in its parent context.
    ///
    /// # Arguments
    ///
    /// * `html` - HTML markup or text to parse or set.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn set_outer_html(&self, html: &str);

    /// Return the text content of this element’s descendants.
    ///
    /// # Returns
    ///
    /// The text content of this element’s descendants.
    fn inner_text(&self) -> String;

    /// Replace this element’s children with a text node.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to assign or insert.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn set_inner_text(&self, text: &str);

    /// Return this element’s text content.
    ///
    /// # Returns
    ///
    /// The text content of this element.
    fn outer_text(&self) -> String;

    /// Replace this element with text in its parent.
    ///
    /// # Arguments
    ///
    /// * `text` - The text that replaces this element in its parent.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn set_outer_text(&self, text: &str);
}

/// Operations available on an attribute node.
pub trait Attr: NodeInterface {
    /// Return the local name.
    ///
    /// # Returns
    ///
    /// The attribute’s local name, or `None` if unavailable.
    fn local_name(&self) -> Option<String>;

    /// Return the attribute or node name.
    ///
    /// # Returns
    ///
    /// The attribute name, or `None` if unavailable.
    fn name(&self) -> Option<String>;

    /// Return the attribute value, if present.
    ///
    /// # Returns
    ///
    /// The attribute value, or `None` if it is absent.
    fn value(&self) -> Option<String>;

    /// Set the attribute value.
    ///
    /// # Arguments
    ///
    /// * `value` - The attribute value to match or set.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn set_value(&self, value: &str);

    /// Return the element that owns this attribute, if any.
    ///
    /// # Returns
    ///
    /// The owning element, or `None` if the attribute is detached.
    fn owner_element(&self) -> Option<ElementNode>;
}

/// Convert Unicode scalar indices to UTF-8 byte boundaries. Counts past the
/// end are clamped; offsets past the end are rejected, including empty edits.
fn character_data_range(data: &str, offset: usize, count: usize) -> Result<Range<usize>, DomError> {
    let mut boundaries = data
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(data.len()));
    let start = boundaries.nth(offset).ok_or_else(|| DomError {
        msg: "Character-data offset exceeds length".into(),
    })?;
    let end = if count == 0 {
        start
    } else {
        boundaries.nth(count - 1).unwrap_or(data.len())
    };
    Ok(start..end)
}

/// Character data indexed by Unicode scalar values, including `len()`.
/// Offsets equal to the length are valid; larger offsets return an error.
/// Counts extending beyond the end are clamped. Indices are neither UTF-8
/// byte offsets nor DOM UTF-16 code-unit offsets.
pub trait CharacterData: NodeInterface + ChildNode + NonDocumentTypeChildNode {
    /// Return the number of Unicode scalar values in the character data.
    ///
    /// # Returns
    ///
    /// The number of Unicode scalar values in the character data.
    fn len(&self) -> usize {
        self.node_value().unwrap_or_default().chars().count()
    }

    /// Return whether the character data node is empty.
    ///
    /// # Returns
    ///
    /// True if the character data contains no characters.
    fn is_empty(&self) -> bool {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        if self.node_ptr_().is_null() {
            return true;
        }
        unsafe { self.node_value_unchecked().is_none_or(str::is_empty) }
    }

    /// Return the character data as an owned string.
    ///
    /// # Returns
    ///
    /// The character data, or `None` if unavailable.
    #[inline]
    fn data(&self) -> Option<String> {
        self.node_value()
    }

    /// Replace the character data.
    ///
    /// # Arguments
    ///
    /// * `data` - The replacement character data.
    ///
    /// # Returns
    ///
    /// No value is returned.
    #[inline]
    fn set_data(&self, data: &str) {
        self.set_node_value(data);
    }

    /// Return a range of character data by Unicode scalar-value indices.
    ///
    /// # Arguments
    ///
    /// * `offset` - A zero-based Unicode scalar-value position in the character data.
    /// * `count` - The number of Unicode scalar values to select; a range extending past the end is clamped.
    ///
    /// # Returns
    ///
    /// The requested substring, or a `DOMError` if `offset` exceeds the data length.
    fn substring_data(&self, offset: usize, count: usize) -> Result<String, DomError> {
        let data = self.data().ok_or_else(|| DomError {
            msg: "Character data is unavailable".into(),
        })?;
        Ok(data[character_data_range(&data, offset, count)?].to_owned())
    }

    /// Append text to the character data.
    ///
    /// # Arguments
    ///
    /// * `data` - The text to append.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn append_data(&self, data: &str) {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        if data.is_empty() {
            return;
        }
        let mut value = self.data().unwrap_or_default();
        value.push_str(data);
        self.set_data(&value);
    }

    /// Insert text at a Unicode scalar-value index.
    ///
    /// # Arguments
    ///
    /// * `offset` - A zero-based Unicode scalar-value position in the character data.
    /// * `data` - The text to insert.
    ///
    /// # Returns
    ///
    /// No value if insertion succeeds; otherwise a `DOMError` if `offset` exceeds the data length.
    fn insert_data(&self, offset: usize, data: &str) -> Result<(), DomError> {
        self.replace_data(offset, 0, data)
    }

    /// Delete a range of character data by Unicode scalar-value indices.
    ///
    /// # Arguments
    ///
    /// * `offset` - A zero-based Unicode scalar-value position in the character data.
    /// * `count` - The number of Unicode scalar values to select; a range extending past the end is clamped.
    ///
    /// # Returns
    ///
    /// No value if deletion succeeds; otherwise a `DOMError` if `offset` exceeds the data length.
    fn delete_data(&self, offset: usize, count: usize) -> Result<(), DomError> {
        self.replace_data(offset, count, "")
    }

    /// Replace a range of character data by Unicode scalar-value indices.
    ///
    /// # Arguments
    ///
    /// * `offset` - A zero-based Unicode scalar-value position in the character data.
    /// * `count` - The number of Unicode scalar values to select; a range extending past the end is clamped.
    /// * `data` - The replacement character data.
    ///
    /// # Returns
    ///
    /// No value if replacement succeeds; otherwise a `DOMError` if `offset` exceeds the data length.
    fn replace_data(&self, offset: usize, count: usize, data: &str) -> Result<(), DomError> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        let mut value = self.data().ok_or_else(|| DomError {
            msg: "Character data is unavailable".into(),
        })?;
        let range = character_data_range(&value, offset, count)?;
        if &value[range.clone()] != data {
            value.replace_range(range, data);
            self.set_data(&value);
        }
        Ok(())
    }
}

/// Operations available on a text node.
pub trait Text: CharacterData {}

/// Operations available on a CDATA section node.
pub trait CdataSection: CharacterData {}

/// Operations available on a processing instruction node.
pub trait ProcessingInstruction: CharacterData {
    /// Return the processing instruction target.
    ///
    /// # Returns
    ///
    /// The processing-instruction target, or `None` if unavailable.
    fn target(&self) -> Option<String>;
}

/// Operations available on a comment node.
pub trait Comment: CharacterData {}

/// Operations available on a notation node.
pub trait Notation: NodeInterface {
    /// Return the public identifier, if present.
    ///
    /// # Returns
    ///
    /// The public identifier, or `None` if unavailable.
    fn public_id(&self) -> Option<String>;
    /// Return the system identifier, if present.
    ///
    /// # Returns
    ///
    /// The system identifier, or `None` if unavailable.
    fn system_id(&self) -> Option<String>;
}
