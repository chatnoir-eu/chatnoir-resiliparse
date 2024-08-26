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

use std::cmp::max;
use std::fmt::{Debug, Display};
use std::ops::Add;
use std::ptr;
use std::ptr::addr_of;
use crate::parse::html::css::{CSSParserError, CSSSelectorList, TraverseAction};
use crate::parse::html::dom::*;
use crate::parse::html::dom::coll::*;
use crate::parse::html::dom::iter::*;
use crate::parse::html::lexbor::{str_from_lxb_char_t, str_from_lxb_str_cb, str_from_lxb_str_t};


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
    Notation = 0x0C,    // legacy
}

pub(crate) trait NodeInterfaceBaseImpl {
    fn new(tree: &Arc<HTMLDocument>, node: *mut lxb_dom_node_t) -> Option<Self> where Self: Sized;
    fn tree_(&self) -> Arc<HTMLDocument>;
    fn node_ptr_(&self) -> *mut lxb_dom_node_t;
    fn reset_node_ptr_(&mut self);

    #[inline(always)]
    unsafe fn doc_ptr_unchecked(&self) -> *mut lxb_dom_document_t {
        (*self.node_ptr_()).owner_document
    }

    unsafe fn iter_raw(&self) -> NodeIteratorRaw {
        NodeIteratorRaw::new(self.node_ptr_())
    }

    unsafe fn node_name_unchecked(&self) -> Option<&str> {
        str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_node_name)
    }

    /// Node text value.
    unsafe fn node_value_unchecked(&self) -> Option<&str> {
        use crate::third_party::lexbor::lxb_dom_node_type_t::*;
        match (*self.node_ptr_()).type_ {
            LXB_DOM_NODE_TYPE_ATTRIBUTE => str_from_lxb_str_cb(self.node_ptr_(), lxb_dom_attr_value_noi),
            LXB_DOM_NODE_TYPE_TEXT | LXB_DOM_NODE_TYPE_CDATA_SECTION |
            LXB_DOM_NODE_TYPE_COMMENT | LXB_DOM_NODE_TYPE_PROCESSING_INSTRUCTION =>
                str_from_lxb_str_t(addr_of!((*(self.node_ptr_() as *const lxb_dom_character_data_t)).data)),
            _ => None
        }
    }

    unsafe fn can_have_children_unchecked(&self) -> bool {
        matches!((*self.node_ptr_()).type_,
            lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_ELEMENT
                | lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_DOCUMENT
                | lxb_dom_node_type_t::LXB_DOM_NODE_TYPE_DOCUMENT_FRAGMENT
        )
    }

    unsafe fn insert_before_unchecked<'a>(&mut self, node: &'a Node, child: Option<&Node>) -> Option<&'a Node> {
        if let Some(c) = child {
            if c.parent_node()?.node_ptr_() != self.node_ptr_() || !self.can_have_children_unchecked() ||  node.contains(c) {
                return None;
            }
            if node == c {
                return Some(node);
            }
            // TODO: Insert fragment itself once Lexbor bug is fixed: https://github.com/lexbor/lexbor/issues/180
            if let Node::DocumentFragment(d) = node {
                d.child_nodes().iter().for_each(|c2| {
                    lxb_dom_node_insert_before(c.node_ptr_(), c2.node_ptr_());
                });
                // Lexbor doesn't reset the child pointers upon moving elements from DocumentFragments
                (*d.node_ptr_()).first_child = ptr::null_mut();
                (*d.node_ptr_()).last_child = ptr::null_mut();
            } else {
                lxb_dom_node_insert_before(c.node_ptr_(), node.node_ptr_());
            }
            Some(node)
        } else {
            self.append_child_unchecked(node)
        }
    }

    unsafe fn append_child_unchecked<'a>(&mut self, node: &'a Node) -> Option<&'a Node> {
        if !self.can_have_children_unchecked() {
            return None;
        }
        // TODO: Insert fragment itself once Lexbor bug is fixed: https://github.com/lexbor/lexbor/issues/180
        if let Node::DocumentFragment(d) = node {
            d.child_nodes().iter().for_each(|c| {
                lxb_dom_node_insert_child(self.node_ptr_(), c.node_ptr_());
            });
            // Lexbor doesn't reset the child pointers upon moving elements from DocumentFragments
            (*d.node_ptr_()).first_child = ptr::null_mut();
            (*d.node_ptr_()).last_child = ptr::null_mut();
        } else {
            lxb_dom_node_insert_child(self.node_ptr_(), node.node_ptr_());
        }
        Some(node)
    }

    unsafe fn replace_child_unchecked<'a>(&mut self, new_child: &'a Node, old_child: &'a Node) -> Option<&'a Node> {
        if old_child.parent_node()?.node_ptr_() != self.node_ptr_() || !self.can_have_children_unchecked() {
            return None;
        }
        if new_child == old_child {
            return Some(old_child);
        }
        self.insert_before_unchecked(new_child, Some(old_child))?;
        self.remove_child_unchecked(old_child)
    }

    unsafe fn remove_child_unchecked<'a>(&mut self, node: &'a Node) -> Option<&'a Node> {
        if node.parent_node()?.node_ptr_() != self.node_ptr_() || !self.can_have_children_unchecked() {
            return None;
        }
        lxb_dom_node_remove(node.node_ptr_());
        Some(node)
    }
}


/// Base DOM node interface.
#[allow(private_bounds)]
pub trait NodeInterface: NodeInterfaceBaseImpl + Debug + Display {
    fn node_type(&self) -> Option<NodeType> {
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
            _ => None
        }
    }

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
        unsafe { lxb_dom_node_text_content_set(self.node_ptr_(), value.as_ptr(), value.len()); }
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
            Some(wrap_raw_node(&self.tree_(), d.cast())?.into())
        } else {
            None
        }
    }

    /// Parent of this node.
    fn parent_node(&self) -> Option<Node> {
        wrap_raw_node(&self.tree_(), unsafe { self.node_ptr_().as_ref()?.parent })
    }

    #[inline]
    fn parent_element(&self) -> Option<ElementNode> {
        if let Node::Element(n) = self.parent_node()? {
            Some(n)
        } else {
            None
        }
    }

    #[inline]
    fn has_child_nodes(&self) -> bool {
        self.first_child().is_some()
    }

    fn contains(&self, node: &Node) -> bool {
        check_nodes!(self, node);
        if self.node_ptr_() == node.node_ptr_() {
            return true;
        }
        unsafe {
            self.iter_raw()
                .find(|&n| n == node.node_ptr_())
                .is_some()
        }
    }

    /// List of child nodes.
    fn child_nodes(&self) -> NodeList {
        NodeList::new_live(&wrap_raw_node(&self.tree_(), self.node_ptr_()).unwrap(), None, |n, _| {
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
        wrap_raw_node(&self.tree_(), unsafe { self.node_ptr_().as_ref()?.first_child })
    }

    /// Last child element of this DOM node.
    fn last_child(&self) -> Option<Node> {
        wrap_raw_node(&self.tree_(), unsafe { self.node_ptr_().as_ref()?.last_child })
    }

    /// Previous sibling node.
    fn previous_sibling(&self) -> Option<Node> {
        wrap_raw_node(&self.tree_(), unsafe { self.node_ptr_().as_ref()?.prev })
    }

    /// Next sibling node.
    fn next_sibling(&self) -> Option<Node> {
        wrap_raw_node(&self.tree_(), unsafe { self.node_ptr_().as_ref()?.next })
    }

    fn clone_node(&self, deep: bool) -> Option<Node> {
        check_node!(self);
        wrap_raw_node(&self.tree_(), unsafe { lxb_dom_node_clone(self.node_ptr_(), deep) })
    }

    fn insert_before<'a>(&mut self, node: &'a Node, child: Option<&'a Node>) -> Option<&'a Node> {
        check_nodes!(self, node);
        if child.is_some() {
            check_nodes!(node, child?);
            if self.node_ptr_() == child?.node_ptr_() {
                return None;
            }
        }
        unsafe { self.insert_before_unchecked(node, child) }
    }

    fn append_child<'a>(&mut self, node: &'a Node) -> Option<&'a Node> {
        check_nodes!(self, node);
        unsafe { self.append_child_unchecked(node) }
    }

    fn replace_child<'a>(&mut self, node: &'a Node, child: &'a Node) -> Option<&'a Node> {
        check_nodes!(self, node);
        check_nodes!(node, child);
        unsafe { self.replace_child_unchecked(node, child) }
    }

    fn remove_child<'a>(&mut self, node: &'a Node) -> Option<&'a Node> {
        check_nodes!(self, node);
        unsafe { self.remove_child_unchecked(node) }
    }

    fn iter(&self) -> NodeIterator where Self: Sized {
        NodeIterator::new(self)
    }

    fn iter_elements(&self) -> ElementIterator where Self: Sized {
        ElementIterator::new(self)
    }

    fn decompose(&mut self) {
        use crate::third_party::lexbor::lxb_dom_node_type_t::*;
        unsafe {
            if !self.node_ptr_().is_null() && (*self.node_ptr_()).parent.is_null() && (*self.node_ptr_()).type_ != LXB_DOM_NODE_TYPE_DOCUMENT {
                lxb_dom_node_destroy_deep(self.node_ptr_());
                self.reset_node_ptr_();
            }
        }
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
    fn create_processing_instruction(&mut self, target: &str, data: &str) -> Result<ProcessingInstructionNode, DOMError>;
    fn create_attribute(&mut self, local_name: &str) -> Result<AttrNode, DOMError>;
}

pub trait DocumentFragment: DocumentOrShadowRoot + ParentNode + NonElementParentNode {}

/// ParentNode mixin trait.
pub trait ParentNode: NodeInterface {

    /// List of child element nodes.
    fn children(&self) -> HTMLCollection;

    /// First element child of this DOM node.
    fn first_element_child(&self) -> Option<ElementNode> {
        let mut child = self.first_child()?;
        loop {
            match child {
                Node::Element(c) => return Some(c),
                _ => { child = child.next_sibling()? }
            }
        }
    }

    /// Last element child element of this DOM node.
    fn last_element_child(&self) -> Option<ElementNode> {
        let mut child = self.last_child()?;
        loop {
            if let Node::Element(c) = child {
                return Some(c);
            }
            child = child.previous_sibling()?;
        }
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

    fn prepend(&mut self, nodes: &[&Node]) {
        let fc = self.first_child();
        nodes.iter().rev().for_each(|&n| {
            self.insert_before(n, fc.as_ref());
        });
    }

    fn append(&mut self, nodes: &[&Node]) {
        nodes.iter().for_each(|&n| {
            self.append_child(n);
        });
    }

    fn replace_children(&mut self, nodes: &[&Node]) {
        while let Some(c) = self.first_child() {
            self.remove_child(&c);
        }
        self.append(nodes);
    }

    fn query_selector(&self, selectors: &str) -> Result<Option<ElementNode>, CSSParserError> {
        let sel_list = CSSSelectorList::parse_selectors(&self.tree_(), selectors)?;
        let mut result = Vec::<ElementNode>::with_capacity(1);
        sel_list.match_elements(&self.as_node(), |e, _, ctx| {
            ctx.push(e);
            TraverseAction::Stop
        }, &mut result);
        Ok(result.pop())
    }

    fn query_selector_all(&self, selectors: &str) -> Result<ElementNodeList, CSSParserError> {
        let sel_list = CSSSelectorList::parse_selectors(&self.tree_(), selectors)?;
        let mut result = Vec::<ElementNode>::new();
        sel_list.match_elements(&self.as_node(), |e, _, ctx| {
            ctx.push(e);
            TraverseAction::Ok
        }, &mut result);
        Ok(ElementNodeList::from(result))
    }
}

/// NonElementParentNode mixin trait.
pub trait NonElementParentNode: NodeInterface {
    fn get_element_by_id(&self, id: &str) -> Option<ElementNode> {
        unsafe { get_element_by_id(&self.as_node(), id, false) }
    }
    
    fn get_element_by_id_case(&self, id: &str, case_insensitive: bool) -> Option<ElementNode> {
        unsafe { get_element_by_id(&self.as_node(), id, case_insensitive) }
    }
}

/// ChildNode mixin trait.
pub trait ChildNode: NodeInterface {
    fn before(&mut self, nodes: &[&mut Node]) {
        if let Some(p) = &mut self.parent_node() {
            let anchor = self.parent_node();
            nodes.iter().for_each(|n| {
                p.insert_before(n, anchor.as_ref());
            });
        }
    }

    fn after(&mut self, nodes: &[&mut Node]) {
        if let Some(p) = &mut self.parent_node() {
            let anchor = self.next_sibling();
            nodes.iter().for_each(|n| {
                p.insert_before(n, anchor.as_ref());
            });
        }
    }

    fn replace_with(&mut self, nodes: &[&mut Node]) {
        self.before(nodes);
        self.remove();
    }

    fn remove(&mut self) {
        check_node!(self);
        unsafe { lxb_dom_node_remove(self.node_ptr_()); }
    }
}

/// NonDocumentTypeChildNode mixin trait.
pub trait NonDocumentTypeChildNode: NodeInterface {
    /// Previous sibling element node.
    fn previous_element_sibling(&self) -> Option<ElementNode> {
        let mut p = self.previous_sibling()?;
        loop {
            if let Node::Element(s) = p {
                return Some(s);
            }
            p = p.previous_sibling()?;
        }
    }

    /// Next sibling element node.
    fn next_element_sibling(&self) -> Option<ElementNode> {
        let mut p = self.next_sibling()?;
        loop {
            if let Node::Element(s) = p {
                return Some(s);
            }
            p = p.next_sibling()?;
        }
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
    fn class_list(&self) -> DOMTokenList;
    fn class_list_mut(&mut self) -> DOMTokenListMut;

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

pub trait CharacterData: NodeInterface + ChildNode + NonDocumentTypeChildNode {
    fn len(&self) -> usize {
        self.node_value().unwrap_or_default().len()
    }

    #[inline]
    fn data(&self) -> Option<String> {
        self.node_value()
    }

    #[inline]
    fn set_data(&mut self, data: &str) {
        self.set_node_value(data);
    }

    fn substring_data(&self, offset: usize, count: usize) -> Option<String> {
        Some(self.data()?.chars().into_iter().skip(offset).take(count).collect())
    }

    fn append_data(&mut self, data: &str) {
        if data.is_empty() {
            return;
        }
        self.set_data(&self.data().unwrap_or_default().add(data));
    }

    fn insert_data(&mut self, offset: usize, data: &str) {
        if data.is_empty() {
            return;
        }
        if let Some(s) = self.data() {
            let mut s_new = String::with_capacity(s.len() + data.len());
            s.chars().into_iter().enumerate().for_each(|(i, c)| {
                if i == offset {
                    s_new.push_str(data);
                }
                s_new.push(c);
            });
            self.set_data(&s_new);
        }
    }

    fn delete_data(&mut self, offset: usize, count: usize) {
        if let Some(s) = self.data() {
            let mut s_new = String::with_capacity(max(0, s.len() - count));
            s.chars().into_iter().enumerate().for_each(|(i, c)| {
                if i < offset || i >= offset + count {
                    s_new.push(c);
                }
            });
            self.set_data(&s_new);
        }
    }

    fn replace_data(&mut self, offset: usize, count: usize, data: &str) {
        if let Some(s) = self.data() {
            let mut s_new = String::with_capacity(max(0, s.len() - count + data.len()));
            s.chars().into_iter().enumerate().for_each(|(i, c)| {
                if i < offset {
                    s_new.push(c);
                }
                if i == offset {
                    s_new.push_str(data);
                }
                if i >= offset + count {
                    s_new.push(c);
                }
            });
            self.set_data(&s_new);
        }
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
