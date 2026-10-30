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

use super::*;
use crate::parse::html::dom::traits::*;
use crate::parse::html::tree::HtmlTree;

#[test]
fn named_lookup_is_ordered_and_live() {
    let tree = HtmlTree::parse("<a name=target>First</a><a id=target>Second</a><a>Third</a>").unwrap();
    let body = tree.body().unwrap();
    let live = body.children();
    let snapshot = live.snapshot();
    let first = body.first_element_child().unwrap();
    // Lookup follows document order across both name and id matches.
    assert_eq!(live.named_item("target").unwrap(), first);
    assert_eq!(snapshot.named_item("target").unwrap(), &first);
    assert!(live.named_item("absent").is_none());
    assert!(snapshot.named_item("absent").is_none());
    // Removing the first match updates the live view, not snapshot membership.
    first.remove();
    assert_eq!(live.named_item("target").unwrap().text_content().as_deref(), Some("Second"));
    assert_eq!(snapshot.named_item("target").unwrap(), &first);
    assert_eq!(snapshot.len(), 3);
    assert_eq!(live.len(), 2);
}

#[test]
fn attribute_views_and_snapshots() {
    let tree = HtmlTree::parse("<p id=original title=example>Text</p>").unwrap();
    let element = tree.body().unwrap().first_element_child().unwrap();
    let attributes = element.attributes();
    let snapshot = attributes.snapshot();
    let slice_copy: NodeList<AttrNode> = snapshot.as_slice().into();
    assert_eq!(slice_copy.len(), 2);
    assert_eq!(attributes.item(0).unwrap(), snapshot.item(0).unwrap().clone());
    assert!(attributes.item(100).is_none());
    assert_eq!((&attributes).into_iter().count(), 2);
    assert_eq!(attributes.values().len(), 2);
    // Replace one attribute: unchanged length must not hide changed membership.
    attributes.set("data-added", "new");
    attributes.remove("title");
    assert_eq!(attributes.len(), 2);
    assert_eq!(snapshot.len(), 2);
    assert!(attributes.get_named_item("title").is_none());
    assert!(slice_copy.iter().any(|attr| attr.name().as_deref() == Some("title")));
}

#[test]
fn owned_class_tokens_remain_live() {
    let tree = HtmlTree::parse("<p class='one one two'>Text</p>").unwrap();
    let element = tree.body().unwrap().first_element_child().unwrap();
    let owned: DomTokenListOwned = element.class_list().into();
    assert_eq!(owned.item(0).as_deref(), Some("one"));
    assert_eq!(owned.item(1).as_deref(), Some("two"));
    assert!(owned.item(2).is_none());
    assert_eq!(format!("{owned}"), "[\"one\", \"two\"]");
    assert_eq!(format!("{:?}", element.class_list()), "[\"one\", \"two\"]");
    element.set_class_name("three");
    assert_eq!(owned.values(), ["three"]);
    owned.set_value("");
    assert!(owned.is_empty());
    element.set_class_name("four five");
    // The owned view keeps the document alive after the original handles are dropped.
    drop(element);
    drop(tree);
    assert_eq!(owned.into_iter().collect::<Vec<_>>(), ["four", "five"]);
}

#[test]
fn decomposed_subtrees_empty_live_collections() {
    let tree = HtmlTree::parse("<div><p>First</p><p>Second</p></div>").unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    let live = root.children();
    let snapshot = live.snapshot();
    // Destruction clears live results while the snapshot retains its membership.
    root.decompose();
    assert!(live.is_empty());
    assert!(live.item(0).is_none());
    assert_eq!((&live).into_iter().count(), 0);
    assert_eq!(snapshot.len(), 2);
}
