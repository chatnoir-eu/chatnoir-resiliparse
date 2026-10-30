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
fn wrapped_character_nodes_preserve_kinds() {
    let tree = HtmlTree::parse("").unwrap();
    let document = tree.document().unwrap();
    let parent = document.create_element("div").unwrap();
    let cdata = document.create_cdata_section("data").unwrap();
    let instruction = document.create_processing_instruction("target", "value").unwrap();
    let comment = document.create_comment("comment").unwrap();
    parent.append(&[cdata.as_noderef(), instruction.as_noderef(), comment.as_noderef()]);
    // Traversal must recover concrete variants from the native pointers, not just their text.
    let nodes = parent.child_nodes().snapshot();
    assert!(matches!(nodes.item(0), Some(Node::CdataSection(_))));
    assert!(matches!(nodes.item(1), Some(Node::ProcessingInstruction(_))));
    assert!(matches!(nodes.item(2), Some(Node::Comment(_))));
    assert_eq!(nodes.item(0).unwrap(), &cdata.as_node());
    assert_eq!(nodes.item(1).unwrap(), &instruction.as_node());
    assert_eq!(nodes.item(2).unwrap(), &comment.as_node());
}

#[test]
fn attribute_wrapping_preserves_identity() {
    let tree = HtmlTree::parse("").unwrap();
    let document = tree.document().unwrap();
    let attribute = document.create_attribute("data-name").unwrap();
    attribute.set_value("value");
    // Attribute nodes cannot be child nodes, but the native wrapper still has to handle their kind.
    let wrapped = wrap_any_raw_node(attribute.tree(), attribute.node_ptr()).unwrap();
    assert!(matches!(wrapped, Node::Attribute(_)));
    assert_eq!(wrapped, attribute.as_node());
    assert_eq!(wrapped.node_value().as_deref(), Some("value"));
    assert!(wrap_any_raw_node(attribute.tree(), ptr::null_mut()).is_none());
}

#[test]
fn queries_on_decomposed_handles_are_empty() {
    let tree = HtmlTree::parse("<div><p id=found class=found data-test=yes>Text</p></div>").unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    root.decompose();
    // Exercise the helpers directly: live collections reject invalid handles before querying.
    unsafe {
        assert!(get_element_by_id(&root.as_noderef(), "found", false).is_none());
        assert!(get_elements_by_attr(&root.as_noderef(), "data-test", "yes", false).is_empty());
        assert!(get_elements_by_tag_name(&root.as_node(), "p").is_empty());
        assert!(get_elements_by_class_name(&root.as_node(), "found").is_empty());
    }
}
