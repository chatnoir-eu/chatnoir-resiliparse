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

use crate::parse::html::dom::coll::*;
use crate::parse::html::dom::node::*;
use crate::parse::html::dom::traits::*;
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
const HTML_UNCLOSED_TITLE: &str = "<!doctype html><html><head><title>Test--></head><body>foo</body></html>";
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

    let tree_quirk = HTMLTree::from_str(HTML_UNCLOSED_TITLE).unwrap();
    tree_quirk.document().unwrap();
    assert_eq!(tree_quirk.head().unwrap().child_nodes().len(), 1);
    assert_eq!(tree_quirk.title().unwrap(), "Test--></head><body>foo</body></html>");
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

    tree.document().unwrap().get_element_by_id("foo").unwrap();
    assert_eq!(tree.document().unwrap().get_element_by_id("foo").unwrap().node_name().unwrap(), "MAIN");

    let meta = tree.head().unwrap().get_elements_by_tag_name("meta");
    assert_eq!(meta.len(), 1);
    assert_eq!(meta.item(0).unwrap().tag_name().unwrap(), "META");

    let bar_class = tree.body().unwrap().get_elements_by_class_name("bar");
    assert_eq!(bar_class.len(), 2);
    assert_eq!(bar_class.item(0).unwrap().tag_name().unwrap(), "SPAN");
    assert_eq!(bar_class.item(1).unwrap().tag_name().unwrap(), "A");

    let lang_en = tree.document().unwrap().get_elements_by_attr("lang", "en");
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

    // Find closest matching ancestor (or self)
    assert_eq!(tree.body().unwrap().closest("body").unwrap().unwrap(), tree.body().unwrap());
    assert!(tree.body().unwrap().closest("htmlx").unwrap().is_none());
    assert_eq!(tree.body().unwrap().closest("html").unwrap().unwrap(), tree.document().unwrap().first_element_child().unwrap());
    let child = tree.body().unwrap().query_selector(".bar").unwrap().unwrap();
    assert_eq!(child.closest("p").unwrap().unwrap(), tree.body().unwrap().query_selector("#a").unwrap().unwrap());
    assert_eq!(child.closest("main").unwrap().unwrap(), tree.body().unwrap().query_selector("#foo").unwrap().unwrap());

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
    assert_eq!(coll.values()[..2].len(), 2);
    assert_eq!(coll.values()[..2][0].id().unwrap(), "a");
    assert_eq!(coll.values()[..2][1].class_name().unwrap(), "bar");

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
    let coll1 = tree.body().unwrap().get_elements_by_tag_name("p");
    let coll2 = tree.body().unwrap().get_elements_by_class_name("bar");
    let coll3 = tree.body().unwrap().query_selector_all("p").unwrap();

    assert_eq!(coll1.len(), 2);
    assert_eq!(coll2.len(), 2);
    assert_eq!(coll3.len(), 2);

    tree.document().unwrap().get_element_by_id("a").unwrap().remove();
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
    let mut element = tree.body().unwrap().query_selector("#b a").unwrap().unwrap();

    // List attribute nodes and names
    assert_eq!(element.attributes().iter().zip(element.attribute_names().iter()).map(|(a1, a2)| {
        assert_eq!(a1.name().unwrap(), *a2);
        assert_eq!(a1.value(), element.attribute(a1.name().unwrap().as_str()));
        assert!(a1.value().is_some());
    }).count(), 2);

    // Get and set ID
    assert!(element.attribute("id").is_none());
    assert!(element.id().is_none());
    assert_eq!(element.attribute_or("id", "default"), "default");
    assert_eq!(element.attribute_or_default("id"), "");
    element.set_id("abc");
    assert_eq!(element.id().unwrap(), "abc");
    assert_eq!(element.id().unwrap(), element.attribute("id").unwrap());
    assert_eq!(element.attributes().len(), 3);
    assert_eq!(element.attribute_names().len(), 3);

    // Get and set other attributes
    assert!(element.attribute("lang").is_none());
    element.set_attribute("lang", "en");
    assert_eq!(element.attribute("lang").unwrap(), "en");
    assert_eq!(element.attribute_names(), ["href", "class", "id", "lang"]);
    element.remove_attribute("lang");
    assert_eq!(element.attribute_names(), ["href", "class", "id"]);
    assert!(element.attribute("lang").is_none());
    assert!(element.attribute_node("lang").is_none());

    // Test individual attribute nodes
    let mut attr = element.attribute_node("href").unwrap();
    assert!(attr.parent_node().is_none());
    assert_eq!(attr.name().unwrap(), "href");
    assert_eq!(attr.local_name().unwrap(), attr.name().unwrap());
    assert_eq!(attr.value().unwrap(), "https://example.com");
    assert_eq!(attr.node_value().unwrap(), attr.value().unwrap());
    assert_eq!(attr.child_nodes().len(), 0);

    // Cannot append children to attributes
    assert!(attr.append_child(
        &tree.document().unwrap().create_element("foo").unwrap().into()).is_none());
    assert_eq!(attr.child_nodes().len(), 0);
}

#[test]
fn test_empty_attribute() {
    let tree = HTMLTree::from_str(r#"<div>
        <input type="checkbox" checked>
        <div class="foo"></div>
        <div class></div>
        <div class=""></div>
        <div id="foo"></div>
        <div id></div>
        <div id=""></div>
        <div foo></div>
        <div foo=""></div>"#).unwrap();

    let input_element = tree.body().unwrap().query_selector("input").unwrap().unwrap();

    assert!(input_element.has_attribute("type"));
    assert_eq!(input_element.attribute("type").unwrap(), "checkbox");

    assert!(input_element.has_attribute("checked"));
    assert!(input_element.attribute("checked").is_none());
    assert!(!input_element.has_attribute("checkedx"));
    assert!(input_element.attribute("checkedx").is_none());

    // Test empty attribute selection
    // There used to be a Lexbor crash with this: https://github.com/lexbor/lexbor/pull/148
    assert_eq!(tree.body().unwrap().query_selector_all(".foo").unwrap().len(), 1);
    assert_eq!(tree.body().unwrap().query_selector_all("#foo").unwrap().len(), 1);
    assert_eq!(tree.body().unwrap().query_selector_all("[class]").unwrap().len(), 3);
    assert_eq!(tree.body().unwrap().query_selector_all("[id]").unwrap().len(), 3);
    assert!(!tree.document().unwrap().get_element_by_id("foo").is_none());
    assert!(tree.document().unwrap().get_element_by_id("foox").is_none());
    assert_eq!(tree.body().unwrap().get_elements_by_class_name("foo").len(), 1);
    assert_eq!(tree.body().unwrap().get_elements_by_class_name("").len(), 0);     // This shouldn't match anything);
    assert_eq!(tree.body().unwrap().get_elements_by_attr("class", "foo").len(), 1);
    assert_eq!(tree.body().unwrap().get_elements_by_attr("class", "").len(), 2);
    assert_eq!(tree.body().unwrap().get_elements_by_attr("id", "").len(), 2);
    assert_eq!(tree.body().unwrap().get_elements_by_attr("foo", "").len(), 2);
}

#[test]
fn test_serialization() {
    let tree = HTMLTree::from_str(HTML).unwrap();

    assert_eq!(tree.document().unwrap().get_element_by_id("a").unwrap().outer_text(), "Hello world!");
    assert_eq!(tree.document().unwrap().get_element_by_id("a").unwrap().outer_html(), r#"<p id="a">Hello <span class="bar">world</span>!</p>"#);

    assert_eq!(tree.document().unwrap().get_element_by_id("a").unwrap().inner_text(), "Hello world!");
    assert_eq!(tree.document().unwrap().get_element_by_id("a").unwrap().inner_html(), r#"Hello <span class="bar">world</span>!"#);

    assert_eq!(tree.head().unwrap().query_selector("title").unwrap().unwrap().to_string(), "<title>Example page</title>");

    assert_eq!(format!("{:?}", tree.head().unwrap()), "<head>");
    assert_eq!(format!("{:?}", tree.head().unwrap().query_selector("title").unwrap().unwrap()), "<title>");
    assert_eq!(format!("{:?}", tree.body().unwrap().query_selector("main").unwrap().unwrap()), r#"<main id="foo">"#);

    let text = tree.body().unwrap().query_selector("#b").unwrap().unwrap().first_child().unwrap();
    match text {
        Node::Text(t) => assert_eq!(t.node_value().unwrap(), "Hello "),
        _ => assert!(false, "Node is not a text node.")
    }

    let inputs = HTMLTree::from_str(r#"
        <input id="a" type="checkbox" checked>
        <input id="b" type="checkbox" checked="">
        <input id="c" type="checkbox" checked="checked">
        "#).unwrap();
    assert_eq!(format!("{:?}", inputs.document().unwrap().get_element_by_id("a").unwrap()), r#"<input id="a" type="checkbox" checked>"#);
    assert_eq!(format!("{:?}", inputs.document().unwrap().get_element_by_id("b").unwrap()), r#"<input id="b" type="checkbox" checked="">"#);
    assert_eq!(format!("{:?}", inputs.document().unwrap().get_element_by_id("c").unwrap()), r#"<input id="c" type="checkbox" checked="checked">"#);

    assert_eq!(format!("{:?}", tree.body().unwrap().query_selector("main").unwrap().unwrap()), r#"<main id="foo">"#);
}

#[test]
fn test_traversal() {
    let tree = HTMLTree::from_str(HTML).unwrap();
    let root = tree.document().unwrap().get_element_by_id("a").unwrap();

    let node_names = root.iter().map(|n| n.node_name().unwrap()).collect::<Vec<String>>();
    assert_eq!(node_names, ["P", "#text", "SPAN", "#text", "#text"]);

    let tag_names = root.iter().flat_map(|n| {
        if let Node::Element(e) = n { e.tag_name() } else { None }
    }).collect::<Vec<String>>();
    assert_eq!(tag_names, ["P", "SPAN"]);

    let child_nodes = tree.document().unwrap().get_element_by_id("foo").unwrap().child_nodes();
    let child_node_names = child_nodes.iter()
        .map(|n| n.node_name().unwrap()).collect::<Vec<String>>();
    assert_eq!(child_node_names, ["#text", "P", "#text", "P", "#text"]);

    child_nodes.iter().for_each(|n| {
        match n {
            Node::Text(n) => assert_eq!(n.node_name().unwrap(), "#text"),
            Node::Element(n) => assert_eq!(n.node_name().unwrap(), "P"),
            _ => assert!(false, "Invalid node type")
        }
    });
}

#[test]
fn test_callback_traversal() {

}

#[test]
fn test_children() {
    let tree = HTMLTree::from_str(HTML).unwrap();
    let element = tree.document().unwrap().get_element_by_id("a").unwrap();

    assert_eq!(element, element.first_child().unwrap().parent_element().unwrap());
    assert_eq!(element, element.last_child().unwrap().parent_element().unwrap());
    assert_eq!(element.first_child().unwrap().next_sibling(), element.last_child().unwrap().previous_sibling());

    let t: TextNode = element.first_child().unwrap().into();
    assert!(t.first_child().is_none());
    assert_eq!(t.node_value().unwrap(), t.text_content().unwrap());
    assert_eq!(t.node_value().unwrap(), "Hello ");

    let e_first = element.first_element_child().unwrap();
    assert!(e_first.node_value().is_none());
    assert_eq!(e_first.text_content().unwrap(), "world");

    let t: TextNode = element.last_child().unwrap().into();
    assert!(t.last_child().is_none());
    assert_eq!(t.node_value().unwrap(), t.text_content().unwrap());
    assert_eq!(t.node_value().unwrap(), "!");

    let e_last = element.last_element_child().unwrap();
    assert!(e_last.node_value().is_none());
    assert_eq!(e_last, e_first);

    assert_eq!(element.next_element_sibling().unwrap().tag_name().unwrap(), "P");
    assert!(element.next_element_sibling().unwrap().next_sibling().is_some());
    assert!(element.next_element_sibling().unwrap().next_sibling().unwrap().next_sibling().is_none());
    assert!(element.next_element_sibling().unwrap().next_element_sibling().is_none());
}

#[test]
fn test_siblings() {
    let tree = HTMLTree::from_str(HTML).unwrap();
    let element = tree.document().unwrap().get_element_by_id("a").unwrap();

    let e_next: ElementNode = element.first_child().unwrap().next_sibling().unwrap().into();
    assert_eq!(e_next, element.first_element_child().unwrap());
    assert_eq!(e_next.tag_name().unwrap(), "SPAN");
    assert_eq!(e_next.class_name().unwrap(), "bar");

    assert!(element.previous_sibling().is_some());
    assert!(element.previous_element_sibling().is_none());

    let e_prev: ElementNode = element.last_child().unwrap().previous_sibling().unwrap().into();
    assert_eq!(e_prev, element.last_element_child().unwrap());
    assert_eq!(e_prev.tag_name().unwrap(), "SPAN");
    assert_eq!(e_prev.class_name().unwrap(), "bar");

    let element1 = tree.document().unwrap().get_element_by_id("foo").unwrap().first_element_child().unwrap();
    assert_eq!(element1.id().unwrap(), "a");
    assert_eq!(TextNode::from(element1.next_sibling().unwrap()).node_value().unwrap().trim(), "");
    assert_eq!(element1.next_element_sibling().unwrap().text_content().unwrap(), "Hello DOM!");
    assert!(element1.previous_sibling().unwrap().previous_sibling().is_none());
    assert!(element1.previous_element_sibling().is_none());

    let element2 = tree.document().unwrap().get_element_by_id("foo").unwrap().last_element_child().unwrap();
    assert_eq!(element2.id().unwrap(), "b");
    assert_eq!(TextNode::from(element2.previous_sibling().unwrap()).node_value().unwrap().trim(), "");
    assert_eq!(element2.previous_element_sibling().unwrap().text_content().unwrap(), "Hello world!");
    assert!(element2.next_sibling().unwrap().next_sibling().is_none());
    assert!(element2.next_element_sibling().is_none());

    assert_eq!(element1.next_element_sibling().unwrap(), element2);
    assert_eq!(element2.previous_element_sibling().unwrap(), element1);
}

#[test]
fn create_nodes() {
    let mut doc = HTMLTree::from_str(HTML).unwrap().document().unwrap();

    let mut frag = doc.create_document_fragment().unwrap();

    let text = doc.create_text_node("Hello World").unwrap();
    assert_eq!(text.node_value().unwrap(), "Hello World");

    let mut element = doc.create_element("foo").unwrap();
    assert_eq!(element.tag_name().unwrap(), "FOO");

    let mut attr = doc.create_attribute("class").unwrap();
    assert_eq!(attr.node_name().unwrap(), "class");

    let comment = doc.create_comment("Some comment").unwrap();
    assert_eq!(comment.node_value().unwrap(), "Some comment");

    let cdata = doc.create_cdata_section("Foo <bar> <baz> </bar>").unwrap();
    assert_eq!(cdata.node_value().unwrap(), "Foo <bar> <baz> </bar>");
    assert_eq!(cdata.to_string(), "");  // Not supported for HTML documents

    let proc = doc.create_processing_instruction("xml", "version=\"1.0\" foo=\"bar\"").unwrap();
    assert_eq!(proc.target().unwrap(), "xml");
    assert_eq!(proc.node_value().unwrap(), "version=\"1.0\" foo=\"bar\"");
    assert_eq!(proc.to_string(), "<?xml version=\"1.0\" foo=\"bar\">");

    frag.append_child(&element.as_node());
    element.append(&[&text.into(), &comment.into(), &proc.into()]);
    attr.set_value("abc");
    assert_eq!(attr.node_value().unwrap(), "abc");

    element.set_attribute_node(&attr);
    assert!(element.has_attribute("class"));
    element.set_attribute("class", "foobar");
    assert_eq!(attr.node_value().unwrap(), "foobar");

    element.set_attribute("abc", "def");
    assert_eq!(element.to_string(), r##"<foo class="foobar" abc="def">Hello World<!--Some comment--><?xml version="1.0" foo="bar"></foo>"##);

    // Appending a DocumentFragment moves the child nodes into the document.
    doc.first_element_child().unwrap().append_child(&frag.as_node());
    assert_eq!(doc.first_element_child().unwrap().last_child().unwrap(), element.as_node());
    assert!(frag.first_child().is_none());
}

#[test]
fn node_reference_counting() {
    let tree =  HTMLTree::from_str(HTML).unwrap();
    let doc = tree.document().unwrap();

    // Keep grandchild reference
    let mut grandchild = doc.first_element_child().unwrap().first_element_child().unwrap();

    // Minimum reference count should be 2 (1 Lexbor + 1 Resiliparse)
    assert_eq!(unsafe { *grandchild.node_ptr_() }.ref_count, 2);

    // Create a few more clones
    let mut grandchild2 = grandchild.clone();
    assert!(grandchild2.has_child_nodes());
    assert_eq!(unsafe { *grandchild.node_ptr_() }.ref_count, 3);
    assert_eq!(unsafe { *grandchild2.node_ptr_() }.ref_count, 3);

    let grandchild3 = grandchild.clone();
    assert!(grandchild2.has_child_nodes());
    assert_eq!(unsafe { *grandchild.node_ptr_() }.ref_count, 4);
    assert_eq!(unsafe { *grandchild3.node_ptr_() }.ref_count, 4);

    // Check parent state
    assert!(doc.first_element_child().is_some());
    assert!(grandchild.has_child_nodes());

    // Decompose parent
    doc.first_element_child().unwrap().decompose();
    assert!(!doc.first_element_child().is_some());
    assert_eq!(unsafe { *grandchild.node_ptr_() }.ref_count, 3);
    assert_eq!(unsafe { *grandchild2.node_ptr_() }.ref_count, 3);
    assert_eq!(unsafe { *grandchild3.node_ptr_() }.ref_count, 3);

    // Test that grandchild is still valid, but empty.
    // Thanks to reference counting, this should not cause segfaults!
    assert_eq!(grandchild.node_name().unwrap(), "HEAD");
    assert_eq!(grandchild, grandchild2);
    assert_eq!(grandchild, grandchild3);
    assert!(!grandchild.has_child_nodes());
    assert!(!grandchild2.has_child_nodes());
    assert!(!grandchild3.has_child_nodes());

    // Decompose empty grandchild and its clones
    grandchild.decompose();
    assert!(grandchild.node_name().is_none());

    assert!(grandchild.node_ptr_().is_null());
    assert!(!grandchild2.node_ptr_().is_null());
    assert!(!grandchild3.node_ptr_().is_null());
    assert_eq!(unsafe { *grandchild2.node_ptr_() }.ref_count, 2);
    assert_eq!(unsafe { *grandchild3.node_ptr_() }.ref_count, 2);

    // Drop tree and document in the "wrong" order
    drop(tree);
    drop(doc);
    assert_eq!(grandchild3.node_name().unwrap(), "HEAD");

    // Decompose dangling grandchild2
    grandchild2.decompose();
    assert!(grandchild2.node_ptr_().is_null());
    assert!(!grandchild3.node_ptr_().is_null());
    assert_eq!(unsafe { *grandchild3.node_ptr_() }.ref_count, 1);
}
