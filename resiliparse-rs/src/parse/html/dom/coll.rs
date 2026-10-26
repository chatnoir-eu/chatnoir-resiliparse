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

use crate::parse::html::dom::node::{AttrNode, ElementNode, Node, NodeRef};
use crate::parse::html::dom::traits::{Element, NodeInterface, NodeInterfaceBaseImpl};
use crate::parse::html::selector::{CssParserError, CssSelector};

/// A snapshot of collection items.
///
/// Nodes remain shared DOM handles. Length, indexing, and borrowed iteration never clone items or
/// re-run a query.
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
    /// Return the item at the given zero-based index, if present.
    ///
    /// # Arguments
    ///
    /// * `index` - A zero-based item or token index.
    ///
    /// # Returns
    ///
    /// The item at `index`, or `None` if the index is out of bounds.
    pub fn item(&self, index: usize) -> Option<&T> {
        self.items.get(index)
    }

    /// Return the number of items.
    ///
    /// # Returns
    ///
    /// The number of items in the snapshot.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Return whether the collection contains no items.
    ///
    /// # Returns
    ///
    /// True if the list contains no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Iterate over the items in this snapshot without cloning them.
    ///
    /// # Returns
    ///
    /// An iterator over references to the items in this snapshot.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.items.iter()
    }

    /// Borrow the items as a slice.
    ///
    /// # Returns
    ///
    /// A borrowed slice of the snapshot’s items.
    pub fn as_slice(&self) -> &[T] {
        &self.items
    }

    /// Consume the list and return its items.
    ///
    /// # Returns
    ///
    /// The items moved out of this snapshot.
    pub fn into_vec(self) -> Vec<T> {
        self.items
    }

    /// Clone the snapshot's items into a new vector.
    ///
    /// # Returns
    ///
    /// A vector containing clones of the items in this snapshot.
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

/// Retained live view of a selected DOM collection.
///
/// Each access evaluates the retained query and allocates its results.
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
    ///
    /// # Returns
    ///
    /// A snapshot containing the query results at the time of this call.
    pub fn snapshot(&self) -> NodeList<T> {
        let document = self.node.tree_();
        let _guard = document.doc_ptr();
        if self.node.node_ptr_().is_null() {
            return NodeList::default();
        }
        (self.query)(&self.node, self.data.as_ref()).into()
    }
    /// Return the item at the given zero-based index, if present.
    ///
    /// # Arguments
    ///
    /// * `index` - A zero-based index into the collection or token list.
    ///
    /// # Returns
    ///
    /// The item at `index` in the current query results, or `None` if the index is out of bounds.
    pub fn item(&self, index: usize) -> Option<T> {
        self.iter().nth(index)
    }
    /// Return the collection items as a vector.
    ///
    /// # Returns
    ///
    /// A vector containing the current query results.
    pub fn values(&self) -> Vec<T> {
        self.snapshot().into_vec()
    }
    /// Return the number of items.
    ///
    /// # Returns
    ///
    /// The number of items returned by the current query.
    pub fn len(&self) -> usize {
        self.snapshot().len()
    }
    /// Return whether the collection contains no items.
    ///
    /// # Returns
    ///
    /// True if the live query returns no items.
    pub fn is_empty(&self) -> bool {
        self.snapshot().is_empty()
    }
    /// Evaluate once; the iterator owns that snapshot's items.
    ///
    /// # Returns
    ///
    /// An iterator over a snapshot of the current query results.
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

/// A snapshot list of element handles.
pub type ElementNodeList = NodeList<ElementNode>;

/// A live query of element handles.
pub type HTMLCollection = LiveCollection<ElementNode>;

/// A live attribute map tied to its owning element.
///
/// Name lookup and mutation use the element's attribute API directly.
pub struct NamedNodeMap {
    element: ElementNode,
    attributes: LiveCollection<AttrNode>,
}

impl NamedNodeMap {
    pub(crate) fn new(element: ElementNode, attributes: LiveCollection<AttrNode>) -> Self {
        Self { element, attributes }
    }

    /// Return the attribute with this qualified name, if present.
    ///
    /// # Arguments
    ///
    /// * `name` - The qualified attribute name to set.
    ///
    /// # Returns
    ///
    /// The attribute with this qualified name, or `None` if it is absent.
    pub fn get_named_item(&self, name: &str) -> Option<AttrNode> {
        self.element.attribute_node(name)
    }

    /// Set an attribute value, creating the attribute if needed.
    ///
    /// # Arguments
    ///
    /// * `name` - The qualified attribute name to remove.
    /// * `value` - The value to assign or compare.
    ///
    /// # Returns
    ///
    /// No value is returned.
    pub fn set(&self, name: &str, value: &str) {
        self.element.set_attribute(name, value);
    }

    /// Remove the named attribute if it exists.
    ///
    /// # Arguments
    ///
    /// * `name` - The ID or `name` attribute value to find.
    ///
    /// # Returns
    ///
    /// No value is returned.
    pub fn remove(&self, name: &str) {
        self.element.remove_attribute(name);
    }

    /// Evaluate the live query once and return a fixed-membership list.
    ///
    /// # Returns
    ///
    /// A snapshot containing the attributes present at the time of this call.
    pub fn snapshot(&self) -> NodeList<AttrNode> {
        self.attributes.snapshot()
    }

    /// Return the item at the given zero-based index, if present.
    ///
    /// # Arguments
    ///
    /// * `index` - A zero-based index into the collection or token list.
    ///
    /// # Returns
    ///
    /// The attribute at `index` in the current results, or `None` if the index is out of bounds.
    pub fn item(&self, index: usize) -> Option<AttrNode> {
        self.attributes.item(index)
    }

    /// Return the collection items as a vector.
    ///
    /// # Returns
    ///
    /// A vector containing the attributes currently on the element.
    pub fn values(&self) -> Vec<AttrNode> {
        self.attributes.values()
    }

    /// Return the number of items.
    ///
    /// # Returns
    ///
    /// The number of attributes currently on the element.
    pub fn len(&self) -> usize {
        self.attributes.len()
    }

    /// Return whether the collection contains no items.
    ///
    /// # Returns
    ///
    /// True if the element has no attributes.
    pub fn is_empty(&self) -> bool {
        self.attributes.is_empty()
    }

    /// Evaluate the query and iterate over its resulting items.
    ///
    /// # Returns
    ///
    /// An iterator over the attributes currently on the element.
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
    /// Return the first element whose `id` or `name` matches the given value.
    ///
    /// # Arguments
    ///
    /// * `name` - The element ID or `name` value to find.
    ///
    /// # Returns
    ///
    /// The first element whose `id` or `name` matches, or `None` if no element matches.
    pub fn named_item(&self, name: &str) -> Option<ElementNode> {
        self.iter()
            .find(|e| e.id().as_deref() == Some(name) || e.attribute("name").as_deref() == Some(name))
    }
}

impl ElementNodeList {
    /// Return the first element whose `id` or `name` matches the given value.
    ///
    /// # Arguments
    ///
    /// * `name` - The ID or `name` attribute value to find.
    ///
    /// # Returns
    ///
    /// A reference to the first matching element, or `None` if no element matches.
    pub fn named_item(&self, name: &str) -> Option<&ElementNode> {
        self.iter()
            .find(|e| e.id().filter(|i| i == name).is_some() || e.attribute("name").filter(|n| n == name).is_some())
    }

    /// Return descendant elements whose tag name matches.
    ///
    /// # Arguments
    ///
    /// * `qualified_name` - The tag name to match.
    ///
    /// # Returns
    ///
    /// A snapshot of descendant elements with the requested tag name.
    pub fn elements_by_tag_name(&self, qualified_name: &str) -> ElementNodeList {
        let mut coll = Vec::default();
        self.iter()
            .for_each(|e| coll.extend(e.get_elements_by_tag_name(qualified_name).snapshot()));
        ElementNodeList::from(coll)
    }

    /// Return descendant elements containing all requested classes.
    ///
    /// # Arguments
    ///
    /// * `class_names` - The whitespace-separated class names to find.
    ///
    /// # Returns
    ///
    /// A snapshot of descendant elements containing all requested classes.
    pub fn elements_by_class_name(&self, class_names: &str) -> ElementNodeList {
        let mut coll = Vec::default();
        self.iter()
            .for_each(|e| coll.extend(e.get_elements_by_class_name(class_names).snapshot()));
        ElementNodeList::from(coll)
    }

    /// Return descendant elements whose named attribute has the given value.
    ///
    /// # Arguments
    ///
    /// * `qualified_name` - The qualified attribute name to match.
    /// * `value` - The attribute value to match or set.
    ///
    /// # Returns
    ///
    /// A snapshot of descendant elements with the requested attribute value.
    #[inline(always)]
    pub fn elements_by_attr(&self, qualified_name: &str, value: &str) -> ElementNodeList {
        self.elements_by_attr_case(qualified_name, value, false)
    }

    /// Return descendant elements whose named attribute has the given value, with optional case-insensitive comparison.
    ///
    /// # Arguments
    ///
    /// * `qualified_name` - A qualified tag or attribute name.
    /// * `value` - The value to assign or compare.
    /// * `case_insensitive` - Whether name or value comparison ignores character case.
    ///
    /// # Returns
    ///
    /// A snapshot of descendant elements with the requested attribute value.
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

    /// Return the first matching descendant element, if any.
    ///
    /// # Arguments
    ///
    /// * `selectors` - A CSS selector list.
    ///
    /// # Returns
    ///
    /// The first matching descendant element across the roots, or `None` if no element matches.
    #[inline(always)]
    pub fn query_selector(&self, selectors: &str) -> Result<Option<ElementNode>, CssParserError> {
        self.query_selector_compiled(&CssSelector::new(selectors)?)
    }

    /// Query each root using a previously compiled selector list.
    ///
    /// # Arguments
    ///
    /// * `selectors` - The CSS selector list to parse or apply.
    ///
    /// # Returns
    ///
    /// The first matching descendant element across the roots, or `None` if no element matches.
    pub fn query_selector_compiled(&self, selectors: &CssSelector) -> Result<Option<ElementNode>, CssParserError> {
        for root in self {
            if let Some(element) = selectors.query_selector(&root.as_noderef())? {
                return Ok(Some(element));
            }
        }
        Ok(None)
    }

    /// Return all matching descendant elements in tree order.
    ///
    /// # Arguments
    ///
    /// * `selectors` - A CSS selector list.
    ///
    /// # Returns
    ///
    /// A snapshot of unique matching descendant elements in tree order.
    #[inline(always)]
    pub fn query_selector_all(&self, selectors: &str) -> Result<ElementNodeList, CssParserError> {
        self.query_selector_all_compiled(&CssSelector::new(selectors)?)
    }

    /// Collect unique matches across the roots in tree order.
    ///
    /// # Arguments
    ///
    /// * `selectors` - The CSS selector list to parse or apply.
    ///
    /// # Returns
    ///
    /// A snapshot of unique matching descendant elements in tree order.
    pub fn query_selector_all_compiled(&self, selectors: &CssSelector) -> Result<ElementNodeList, CssParserError> {
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

    /// Return whether the element matches this selector list.
    ///
    /// # Arguments
    ///
    /// * `selectors` - A CSS selector list.
    ///
    /// # Returns
    ///
    /// True if any element in the collection matches the selector list.
    #[inline(always)]
    pub fn matches(&self, selectors: &str) -> Result<bool, CssParserError> {
        self.matches_compiled(&CssSelector::new(selectors)?)
    }

    /// Return whether any element in the collection matches the selector list.
    ///
    /// # Arguments
    ///
    /// * `selectors` - The CSS selector list to parse or apply.
    ///
    /// # Returns
    ///
    /// True if any element in the collection matches the selector list.
    pub fn matches_compiled(&self, selectors: &CssSelector) -> Result<bool, CssParserError> {
        for element in self {
            if selectors.matches(element)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

// ---------------------------------------- DOMTokenList impl --------------------------------------

/// Read-only operations for a whitespace-separated DOM token list.
pub trait DOMTokenListInterface: IntoIterator + PartialEq + Debug + Display + PartialEq {
    /// Return the raw token-list string.
    ///
    /// # Returns
    ///
    /// The raw token-list string.
    fn value(&self) -> String;

    /// Return the unique tokens in their original order.
    ///
    /// # Returns
    ///
    /// The unique tokens in their original order.
    fn values(&self) -> Vec<String> {
        let mut h = HashSet::new();
        self.value()
            .split_ascii_whitespace()
            .filter(|&v| h.insert(v))
            .map(String::from)
            .collect()
    }

    /// Return the token at the given zero-based index, if present.
    ///
    /// # Arguments
    ///
    /// * `index` - A zero-based item or token index.
    ///
    /// # Returns
    ///
    /// The token at `index`, or `None` if the index is out of bounds.
    fn item(&self, index: usize) -> Option<String> {
        Some(self.values().get(index)?.to_owned())
    }

    /// Return whether the token list contains this token.
    ///
    /// # Arguments
    ///
    /// * `token` - The token to check or change.
    ///
    /// # Returns
    ///
    /// True if the token list contains `token`.
    fn contains(&self, token: &str) -> bool {
        self.iter().any(|s| s == token)
    }

    /// Iterate over the unique tokens in order.
    ///
    /// # Returns
    ///
    /// An iterator over the unique tokens in the token list.
    #[inline]
    fn iter(&self) -> vec::IntoIter<String> {
        self.values().into_iter()
    }

    /// Return the number of unique tokens.
    ///
    /// # Returns
    ///
    /// The number of unique tokens in the token list.
    #[inline]
    fn len(&self) -> usize {
        self.values().len()
    }

    /// Return whether the token list contains no tokens.
    ///
    /// # Returns
    ///
    /// True if the token list contains no tokens.
    #[inline]
    fn is_empty(&self) -> bool {
        self.values().is_empty()
    }
}

pub(crate) trait DOMTokenListBaseImpl {
    fn element_(&self) -> &ElementNode;
}

/// Mutation operations for a whitespace-separated DOM token list.
#[allow(private_bounds)]
pub trait DOMTokenListMutInterface: DOMTokenListInterface + DOMTokenListBaseImpl {
    /// Replace the raw token-list string.
    ///
    /// # Arguments
    ///
    /// * `value` - The attribute value to match or set.
    ///
    /// # Returns
    ///
    /// No value is returned.
    fn set_value(&self, value: &str);

    /// Add tokens that are not already present.
    ///
    /// # Arguments
    ///
    /// * `tokens` - The tokens to add or remove.
    ///
    /// # Returns
    ///
    /// No value is returned.
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

    /// Remove each supplied token if present.
    ///
    /// # Arguments
    ///
    /// * `tokens` - The tokens to remove from the list.
    ///
    /// # Returns
    ///
    /// No value is returned.
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

    /// Replace an existing token and return whether it was found.
    ///
    /// # Arguments
    ///
    /// * `old_token` - The token to replace.
    /// * `new_token` - The replacement token.
    ///
    /// # Returns
    ///
    /// True if `old_token` was present and replaced.
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

    /// Add or remove a token according to `force`, then return whether present.
    ///
    /// # Arguments
    ///
    /// * `token` - The token to add or remove.
    /// * `force` - Whether the attribute or token should be present; `None` toggles its current state.
    ///
    /// # Returns
    ///
    /// True if `token` is present after the operation.
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

/// A live view of an element’s class tokens.
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

/// An owned view of an element’s class tokens.
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
