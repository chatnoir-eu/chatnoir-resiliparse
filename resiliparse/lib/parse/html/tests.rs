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


use std::str::FromStr;
use crate::parse::html::dom::*;
use crate::parse::html::tree::HTMLTree;

const HTML: &str = r#"<!doctype html>
    <html lang="en">
    <head>
        <meta charset="utf-8">
        <title>Example page</title>
    </head>
    <body>
    <main id="foo">
        <p id="a">Hello <span class="bar">world</span>!</p>
        <p id="b" class="dom">Hello <a href="https://example.com" class="bar baz">DOM</a>!</p>
    </main>
    <!-- A comment -->
    </body>
</html>"#;

const HTML_NO_HEAD: &str = "<!doctype html><body><span></span></body>";
const HTML_NO_BODY: &str = "<!doctype html><head><title>Title</title></head>";
const HTML_NO_TITLE: &str = "<!doctype html><head></head></body>";
const HTML_NO_TITLE_SVG: &str = "<!doctype html><svg xmlns=\"http://www.w3.org/2000/svg\"><title>SVG Title</title></svg>";
const HTML_UNCLOSED_HEAD: &str = "<!doctype html><head><title>Title</title><span></span>";

#[test]
fn parse_from_str() {
    let _tree1 = HTMLTree::from_str(HTML).unwrap();
    let _tree2 = HTMLTree::from_str("<html></html>").unwrap();
}

#[test]
fn parse_from_string() {
    let _tree1 = HTMLTree::try_from(HTML.to_owned()).unwrap();
}

#[test]
fn parse_from_bytes() {
    let _tree3 = HTMLTree::try_from(HTML.as_bytes()).unwrap();
}

#[test]
fn test_parse_quirks() {
    let tree_quirk = HTMLTree::from_str(HTML_NO_HEAD).unwrap();
    tree_quirk.document().unwrap();
    assert_eq!(tree_quirk.head().unwrap().child_nodes().len(), 0);
    assert_eq!(tree_quirk.body().unwrap().child_nodes().len(), 1);

    let tree_quirk = HTMLTree::from_str(HTML_NO_BODY).unwrap();
    tree_quirk.document().unwrap();
    assert_eq!(tree_quirk.head().unwrap().child_nodes().len(), 1);
    assert_eq!(tree_quirk.title().unwrap(), "Title");
    assert_eq!(tree_quirk.body().unwrap().child_nodes().len(), 0);

    let tree_quirk = HTMLTree::from_str(HTML_NO_TITLE).unwrap();
    tree_quirk.document().unwrap();
    assert_eq!(tree_quirk.head().unwrap().child_nodes().len(), 0);
    assert!(tree_quirk.title().is_none());
    assert_eq!(tree_quirk.body().unwrap().child_nodes().len(), 0);

    let tree_quirk = HTMLTree::from_str(HTML_NO_TITLE_SVG).unwrap();
    tree_quirk.document().unwrap();
    tree_quirk.head().unwrap();
    assert!(tree_quirk.title().is_none());
    assert_eq!(tree_quirk.body().unwrap().child_nodes().len(), 1);

    let tree_quirk = HTMLTree::from_str(HTML_UNCLOSED_HEAD).unwrap();
    tree_quirk.document().unwrap();
    tree_quirk.head().unwrap();
    assert_eq!(tree_quirk.head().unwrap().child_nodes().len(), 1);
    assert_eq!(tree_quirk.title().unwrap(), "Title");
    assert_eq!(tree_quirk.body().unwrap().child_nodes().len(), 1);
}

#[test]
fn test_node_equality() {
    let tree = HTMLTree::from_str(HTML).unwrap();

    assert_ne!(tree.body().unwrap(), tree.head().unwrap());
    assert_eq!(tree.head().unwrap(), tree.head().unwrap());
    assert_eq!(tree.body().unwrap(), tree.body().unwrap());

    let a1 = tree.body().unwrap().query_selector("#a").unwrap().unwrap();
    let a2 = tree.body().unwrap().query_selector("#a").unwrap().unwrap();
    let b1 = tree.body().unwrap().query_selector("#b").unwrap().unwrap();
    let b2 = tree.body().unwrap().query_selector("#b").unwrap().unwrap();

    assert_ne!(a1, b1);
    assert_ne!(a2, b2);
    assert_eq!(a1, a2);
    assert_eq!(b1, b2);
}

#[test]
fn test_selection() {
    let tree = HTMLTree::from_str(HTML).unwrap();

    tree.document().unwrap().element_by_id("foo").unwrap();
    assert_eq!(tree.document().unwrap().element_by_id("foo").unwrap().node_name().unwrap(), "MAIN");

    let meta = tree.head().unwrap().elements_by_tag_name("meta");
    assert_eq!(meta.len(), 1);
    assert_eq!(meta.item(0).unwrap().tag_name().unwrap(), "META");

    let bar_class = tree.body().unwrap().elements_by_class_name("bar");
    assert_eq!(bar_class.len(), 2);
    assert_eq!(bar_class.item(0).unwrap().tag_name().unwrap(), "SPAN");
    assert_eq!(bar_class.item(1).unwrap().tag_name().unwrap(), "A");

    // lang_en = tree.document.get_elements_by_attr('lang', 'en')
    // assert (type(lang_en)) is DOMCollection
    // assert len(lang_en) == 1
    // assert lang_en[0].hasattr('lang')
    // assert lang_en[0].tag == 'html'
    //

    let match_css = tree.document().unwrap().query_selector("body > main p:last-child").unwrap().unwrap();
    assert_eq!(match_css.tag_name().unwrap(), "P");

    let match_css_all = tree.body().unwrap().query_selector_all("main *").unwrap();
    assert_eq!(match_css_all.len(), 4);
    assert_eq!(match_css_all.item(0).unwrap().tag_name().unwrap(), "P");
    assert_eq!(match_css_all.item(1).unwrap().tag_name().unwrap(), "SPAN");
    assert_eq!(match_css_all.item(2).unwrap().tag_name().unwrap(), "P");
    assert_eq!(match_css_all.item(3).unwrap().tag_name().unwrap(), "A");

    // Check whether element would be matched
    assert!(tree.body().unwrap().matches("body").unwrap());
    assert!(tree.body().unwrap().matches("html > body").unwrap());
    assert!(tree.body().unwrap().first_element_child().unwrap().matches("#foo").unwrap());
    assert!(tree.body().unwrap().first_element_child().unwrap().matches("main").unwrap());
    assert!(!tree.body().unwrap().matches(".barbaz").unwrap());
    assert!(!tree.body().unwrap().first_element_child().unwrap().matches("div").unwrap());

    // Invalid CSS selector
    assert!(tree.body().unwrap().query_selector("..abc").is_err());
}
