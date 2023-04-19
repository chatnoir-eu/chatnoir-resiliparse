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

use std::cmp::max;
use std::fmt::{Debug, Display};
use std::ops::Add;

use crate::parse::html::css::{CSSParserError, CSSSelectorList, TraverseAction};
use crate::parse::html::dom::coll::*;
use crate::parse::html::dom::*;
use crate::parse::html::dom::iter::{ElementIterator, NodeIterator};
use crate::parse::html::dom::node_base::NodeBase;

/// Base DOM node interface.
pub trait NodeInterface: Debug + Display {
    unsafe fn node_name_unchecked(&self) -> Option<&str>;
    unsafe fn node_value_unchecked(&self) -> Option<&str>;

    fn upcast(&self) -> &NodeBase;
    fn upcast_mut(&mut self) -> &mut NodeBase;
    fn as_noderef(&self) -> NodeRef;
    fn to_node(&self) -> Node;

    fn node_name(&self) -> Option<String>;
    fn node_value(&self) -> Option<String>;
    fn set_node_value(&mut self, value: &str);
    fn text_content(&self) -> Option<String>;
    fn set_text_content(&mut self, content: &str);

    fn owner_document(&self) -> Option<DocumentNode>;
    fn parent_node(&self) -> Option<Node>;
    fn parent_element(&self) -> Option<ElementNode>;

    fn has_child_nodes(&self) -> bool;
    fn contains(&self, node: &Node) -> bool;
    fn child_nodes(&self) -> NodeList;
    fn first_child(&self) -> Option<Node>;
    fn last_child(&self) -> Option<Node>;
    fn previous_sibling(&self) -> Option<Node>;
    fn next_sibling(&self) -> Option<Node>;
    fn clone_node(&self, deep: bool) -> Option<Node>;

    fn insert_before<'a>(&mut self, node: &'a Node, child: Option<&'a Node>) -> Option<&'a Node>;
    fn append_child<'a>(&mut self, node: &'a Node) -> Option<&'a Node>;
    fn replace_child<'a>(&mut self, node: &'a Node, child: &'a Node) -> Option<&'a Node>;
    fn remove_child<'a>(&mut self, node: &'a Node) -> Option<&'a Node>;

    fn iter(&self) -> NodeIterator;
    fn iter_elements(&self) -> ElementIterator;
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
    fn document_element(&self) -> Option<DocumentNode>;

    fn elements_by_tag_name(&self, qualified_name: &str) -> HTMLCollection;
    fn elements_by_class_name(&self, qualified_name: &str) -> HTMLCollection;
    fn elements_by_attr(&self, qualified_name: &str, value: &str) -> HTMLCollection;

    fn create_element(&mut self, local_name: &str) -> Result<ElementNode, DOMError>;
    fn create_document_fragment(&mut self) -> Result<DocumentFragmentNode, DOMError>;
    fn create_text_node(&mut self, data: &str) -> Result<TextNode, DOMError>;
    fn create_cdata_section(&mut self, data: &str) -> Result<CDataSectionNode, DOMError>;
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
        let sel_list = CSSSelectorList::parse_selectors(&self.upcast().tree, selectors)?;
        let mut result = Vec::<ElementNode>::with_capacity(1);
        sel_list.match_elements(self.as_noderef(), |e, _, ctx| {
            ctx.push(e);
            TraverseAction::Stop
        }, &mut result);
        Ok(result.pop())
    }

    fn query_selector_all(&self, selectors: &str) -> Result<ElementNodeList, CSSParserError> {
        let sel_list = CSSSelectorList::parse_selectors(&self.upcast().tree, selectors)?;
        let mut result = Vec::<ElementNode>::new();
        sel_list.match_elements(self.as_noderef(), |e, _, ctx| {
            ctx.push(e);
            TraverseAction::Ok
        }, &mut result);
        Ok(ElementNodeList::from(result))
    }
}

/// NonElementParentNode mixin trait.
pub trait NonElementParentNode: NodeInterface {
    fn element_by_id(&self, element_id: &str) -> Option<ElementNode> {
        unsafe { element_by_id(self.upcast(), element_id) }
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
        let node = self.upcast_mut();
        check_node!(node);
        unsafe { lxb_dom_node_remove(node.node); }
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
    fn elements_by_tag_name(&self, qualified_name: &str) -> HTMLCollection;
    fn elements_by_class_name(&self, class_names: &str) -> HTMLCollection;
    fn elements_by_attr(&self, qualified_name: &str, value: &str) -> HTMLCollection;

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

    fn owner_element(&self) -> Option<Node>;
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

pub trait CDataSection: CharacterData {}

pub trait ProcessingInstruction: CharacterData {
    fn target(&self) -> Option<String>;
}

pub trait Comment: CharacterData {}
