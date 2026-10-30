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
use crate::parse::html::tree::HtmlTree;

#[test]
fn document_type_identifiers() {
    let tree = HtmlTree::parse(
        "<!DOCTYPE html PUBLIC '-//W3C//DTD HTML 4.01//EN' 'http://www.w3.org/TR/html4/strict.dtd'><p>Text</p>",
    )
    .unwrap();
    let doctype = tree.document().unwrap().doctype().unwrap();
    assert_eq!(doctype.name().as_deref(), Some("html"));
    assert_eq!(doctype.public_id().as_deref(), Some("-//W3C//DTD HTML 4.01//EN"));
    assert_eq!(doctype.system_id().as_deref(), Some("http://www.w3.org/TR/html4/strict.dtd"));
    let plain = HtmlTree::parse("<!doctype html>").unwrap();
    let doctype = plain.document().unwrap().doctype().unwrap();
    assert_eq!(doctype.public_id().as_deref(), Some(""));
    assert_eq!(doctype.system_id().as_deref(), Some(""));
}

#[test]
fn document_queries_are_live() {
    let tree = HtmlTree::parse("<p class=match data-label=Value>First</p>").unwrap();
    let document = tree.document().unwrap();
    let by_tag = document.get_elements_by_tag_name("p");
    let by_class = document.get_elements_by_class_name("match");
    let exact = document.get_elements_by_attr("data-label", "Value");
    let insensitive = document.get_elements_by_attr_case("data-label", "value", true);
    assert!(document.get_elements_by_attr("data-label", "value").is_empty());
    for collection in [&by_tag, &by_class, &exact, &insensitive] {
        assert_eq!(collection.len(), 1);
    }
    let added = document.create_element("p").unwrap();
    added.set_class_name("match");
    added.set_attribute("data-label", "Value");
    tree.body().unwrap().append_child(&added.as_noderef());
    for collection in [&by_tag, &by_class, &exact, &insensitive] {
        assert_eq!(collection.len(), 2);
        assert_eq!(collection.item(1).unwrap(), added);
    }
}

#[test]
fn invalid_document_rejects_node_creation() {
    let tree = HtmlTree::parse("").unwrap();
    let document = tree.document().unwrap();
    document.decompose();
    // A decomposed wrapper must fail before calling any native allocation function.
    let errors = [
        document.create_element("p").unwrap_err(),
        document.create_document_fragment().unwrap_err(),
        document.create_text_node("text").unwrap_err(),
        document.create_cdata_section("data").unwrap_err(),
        document.create_comment("comment").unwrap_err(),
        document.create_processing_instruction("target", "data").unwrap_err(),
        document.create_attribute("name").unwrap_err(),
    ];
    for error in errors {
        assert_eq!(error.to_string(), "DOM error: Invalid document.");
    }
}

#[test]
fn attribute_owner_tracks_attachment() {
    let tree = HtmlTree::parse("<p></p>").unwrap();
    let document = tree.document().unwrap();
    let element = tree.body().unwrap().first_element_child().unwrap();
    let attribute = document.create_attribute("data-key").unwrap();
    assert!(attribute.owner_element().is_none());
    attribute.set_value("original");
    element.set_attribute_node(&attribute);
    assert_eq!(attribute.owner_element().unwrap(), element);
    attribute.set_value("changed");
    assert_eq!(element.attribute("data-key").as_deref(), Some("changed"));
    element.remove_attribute("data-key");
    assert!(attribute.owner_element().is_none());
    assert_eq!(attribute.value().as_deref(), Some("changed"));
}

#[test]
fn attribute_toggle_preserves_existing_value() {
    let tree = HtmlTree::parse("<p data-key=value></p>").unwrap();
    let element = tree.body().unwrap().first_element_child().unwrap();
    assert!(element.toggle_attribute("data-key", Some(true)));
    assert_eq!(element.attribute("data-key").as_deref(), Some("value"));
    assert!(!element.toggle_attribute("data-key", None));
    assert!(!element.has_attribute("data-key"));
    assert!(element.toggle_attribute("data-key", None));
    assert_eq!(element.attribute("data-key").as_deref(), Some(""));
    assert!(!element.toggle_attribute("data-key", Some(false)));
    assert!(!element.toggle_attribute("data-key", Some(false)));
}

#[test]
fn borrowed_handles_preserve_identity_and_mutations() {
    let tree = HtmlTree::parse("<p>Old</p>").unwrap();
    let mut element = tree.body().unwrap().first_element_child().unwrap();
    let generic = element.as_node();
    assert_eq!(element, generic);
    assert_eq!(generic, element);
    assert_eq!(element.as_noderef(), element);
    assert_eq!(element, element.as_noderef());
    assert_eq!(element.as_noderef().to_string(), "<p>Old</p>");
    let mut borrowed = element.as_noderef_mut();
    assert_eq!(borrowed, generic.as_element().unwrap().clone());
    assert_eq!(generic.as_element().unwrap(), &borrowed);
    assert_eq!(borrowed.to_string(), "<p>Old</p>");
    // Dereferencing either borrowed view must affect the same native node.
    let reference: &dyn NodeInterface = &*borrowed;
    reference.set_text_content("First");
    let reference: &mut dyn NodeInterface = &mut *borrowed;
    reference.set_text_content("Second");
    assert_eq!(generic.text_content().as_deref(), Some("Second"));
}

#[test]
fn invalid_handles_have_no_identity() {
    let tree = HtmlTree::parse("<p>Text</p>").unwrap();
    let element = tree.body().unwrap().first_element_child().unwrap();
    element.decompose();
    let clone = element.clone();
    assert_ne!(element, clone);
    assert!(clone.identity().is_none());
    assert_eq!(format!("{element:?}"), "<#undef>");
    assert_eq!(clone.to_string(), "");
}

#[test]
fn outer_html_ignores_unreplaceable_elements() {
    let tree = HtmlTree::parse("<p>Text</p>").unwrap();
    let document = tree.document().unwrap();
    let detached = document.create_element("p").unwrap();
    detached.set_outer_html("<div>Replacement</div>");
    assert_eq!(detached.outer_html(), "<p></p>");
    let root = document.document_element().unwrap();
    let before = root.outer_html();
    // Replacing the document element through outer_html is explicitly unsupported.
    root.set_outer_html("<p>Replacement</p>");
    assert_eq!(root.outer_html(), before);
}

#[test]
fn text_setters_do_not_parse_markup() {
    let tree = HtmlTree::parse("<p><b>Old</b></p>").unwrap();
    let element = tree.body().unwrap().first_element_child().unwrap();
    element.set_inner_text("<i>Literal</i>");
    assert_eq!(element.inner_html(), "&lt;i&gt;Literal&lt;/i&gt;");
    assert_eq!(element.inner_text(), "<i>Literal</i>");
}

#[test]
fn identical_markup_in_different_documents_is_not_equal() {
    // Node equality is native identity, not a comparison of markup or node contents.
    let first = HtmlTree::parse("<p>Same</p>")
        .unwrap()
        .body()
        .unwrap()
        .first_element_child()
        .unwrap();
    let second = HtmlTree::parse("<p>Same</p>")
        .unwrap()
        .body()
        .unwrap()
        .first_element_child()
        .unwrap();
    assert_eq!(first.outer_html(), second.outer_html());
    assert_ne!(first, second);
    assert_ne!(first.as_node(), second.as_node());
}

#[test]
fn parent_children_exclude_non_elements() {
    let tree = HtmlTree::parse("<!doctype html><!-- comment --><p>Text</p>").unwrap();
    let document = tree.document().unwrap();
    assert_eq!(document.children().len(), 1);
    assert_eq!(document.children().item(0).unwrap().local_name().as_deref(), Some("html"));
    let fragment = document.create_document_fragment().unwrap();
    let text = document.create_text_node("text").unwrap();
    let comment = document.create_comment("comment").unwrap();
    let first = document.create_element("a").unwrap();
    let second = document.create_element("b").unwrap();
    let children = fragment.children();
    fragment.append(&[
        text.as_noderef(),
        first.as_noderef(),
        comment.as_noderef(),
        second.as_noderef(),
    ]);
    assert_eq!(children.values(), [first.clone(), second.clone()]);
    first.remove();
    assert_eq!(children.values(), [second]);
}
