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

//! Node and element collections.
//!
//! `NodeList<T>` stores snapshot membership in a vector. `query_selector_all`
//! returns an element snapshot; `child_nodes`, `children`, and `get_elements_by_*`
//! return live queries. Call [`LiveCollection::snapshot`] to evaluate a live query
//! once, then borrow its items or consume them without cloning. Attribute access
//! uses [`NamedNodeMap`], which also supports explicit snapshots.
//!
//! Snapshot `item` returns `Option<&T>`; use `.cloned()` when an owned handle is
//! needed. `iter()` and `&list` borrow items, while `into_iter()` moves them.
//! Collection-level selector helpers return snapshots of their combined results.

use std::collections::HashSet;
use std::fmt::{Debug, Display, Formatter};
use std::vec;

use crate::parse::html::css::{CSSParserError, Selector};
use crate::parse::html::dom::node::{AttrNode, ElementNode, Node, NodeRef};
use crate::parse::html::dom::traits::{Element, NodeInterface, NodeInterfaceBaseImpl};

/// A snapshot of collection membership. Nodes themselves remain shared DOM handles.
/// Length, indexing and borrowed iteration never clone items or re-run a query.
#[derive(Clone, Debug)]
pub struct NodeList<T = Node> {
    items: Vec<T>,
}

impl<T> Default for NodeList<T> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<T: Clone> From<&[T]> for NodeList<T> {
    fn from(items: &[T]) -> Self {
        Self { items: items.to_vec() }
    }
}

impl<T> From<Vec<T>> for NodeList<T> {
    fn from(items: Vec<T>) -> Self {
        Self { items }
    }
}

impl<T> NodeList<T> {
    pub fn item(&self, index: usize) -> Option<&T> {
        self.items.get(index)
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.items.iter()
    }
    pub fn as_slice(&self) -> &[T] {
        &self.items
    }
    pub fn into_vec(self) -> Vec<T> {
        self.items
    }
    /// Clone the snapshot's items into a new vector.
    pub fn values(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.items.clone()
    }
}

impl<T> std::ops::Index<usize> for NodeList<T> {
    type Output = T;
    fn index(&self, index: usize) -> &T {
        &self.items[index]
    }
}

impl<'a, T> IntoIterator for &'a NodeList<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T> IntoIterator for NodeList<T> {
    type Item = T;
    type IntoIter = vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<T: Display> Display for NodeList<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.iter().map(|n| Display::fmt(n, f)).collect::<Result<_, _>>()
    }
}

/// A retained DOM query. Each access evaluates the query and allocates its results.
/// Use `snapshot()` once when making several accesses to the same results.
pub struct LiveCollection<T> {
    node: Node,
    data: Option<Box<[String]>>,
    query: fn(&Node, Option<&Box<[String]>>) -> Vec<T>,
}

impl<T> LiveCollection<T> {
    pub(crate) fn new_live(
        node: &NodeRef,
        data: Option<Box<[String]>>,
        query: fn(&Node, Option<&Box<[String]>>) -> Vec<T>,
    ) -> Self {
        Self {
            node: node.as_node(),
            data,
            query,
        }
    }
    /// Evaluate the query once and freeze its membership, without cloning results.
    pub fn snapshot(&self) -> NodeList<T> {
        let document = self.node.tree_();
        let _guard = document.doc_ptr();
        if self.node.node_ptr_().is_null() {
            return NodeList::default();
        }
        (self.query)(&self.node, self.data.as_ref()).into()
    }
    pub fn item(&self, index: usize) -> Option<T> {
        self.iter().nth(index)
    }
    pub fn values(&self) -> Vec<T> {
        self.snapshot().into_vec()
    }
    pub fn len(&self) -> usize {
        self.snapshot().len()
    }
    pub fn is_empty(&self) -> bool {
        self.snapshot().is_empty()
    }
    /// Evaluate once; the iterator owns that snapshot's items.
    pub fn iter(&self) -> vec::IntoIter<T> {
        self.snapshot().into_iter()
    }
}

impl<T> IntoIterator for &LiveCollection<T> {
    type Item = T;
    type IntoIter = vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T: Debug> Debug for LiveCollection<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.snapshot(), f)
    }
}

impl<T: Display> Display for LiveCollection<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.snapshot(), f)
    }
}

pub type ElementNodeList = NodeList<ElementNode>;
pub type HTMLCollection = LiveCollection<ElementNode>;

/// A live attribute map tied to its owning element.
/// Name lookup and mutation use the element's attribute API directly.
pub struct NamedNodeMap {
    element: ElementNode,
    attributes: LiveCollection<AttrNode>,
}

impl NamedNodeMap {
    pub(crate) fn new(element: ElementNode, attributes: LiveCollection<AttrNode>) -> Self {
        Self { element, attributes }
    }
    pub fn get_named_item(&self, name: &str) -> Option<AttrNode> {
        self.element.attribute_node(name)
    }
    pub fn set(&self, name: &str, value: &str) {
        self.element.set_attribute(name, value);
    }
    pub fn remove(&self, name: &str) {
        self.element.remove_attribute(name);
    }
    pub fn snapshot(&self) -> NodeList<AttrNode> {
        self.attributes.snapshot()
    }
    pub fn item(&self, index: usize) -> Option<AttrNode> {
        self.attributes.item(index)
    }
    pub fn values(&self) -> Vec<AttrNode> {
        self.attributes.values()
    }
    pub fn len(&self) -> usize {
        self.attributes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.attributes.is_empty()
    }
    pub fn iter(&self) -> vec::IntoIter<AttrNode> {
        self.attributes.iter()
    }
}

impl IntoIterator for &NamedNodeMap {
    type Item = AttrNode;
    type IntoIter = vec::IntoIter<AttrNode>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl Debug for NamedNodeMap {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.snapshot(), f)
    }
}

impl Display for NamedNodeMap {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.snapshot(), f)
    }
}

impl HTMLCollection {
    pub fn named_item(&self, name: &str) -> Option<ElementNode> {
        self.iter()
            .find(|e| e.id().as_deref() == Some(name) || e.attribute("name").as_deref() == Some(name))
    }
}

impl ElementNodeList {
    pub fn named_item(&self, name: &str) -> Option<&ElementNode> {
        self.iter()
            .find(|e| e.id().filter(|i| i == name).is_some() || e.attribute("name").filter(|n| n == name).is_some())
    }

    pub fn elements_by_tag_name(&self, qualified_name: &str) -> ElementNodeList {
        let mut coll = Vec::default();
        self.iter()
            .for_each(|e| coll.extend(e.get_elements_by_tag_name(qualified_name).snapshot()));
        ElementNodeList::from(coll)
    }

    pub fn elements_by_class_name(&self, class_names: &str) -> ElementNodeList {
        let mut coll = Vec::default();
        self.iter()
            .for_each(|e| coll.extend(e.get_elements_by_class_name(class_names).snapshot()));
        ElementNodeList::from(coll)
    }

    #[inline(always)]
    pub fn elements_by_attr(&self, qualified_name: &str, value: &str) -> ElementNodeList {
        self.elements_by_attr_case(qualified_name, value, false)
    }

    pub fn elements_by_attr_case(&self, qualified_name: &str, value: &str, case_insensitive: bool) -> ElementNodeList {
        let mut coll = Vec::default();
        self.iter().for_each(|e| {
            coll.extend(
                e.get_elements_by_attr_case(qualified_name, value, case_insensitive)
                    .snapshot(),
            )
        });
        ElementNodeList::from(coll)
    }

    #[inline(always)]
    pub fn query_selector(&self, selectors: &str) -> Result<Option<ElementNode>, CSSParserError> {
        self.query_selector_compiled(&Selector::new(selectors)?)
    }

    pub fn query_selector_compiled(&self, selectors: &Selector) -> Result<Option<ElementNode>, CSSParserError> {
        for root in self {
            if let Some(element) = selectors.query_selector(&root.as_noderef())? {
                return Ok(Some(element));
            }
        }
        Ok(None)
    }

    #[inline(always)]
    pub fn query_selector_all(&self, selectors: &str) -> Result<ElementNodeList, CSSParserError> {
        self.query_selector_all_compiled(&Selector::new(selectors)?)
    }

    pub fn query_selector_all_compiled(&self, selectors: &Selector) -> Result<ElementNodeList, CSSParserError> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        for root in self {
            for element in selectors.query_selector_all(&root.as_noderef())? {
                if seen.insert(element.node_ptr_()) {
                    result.push(element);
                }
            }
        }
        Ok(result.into())
    }

    #[inline(always)]
    pub fn matches(&self, selectors: &str) -> Result<bool, CSSParserError> {
        self.matches_compiled(&Selector::new(selectors)?)
    }

    pub fn matches_compiled(&self, selectors: &Selector) -> Result<bool, CSSParserError> {
        for element in self {
            if selectors.matches(element)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

// ---------------------------------------- DOMTokenList impl --------------------------------------

pub trait DOMTokenListInterface: IntoIterator + PartialEq + Debug + Display + PartialEq {
    fn value(&self) -> String;

    fn values(&self) -> Vec<String> {
        let mut h = HashSet::new();
        self.value()
            .split_ascii_whitespace()
            .filter(|&v| h.insert(v))
            .map(String::from)
            .collect()
    }

    fn item(&self, index: usize) -> Option<String> {
        Some(self.values().get(index)?.to_owned())
    }

    fn contains(&self, token: &str) -> bool {
        self.iter().any(|s| s == token)
    }

    #[inline]
    fn iter(&self) -> vec::IntoIter<String> {
        self.values().into_iter()
    }

    #[inline]
    fn len(&self) -> usize {
        self.values().len()
    }
}

pub(crate) trait DOMTokenListBaseImpl {
    fn element_(&self) -> &ElementNode;
}

#[allow(private_bounds)]
pub trait DOMTokenListMutInterface: DOMTokenListInterface + DOMTokenListBaseImpl {
    fn set_value(&self, value: &str);

    fn add(&self, tokens: &[&str]) {
        let document = self.element_().tree_();
        let _guard = document.doc_ptr();
        let mut values = self.values();
        tokens.iter().for_each(|t: &&str| {
            let t_owned = (*t).to_owned();
            if !values.contains(&t_owned) {
                values.push(t_owned);
            }
        });
        self.set_value(&values.join(" "));
    }

    fn remove(&self, tokens: &[&str]) {
        let document = self.element_().tree_();
        let _guard = document.doc_ptr();
        self.set_value(
            &self
                .iter()
                .filter(|t: &String| !tokens.contains(&t.as_str()))
                .collect::<Vec<_>>()
                .join(" "),
        );
    }

    fn replace(&self, old_token: &str, new_token: &str) -> bool {
        let document = self.element_().tree_();
        let _guard = document.doc_ptr();
        let mut repl = false;
        self.set_value(
            &self
                .iter()
                .map(|t: String| {
                    if t == old_token {
                        repl = true;
                        new_token.to_owned()
                    } else {
                        t
                    }
                })
                .collect::<Vec<_>>()
                .join(" "),
        );
        repl
    }

    fn toggle(&self, token: &str, force: Option<bool>) -> bool {
        let document = self.element_().tree_();
        let _guard = document.doc_ptr();
        if let Some(f) = force {
            if f {
                self.add(&[token]);
            } else {
                self.remove(&[token])
            }
            return f;
        }

        if self.contains(token) {
            self.remove(&[token]);
            false
        } else {
            self.add(&[token]);
            true
        }
    }
}

// Cannot derive default implementations for sub traits (yet)
macro_rules! dom_token_list_impl {
    ($Self: ty) => {
        impl DOMTokenListBaseImpl for $Self {
            fn element_(&self) -> &ElementNode {
                &self.element
            }
        }

        impl DOMTokenListMutInterface for $Self {
            fn set_value(&self, value: &str) {
                self.element.set_class_name(value);
            }
        }

        impl DOMTokenListInterface for $Self {
            #[inline]
            fn value(&self) -> String {
                self.element.class_name().unwrap_or_default()
            }
        }

        impl IntoIterator for $Self {
            type Item = String;
            type IntoIter = vec::IntoIter<String>;

            fn into_iter(self) -> Self::IntoIter {
                self.iter()
            }
        }

        impl PartialEq<&[&str]> for $Self {
            fn eq(&self, other: &&[&str]) -> bool {
                let val = self.values();
                if other.len() != val.len() {
                    return false;
                }
                val.iter().zip(other.iter()).find(|(a, b)| a != *b).is_none()
            }
        }

        impl Debug for $Self {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                write!(f, "[")?;
                for (i, s) in self.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    f.write_str(&format!("{:?}", s))?;
                }
                write!(f, "]")
            }
        }

        impl Display for $Self {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                Debug::fmt(self, f)
            }
        }
    };
}

#[derive(PartialEq)]
pub struct DOMTokenList<'a> {
    element: &'a ElementNode,
}

impl<'a> DOMTokenList<'a> {
    pub(crate) fn new(element: &'a ElementNode) -> Self {
        Self { element }
    }
}

dom_token_list_impl!(DOMTokenList<'_>);

/// Compatibility name for a shared token-list view; mutation is document-locked.
pub type DOMTokenListMut<'a> = DOMTokenList<'a>;

#[derive(PartialEq)]
pub struct DOMTokenListOwned {
    element: ElementNode,
}

impl DOMTokenListOwned {
    pub(crate) fn new(element: ElementNode) -> Self {
        Self { element }
    }
}

dom_token_list_impl!(DOMTokenListOwned);

impl From<DOMTokenList<'_>> for DOMTokenListOwned {
    fn from(value: DOMTokenList<'_>) -> Self {
        Self::new(value.element.clone())
    }
}
