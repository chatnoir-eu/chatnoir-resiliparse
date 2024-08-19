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
//! Node and element collection types for storing the results of DOM matching operations.

use std::collections::HashSet;
use std::fmt::{Debug, Display, Formatter};
use std::vec;

use crate::parse::html::css::CSSParserError;
use crate::parse::html::dom::node::{AttrNode, ElementNode, Node};
use crate::parse::html::dom::traits::{Element, ParentNode};


#[derive(Clone)]
pub(super) struct NodeListClosure<T> {
    n: Node,
    d: Option<Box<[String]>>,
    f: fn(&Node, Option<&Box<[String]>>) -> Vec<T>
}

pub struct NodeListGeneric<T> {
    live: Option<NodeListClosure<T>>,
    items: Vec<T>,
}

impl<T> Default for NodeListGeneric<T> {
    fn default() -> Self {
        NodeListGeneric { live: None, items: Vec::default() }
    }
}

impl<T: Clone> From<&[T]> for NodeListGeneric<T>  {
    fn from(items: &[T]) -> Self {
        Self { live: None, items: Vec::from(items) }
    }
}

impl<T: Clone> From<Vec<T>> for NodeListGeneric<T>  {
    fn from(items: Vec<T>) -> Self {
        Self { live: None, items }
    }
}

impl<'a, T: Clone> NodeListGeneric<T> {
    #[inline]
    pub fn item(&self, index: usize) -> Option<T> {
        Some(self.iter().skip(index).next()?)
    }

    #[inline]
    pub fn values(&self) -> Vec<T> {
        self.iter().collect()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    pub(super) fn new_live(node: Node, user_data: Option<Box<[String]>>,
                           f: fn(&Node, Option<&Box<[String]>>) -> Vec<T>) -> Self {
        Self {
            live: Some(NodeListClosure { n: node, d: user_data, f }),
            items: Vec::default()
        }
    }

    pub fn iter(&self) -> vec::IntoIter<T> {
        if let Some(closure) = &self.live {
            (closure.f)(&closure.n, closure.d.as_ref()).into_iter()
        } else {
            self.items.clone().into_iter()
        }
    }
}

impl<T: Clone> IntoIterator for &NodeListGeneric<T> {
    type Item = T;
    type IntoIter = vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T: Clone + Debug> Debug for NodeListGeneric<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for (i, n) in self.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            Debug::fmt(&n, f)?;
        };
        write!(f, "]")
    }
}

pub type NodeList = NodeListGeneric<Node>;
pub type ElementNodeList = NodeListGeneric<ElementNode>;
pub type HTMLCollection = ElementNodeList;
pub type NamedNodeMap = NodeListGeneric<AttrNode>;

impl ElementNodeList {
    pub fn named_item(&self, name: &str) -> Option<ElementNode> {
        self.iter()
            .find(|e| {
                e.id()
                    .filter(|i| i == name).is_some() || e.attribute("name")
                    .filter(|n| n == name).is_some()
            })
    }

    pub fn elements_by_tag_name(&self, qualified_name: &str) -> HTMLCollection {
        let mut coll = Vec::default();
        self.iter().for_each(|e| {
            coll.append(&mut e.get_elements_by_tag_name(qualified_name).iter().collect())
        });
        HTMLCollection::from(coll)
    }

    pub fn elements_by_class_name(&self, class_names: &str) -> HTMLCollection {
        let mut coll = Vec::default();
        self.iter().for_each(|e| {
            coll.append(&mut e.get_elements_by_class_name(class_names).iter().collect())
        });
        HTMLCollection::from(coll)
    }

    #[inline(always)]
    pub fn elements_by_attr(&self, qualified_name: &str, value: &str) -> HTMLCollection {
        self.elements_by_attr_case(qualified_name, value, false)
    }

    pub fn elements_by_attr_case(&self, qualified_name: &str, value: &str, case_insensitive: bool) -> HTMLCollection {
        let mut coll = Vec::default();
        self.iter().for_each(|e| {
            coll.append(&mut e.get_elements_by_attr_case(qualified_name, value, case_insensitive).iter().collect())
        });
        HTMLCollection::from(coll)
    }

     pub fn query_selector(&self, selectors: &str) -> Result<Option<ElementNode>, CSSParserError> {
        for item in self.iter() {
            match item.query_selector(selectors) {
                Ok(Some(e)) => return Ok(Some(e)),
                Ok(None) => continue,
                Err(e) => return Err(e)
            }
        }
        Ok(None)
    }

    pub fn query_selector_all(&self, selectors: &str) -> Result<ElementNodeList, CSSParserError> {
        let mut coll = Vec::default();
        for item in self.iter() {
            match item.query_selector_all(selectors) {
                Ok(e) => coll.append(&mut e.iter().collect()),
                Err(e) => return Err(e)
            }
        }
        Ok(ElementNodeList::from(coll))
    }

    pub fn matches(&self, selectors: &str) -> Result<bool, CSSParserError> {
        for item in self.iter() {
            match item.matches(selectors) {
                Ok(true) => return Ok(true),
                Ok(false) => continue,
                Err(e) => return Err(e)
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
        self.value().split_ascii_whitespace()
            .filter(|&v| h.insert(v))
            .map(String::from)
            .collect()
    }

    fn item(&self, index: usize) -> Option<String> {
        Some(self.values().get(index)?.to_owned())
    }

    fn contains(&self, token: &str) -> bool {
        self.iter().find(|s: &String| s == token).is_some()
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

// Cannot derive default implementations for sub traits (yet)
macro_rules! dom_node_list_impl {
    ($Self: ident) => {
        impl DOMTokenListInterface for $Self<'_> {
            #[inline]
            fn value(&self) -> String {
                self.element.class_name().unwrap_or_default()
            }
        }

        impl IntoIterator for $Self<'_>  {
            type Item = String;
            type IntoIter = vec::IntoIter<String>;

            fn into_iter(self) -> Self::IntoIter {
                self.iter()
            }
        }

        impl PartialEq<&[&str]> for $Self<'_> {
            fn eq(&self, other: &&[&str]) -> bool {
                let val = self.values();
                if other.len() != val.len() {
                    return false;
                }
                val.iter().zip(other.iter()).find(|(a, b)| a != *b).is_none()
            }
        }

        impl Debug for $Self<'_> {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                write!(f, "[")?;
                for (i, s) in self.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    f.write_str(&format!("{:?}", s))?;
                };
                write!(f, "]")
            }
        }

        impl Display for $Self<'_> {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                Debug::fmt(self, f)
            }
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct DOMTokenList<'a> {
    element: &'a ElementNode,
}

impl<'a> DOMTokenList<'a> {
    pub(super) fn new(element: &'a ElementNode) -> Self {
        Self { element }
    }
}

dom_node_list_impl!(DOMTokenList);

#[derive(PartialEq)]
pub struct DOMTokenListMut<'a> {
    element: &'a mut ElementNode
}

impl<'a> DOMTokenListMut<'a> {
    pub(super) fn new(element: &'a mut ElementNode) -> Self {
        Self { element }
    }

    fn update_node(&mut self, values: &Vec<String>) {
        self.element.set_class_name(&values.join(" "));
    }

    pub fn add(&mut self, tokens: &[&str]) {
        let mut values = self.values();
        tokens.iter().for_each(|t: &&str| {
            let t_owned = (*t).to_owned();
            if !values.contains(&t_owned) {
                values.push(t_owned);
            }
        });
        self.update_node(&values);
    }

    pub fn remove(&mut self, tokens: &[&str]) {
        self.update_node(&self
            .iter()
            .filter(|t: &String| !tokens.contains(&t.as_str()))
            .collect()
        );
    }

    pub fn replace(&mut self, old_token: &str, new_token: &str) -> bool {
        let mut repl = false;
        self.update_node(&self
            .iter()
            .map(|t: String| {
                if t == old_token {
                    repl = true;
                    new_token.to_owned()
                } else { t }
            })
            .collect()
        );
        repl
    }

    pub fn toggle(&mut self, token: &str, force: Option<bool>) -> bool {
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

dom_node_list_impl!(DOMTokenListMut);
