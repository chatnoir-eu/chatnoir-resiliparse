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

    let lang_en = tree.document().unwrap().elements_by_attr("lang", "en");
    assert_eq!(lang_en.len(), 1);
    assert!(lang_en.item(0).unwrap().has_attribute("lang"));
    assert_eq!(lang_en.item(0).unwrap().tag_name().unwrap(), "HTML");

    // CSS match
    assert!(tree.document().unwrap().query_selector("#bar, abc").unwrap().is_none());

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

    // Invalid CSS selectors
    assert!(tree.body().unwrap().query_selector("#a < abc").is_err());
    assert!(tree.body().unwrap().query_selector_all("#a < abc").is_err());
}

#[test]
fn test_static_collection() {
    let tree = HTMLTree::from_str(HTML).unwrap();
    let coll = tree.body().unwrap().query_selector_all("main *").unwrap();

    // Basic element attributes
    assert_eq!(coll.len(), 4);
    assert_eq!(coll.item(0).unwrap().id().unwrap(), "a");
    assert_eq!(coll.item(coll.len() - 1).unwrap().class_name().unwrap(), "bar baz");
    assert_eq!(coll.items()[..2].len(), 2);
    assert_eq!(coll.items()[..2][0].id().unwrap(), "a");
    assert_eq!(coll.items()[..2][1].class_name().unwrap(), "bar");

    // Iteration
    assert_eq!(coll.iter().map(|e| assert!(e.tag_name().is_some())).count(), coll.len());

    // Collection match forwarding
    let coll = tree.body().unwrap().query_selector_all("p").unwrap();
    coll.elements_by_class_name("bar").iter()
        .zip(coll.query_selector_all(".bar").unwrap().iter())
        .for_each(|(a, b)| assert_eq!(a, b));
    assert_eq!(coll.elements_by_attr("href", "https://example.com").len(), 1);
    assert_eq!(coll.elements_by_tag_name("SPAN").len(), 1);

    // CSS match
    assert!(coll.query_selector("#foo, abc").unwrap().is_none());
    assert_eq!(coll.query_selector(".bar").unwrap().unwrap().tag_name().unwrap(), "SPAN");
    assert_eq!(coll.query_selector_all("span, a").unwrap().len(), 2);
    assert!(coll.matches("#a").unwrap());
    assert!(coll.matches("p").unwrap());
    assert!(!coll.matches(".bar, .baz").unwrap());
    assert!(!coll.matches("main").unwrap());

    // Invalid CSS selectors
    assert!(coll.query_selector("#a < abc").is_err());
    assert!(coll.query_selector_all("#a < abc").is_err());
}

#[test]
fn test_dynamic_collection() {
    let tree = HTMLTree::from_str(HTML).unwrap();
    let coll1 = tree.body().unwrap().elements_by_tag_name("p");
    let coll2 = tree.body().unwrap().elements_by_class_name("bar");
    let coll3 = tree.body().unwrap().query_selector_all("p").unwrap();

    assert_eq!(coll1.len(), 2);
    assert_eq!(coll2.len(), 2);
    assert_eq!(coll3.len(), 2);

    tree.document().unwrap().element_by_id("a").unwrap().remove();
    assert_eq!(coll1.len(), 1);
    assert_eq!(coll2.len(), 1);
    assert_eq!(coll3.len(), 2);     // CSS match collection are static
}

#[test]
fn test_class_list() {
    let tree = HTMLTree::from_str(HTML).unwrap();
    let mut a = tree.body().unwrap().query_selector("#b a").unwrap().unwrap();

    assert!(a.has_attribute("class"));
    assert_eq!(a.class_list_mut().values(), a.class_list().values());
    assert_eq!(a.class_name().unwrap(), "bar baz");
    assert_eq!(a.class_list().len(), 2);
    assert_eq!(a.class_list_mut().len(), 2);
    assert_eq!(a.class_list(), a.class_list());
    assert_eq!(a.class_list(), ["bar", "baz"].as_slice());
    assert_ne!(a.class_list(), ["bar"].as_slice());
    assert_ne!(a.class_list(), ["baz", "bar"].as_slice());
    assert_ne!(a.class_list(), ["x"].as_slice());

    assert_eq!(a.class_list(), ["bar", "baz"].as_slice());
    a.class_list_mut().add(&["abc"]);
    assert_eq!(a.class_list().len(), 3);
    assert_eq!(a.class_list(), ["bar", "baz", "abc"].as_slice());
    assert_eq!(a.class_name().unwrap(), "bar baz abc");

    a.class_list_mut().remove(&["baz"]);
    assert_eq!(a.class_list(), ["bar", "abc"].as_slice());
    assert_eq!(a.class_name().unwrap(), "bar abc");

    assert!(!a.class_list_mut().toggle("bar", None));
    assert_eq!(a.class_list(), ["abc"].as_slice());
    assert!(a.class_list_mut().toggle("bar", None));
    assert_eq!(a.class_list(), ["abc", "bar"].as_slice());
    assert!(a.class_list_mut().toggle("bar", Some(true)));
    assert_eq!(a.class_list(), ["abc", "bar"].as_slice());
    assert!(a.class_list_mut().toggle("baz", Some(true)));
    assert_eq!(a.class_list(), ["abc", "bar", "baz"].as_slice());
    assert!(!a.class_list_mut().toggle("baz", Some(false)));
    assert_eq!(a.class_list(), ["abc", "bar"].as_slice());
    assert!(!a.class_list_mut().toggle("baz", Some(false)));
    assert_eq!(a.class_list(), ["abc", "bar"].as_slice());

    assert!(a.class_list_mut().replace("abc", "def"));
    assert_eq!(a.class_list(), ["def", "bar"].as_slice());
    assert!(!a.class_list_mut().replace("uvw", "xyz"));
    assert_eq!(a.class_list(), ["def", "bar"].as_slice());
}

#[test]
fn test_attributes() {
    let tree = HTMLTree::from_str(HTML).unwrap();
    let mut a = tree.body().unwrap().query_selector("#b a").unwrap().unwrap();

    assert!(a.attribute("id").is_none());
    assert!(a.id().is_none());
    assert_eq!(a.attribute_or("id", "default"), "default");
    assert_eq!(a.attribute_or_default("id"), "");
    a.set_id("abc");
    assert_eq!(a.id().unwrap(), "abc");
    assert_eq!(a.id().unwrap(), a.attribute("id").unwrap());

    assert!(a.attribute("lang").is_none());
    a.set_attribute("lang", "en");
    assert_eq!(a.attribute("lang").unwrap(), "en");

    assert_eq!(a.attribute_names(), ["href", "class", "id", "lang"]);
    a.remove_attribute("lang");
    assert_eq!(a.attribute_names(), ["href", "class", "id"]);
    assert!(a.attribute("lang").is_none());
    assert!(a.attribute_node("lang").is_none());

    let mut attr = a.attribute_node("href").unwrap();
    assert!(attr.parent_node().is_none());
    assert_eq!(attr.name().unwrap(), "href");
    assert_eq!(attr.local_name().unwrap(), attr.name().unwrap());
    assert_eq!(attr.value().unwrap(), "https://example.com");
    assert_eq!(attr.node_value().unwrap(), attr.value().unwrap());
    assert_eq!(attr.child_nodes().len(), 0);

    // Cannot append children to attributes
    assert!(attr.append_child(
        &tree.document().unwrap().create_element("foo").into()).is_none());
    assert_eq!(attr.child_nodes().len(), 0);
}
