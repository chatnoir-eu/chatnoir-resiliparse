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

#[test]
fn string_conversions_recover_malformed_markup() {
    // Every conversion should use the same forgiving HTML parser and insert missing structure.
    let input = "<title>A &amp; B</title><p>one<b>two";
    for tree in [
        HtmlTree::try_from(input).unwrap(),
        input.parse::<HtmlTree>().unwrap(),
        HtmlTree::try_from(input.to_owned()).unwrap(),
    ] {
        assert_eq!(tree.title().as_deref(), Some("A & B"));
        assert_eq!(tree.head().unwrap().local_name().as_deref(), Some("head"));
        assert_eq!(tree.body().unwrap().inner_html(), "<p>one<b>two</b></p>");
    }
}

#[test]
fn bytes_replace_invalid_utf8() {
    // Invalid and truncated sequences become replacement characters, not decoding errors or lost text.
    let tree = HtmlTree::try_from(b"<p>A\xffB\xe2\x82".as_slice()).unwrap();
    assert_eq!(tree.body().unwrap().text_content().as_deref(), Some("A\u{fffd}B\u{fffd}"));
}

#[test]
fn tree_accessors_share_document_identity() {
    let tree = HtmlTree::parse("<title>Original</title><p>Body</p>").unwrap();
    let document = tree.document().unwrap();
    let head = tree.head().unwrap();
    let body = tree.body().unwrap();
    assert_eq!(head.owner_document().unwrap(), document);
    assert_eq!(body.owner_document().unwrap(), document);
    let title = head.query_selector("title").unwrap().unwrap();
    title.set_text_content("Changed");
    assert_eq!(tree.title().as_deref(), Some("Changed"));
    // A node handle must keep its document alive after the tree handle is dropped.
    drop(tree);
    drop(document);
    drop(head);
    drop(body);
    assert_eq!(title.text_content().as_deref(), Some("Changed"));
}
