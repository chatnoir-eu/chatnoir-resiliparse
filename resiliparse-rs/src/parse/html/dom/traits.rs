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

use crate::parse::html::css::{CSSParserError, CSSSelectorList, TraverseAction};
use crate::parse::html::dom::coll::*;
use crate::parse::html::dom::iter::*;
use crate::parse::html::dom::*;
use crate::parse::html::lexbor::{str_from_lxb_char_t, str_from_lxb_str_cb, str_from_lxb_str_t};
use std::fmt::{Debug, Display};
use std::ops::Range;
use std::ptr::{self, addr_of};

#[derive(Clone, PartialEq, Eq, Debug)]
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
    Notation = 0x0C, // legacy
}

/// Shared implementation for before, after, and replace_with. Caller holds
/// the document lock so the anchor remains valid throughout the operation.
unsafe fn splice_siblings_unchecked(
    receiver: NodeRef,
    nodes: &[&mut Node],
    mut anchor: Option<Node>,
    removed: &[*mut lxb_dom_node_t],
) {
    if let Some(mut parent) = receiver.parent_node() {
        let nodes: Vec<_> = nodes.iter().map(|node| node.as_noderef()).collect();
        // Keep an anchor that will not itself be moved by this operation.
        while let Some(node) = &anchor {
            if !nodes.iter().any(|input| *input == node.as_noderef()) {
                break;
            }
            anchor = node.next_sibling();
        }
        unsafe {
            parent.splice_children_unchecked(
                &nodes,
                anchor.as_ref().map_or(ptr::null_mut(), |node| node.node_ptr_()),
                removed,
            );
        }
    }
}

pub(crate) trait NodeInterfaceBaseImpl {
    fn new(tree: &Arc<HTMLDocument>, node: *mut lxb_dom_node_t) -> Self
    where
        Self: Sized;
    fn tree_(&self) -> Arc<HTMLDocument>;
    fn node_ptr_(&self) -> *mut lxb_dom_node_t;
    fn reset_node_ptr_(&mut self);

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
        &mut self,
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

    unsafe fn insert_before_unchecked<'a>(
        &mut self,
        node: &NodeRef<'a>,
        child: Option<NodeRef>,
    ) -> Option<NodeRef<'a>> {
        let reference = child.as_ref().map_or(ptr::null_mut(), |child| child.node_ptr_());
        unsafe { self.splice_children_unchecked(&[node.clone()], reference, &[]) }.then(|| node.clone())
    }

    unsafe fn append_child_unchecked<'a>(&mut self, node: &NodeRef<'a>) -> Option<NodeRef<'a>> {
        unsafe { self.insert_before_unchecked(node, None) }
    }

    unsafe fn replace_child_unchecked<'a>(
        &mut self,
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

    unsafe fn remove_child_unchecked<'a>(&mut self, node: &NodeRef<'a>) -> Option<NodeRef<'a>> {
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

/// Base DOM node interface.
#[allow(private_bounds)]
pub trait NodeInterface: NodeInterfaceBaseImpl + Debug + Display {
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

    fn as_noderef(&self) -> NodeRef<'_>;
    fn as_noderef_mut(&mut self) -> NodeRefMut<'_>;
    fn as_node(&self) -> Node;
    fn into_node(self) -> Node;

    /// DOM element tag or node name.
    fn node_name(&self) -> Option<String> {
        check_node!(self);
        unsafe { Some(self.node_name_unchecked()?.to_owned()) }
    }

    /// Node text value.
    #[inline]
    fn node_value(&self) -> Option<String> {
        check_node!(self);
        unsafe { Some(self.node_value_unchecked()?.to_owned()) }
    }

    fn set_node_value(&mut self, value: &str) {
        check_node!(self);
        unsafe {
            self.tree_().mark_mutated();
            lxb_dom_node_text_content_set(self.node_ptr_(), value.as_ptr(), value.len());
        }
    }

    /// Text contents of this DOM node and its children.
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

    #[inline]
    fn set_text_content(&mut self, content: &str) {
        self.set_node_value(content)
    }

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
    fn parent_node(&self) -> Option<Node> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        wrap_any_raw_node(&self.tree_(), unsafe { self.node_ptr_().as_ref()?.parent })
    }

    #[inline]
    fn parent_element(&self) -> Option<ElementNode> {
        match self.parent_node()? {
            Node::Element(n) => Some(n),
            _ => None,
        }
    }

    #[inline]
    fn has_child_nodes(&self) -> bool {
        self.first_child().is_some()
    }

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
    fn child_nodes(&self) -> NodeList {
        NodeList::new_live(&self.as_noderef(), None, |n, _| {
            let mut nodes = Vec::new();
            let mut child = n.first_child();
            while let Some(c) = child {
                child = c.next_sibling();
                nodes.push(c);
            }
            nodes
        })
    }

    /// First child element of this DOM node.
    fn first_child(&self) -> Option<Node> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        wrap_any_raw_node(&self.tree_(), unsafe { self.node_ptr_().as_ref()?.first_child })
    }

    /// Last child element of this DOM node.
    fn last_child(&self) -> Option<Node> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        wrap_any_raw_node(&self.tree_(), unsafe { self.node_ptr_().as_ref()?.last_child })
    }

    /// Previous sibling node.
    fn previous_sibling(&self) -> Option<Node> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        wrap_any_raw_node(&self.tree_(), unsafe { self.node_ptr_().as_ref()?.prev })
    }

    /// Next sibling node.
    fn next_sibling(&self) -> Option<Node> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        wrap_any_raw_node(&self.tree_(), unsafe { self.node_ptr_().as_ref()?.next })
    }

    fn clone_node(&self, deep: bool) -> Option<Node> {
        check_node!(self);
        wrap_any_raw_node(&self.tree_(), unsafe { lxb_dom_node_clone(self.node_ptr_(), deep) })
    }

    fn insert_before<'a>(&mut self, node: &NodeRef<'a>, child: Option<NodeRef<'a>>) -> Option<NodeRef<'a>> {
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

    fn append_child<'a>(&mut self, node: &NodeRef<'a>) -> Option<NodeRef<'a>> {
        check_nodes!(self, node);
        if self.node_ptr_() == node.node_ptr_() {
            return None;
        }
        unsafe { self.append_child_unchecked(node) }
    }

    fn replace_child<'a>(&mut self, node: &NodeRef<'a>, child: &NodeRef<'a>) -> Option<NodeRef<'a>> {
        check_nodes!(self, node);
        check_nodes!(node, child);
        if self.node_ptr_() == node.node_ptr_() {
            return None;
        }
        unsafe { self.replace_child_unchecked(node, child) }
    }

    fn remove_child<'a>(&mut self, node: &NodeRef<'a>) -> Option<NodeRef<'a>> {
        check_nodes!(self, node);
        unsafe { self.remove_child_unchecked(node) }
    }

    fn iter(&self) -> NodeIterator<'_>
    where
        Self: Sized,
    {
        NodeIterator::new(self.as_noderef())
    }

    fn iter_elements(&self) -> ElementIterator<'_>
    where
        Self: Sized,
    {
        ElementIterator::new(self)
    }

    fn decompose(&mut self) {
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
    unsafe fn name_unchecked(&self) -> Option<&str>;
    unsafe fn public_id_unchecked(&self) -> Option<&str>;
    unsafe fn system_id_unchecked(&self) -> Option<&str>;

    fn name(&self) -> Option<String>;
    fn public_id(&self) -> Option<String>;
    fn system_id(&self) -> Option<String>;
}

pub trait DocumentOrShadowRoot: NodeInterface {}

pub trait ShadowRoot: DocumentOrShadowRoot {}

/// Document node.
pub trait Document: DocumentOrShadowRoot + ParentNode + NonElementParentNode {
    fn doctype(&self) -> Option<DocumentTypeNode>;
    fn document_element(&self) -> Option<ElementNode>;

    fn get_elements_by_tag_name(&self, qualified_name: &str) -> HTMLCollection;
    fn get_elements_by_class_name(&self, qualified_name: &str) -> HTMLCollection;
    fn get_elements_by_attr(&self, qualified_name: &str, value: &str) -> HTMLCollection;
    fn get_elements_by_attr_case(&self, qualified_name: &str, value: &str, case_insensitive: bool) -> HTMLCollection;

    fn create_element(&mut self, local_name: &str) -> Result<ElementNode, DOMError>;
    fn create_document_fragment(&mut self) -> Result<DocumentFragmentNode, DOMError>;
    fn create_text_node(&mut self, data: &str) -> Result<TextNode, DOMError>;
    fn create_cdata_section(&mut self, data: &str) -> Result<CdataSectionNode, DOMError>;
    fn create_comment(&mut self, data: &str) -> Result<CommentNode, DOMError>;
    fn create_processing_instruction(
        &mut self,
        target: &str,
        data: &str,
    ) -> Result<ProcessingInstructionNode, DOMError>;
    fn create_attribute(&mut self, local_name: &str) -> Result<AttrNode, DOMError>;
}

pub trait DocumentFragment: DocumentOrShadowRoot + ParentNode + NonElementParentNode {}

/// ParentNode mixin trait.
pub trait ParentNode: NodeInterface {
    /// List of child element nodes.
    fn children(&self) -> HTMLCollection;

    /// First element child of this DOM node.
    fn first_element_child(&self) -> Option<ElementNode> {
        check_node!(self);
        unsafe { next_element_unchecked(&self.tree_(), (*self.node_ptr_()).first_child) }
    }

    /// Last element child element of this DOM node.
    fn last_element_child(&self) -> Option<ElementNode> {
        check_node!(self);
        unsafe { previous_element_unchecked(&self.tree_(), (*self.node_ptr_()).last_child) }
    }

    fn child_element_count(&self) -> usize {
        let mut child = self.first_element_child();
        let mut count = 0;
        while let Some(c) = child {
            child = c.next_element_sibling();
            count += 1;
        }
        count
    }

    fn prepend(&mut self, nodes: &[NodeRef]) {
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

    fn append(&mut self, nodes: &[NodeRef]) {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        unsafe {
            self.splice_children_unchecked(nodes, ptr::null_mut(), &[]);
        }
    }

    fn replace_children(&mut self, nodes: &[NodeRef]) {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        let children = self.child_nodes().values();
        let removed: Vec<_> = children.iter().map(|node| node.node_ptr_()).collect();
        unsafe {
            self.splice_children_unchecked(nodes, ptr::null_mut(), &removed);
        }
    }

    fn query_selector(&self, selectors: &str) -> Result<Option<ElementNode>, CSSParserError> {
        let sel_list = CSSSelectorList::parse_selectors(&self.tree_(), selectors)?;
        let mut result = Vec::<ElementNode>::with_capacity(1);
        sel_list.match_elements(
            &self.as_noderef(),
            |e, _, ctx| {
                ctx.push(e);
                TraverseAction::Stop
            },
            &mut result,
        );
        Ok(result.pop())
    }

    fn query_selector_all(&self, selectors: &str) -> Result<ElementNodeList, CSSParserError> {
        let sel_list = CSSSelectorList::parse_selectors(&self.tree_(), selectors)?;
        let mut result = Vec::<ElementNode>::new();
        sel_list.match_elements(
            &self.as_noderef(),
            |e, _, ctx| {
                ctx.push(e);
                TraverseAction::Ok
            },
            &mut result,
        );
        Ok(ElementNodeList::from(result))
    }
}

/// NonElementParentNode mixin trait.
pub trait NonElementParentNode: NodeInterface {
    fn get_element_by_id(&self, id: &str) -> Option<ElementNode> {
        unsafe { get_element_by_id(&self.as_noderef(), id, false) }
    }

    fn get_element_by_id_case(&self, id: &str, case_insensitive: bool) -> Option<ElementNode> {
        unsafe { get_element_by_id(&self.as_noderef(), id, case_insensitive) }
    }
}

/// ChildNode mixin trait.
pub trait ChildNode: NodeInterface {
    fn before(&mut self, nodes: &[&mut Node]) {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        unsafe {
            splice_siblings_unchecked(self.as_noderef(), nodes, Some(self.as_node()), &[]);
        }
    }

    fn after(&mut self, nodes: &[&mut Node]) {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        unsafe {
            splice_siblings_unchecked(self.as_noderef(), nodes, self.next_sibling(), &[]);
        }
    }

    fn replace_with(&mut self, nodes: &[&mut Node]) {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        unsafe {
            splice_siblings_unchecked(self.as_noderef(), nodes, Some(self.as_node()), &[self.node_ptr_()]);
        }
    }

    fn remove(&mut self) {
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
    fn previous_element_sibling(&self) -> Option<ElementNode> {
        check_node!(self);
        unsafe { previous_element_unchecked(&self.tree_(), (*self.node_ptr_()).prev) }
    }

    /// Next sibling element node.
    fn next_element_sibling(&self) -> Option<ElementNode> {
        check_node!(self);
        unsafe { next_element_unchecked(&self.tree_(), (*self.node_ptr_()).next) }
    }
}

/// HTML Element mixin trait.
pub trait Element: ParentNode + ChildNode + NonDocumentTypeChildNode {
    unsafe fn tag_name_unchecked(&self) -> Option<&str>;
    unsafe fn local_name_unchecked(&self) -> Option<&str>;
    unsafe fn id_unchecked(&self) -> Option<&str>;
    unsafe fn name_unchecked(&self) -> Option<&str>;
    unsafe fn class_name_unchecked(&self) -> Option<&str>;
    unsafe fn attribute_unchecked(&self, qualified_name: &str) -> Option<&str>;
    unsafe fn attribute_names_unchecked(&self) -> Vec<&str>;

    fn tag_name(&self) -> Option<String>;
    fn local_name(&self) -> Option<String>;
    fn id(&self) -> Option<String>;
    fn set_id(&mut self, id: &str);
    fn class_name(&self) -> Option<String>;
    fn set_class_name(&mut self, class_name: &str);
    fn class_list(&self) -> DOMTokenList<'_>;
    fn class_list_mut(&mut self) -> DOMTokenListMut<'_>;

    fn attribute(&self, qualified_name: &str) -> Option<String>;
    fn attribute_or(&self, qualified_name: &str, default: &str) -> String;
    fn attribute_or_default(&self, qualified_name: &str) -> String;
    fn attribute_node(&self, qualified_name: &str) -> Option<AttrNode>;
    fn attribute_names(&self) -> Vec<String>;
    fn attributes(&self) -> NamedNodeMap;
    fn set_attribute(&mut self, qualified_name: &str, value: &str);
    fn set_attribute_node(&mut self, attribute: &AttrNode);
    fn remove_attribute(&mut self, qualified_name: &str);
    fn toggle_attribute(&mut self, qualified_name: &str, force: Option<bool>) -> bool;
    fn has_attribute(&self, qualified_name: &str) -> bool;

    fn closest(&self, selectors: &str) -> Result<Option<ElementNode>, CSSParserError>;
    fn matches(&self, selectors: &str) -> Result<bool, CSSParserError>;
    fn get_elements_by_tag_name(&self, name: &str) -> HTMLCollection;
    fn get_elements_by_class_name(&self, class_names: &str) -> HTMLCollection;
    fn get_elements_by_attr(&self, name: &str, value: &str) -> HTMLCollection;
    fn get_elements_by_attr_case(&self, name: &str, value: &str, case_insensitive: bool) -> HTMLCollection;

    fn inner_html(&self) -> String;
    fn set_inner_html(&mut self, html: &str);
    fn outer_html(&self) -> String;
    fn set_outer_html(&mut self, html: &str);
    fn inner_text(&self) -> String;
    fn set_inner_text(&mut self, text: &str);
    fn outer_text(&self) -> String;
    fn set_outer_text(&mut self, text: &str);
}

pub trait Attr: NodeInterface {
    unsafe fn name_unchecked(&self) -> Option<&str>;
    unsafe fn local_name_unchecked(&self) -> Option<&str>;
    unsafe fn value_unchecked(&self) -> Option<&str>;

    fn local_name(&self) -> Option<String>;
    fn name(&self) -> Option<String>;
    fn value(&self) -> Option<String>;
    fn set_value(&mut self, value: &str);

    fn owner_element(&self) -> Option<ElementNode>;
}

/// Convert Unicode scalar indices to UTF-8 byte boundaries. Counts past the
/// end are clamped; offsets past the end are rejected, including empty edits.
fn character_data_range(data: &str, offset: usize, count: usize) -> Result<Range<usize>, DOMError> {
    let mut boundaries = data
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(data.len()));
    let start = boundaries.nth(offset).ok_or_else(|| DOMError {
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
    fn len(&self) -> usize {
        self.node_value().unwrap_or_default().chars().count()
    }

    #[inline]
    fn data(&self) -> Option<String> {
        self.node_value()
    }

    #[inline]
    fn set_data(&mut self, data: &str) {
        check_node!(self);
        unsafe {
            self.tree_().mark_mutated();
            // The generic text-content setter does not handle CDATA.
            lxb_dom_character_data_replace(self.node_ptr_().cast(), data.as_ptr(), data.len(), 0, 0);
        }
    }

    fn substring_data(&self, offset: usize, count: usize) -> Result<String, DOMError> {
        let data = self.data().ok_or_else(|| DOMError {
            msg: "Character data is unavailable".into(),
        })?;
        Ok(data[character_data_range(&data, offset, count)?].to_owned())
    }

    fn append_data(&mut self, data: &str) {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        if data.is_empty() {
            return;
        }
        let mut value = self.data().unwrap_or_default();
        value.push_str(data);
        self.set_data(&value);
    }

    fn insert_data(&mut self, offset: usize, data: &str) -> Result<(), DOMError> {
        self.replace_data(offset, 0, data)
    }

    fn delete_data(&mut self, offset: usize, count: usize) -> Result<(), DOMError> {
        self.replace_data(offset, count, "")
    }

    fn replace_data(&mut self, offset: usize, count: usize, data: &str) -> Result<(), DOMError> {
        let document = self.tree_();
        let _guard = document.doc_ptr();
        let mut value = self.data().ok_or_else(|| DOMError {
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

pub trait Text: CharacterData {}

pub trait CdataSection: CharacterData {}

pub trait ProcessingInstruction: CharacterData {
    fn target(&self) -> Option<String>;
}

pub trait Comment: CharacterData {}

pub trait Notation: NodeInterface {
    unsafe fn public_id_unchecked(&self) -> Option<&str>;
    unsafe fn system_id_unchecked(&self) -> Option<&str>;
    fn public_id(&self) -> Option<String>;
    fn system_id(&self) -> Option<String>;
}
