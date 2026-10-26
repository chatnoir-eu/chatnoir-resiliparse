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
use crate::parse::html::dom::iter::TraversalError;
use crate::parse::html::dom::node::*;
use crate::parse::html::dom::traits::*;
use crate::parse::html::tree::HtmlTree;

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
const HTML_NO_TITLE_SVG: &str =
    "<!doctype html><svg xmlns=\"http://www.w3.org/2000/svg\"><title>SVG Title</title></svg>";
const HTML_UNCLOSED_TITLE: &str = "<!doctype html><html><head><title>Test--></head><body>foo</body></html>";
const HTML_UNCLOSED_HEAD: &str = "<!doctype html><head><title>Title</title><span></span>";

#[test]
fn parse_from_str() {
    let _tree1 = HtmlTree::from_str(HTML).unwrap();
    let _tree2 = HtmlTree::from_str("<html></html>").unwrap();
}

#[test]
fn parse_from_string() {
    let _tree1 = HtmlTree::try_from(HTML.to_owned()).unwrap();
}

#[test]
fn parse_from_bytes() {
    let _tree3 = HtmlTree::try_from(HTML.as_bytes()).unwrap();
}

#[test]
fn test_parse_quirks() {
    let tree_quirk = HtmlTree::from_str(HTML_NO_HEAD).unwrap();
    tree_quirk.document().unwrap();
    assert_eq!(tree_quirk.head().unwrap().child_nodes().len(), 0);
    assert_eq!(tree_quirk.body().unwrap().child_nodes().len(), 1);

    let tree_quirk = HtmlTree::from_str(HTML_NO_BODY).unwrap();
    tree_quirk.document().unwrap();
    assert_eq!(tree_quirk.head().unwrap().child_nodes().len(), 1);
    assert_eq!(tree_quirk.title().unwrap(), "Title");
    assert_eq!(tree_quirk.body().unwrap().child_nodes().len(), 0);

    let tree_quirk = HtmlTree::from_str(HTML_NO_TITLE).unwrap();
    tree_quirk.document().unwrap();
    assert_eq!(tree_quirk.head().unwrap().child_nodes().len(), 0);
    assert!(tree_quirk.title().is_none());
    assert_eq!(tree_quirk.body().unwrap().child_nodes().len(), 0);

    let tree_quirk = HtmlTree::from_str(HTML_UNCLOSED_TITLE).unwrap();
    tree_quirk.document().unwrap();
    assert_eq!(tree_quirk.head().unwrap().child_nodes().len(), 1);
    assert_eq!(tree_quirk.title().unwrap(), "Test--></head><body>foo</body></html>");
    assert_eq!(tree_quirk.body().unwrap().child_nodes().len(), 0);

    let tree_quirk = HtmlTree::from_str(HTML_NO_TITLE_SVG).unwrap();
    tree_quirk.document().unwrap();
    tree_quirk.head().unwrap();
    assert!(tree_quirk.title().is_none());
    assert_eq!(tree_quirk.body().unwrap().child_nodes().len(), 1);

    let tree_quirk = HtmlTree::from_str(HTML_UNCLOSED_HEAD).unwrap();
    tree_quirk.document().unwrap();
    tree_quirk.head().unwrap();
    assert_eq!(tree_quirk.head().unwrap().child_nodes().len(), 1);
    assert_eq!(tree_quirk.title().unwrap(), "Title");
    assert_eq!(tree_quirk.body().unwrap().child_nodes().len(), 1);
}

#[test]
fn test_node_equality() {
    let tree = HtmlTree::from_str(HTML).unwrap();

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
fn owned_dom_handles_are_send_and_can_be_shared_with_mutexes() {
    use crate::parse::html::dom::iter::NodeIteratorOwned;
    use std::sync::Mutex;
    fn assert_send<T: Send>() {}
    fn assert_mutex_sync<T: Send>() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Mutex<T>>();
    }
    assert_send::<HtmlTree>();
    assert_send::<Node>();
    assert_send::<NodeIteratorOwned>();
    assert_send::<NodeList>();
    assert_send::<ElementNodeList>();
    assert_send::<NamedNodeMap>();
    assert_send::<DomTokenListOwned>();
    assert_mutex_sync::<HtmlTree>();
    assert_mutex_sync::<Node>();
    assert_mutex_sync::<NodeIteratorOwned>();
}

#[test]
fn users_can_share_owned_handles_with_a_mutex() {
    use std::sync::{Arc, Mutex};
    let tree = HtmlTree::parse("<div></div>").unwrap();
    let node = Arc::new(Mutex::new(tree.body().unwrap().first_element_child().unwrap()));
    let workers: Vec<_> = (0..2)
        .map(|worker| {
            let node = node.clone();
            std::thread::spawn(move || {
                for value in 0..32 {
                    node.lock()
                        .unwrap()
                        .set_attribute(&format!("data-{worker}"), &value.to_string());
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    let node = node.lock().unwrap();
    assert_eq!(node.attribute("data-0").as_deref(), Some("31"));
    assert_eq!(node.attribute("data-1").as_deref(), Some("31"));
}

#[test]
fn containment_includes_the_receiver_and_aliases() {
    let tree = HtmlTree::parse("<div><span>text</span></div>").unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    let alias = root.clone();
    let child = root.first_child().unwrap();
    assert!(root.contains(&root.as_noderef()));
    assert!(root.contains(&alias.as_noderef()));
    assert!(root.contains(&child.as_noderef()));
    assert!(!child.contains(&root.as_noderef()));

    let other = HtmlTree::parse("<div></div>").unwrap();
    assert!(!root.contains(&other.body().unwrap().as_noderef()));
    let invalid = tree.document().unwrap().create_element("i").unwrap();
    invalid.decompose();
    assert!(!invalid.contains(&invalid.as_noderef()));
    assert!(!root.contains(&invalid.as_noderef()));
}

#[test]
fn inserting_a_child_before_itself_is_a_noop() {
    let tree = HtmlTree::parse("<div><a></a><b></b><i></i></div>").unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    let child = root.first_child().unwrap().next_sibling().unwrap();
    let alias = child.clone();
    let iter = root.clone().into_iter();
    let result = root.insert_before(&child.as_noderef(), Some(alias.as_noderef()));
    assert_eq!(result.unwrap(), child.as_noderef());
    assert_eq!(root.inner_html(), "<a></a><b></b><i></i>");
    // Successful identity operations must not invalidate active traversal.
    assert_eq!(iter.collect::<Result<Vec<_>, _>>().unwrap().len(), 4);
}

#[test]
fn replacing_a_child_with_itself_is_a_noop() {
    let tree = HtmlTree::parse("<div><a></a><b></b><i></i></div>").unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    let child = root.first_child().unwrap().next_sibling().unwrap();
    let alias = child.clone();
    let iter = root.clone().into_iter();
    assert_eq!(root.replace_child(&child.as_noderef(), &alias.as_noderef()).unwrap(), child.as_noderef());
    assert_eq!(root.inner_html(), "<a></a><b></b><i></i>");
    assert_eq!(iter.collect::<Result<Vec<_>, _>>().unwrap().len(), 4);
}

#[test]
fn identity_noops_still_require_valid_children() {
    let tree = HtmlTree::parse("<div><a></a></div><section><b></b></section>").unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    let child = root.first_child().unwrap();
    let outsider = tree
        .body()
        .unwrap()
        .last_element_child()
        .unwrap()
        .first_child()
        .unwrap();
    let alias = root.as_node();
    let invalid = tree.document().unwrap().create_element("i").unwrap();
    invalid.decompose();
    let iter = root.clone().into_iter();

    assert!(
        root.insert_before(&outsider.as_noderef(), Some(outsider.as_noderef()))
            .is_none()
    );
    assert!(
        root.replace_child(&outsider.as_noderef(), &outsider.as_noderef())
            .is_none()
    );
    assert!(root.append_child(&alias.as_noderef()).is_none());
    assert!(
        root.insert_before(&alias.as_noderef(), Some(child.as_noderef()))
            .is_none()
    );
    assert!(root.insert_before(&alias.as_noderef(), None).is_none());
    assert!(root.replace_child(&alias.as_noderef(), &child.as_noderef()).is_none());
    assert!(root.remove_child(&alias.as_noderef()).is_none());
    assert!(
        root.insert_before(&invalid.as_noderef(), Some(invalid.as_noderef()))
            .is_none()
    );
    assert!(
        root.replace_child(&invalid.as_noderef(), &invalid.as_noderef())
            .is_none()
    );

    let other = HtmlTree::parse("<p>foreign</p>").unwrap();
    let foreign = other.body().unwrap().first_child().unwrap();
    assert!(
        root.insert_before(&foreign.as_noderef(), Some(foreign.as_noderef()))
            .is_none()
    );
    assert!(
        root.replace_child(&foreign.as_noderef(), &foreign.as_noderef())
            .is_none()
    );
    assert_eq!(root.inner_html(), "<a></a>");
    assert_eq!(iter.collect::<Result<Vec<_>, _>>().unwrap().len(), 2);
}

// Bound both walks by the expected length so broken links/cycles fail rather
// than hanging. Check ownership and both directions, not just serialization.
fn assert_child_links(parent: &impl NodeInterface, expected: &[Node]) {
    assert_eq!(parent.first_child(), expected.first().cloned());
    assert_eq!(parent.last_child(), expected.last().cloned());
    let mut next = parent.first_child();
    for (index, child) in expected.iter().enumerate() {
        assert_eq!(next.as_ref(), Some(child));
        assert_eq!(child.parent_node(), Some(parent.as_node()));
        assert_eq!(child.previous_sibling(), index.checked_sub(1).map(|i| expected[i].clone()));
        next = child.next_sibling();
    }
    assert!(next.is_none());
    let mut previous = parent.last_child();
    for child in expected.iter().rev() {
        assert_eq!(previous.as_ref(), Some(child));
        previous = child.previous_sibling();
    }
    assert!(previous.is_none());
}

#[test]
fn moving_nodes_repairs_both_parents() {
    for operation in ["append", "insert", "replace"] {
        for index in 0..3 {
            let tree = HtmlTree::parse(
                "<section><a><b>nested</b></a><i></i><em></em></section><div><p></p><span></span></div>",
            )
            .unwrap();
            let source = tree.body().unwrap().first_element_child().unwrap();
            let target = tree.body().unwrap().last_element_child().unwrap();
            let mut source_children = source.child_nodes().values();
            let mut target_children = target.child_nodes().values();
            let moved = source_children.remove(index);
            let original_html = moved.to_string();
            match operation {
                "append" => {
                    assert_eq!(target.append_child(&moved.as_noderef()).unwrap(), moved.as_noderef());
                    target_children.push(moved.clone());
                }
                "insert" => {
                    assert_eq!(
                        target
                            .insert_before(&moved.as_noderef(), Some(target_children[1].as_noderef()))
                            .unwrap(),
                        moved.as_noderef()
                    );
                    target_children.insert(1, moved.clone());
                }
                "replace" => {
                    let removed = target_children[0].clone();
                    assert_eq!(
                        target
                            .replace_child(&moved.as_noderef(), &removed.as_noderef())
                            .unwrap(),
                        removed.as_noderef()
                    );
                    assert!(removed.parent_node().is_none());
                    assert!(removed.previous_sibling().is_none());
                    assert!(removed.next_sibling().is_none());
                    target_children[0] = moved.clone();
                }
                _ => unreachable!(),
            }
            assert_child_links(&source, &source_children);
            assert_child_links(&target, &target_children);
            // Destroying the former parent must not visit the transferred node.
            source.decompose();
            assert_eq!(moved.to_string(), original_html);
            assert_child_links(&target, &target_children);
        }
    }
}

#[test]
fn moving_siblings_repairs_first_last_and_adjacent_links() {
    let cases: &[(&str, usize, usize, &[usize])] = &[
        ("append", 0, 0, &[1, 2, 0]),
        ("append", 1, 0, &[0, 2, 1]),
        ("append", 2, 0, &[0, 1, 2]),
        ("insert", 2, 0, &[2, 0, 1]),
        ("insert", 0, 2, &[1, 0, 2]),
        ("insert", 1, 0, &[1, 0, 2]),
        ("replace", 2, 0, &[2, 1]),
        ("replace", 0, 2, &[1, 0]),
        ("replace", 2, 1, &[0, 2]),
    ];
    for &(operation, moved_index, reference_index, order) in cases {
        let tree = HtmlTree::parse("<div><a></a><b></b><i></i></div>").unwrap();
        let parent = tree.body().unwrap().first_element_child().unwrap();
        let children = parent.child_nodes().values();
        let moved = children[moved_index].as_noderef();
        let reference = children[reference_index].as_noderef();
        match operation {
            "append" => {
                parent.append_child(&moved).unwrap();
            }
            "insert" => {
                parent.insert_before(&moved, Some(reference)).unwrap();
            }
            "replace" => {
                parent.replace_child(&moved, &reference).unwrap();
            }
            _ => unreachable!(),
        }
        let expected: Vec<_> = order.iter().map(|&index| children[index].clone()).collect();
        assert_child_links(&parent, &expected);
    }
}

#[test]
fn moving_fragment_children_empties_the_fragment_and_preserves_subtrees() {
    for operation in ["append", "insert", "replace"] {
        let tree = HtmlTree::parse(
            "<section>before<a><b>nested</b></a><!--comment-->after</section><div><i></i><em></em></div>",
        )
        .unwrap();
        let source = tree.body().unwrap().first_element_child().unwrap();
        let target = tree.body().unwrap().last_element_child().unwrap();
        let children = source.child_nodes().values();
        let original_html: Vec<_> = children.iter().map(ToString::to_string).collect();
        let fragment = tree.document().unwrap().create_document_fragment().unwrap();
        fragment.append(&children.iter().map(|node| node.as_noderef()).collect::<Vec<_>>());
        assert_child_links(&source, &[]);
        assert_child_links(&fragment, &children);
        let mut expected = target.child_nodes().values();
        match operation {
            "append" => {
                target.append_child(&fragment.as_noderef()).unwrap();
                expected.extend(children.iter().cloned());
            }
            "insert" => {
                target
                    .insert_before(&fragment.as_noderef(), Some(expected[1].as_noderef()))
                    .unwrap();
                expected.splice(1..1, children.iter().cloned());
            }
            "replace" => {
                let removed = expected[0].clone();
                target
                    .replace_child(&fragment.as_noderef(), &removed.as_noderef())
                    .unwrap();
                assert!(removed.parent_node().is_none());
                expected.splice(0..1, children.iter().cloned());
            }
            _ => unreachable!(),
        }
        assert_child_links(&fragment, &[]);
        assert_child_links(&target, &expected);
        fragment.decompose();
        assert_eq!(children.iter().map(ToString::to_string).collect::<Vec<_>>(), original_html);
        assert_child_links(&target, &expected);
    }
}

#[test]
fn invalid_moves_leave_source_links_and_iterators_unchanged() {
    let tree = HtmlTree::parse("<section><a></a><div><i></i></div></section><aside></aside>").unwrap();
    let source = tree.body().unwrap().first_element_child().unwrap();
    let target = source.last_element_child().unwrap();
    let moving = source.first_child().unwrap();
    let reference = target.first_child().unwrap();
    let outsider = tree.body().unwrap().last_child().unwrap();
    let iter = source.clone().into_iter();
    assert!(
        target
            .insert_before(&moving.as_noderef(), Some(outsider.as_noderef()))
            .is_none()
    );
    assert!(
        target
            .replace_child(&moving.as_noderef(), &outsider.as_noderef())
            .is_none()
    );
    assert!(target.append_child(&source.as_noderef()).is_none());
    assert!(
        target
            .insert_before(&source.as_noderef(), Some(reference.as_noderef()))
            .is_none()
    );
    assert!(
        target
            .replace_child(&source.as_noderef(), &reference.as_noderef())
            .is_none()
    );
    let other = HtmlTree::parse("<div></div>").unwrap();
    assert!(other.body().unwrap().append_child(&moving.as_noderef()).is_none());
    assert_child_links(&source, &[moving, target.as_node()]);
    assert_child_links(&target, &[reference]);
    assert_eq!(iter.collect::<Result<Vec<_>, _>>().unwrap().len(), 4);
    // A sole child moved out must clear both endpoints of its former parent.
    source
        .append_child(&target.first_child().unwrap().as_noderef())
        .unwrap();
    assert_child_links(&target, &[]);
}

#[test]
fn fragment_cycle_is_rejected_before_moving_any_children() {
    let tree = HtmlTree::parse("<div><i></i></div><section><span></span></section>").unwrap();
    let children = tree.body().unwrap().child_nodes().values();
    let fragment = tree.document().unwrap().create_document_fragment().unwrap();
    fragment.append(&children.iter().map(|node| node.as_noderef()).collect::<Vec<_>>());
    let mut destination = tree.body().unwrap();
    // The destination is inside the fragment's second child. Moving the first
    // child before detecting the cycle would leave a partially moved fragment.
    if let Node::Element(element) = &children[1] {
        destination = element.first_element_child().unwrap();
    }
    let doc = tree.document().unwrap();
    let leaf = doc.create_element("b").unwrap();
    destination.append_child(&leaf.as_noderef()).unwrap();
    let iter = fragment.clone().into_iter();
    assert!(destination.append_child(&fragment.as_noderef()).is_none());
    assert!(
        destination
            .insert_before(&fragment.as_noderef(), Some(leaf.as_noderef()))
            .is_none()
    );
    assert!(
        destination
            .replace_child(&fragment.as_noderef(), &leaf.as_noderef())
            .is_none()
    );
    assert_child_links(&fragment, &children);
    assert_child_links(&destination, &[leaf.as_node()]);
    assert_eq!(iter.collect::<Result<Vec<_>, _>>().unwrap().len(), 6);
}

#[test]
fn outer_text_detaches_the_promoted_text_node() {
    let tree = HtmlTree::parse("<div><i>before</i><p><b>old</b></p><em>after</em></div>").unwrap();
    let parent = tree.body().unwrap().first_element_child().unwrap();
    let old = parent.query_selector("p").unwrap().unwrap();
    let first = parent.first_child().unwrap();
    let last = parent.last_child().unwrap();
    old.set_outer_text("replacement");
    let text = first.next_sibling().unwrap();
    assert_eq!(text.node_value().unwrap(), "replacement");
    assert_child_links(&old, &[]);
    assert!(old.parent_node().is_none());
    old.decompose();
    assert_child_links(&parent, &[first, text, last]);
    assert_eq!(parent.inner_html(), "<i>before</i>replacement<em>after</em>");
}

#[test]
fn test_selection() {
    let tree = HtmlTree::from_str(HTML).unwrap();

    tree.document().unwrap().get_element_by_id("foo").unwrap();
    assert_eq!(
        tree.document()
            .unwrap()
            .get_element_by_id("foo")
            .unwrap()
            .node_name()
            .unwrap(),
        "MAIN"
    );

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

    let match_css = tree
        .document()
        .unwrap()
        .query_selector("body > main p:last-child")
        .unwrap()
        .unwrap();
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
    assert!(
        tree.body()
            .unwrap()
            .first_element_child()
            .unwrap()
            .matches("#foo")
            .unwrap()
    );
    assert!(
        tree.body()
            .unwrap()
            .first_element_child()
            .unwrap()
            .matches("main")
            .unwrap()
    );
    assert!(!tree.body().unwrap().matches(".barbaz").unwrap());
    assert!(
        !tree
            .body()
            .unwrap()
            .first_element_child()
            .unwrap()
            .matches("div")
            .unwrap()
    );

    // Find the closest matching ancestor (or self)
    assert_eq!(tree.body().unwrap().closest("body").unwrap().unwrap(), tree.body().unwrap());
    assert!(tree.body().unwrap().closest("htmlx").unwrap().is_none());
    assert_eq!(
        tree.document()
            .unwrap()
            .query_selector(".bar.baz")
            .unwrap()
            .unwrap()
            .closest("[id]")
            .unwrap()
            .unwrap(),
        tree.document().unwrap().query_selector("p#b").unwrap().unwrap()
    );
    assert_eq!(
        tree.body().unwrap().closest("html").unwrap().unwrap(),
        tree.document().unwrap().first_element_child().unwrap()
    );

    let child = tree.body().unwrap().query_selector(".bar").unwrap().unwrap();
    assert_eq!(child.closest("p").unwrap().unwrap(), tree.body().unwrap().query_selector("#a").unwrap().unwrap());
    assert_eq!(
        child.closest("main").unwrap().unwrap(),
        tree.body().unwrap().query_selector("#foo").unwrap().unwrap()
    );

    // Invalid CSS selectors
    assert!(tree.body().unwrap().query_selector("#a < abc").is_err());
    assert!(tree.body().unwrap().query_selector_all("#a < abc").is_err());
}

#[test]
fn test_static_collection() {
    let tree = HtmlTree::from_str(HTML).unwrap();
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
    coll.elements_by_class_name("bar")
        .iter()
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
    let tree = HtmlTree::from_str(HTML).unwrap();
    let coll1 = tree.body().unwrap().get_elements_by_tag_name("p");
    let coll2 = tree.body().unwrap().get_elements_by_class_name("bar");
    let coll3 = tree.body().unwrap().query_selector_all("p").unwrap();

    assert_eq!(coll1.len(), 2);
    assert_eq!(coll2.len(), 2);
    assert_eq!(coll3.len(), 2);

    tree.document().unwrap().get_element_by_id("a").unwrap().remove();
    assert_eq!(coll1.len(), 1);
    assert_eq!(coll2.len(), 1);
    assert_eq!(coll3.len(), 2); // CSS match collection are static
}

#[test]
fn test_class_list() {
    let tree = HtmlTree::from_str(HTML).unwrap();
    let a = tree.body().unwrap().query_selector("#b a").unwrap().unwrap();

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
    let tree = HtmlTree::from_str(HTML).unwrap();
    let element = tree.body().unwrap().query_selector("#b a").unwrap().unwrap();

    // List attribute nodes and names
    assert_eq!(
        element
            .attributes()
            .iter()
            .zip(element.attribute_names().iter())
            .map(|(a1, a2)| {
                assert_eq!(a1.name().unwrap(), *a2);
                assert_eq!(a1.value(), element.attribute(a1.name().unwrap().as_str()));
                assert!(a1.value().is_some());
            })
            .count(),
        2
    );

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
    let attr = element.attribute_node("href").unwrap();
    assert!(attr.parent_node().is_none());
    assert_eq!(attr.name().unwrap(), "href");
    assert_eq!(attr.local_name().unwrap(), attr.name().unwrap());
    assert_eq!(attr.value().unwrap(), "https://example.com");
    assert_eq!(attr.node_value().unwrap(), attr.value().unwrap());
    assert_eq!(attr.child_nodes().len(), 0);

    // Cannot append children to attributes
    assert!(
        attr.append_child(&tree.document().unwrap().create_element("foo").unwrap().as_noderef())
            .is_none()
    );
    assert_eq!(attr.child_nodes().len(), 0);
}

#[test]
fn test_empty_attribute() {
    let tree = HtmlTree::from_str(
        r#"<div>
        <input type="checkbox" checked>
        <div class="foo"></div>
        <div class></div>
        <div class=""></div>
        <div id="foo"></div>
        <div id></div>
        <div id=""></div>
        <div foo></div>
        <div foo=""></div>"#,
    )
    .unwrap();

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
    assert_eq!(tree.body().unwrap().get_elements_by_class_name("").len(), 0); // This shouldn't match anything);
    assert_eq!(tree.body().unwrap().get_elements_by_attr("class", "foo").len(), 1);
    assert_eq!(tree.body().unwrap().get_elements_by_attr("class", "").len(), 2);
    assert_eq!(tree.body().unwrap().get_elements_by_attr("id", "").len(), 2);
    assert_eq!(tree.body().unwrap().get_elements_by_attr("foo", "").len(), 2);
}

#[test]
fn test_serialization() {
    let tree = HtmlTree::from_str(HTML).unwrap();

    assert_eq!(tree.document().unwrap().get_element_by_id("a").unwrap().outer_text(), "Hello world!");
    assert_eq!(
        tree.document().unwrap().get_element_by_id("a").unwrap().outer_html(),
        r#"<p id="a">Hello <span class="bar">world</span>!</p>"#
    );

    assert_eq!(tree.document().unwrap().get_element_by_id("a").unwrap().inner_text(), "Hello world!");
    assert_eq!(
        tree.document().unwrap().get_element_by_id("a").unwrap().inner_html(),
        r#"Hello <span class="bar">world</span>!"#
    );

    assert_eq!(
        tree.head()
            .unwrap()
            .query_selector("title")
            .unwrap()
            .unwrap()
            .to_string(),
        "<title>Example page</title>"
    );

    assert_eq!(format!("{:?}", tree.head().unwrap()), "<head>");
    assert_eq!(format!("{:?}", tree.head().unwrap().query_selector("title").unwrap().unwrap()), "<title>");
    assert_eq!(format!("{:?}", tree.body().unwrap().query_selector("main").unwrap().unwrap()), r#"<main id="foo">"#);

    let text = tree
        .body()
        .unwrap()
        .query_selector("#b")
        .unwrap()
        .unwrap()
        .first_child()
        .unwrap();
    match text {
        Node::Text(t) => assert_eq!(t.node_value().unwrap(), "Hello "),
        _ => assert!(false, "Node is not a text node."),
    }

    let inputs = HtmlTree::from_str(
        r#"
        <input id="a" type="checkbox" checked>
        <input id="b" type="checkbox" checked="">
        <input id="c" type="checkbox" checked="checked">
        "#,
    )
    .unwrap();
    assert_eq!(
        format!("{:?}", inputs.document().unwrap().get_element_by_id("a").unwrap()),
        r#"<input id="a" type="checkbox" checked>"#
    );
    assert_eq!(
        format!("{:?}", inputs.document().unwrap().get_element_by_id("b").unwrap()),
        r#"<input id="b" type="checkbox" checked="">"#
    );
    assert_eq!(
        format!("{:?}", inputs.document().unwrap().get_element_by_id("c").unwrap()),
        r#"<input id="c" type="checkbox" checked="checked">"#
    );

    assert_eq!(format!("{:?}", tree.body().unwrap().query_selector("main").unwrap().unwrap()), r#"<main id="foo">"#);
}

#[test]
fn test_traversal() {
    let tree = HtmlTree::from_str(HTML).unwrap();
    let root = tree.document().unwrap().get_element_by_id("a").unwrap();

    let node_names = root
        .iter()
        .map(|n| n.unwrap().node_name().unwrap())
        .collect::<Vec<String>>();
    assert_eq!(node_names, ["P", "#text", "SPAN", "#text", "#text"]);

    let tag_names = root
        .iter()
        .flat_map(|n| {
            if let Node::Element(e) = n.unwrap() {
                e.tag_name()
            } else {
                None
            }
        })
        .collect::<Vec<String>>();
    assert_eq!(tag_names, ["P", "SPAN"]);

    let child_nodes = tree.document().unwrap().get_element_by_id("foo").unwrap().child_nodes();
    let child_node_names = child_nodes
        .iter()
        .map(|n| n.node_name().unwrap())
        .collect::<Vec<String>>();
    assert_eq!(child_node_names, ["#text", "P", "#text", "P", "#text"]);

    child_nodes.iter().for_each(|n| match n {
        Node::Text(n) => assert_eq!(n.node_name().unwrap(), "#text"),
        Node::Element(n) => assert_eq!(n.node_name().unwrap(), "P"),
        _ => assert!(false, "Invalid node type"),
    });
}

#[test]
fn test_callback_traversal() {}

#[test]
fn element_iterator_visits_only_elements_once() {
    let tree = HtmlTree::parse("<div>one<span>two</span><!-- comment --><b>three</b></div>").unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    let elements = root.iter_elements().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(elements.iter().map(|e| e.tag_name().unwrap()).collect::<Vec<_>>(), ["DIV", "SPAN", "B"]);
    assert!(elements.iter().all(|e| e.node_type() == Some(NodeType::Element)));
    assert_eq!(elements[0], root);
    assert_eq!(root.last_element_child().unwrap().iter_elements().count(), 1);
}

#[test]
fn sibling_mutations_preserve_input_order() {
    let tree = HtmlTree::parse("<div><i>old</i></div>").unwrap();
    let doc = tree.document().unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    let old = root.first_element_child().unwrap();
    let a = doc.create_element("a").unwrap().into_node();
    let b = doc.create_element("b").unwrap().into_node();
    old.before(&[a.as_noderef(), b.as_noderef()]);
    assert_eq!(root.inner_html(), "<a></a><b></b><i>old</i>");

    let c = doc.create_element("strong").unwrap().into_node();
    let d = doc.create_text_node("end").unwrap().into_node();
    old.after(&[c.as_noderef(), d.as_noderef()]);
    assert_eq!(root.inner_html(), "<a></a><b></b><i>old</i><strong></strong>end");

    // Move already attached siblings, repairing their former positions.
    old.replace_with(&[c.as_noderef(), b.as_noderef()]);
    assert_eq!(root.inner_html(), "<a></a><strong></strong><b></b>end");
    assert!(old.parent_node().is_none());
    assert_eq!(old.inner_html(), "old");
    assert_eq!(root.last_child().unwrap().previous_sibling().unwrap(), b);
}

#[test]
fn prepend_preserves_order_and_handles_existing_children() {
    let tree = HtmlTree::parse("<div><a></a><b></b><i></i></div>").unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    let a = root.first_child().unwrap();
    let b = a.next_sibling().unwrap();
    root.prepend(&[b.as_noderef(), a.as_noderef()]);
    assert_eq!(root.inner_html(), "<b></b><a></a><i></i>");

    let doc = tree.document().unwrap();
    let empty = doc.create_element("div").unwrap();
    let c = doc.create_element("strong").unwrap();
    let d = doc.create_text_node("end").unwrap();
    empty.prepend(&[c.as_noderef(), d.as_noderef()]);
    assert_eq!(empty.inner_html(), "<strong></strong>end");
}

#[test]
fn sibling_mutations_handle_the_receiver_in_the_input() {
    let tree = HtmlTree::parse("<div><a></a><b></b><i></i></div>").unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    let a = root.first_element_child().unwrap();
    let same_a = a.as_node();
    let b = a.next_sibling().unwrap();
    a.before(&[b.as_noderef(), same_a.as_noderef()]);
    assert_eq!(root.inner_html(), "<b></b><a></a><i></i>");

    a.replace_with(&[same_a.as_noderef(), b.as_noderef()]);
    assert_eq!(root.inner_html(), "<a></a><b></b><i></i>");
    assert_eq!(a.parent_node().unwrap(), root.as_node());

    a.after(&[b.as_noderef()]);
    assert_eq!(root.inner_html(), "<a></a><b></b><i></i>");
    a.replace_with(&[]);
    assert_eq!(root.inner_html(), "<b></b><i></i>");
    assert!(a.parent_node().is_none());
}

#[test]
fn outer_html_replaces_only_the_receiver_and_preserves_nesting() {
    let tree = HtmlTree::parse("<div>before<p><i>original</i></p>after</div>").unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    let old = root.first_element_child().unwrap();
    old.set_outer_html("<section><b>nested</b>text</section><span>second</span>");
    assert_eq!(root.inner_html(), "before<section><b>nested</b>text</section><span>second</span>after");
    assert!(old.parent_node().is_none());
    assert_eq!(old.inner_html(), "<i>original</i>");
    root.first_element_child().unwrap().set_outer_html("");
    assert_eq!(root.inner_html(), "before<span>second</span>after");
}

#[test]
fn outer_html_uses_the_parent_parsing_context() {
    let tree = HtmlTree::parse("<table><tbody><tr><td>old</td></tr></tbody></table>").unwrap();
    let tbody = tree.body().unwrap().query_selector("tbody").unwrap().unwrap();
    tbody
        .first_element_child()
        .unwrap()
        .set_outer_html("<tr><td><b>new</b></td></tr><tr><td>two</td></tr>");
    assert_eq!(tbody.inner_html(), "<tr><td><b>new</b></td></tr><tr><td>two</td></tr>");

    let doc = tree.document().unwrap();
    let fragment = doc.create_document_fragment().unwrap();
    let child = doc.create_element("p").unwrap();
    fragment.append_child(&child.as_noderef());
    child.set_outer_html("<b>one</b><i>two</i>");
    assert_eq!(fragment.first_child().unwrap().node_name().unwrap(), "B");
    assert_eq!(fragment.last_child().unwrap().node_name().unwrap(), "I");
    assert!(child.parent_node().is_none());
}

#[test]
fn node_iterator_reports_subtree_destruction_once() {
    let tree = HtmlTree::parse("<div>one<span>two</span><!-- comment --></div>").unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    let alias = root.clone();
    let mut iter = root.iter();
    assert_eq!(iter.next().unwrap().unwrap(), root.as_node());
    // This destroys the pending child. No stale native pointer may be followed.
    alias.decompose();
    assert_eq!(iter.next(), Some(Err(TraversalError::TreeModified)));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
}

#[test]
fn owned_iterator_reports_reparenting() {
    let tree = HtmlTree::parse("<div><a></a><b></b></div><section></section>").unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    let a = root.first_child().unwrap();
    let other = tree.body().unwrap().last_element_child().unwrap();
    let mut iter = root.clone().into_iter();
    assert_eq!(iter.next().unwrap().unwrap(), root.as_node());
    other.append_child(&a.as_noderef());
    assert_eq!(iter.next(), Some(Err(TraversalError::TreeModified)));
    assert_eq!(iter.next(), None);
    assert_eq!(other.inner_html(), "<a></a>");
    assert_eq!(root.inner_html(), "<b></b>");
}

#[test]
fn element_iterator_reports_html_replacement() {
    let tree = HtmlTree::parse("<div>text<span><b>old</b></span><i></i></div>").unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    let alias = root.clone();
    let mut iter = root.iter_elements();
    assert_eq!(iter.next().unwrap().unwrap(), root);
    alias.set_inner_html("<p>new</p>");
    assert_eq!(iter.next(), Some(Err(TraversalError::TreeModified)));
    assert_eq!(iter.next(), None);
}

#[test]
fn iterator_construction_is_lazy() {
    let tree = HtmlTree::parse("<div><span>text</span></div>").unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    let child = root.first_child().unwrap();
    let references = unsafe { (*child.node_ptr_()).ref_count };
    let iter = root.iter();
    // A snapshot would have retained every descendant at construction.
    assert_eq!(unsafe { (*child.node_ptr_()).ref_count }, references);
    assert_eq!(iter.collect::<Result<Vec<_>, _>>().unwrap().len(), 3);
}

#[test]
fn mutations_before_the_first_step_invalidate_traversal() {
    let tree = HtmlTree::parse("<div>old<span>text</span></div>").unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    let mut iter = root.iter();
    root.clone().set_text_content("new");
    assert_eq!(iter.next(), Some(Err(TraversalError::TreeModified)));
    assert_eq!(iter.next(), None);
}

#[test]
fn exhausted_iterators_stay_exhausted_after_mutation() {
    use std::iter::FusedIterator;
    fn assert_fused<T: FusedIterator>(_: &T) {}
    let tree = HtmlTree::parse("<div></div>").unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    let mut borrowed = root.iter();
    let mut owned = root.clone().into_iter();
    let mut elements = root.iter_elements();
    assert_fused(&borrowed);
    assert_fused(&owned);
    assert_fused(&elements);
    assert!(borrowed.next().unwrap().is_ok());
    assert!(owned.next().unwrap().is_ok());
    assert!(elements.next().unwrap().is_ok());
    root.clone().set_inner_html("<p>new</p>");
    assert_eq!(borrowed.next(), None);
    assert_eq!(owned.next(), None);
    assert_eq!(elements.next(), None);
    assert_eq!(borrowed.next(), None);
}

#[test]
fn cloning_reading_and_other_documents_do_not_invalidate_traversal() {
    let tree = HtmlTree::parse("<div><span>text</span></div>").unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    let iter = root.iter();
    let clone = root.clone();
    assert_eq!(clone.inner_html(), "<span>text</span>");
    drop(clone);
    let other = HtmlTree::parse("<p>other</p>").unwrap();
    other.body().unwrap().set_inner_html("changed");
    assert_eq!(iter.collect::<Result<Vec<_>, _>>().unwrap().len(), 3);
}

#[test]
fn mutation_entry_points_invalidate_pending_nodes() {
    let mutations: &[fn(&mut ElementNode)] = &[
        |node| node.set_inner_html("<i>replacement</i>"),
        |node| node.set_outer_html("<i>replacement</i>"),
        |node| node.set_text_content("replacement"),
        |node| node.set_outer_text("replacement"),
        |node| node.set_attribute("class", "changed"),
        |node| node.remove_attribute("id"),
        |node| node.first_element_child().unwrap().remove(),
        |node| {
            node.remove_child(&node.first_child().unwrap().as_noderef());
        },
        |node| node.replace_children(&[]),
        |node| node.decompose(),
    ];
    for mutate in mutations {
        let tree = HtmlTree::parse("<div id='root'><span>old</span></div>").unwrap();
        let root = tree.body().unwrap().first_element_child().unwrap();
        let mut iter = root.iter();
        assert!(iter.next().unwrap().is_ok());
        mutate(&mut root.clone());
        assert_eq!(iter.next(), Some(Err(TraversalError::TreeModified)));
        assert_eq!(iter.next(), None);
    }
}

#[test]
fn dropping_an_orphan_subtree_invalidates_traversal() {
    let tree = HtmlTree::parse("<div><span><b>old</b></span></div><section>keep</section>").unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    let orphan = root.first_child().unwrap();
    // Release Lexbor's reference to the retained descendant.
    root.clone().decompose();
    assert_eq!(unsafe { (*orphan.node_ptr_()).ref_count }, 1);
    let other = tree.body().unwrap().last_element_child().unwrap();
    let mut iter = other.iter();
    drop(orphan);
    assert_eq!(iter.next(), Some(Err(TraversalError::TreeModified)));
    assert_eq!(iter.next(), None);
}

#[test]
fn mutation_on_another_thread_invalidates_traversal() {
    let tree = HtmlTree::parse("<div>old<span>text</span></div>").unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    let mut iter = root.clone().into_iter();
    let alias = root.clone();
    std::thread::spawn(move || alias.decompose()).join().unwrap();
    assert_eq!(iter.next(), Some(Err(TraversalError::TreeModified)));
    assert_eq!(iter.next(), None);
}

#[test]
fn traversal_steps_are_serialized_with_concurrent_replacement() {
    use std::sync::{Arc, Barrier};
    let tree = HtmlTree::parse("<div><span>old</span></div>").unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    let start = Arc::new(Barrier::new(2));
    let writer_start = start.clone();
    let alias = root.clone();
    let writer = std::thread::spawn(move || {
        writer_start.wait();
        for _ in 0..64 {
            alias.set_inner_html("<p>new<b>nested</b></p><i>last</i>");
            std::thread::yield_now();
        }
    });
    start.wait();
    for _ in 0..64 {
        let result = root.iter().collect::<Result<Vec<_>, _>>();
        assert!(result.is_ok() || result == Err(TraversalError::TreeModified));
        std::thread::yield_now();
    }
    writer.join().unwrap();
}

#[test]
fn test_children() {
    let tree = HtmlTree::from_str(HTML).unwrap();
    let element = tree.document().unwrap().get_element_by_id("a").unwrap();

    assert_eq!(element, element.first_child().unwrap().parent_element().unwrap());
    assert_eq!(element, element.last_child().unwrap().parent_element().unwrap());
    assert_eq!(element.first_child().unwrap().next_sibling(), element.last_child().unwrap().previous_sibling());

    let t: TextNode = element.first_child().unwrap().try_into().unwrap();
    assert!(t.first_child().is_none());
    assert_eq!(t.node_value().unwrap(), t.text_content().unwrap());
    assert_eq!(t.node_value().unwrap(), "Hello ");

    let e_first = element.first_element_child().unwrap();
    assert!(e_first.node_value().is_none());
    assert_eq!(e_first.text_content().unwrap(), "world");

    let t: TextNode = element.last_child().unwrap().try_into().unwrap();
    assert!(t.last_child().is_none());
    assert_eq!(t.node_value().unwrap(), t.text_content().unwrap());
    assert_eq!(t.node_value().unwrap(), "!");

    let e_last = element.last_element_child().unwrap();
    assert!(e_last.node_value().is_none());
    assert_eq!(e_last, e_first);

    assert_eq!(element.next_element_sibling().unwrap().tag_name().unwrap(), "P");
    assert!(element.next_element_sibling().unwrap().next_sibling().is_some());
    assert!(
        element
            .next_element_sibling()
            .unwrap()
            .next_sibling()
            .unwrap()
            .next_sibling()
            .is_none()
    );
    assert!(element.next_element_sibling().unwrap().next_element_sibling().is_none());
}

#[test]
fn test_siblings() {
    let tree = HtmlTree::from_str(HTML).unwrap();
    let element = tree.document().unwrap().get_element_by_id("a").unwrap();

    let e_next: ElementNode = element
        .first_child()
        .unwrap()
        .next_sibling()
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(e_next, element.first_element_child().unwrap());
    assert_eq!(e_next.tag_name().unwrap(), "SPAN");
    assert_eq!(e_next.class_name().unwrap(), "bar");

    assert!(element.previous_sibling().is_some());
    assert!(element.previous_element_sibling().is_none());

    let e_prev: ElementNode = element
        .last_child()
        .unwrap()
        .previous_sibling()
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(e_prev, element.last_element_child().unwrap());
    assert_eq!(e_prev.tag_name().unwrap(), "SPAN");
    assert_eq!(e_prev.class_name().unwrap(), "bar");

    let element1 = tree
        .document()
        .unwrap()
        .get_element_by_id("foo")
        .unwrap()
        .first_element_child()
        .unwrap();
    assert_eq!(element1.id().unwrap(), "a");
    assert_eq!(
        TextNode::try_from(element1.next_sibling().unwrap())
            .unwrap()
            .node_value()
            .unwrap()
            .trim(),
        ""
    );
    assert_eq!(element1.next_element_sibling().unwrap().text_content().unwrap(), "Hello DOM!");
    assert!(element1.previous_sibling().unwrap().previous_sibling().is_none());
    assert!(element1.previous_element_sibling().is_none());

    let element2 = tree
        .document()
        .unwrap()
        .get_element_by_id("foo")
        .unwrap()
        .last_element_child()
        .unwrap();
    assert_eq!(element2.id().unwrap(), "b");
    assert_eq!(
        TextNode::try_from(element2.previous_sibling().unwrap())
            .unwrap()
            .node_value()
            .unwrap()
            .trim(),
        ""
    );
    assert_eq!(element2.previous_element_sibling().unwrap().text_content().unwrap(), "Hello world!");
    assert!(element2.next_sibling().unwrap().next_sibling().is_none());
    assert!(element2.next_element_sibling().is_none());

    assert_eq!(element1.next_element_sibling().unwrap(), element2);
    assert_eq!(element2.previous_element_sibling().unwrap(), element1);
}

#[test]
fn create_nodes() {
    let doc = HtmlTree::from_str(HTML).unwrap().document().unwrap();

    let frag = doc.create_document_fragment().unwrap();

    let text = doc.create_text_node("Hello World").unwrap();
    assert_eq!(text.node_value().unwrap(), "Hello World");

    let element = doc.create_element("foo").unwrap();
    assert_eq!(element.tag_name().unwrap(), "FOO");

    let attr = doc.create_attribute("class").unwrap();
    assert_eq!(attr.node_name().unwrap(), "class");

    let comment = doc.create_comment("Some comment").unwrap();
    assert_eq!(comment.node_value().unwrap(), "Some comment");

    let cdata = doc.create_cdata_section("Foo <bar> <baz> </bar>").unwrap();
    assert_eq!(cdata.node_value().unwrap(), "Foo <bar> <baz> </bar>");
    assert_eq!(cdata.to_string(), ""); // Not supported for HTML documents

    let proc = doc
        .create_processing_instruction("xml", "version=\"1.0\" foo=\"bar\"")
        .unwrap();
    assert_eq!(proc.target().unwrap(), "xml");
    assert_eq!(proc.node_value().unwrap(), "version=\"1.0\" foo=\"bar\"");
    assert_eq!(proc.to_string(), "<?xml version=\"1.0\" foo=\"bar\">");

    frag.append_child(&element.as_noderef());
    element.append(&[text.as_noderef(), comment.as_noderef(), proc.as_noderef()]);
    attr.set_value("abc");
    assert_eq!(attr.node_value().unwrap(), "abc");

    element.set_attribute_node(&attr);
    assert!(element.has_attribute("class"));
    element.set_attribute("class", "foobar");
    assert_eq!(attr.node_value().unwrap(), "foobar");

    element.set_attribute("abc", "def");
    assert_eq!(
        element.to_string(),
        r##"<foo class="foobar" abc="def">Hello World<!--Some comment--><?xml version="1.0" foo="bar"></foo>"##
    );

    // Appending a DocumentFragment moves the child nodes into the document.
    doc.first_element_child().unwrap().append_child(&frag.as_noderef());
    assert_eq!(doc.first_element_child().unwrap().last_child().unwrap(), element.as_node());
    assert!(frag.first_child().is_none());
}

#[test]
fn node_reference_counting() {
    let tree = HtmlTree::from_str(HTML).unwrap();
    let doc = tree.document().unwrap();

    // Keep grandchild reference
    let grandchild = doc.first_element_child().unwrap().first_element_child().unwrap();

    // Minimum reference count should be 2 (1 Lexbor + 1 Resiliparse)
    assert_eq!(unsafe { *(grandchild.node_ptr_()) }.ref_count, 2);

    // Create a few more clones
    let grandchild2 = grandchild.clone();
    assert!(grandchild2.has_child_nodes());
    assert_eq!(unsafe { *(grandchild.node_ptr_()) }.ref_count, 3);
    assert_eq!(unsafe { *(grandchild2.node_ptr_()) }.ref_count, 3);

    let grandchild3 = grandchild.clone();
    assert!(grandchild2.has_child_nodes());
    assert_eq!(unsafe { *(grandchild.node_ptr_()) }.ref_count, 4);
    assert_eq!(unsafe { *(grandchild3.node_ptr_()) }.ref_count, 4);

    // Check parent state
    assert!(doc.first_element_child().is_some());
    assert!(grandchild.has_child_nodes());

    // Decompose parent
    doc.first_element_child().unwrap().decompose();
    assert!(!doc.first_element_child().is_some());
    assert_eq!(unsafe { *(grandchild.node_ptr_()) }.ref_count, 3);
    assert_eq!(unsafe { *(grandchild2.node_ptr_()) }.ref_count, 3);
    assert_eq!(unsafe { *(grandchild3.node_ptr_()) }.ref_count, 3);

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
    assert_eq!(unsafe { *(grandchild2.node_ptr_()) }.ref_count, 2);
    assert_eq!(unsafe { *(grandchild3.node_ptr_()) }.ref_count, 2);

    // Drop tree and document in the "wrong" order
    drop(tree);
    drop(doc);
    assert_eq!(grandchild3.node_name().unwrap(), "HEAD");

    // Decompose dangling grandchild2
    grandchild2.decompose();
    assert!(grandchild2.node_ptr_().is_null());
    assert!(!grandchild3.node_ptr_().is_null());
    assert_eq!(unsafe { *(grandchild3.node_ptr_()) }.ref_count, 1);
}

#[test]
fn hierarchy_rejects_invalid_child_kinds_without_mutation() {
    let tree = HtmlTree::parse("<!doctype html><div><i></i></div>").unwrap();
    let doc = tree.document().unwrap();
    let parent = tree.body().unwrap().first_element_child().unwrap();
    let child = parent.first_child().unwrap();
    let attr = doc.create_attribute("test").unwrap();
    let doctype = doc.doctype().unwrap();
    let fragment = doc.create_document_fragment().unwrap();
    let iter = parent.clone().into_iter();
    for invalid in [attr.as_noderef(), doctype.as_noderef(), doc.as_noderef()] {
        assert!(parent.append_child(&invalid).is_none());
        assert!(parent.insert_before(&invalid, Some(child.as_noderef())).is_none());
        assert!(parent.replace_child(&invalid, &child.as_noderef()).is_none());
        assert!(fragment.append_child(&invalid).is_none());
    }
    assert_child_links(&parent, &[child]);
    assert_child_links(&fragment, &[]);
    assert_eq!(doc.doctype().unwrap(), doctype);
    assert_eq!(iter.collect::<Result<Vec<_>, _>>().unwrap().len(), 2);
}

#[test]
fn document_hierarchy_validates_order_counts_and_fragment_contents() {
    let tree = HtmlTree::parse("<!doctype html><html><body></body></html>").unwrap();
    let doc = tree.document().unwrap();
    let root = doc.document_element().unwrap();
    let doctype = doc.doctype().unwrap();
    let text = doc.create_text_node("text").unwrap();
    let second = doc.create_element("second").unwrap();
    let duplicate_doctype = doctype.clone_node(false).unwrap();
    let fragment = doc.create_document_fragment().unwrap();
    let comment = doc.create_comment("keep").unwrap();
    fragment.append(&[comment.as_noderef(), second.as_noderef()]);
    let original = doc.child_nodes().values();
    let iter = doc.clone().into_iter();
    assert!(doc.append_child(&text.as_noderef()).is_none());
    assert!(doc.append_child(&second.as_noderef()).is_none());
    assert!(doc.append_child(&duplicate_doctype.as_noderef()).is_none());
    assert!(doc.append_child(&doctype.as_noderef()).is_none());
    assert!(
        doc.insert_before(&root.as_noderef(), Some(doctype.as_noderef()))
            .is_none()
    );
    assert!(doc.append_child(&fragment.as_noderef()).is_none());
    assert_child_links(&fragment, &[comment.as_node(), second.as_node()]);
    assert_child_links(&doc, &original);
    assert!(iter.collect::<Result<Vec<_>, _>>().is_ok());

    // Replacing the root must validate the final tree, allowing one new root.
    assert!(doc.replace_child(&fragment.as_noderef(), &root.as_noderef()).is_some());
    assert_child_links(&doc, &[doctype.as_node(), comment.as_node(), second.as_node()]);
    assert_child_links(&fragment, &[]);
    assert!(root.parent_node().is_none());

    // Fragment text and multiple elements are rejected before any child moves.
    fragment.append(&[root.as_noderef(), text.as_noderef()]);
    assert!(
        doc.replace_child(&fragment.as_noderef(), &second.as_noderef())
            .is_none()
    );
    assert_child_links(&fragment, &[root.as_node(), text.as_node()]);
    fragment.remove_child(&text.as_noderef()).unwrap();
    let third = doc.create_element("third").unwrap();
    fragment.append_child(&third.as_noderef()).unwrap();
    assert!(
        doc.replace_child(&fragment.as_noderef(), &second.as_noderef())
            .is_none()
    );
    assert_child_links(&fragment, &[root.as_node(), third.as_node()]);
}

#[test]
fn invalid_batches_are_atomic_including_ancestor_cycles() {
    for operation in 0..6 {
        let tree = HtmlTree::parse("<section><div><i></i></div><aside></aside></section>").unwrap();
        let ancestor = tree.body().unwrap().first_element_child().unwrap().as_node();
        let parent = tree
            .body()
            .unwrap()
            .first_element_child()
            .unwrap()
            .first_element_child()
            .unwrap();
        let receiver = parent.first_element_child().unwrap();
        let valid = tree.document().unwrap().create_element("b").unwrap().as_node();
        let original = parent.child_nodes().values();
        let iter = ancestor.clone().into_iter();
        match operation {
            0 => parent.append(&[valid.as_noderef(), ancestor.as_noderef()]),
            1 => parent.prepend(&[valid.as_noderef(), ancestor.as_noderef()]),
            2 => parent.replace_children(&[valid.as_noderef(), ancestor.as_noderef()]),
            3 => receiver.before(&[valid.as_noderef(), ancestor.as_noderef()]),
            4 => receiver.after(&[valid.as_noderef(), ancestor.as_noderef()]),
            5 => receiver.replace_with(&[valid.as_noderef(), ancestor.as_noderef()]),
            _ => unreachable!(),
        }
        assert_child_links(&parent, &original);
        assert!(valid.parent_node().is_none());
        assert_eq!(ancestor.parent_node().unwrap(), tree.body().unwrap().as_node());
        assert!(iter.collect::<Result<Vec<_>, _>>().is_ok());
    }
}

#[test]
fn document_batches_validate_the_final_tree() {
    let tree = HtmlTree::parse("<!doctype html><html><body></body></html>").unwrap();
    let doc = tree.document().unwrap();
    let root = doc.document_element().unwrap();
    let replacement = doc.create_element("replacement").unwrap().as_node();
    let comment = doc.create_comment("before").unwrap().as_node();
    let text = doc.create_text_node("invalid").unwrap();
    let original = doc.child_nodes().values();
    let iter = doc.clone().into_iter();
    doc.append(&[comment.as_noderef(), replacement.as_noderef()]);
    doc.prepend(&[comment.as_noderef(), replacement.as_noderef()]);
    doc.replace_children(&[replacement.as_noderef(), text.as_noderef()]);
    root.replace_with(&[comment.as_noderef(), replacement.as_noderef(), text.as_noderef()]);
    root.set_outer_text("invalid at document level");
    assert_child_links(&doc, &original);
    assert!(comment.parent_node().is_none());
    assert!(replacement.parent_node().is_none());
    assert!(iter.collect::<Result<Vec<_>, _>>().is_ok());

    root.replace_with(&[comment.as_noderef(), replacement.as_noderef()]);
    assert_child_links(&doc, &[original[0].clone(), comment.clone(), replacement.clone()]);
    // Replacement may also reuse nodes already in the destination document.
    doc.replace_children(&[replacement.as_noderef(), comment.as_noderef()]);
    assert_child_links(&doc, &[replacement, comment]);
}

#[test]
fn batches_handle_duplicate_nodes_and_fragments() {
    let tree = HtmlTree::parse("<div></div>").unwrap();
    let doc = tree.document().unwrap();
    let parent = tree.body().unwrap().first_element_child().unwrap();
    let fragment = doc.create_document_fragment().unwrap();
    let a = doc.create_element("a").unwrap();
    let b = doc.create_element("b").unwrap();
    fragment.append_child(&a.as_noderef()).unwrap();
    parent.append(&[fragment.as_noderef(), b.as_noderef(), fragment.as_noderef()]);
    assert_child_links(&parent, &[a.as_node(), b.as_node()]);
    parent.replace_children(&[a.as_noderef(), b.as_noderef(), a.as_noderef()]);
    assert_child_links(&parent, &[b.as_node(), a.as_node()]);
    fragment.append_child(&a.as_noderef()).unwrap();
    parent.append(&[a.as_noderef(), b.as_noderef(), fragment.as_noderef()]);
    assert_child_links(&parent, &[a.as_node(), b.as_node()]);
    assert_child_links(&fragment, &[]);
}

#[test]
fn character_data_uses_unicode_scalar_indices() {
    let tree = HtmlTree::parse("").unwrap();
    let doc = tree.document().unwrap();
    let mut nodes: Vec<Box<dyn CharacterData>> = vec![
        Box::new(doc.create_text_node("").unwrap()),
        Box::new(doc.create_comment("").unwrap()),
        Box::new(doc.create_cdata_section("").unwrap()),
        Box::new(doc.create_processing_instruction("target", "").unwrap()),
    ];
    for node in &mut nodes {
        assert_eq!(node.len(), 0);
        assert_eq!(node.substring_data(0, usize::MAX).unwrap(), "");
        node.insert_data(0, "aé🙂e\u{301}").unwrap();
        assert_eq!(node.len(), 5); // Combining marks count separately.
        assert_eq!(node.substring_data(1, 2).unwrap(), "é🙂");
        assert_eq!(node.substring_data(3, usize::MAX).unwrap(), "e\u{301}");
        assert_eq!(node.substring_data(5, usize::MAX).unwrap(), "");
        node.insert_data(5, "界").unwrap();
        node.replace_data(6, usize::MAX, "!").unwrap();
        assert_eq!(node.data().unwrap(), "aé🙂e\u{301}界!");
        node.replace_data(1, 2, "ö").unwrap();
        node.delete_data(2, usize::MAX).unwrap();
        assert_eq!(node.data().unwrap(), "aö");
        node.delete_data(2, usize::MAX).unwrap();
        node.replace_data(2, 0, "").unwrap();
        node.append_data("🙂");
        assert_eq!(node.data().unwrap(), "aö🙂");
        node.delete_data(0, usize::MAX).unwrap();
        node.replace_data(0, usize::MAX, "end").unwrap();
        assert_eq!(node.data().unwrap(), "end");
    }
}

#[test]
fn invalid_character_data_offsets_fail_without_mutation() {
    let tree = HtmlTree::parse("<p>aé🙂</p>").unwrap();
    let parent = tree.body().unwrap().first_element_child().unwrap();
    let Node::Text(text) = parent.first_child().unwrap() else {
        panic!("Expected text")
    };
    let iter = parent.clone().into_iter();
    for offset in [4, usize::MAX] {
        assert!(text.substring_data(offset, 0).is_err());
        assert!(text.substring_data(offset, usize::MAX).is_err());
        assert!(text.insert_data(offset, "").is_err());
        assert!(text.insert_data(offset, "new").is_err());
        assert!(text.delete_data(offset, 0).is_err());
        assert!(text.delete_data(offset, usize::MAX).is_err());
        assert!(text.replace_data(offset, usize::MAX, "new").is_err());
        assert_eq!(text.data().unwrap(), "aé🙂");
    }
    assert_eq!(iter.collect::<Result<Vec<_>, _>>().unwrap().len(), 2);
}

#[test]
fn node_value_preserves_parent_subtrees() {
    let tree = HtmlTree::parse("<!doctype html><div><i>keep</i></div>").unwrap();
    let doc = tree.document().unwrap();
    let element = tree.body().unwrap().first_element_child().unwrap();
    let child = element.first_child().unwrap();
    let fragment = doc.create_document_fragment().unwrap();
    let fragment_child = doc.create_text_node("keep").unwrap();
    fragment.append_child(&fragment_child.as_noderef()).unwrap();
    let iter = doc.clone().into_iter();
    let fragment_iter = fragment.clone().into_iter();
    for node in [
        element.as_node(),
        fragment.as_node(),
        doc.as_node(),
        doc.doctype().unwrap().as_node(),
    ] {
        assert_eq!(node.node_value(), None);
        node.set_node_value("ignored");
        assert_eq!(node.node_value(), None);
    }
    doc.set_text_content("ignored");
    doc.doctype().unwrap().set_text_content("ignored");
    assert_child_links(&element, &[child.clone()]);
    assert_child_links(&fragment, &[fragment_child.as_node()]);
    assert!(iter.collect::<Result<Vec<_>, _>>().is_ok());
    assert!(fragment_iter.collect::<Result<Vec<_>, _>>().is_ok());

    element.set_text_content("replacement");
    fragment.set_text_content("replacement");
    assert_eq!(element.text_content().unwrap(), "replacement");
    assert_eq!(fragment.text_content().unwrap(), "replacement");
    assert_eq!(element.child_nodes().len(), 1);
    assert_eq!(fragment.child_nodes().len(), 1);
    assert_ne!(element.first_child().unwrap(), child);
    assert_ne!(fragment.first_child().unwrap(), fragment_child.as_node());
}

#[test]
fn value_and_text_setters_update_attribute_and_character_data() {
    let tree = HtmlTree::parse("").unwrap();
    let doc = tree.document().unwrap();
    let mut nodes = [
        doc.create_attribute("test").unwrap().as_node(),
        doc.create_text_node("").unwrap().as_node(),
        doc.create_comment("").unwrap().as_node(),
        doc.create_cdata_section("").unwrap().as_node(),
        doc.create_processing_instruction("target", "").unwrap().as_node(),
    ];
    for node in &mut nodes {
        node.set_node_value("é🙂");
        assert_eq!(node.node_value().unwrap(), "é🙂");
        node.set_text_content("new");
        assert_eq!(node.node_value().unwrap(), "new");
        node.set_node_value("");
        assert_eq!(node.node_value().unwrap(), "");
    }
}

#[test]
fn node_capability_views_cover_supported_node_kinds_without_cloning() {
    let tree = HtmlTree::parse("<!doctype html><p>text</p>").unwrap();
    let doc = tree.document().unwrap();
    let cases = [
        (doc.as_node(), true, false, false, false),
        (doc.create_document_fragment().unwrap().as_node(), true, false, false, false),
        (tree.body().unwrap().as_node(), true, false, true, false),
        (doc.create_text_node("é🙂").unwrap().as_node(), false, true, false, true),
        (doc.create_cdata_section("é🙂").unwrap().as_node(), false, true, false, false),
        (doc.create_comment("é🙂").unwrap().as_node(), false, true, false, false),
        (doc.create_processing_instruction("target", "é🙂").unwrap().as_node(), false, true, false, false),
        (doc.create_attribute("test").unwrap().as_node(), false, false, false, false),
        (doc.doctype().unwrap().as_node(), false, false, false, false),
    ];
    for (mut node, parent, character_data, element, text) in cases {
        let document = node.tree_();
        let _guard = document.doc_ptr();
        let ref_count = unsafe { (*node.node_ptr_()).ref_count };
        let parent_view = node.as_parent();
        let data_view = node.as_character_data();
        assert_eq!(parent_view.is_some(), parent);
        assert_eq!(data_view.is_some(), character_data);
        assert_eq!(node.as_element().is_some(), element);
        assert_eq!(node.as_text().is_some(), text);
        // Views are Copy; retaining another view must not retain a native handle.
        let _parent_copy = parent_view;
        let _data_copy = data_view;
        assert_eq!(unsafe { (*node.node_ptr_()).ref_count }, ref_count);
        if let Some(data) = data_view {
            assert_eq!(data.len(), 2);
            assert_eq!(data.substring_data(1, 1).unwrap(), "🙂");
        }
        if let Node::Element(element) = &node {
            assert!(std::ptr::eq(node.as_element().unwrap(), element));
        }
        if let Node::Text(text) = &node {
            assert!(std::ptr::eq(node.as_text().unwrap(), text));
        }
        let borrowed = node.as_noderef();
        assert_eq!(borrowed.as_parent().is_some(), parent);
        assert_eq!(borrowed.as_character_data().is_some(), character_data);
        assert_eq!(borrowed.as_element().is_some(), element);
        assert_eq!(borrowed.as_text().is_some(), text);
        let borrowed_mut = node.as_noderef_mut();
        assert_eq!(borrowed_mut.as_parent().is_some(), parent);
        assert_eq!(borrowed_mut.as_character_data().is_some(), character_data);
        assert_eq!(borrowed_mut.as_element().is_some(), element);
        assert_eq!(borrowed_mut.as_text().is_some(), text);
        assert_eq!(unsafe { (*node.node_ptr_()).ref_count }, ref_count);
    }
}

#[test]
fn parent_views_support_queries_and_live_collections() {
    fn count_children<T: ParentNode + ?Sized>(parent: &T) -> usize {
        parent.child_element_count()
    }

    let tree = HtmlTree::parse("<div><a href='/one'>one</a><span>two</span></div>").unwrap();
    let doc = tree.document().unwrap();
    let element = tree.body().unwrap().first_element_child().unwrap();
    let fragment = doc.create_document_fragment().unwrap();
    let detached_link = doc.create_element("a").unwrap();
    fragment.append_child(&detached_link.as_noderef()).unwrap();
    for (node, expected_count) in [(doc.as_node(), 1), (element.as_node(), 2), (fragment.as_node(), 1)] {
        let parent = node.as_parent().unwrap();
        assert_eq!(count_children(&*parent), expected_count);
        assert_eq!(parent.children().len(), expected_count);
        assert_eq!(parent.query_selector_all("a").unwrap().len(), 1);
    }
    let node = element.as_node();
    let parent = node.as_parent().unwrap();
    let children = parent.children();
    let links = parent.query_selector_all("a[href]").unwrap();
    assert_eq!(links.len(), 1);
    element
        .append_child(&doc.create_element("b").unwrap().as_noderef())
        .unwrap();
    assert_eq!(children.len(), 3);
    assert_eq!(parent.child_element_count(), 3);
}

#[test]
fn checked_downcasts_preserve_ownership_on_success_and_failure() {
    fn check<T: NodeInterface + TryFrom<Node, Error = Node>>(node: Node, target: NodeType) {
        let document = node.tree_();
        let _guard = document.doc_ptr();
        let pointer = node.node_ptr_();
        let ref_count = unsafe { (*pointer).ref_count };
        let expected_success = node.node_type() == Some(target);
        let recovered = match T::try_from(node) {
            Ok(concrete) => {
                assert!(expected_success);
                concrete.into_node() // Upcasts remain infallible.
            }
            Err(original) => {
                assert!(!expected_success);
                original
            }
        };
        assert_eq!(recovered.node_ptr_(), pointer);
        assert_eq!(unsafe { (*pointer).ref_count }, ref_count);
    }

    let tree = HtmlTree::parse("<!doctype html><p>text</p>").unwrap();
    let doc = tree.document().unwrap();
    let nodes = [
        doc.as_node(),
        tree.body().unwrap().as_node(),
        doc.doctype().unwrap().as_node(),
        doc.create_document_fragment().unwrap().as_node(),
        doc.create_attribute("test").unwrap().as_node(),
        doc.create_text_node("text").unwrap().as_node(),
        doc.create_cdata_section("text").unwrap().as_node(),
        doc.create_comment("text").unwrap().as_node(),
        doc.create_processing_instruction("target", "text").unwrap().as_node(),
    ];
    for node in nodes {
        check::<DocumentNode>(node.clone(), NodeType::Document);
        check::<ElementNode>(node.clone(), NodeType::Element);
        check::<DocumentTypeNode>(node.clone(), NodeType::DocumentType);
        check::<DocumentFragmentNode>(node.clone(), NodeType::DocumentFragment);
        check::<AttrNode>(node.clone(), NodeType::Attribute);
        check::<TextNode>(node.clone(), NodeType::Text);
        check::<CdataSectionNode>(node.clone(), NodeType::CdataSection);
        check::<CommentNode>(node.clone(), NodeType::Comment);
        check::<ProcessingInstructionNode>(node.clone(), NodeType::ProcessingInstruction);
        check::<NotationNode>(node, NodeType::Notation);
    }
}

// Node uses the same interface as concrete nodes, without trait-object dereferencing.
mod node_interface_api {
    use crate::parse::html::dom::iter::TraversalError;
    use crate::parse::html::dom::node::{Node, NodeType};
    use crate::parse::html::dom::traits::NodeInterface;
    use crate::parse::html::tree::HtmlTree;

    #[test]
    fn common_operations_use_the_shared_interface() {
        let tree = HtmlTree::parse("<p>text</p><b>last</b>").unwrap();
        let body = Node::from(tree.body().unwrap());
        assert_eq!(body.node_type(), Some(NodeType::Element));
        assert_eq!(body.node_name().as_deref(), Some("BODY"));
        assert!(body.owner_document().is_some());
        assert!(body.parent_element().is_some());
        assert!(body.parent_node().is_some());
        assert!(body.has_child_nodes());
        assert_eq!(body.child_nodes().len(), 2);
        let mut first = body.first_child().unwrap();
        let last = body.last_child().unwrap();
        assert_eq!(first.next_sibling().unwrap(), last);
        assert_eq!(last.previous_sibling().unwrap(), first);
        assert!(body.contains(&first.as_noderef()));
        assert_eq!(first.as_node().into_node(), first);
        let copy = first.clone_node(true).unwrap();
        assert_ne!(copy, first);
        assert_eq!(copy.text_content(), first.text_content());
        body.insert_before(&copy.as_noderef(), Some(first.as_noderef()))
            .unwrap();
        assert_eq!(body.first_child().unwrap(), copy);
        body.remove_child(&copy.as_noderef()).unwrap();
        body.replace_child(&copy.as_noderef(), &first.as_noderef()).unwrap();
        body.append_child(&first.as_noderef()).unwrap();
        assert_eq!(body.last_child().unwrap(), first);
        let text = first.first_child().unwrap();
        text.set_node_value("new");
        assert_eq!(text.node_value().as_deref(), Some("new"));
        first.set_text_content("replacement");
        assert_eq!(first.text_content().as_deref(), Some("replacement"));
        assert!(matches!(first.as_noderef_mut(), crate::parse::html::dom::node::NodeRefMut::Element(_)));
        first.decompose();
        assert!(first.node_type().is_none());
    }

    #[test]
    fn borrowed_traversal_preserves_order_and_invalidation() {
        let tree = HtmlTree::parse("<p>before<b>nested</b>after</p>").unwrap();
        let node = Node::from(tree.body().unwrap());
        fn traversal_count<T: NodeInterface>(node: &T) -> usize {
            node.iter().collect::<Result<Vec<_>, _>>().unwrap().len()
        }
        assert_eq!(traversal_count(&node), 6);
        assert_eq!(traversal_count(&tree.body().unwrap()), 6);
        let names = node
            .iter()
            .map(|result| result.unwrap().node_name().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(names, ["BODY", "P", "#text", "B", "#text", "#text"]);
        let names = node
            .iter_elements()
            .map(|result| Node::from(result.unwrap()).node_name().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(names, ["BODY", "P", "B"]);
        let mut nodes = node.iter();
        let mut elements = node.iter_elements();
        let alias = node.as_node();
        alias.set_text_content("changed");
        assert!(matches!(nodes.next(), Some(Err(TraversalError::TreeModified))));
        assert!(matches!(elements.next(), Some(Err(TraversalError::TreeModified))));
        assert!(nodes.next().is_none());
        assert!(elements.next().is_none());
        assert_eq!(node.node_name().as_deref(), Some("BODY"));
    }
}

#[test]
fn shared_capability_views_mutate_through_the_document_owner() {
    let tree = HtmlTree::parse("<div></div>").unwrap();
    let doc = tree.document().unwrap();
    let element = tree.body().unwrap().first_element_child().unwrap().into_node();
    let parent = element.as_parent().unwrap();
    let text = doc.create_text_node("é🙂").unwrap().into_node();
    parent.append(&[text.as_noderef()]);
    text.as_character_data().unwrap().replace_data(1, 1, "界").unwrap();
    assert_eq!(element.text_content().as_deref(), Some("é界"));

    // The iterator and parent view borrow the very same handle being mutated.
    let mut iter = element.iter();
    parent.replace_children(&[doc.create_element("b").unwrap().as_noderef()]);
    assert!(matches!(iter.next(), Some(Err(TraversalError::TreeModified))));
    assert_eq!(parent.child_element_count(), 1);
    assert!(text.parent_node().is_none());
}

#[test]
fn shared_handle_identity_and_owner_survive_moves_and_tree_drop() {
    let tree = HtmlTree::parse("<div><i></i></div><aside></aside>").unwrap();
    let owner = tree.document().unwrap();
    let source = tree.body().unwrap().first_element_child().unwrap();
    let destination = source.next_element_sibling().unwrap();
    let original = source.first_child().unwrap();
    let alias = original.as_node();
    destination.append_child(&original.as_noderef()).unwrap();
    assert_eq!(original, alias);
    assert_eq!(original, destination.first_child().unwrap());
    assert_eq!(original.parent_node().unwrap(), destination.as_node());
    assert!(!source.has_child_nodes());
    drop(tree);
    drop(owner);
    drop(source);
    drop(destination);
    alias.set_text_content("still alive");
    assert_eq!(original.text_content().as_deref(), Some("still alive"));

    // Decomposing one borrowed handle invalidates it and its traversal without
    // consuming the independent native reference held by its clone.
    let mut iter = original.iter();
    original.decompose();
    assert!(original.node_type().is_none());
    assert!(matches!(iter.next(), Some(Err(TraversalError::TreeModified))));
    assert_eq!(alias.node_name().as_deref(), Some("I"));
    assert!(alias.text_content().unwrap().is_empty());
}

#[test]
fn concrete_node_construction_checks_kind_before_retaining() {
    let tree = HtmlTree::parse("<div></div>").unwrap();
    let element = tree.body().unwrap();
    let document = element.tree_();
    let _guard = document.doc_ptr();
    let pointer = element.node_ptr_();
    let ref_count = unsafe { (*pointer).ref_count };

    // Catch panic output, then restore hook
    let previous_hook = std::sync::Arc::new(std::panic::take_hook());
    let fallback_hook = previous_hook.clone();
    let test_thread = std::thread::current().name().map(str::to_owned);
    std::panic::set_hook(Box::new(move |info| {
        if std::thread::current().name() != test_thread.as_deref() {
            fallback_hook(info);
        }
    }));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| TextNode::new(&document, pointer)));
    let _ = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| previous_hook(info)));

    assert!(result.is_err());
    assert_eq!(unsafe { (*pointer).ref_count }, ref_count);
    assert_eq!(element.tag_name().as_deref(), Some("BODY"));
}

#[test]
fn token_list_updates_are_serialized_across_sent_aliases() {
    let tree = HtmlTree::parse("<div></div>").unwrap();
    let element = tree.body().unwrap().first_element_child().unwrap();
    let threads = ["a", "b"].map(|prefix| {
        let alias = element.clone();
        std::thread::spawn(move || {
            let list = alias.class_list();
            for index in 0..64 {
                list.add(&[&format!("{prefix}{index}")]);
            }
        })
    });
    for thread in threads {
        thread.join().unwrap();
    }
    let list = element.class_list();
    assert_eq!(list.len(), 128);
    for prefix in ["a", "b"] {
        for index in 0..64 {
            assert!(list.contains(&format!("{prefix}{index}")));
        }
    }
}

#[test]
fn snapshots_access_and_consume_without_cloning() {
    #[derive(Debug, PartialEq)]
    struct NoClone(usize);
    let list = NodeList::from(vec![NoClone(1), NoClone(2)]);
    assert_eq!(list.len(), 2);
    assert!(!list.is_empty());
    assert!(std::ptr::eq(list.item(0).unwrap(), &list[0]));
    assert!(list.item(2).is_none());
    assert!(list.item(usize::MAX).is_none());
    assert_eq!(list.iter().map(|n| n.0).collect::<Vec<_>>(), [1, 2]);
    assert_eq!((&list).into_iter().count(), 2);
    assert_eq!(list.into_iter().map(|n| n.0).collect::<Vec<_>>(), [1, 2]);
    assert!(NodeList::<NoClone>::default().is_empty());

    struct MustNotClone;
    impl Clone for MustNotClone {
        fn clone(&self) -> Self {
            panic!("snapshot access cloned an item")
        }
    }
    let list = NodeList::from(vec![MustNotClone, MustNotClone]);
    assert_eq!(list.len(), 2);
    assert!(list.item(0).is_some());
    assert_eq!(list.iter().count(), 2);
    assert_eq!(list.into_iter().count(), 2);
}

#[test]
fn live_collections_have_explicit_membership_snapshots() {
    let tree = HtmlTree::from_str("<body><div class='match'></div></body>").unwrap();
    let body = tree.body().unwrap();
    let children = body.child_nodes();
    let elements = body.get_elements_by_class_name("match");
    let child_snapshot = children.snapshot();
    let element_snapshot = elements.snapshot();
    let selector_snapshot = body.query_selector_all(".match").unwrap();
    let div = body.first_element_child().unwrap();
    div.set_class_name("other");
    let new_child = tree.document().unwrap().create_element("span").unwrap();
    body.append(&[new_child.as_noderef()]);
    assert_eq!(children.len(), 2);
    assert_eq!(child_snapshot.len(), 1);
    assert!(elements.is_empty());
    assert_eq!(element_snapshot.len(), 1);
    assert_eq!(selector_snapshot.len(), 1);
    // Membership is frozen, but handles still refer to the live nodes.
    assert_eq!(element_snapshot[0].class_name().as_deref(), Some("other"));
}

#[test]
fn named_attribute_maps_lookup_mutate_and_snapshot() {
    let tree = HtmlTree::from_str("<body id='original'></body>").unwrap();
    let body = tree.body().unwrap();
    let attributes = body.attributes();
    let snapshot = attributes.snapshot();
    assert_eq!(attributes.get_named_item("id").unwrap().value().as_deref(), Some("original"));
    attributes.set("title", "added");
    assert_eq!(body.attribute("title").as_deref(), Some("added"));
    assert_eq!(attributes.len(), 2);
    assert_eq!(snapshot.len(), 1);
    attributes.set("title", "updated");
    assert_eq!(attributes.len(), 2);
    assert_eq!(attributes.get_named_item("title").unwrap().value().as_deref(), Some("updated"));
    attributes.remove("title");
    attributes.remove("missing");
    assert!(attributes.get_named_item("title").is_none());
    assert_eq!(attributes.len(), 1);
    attributes.remove("id");
    assert!(attributes.is_empty());
}

#[test]
fn compiled_selectors_match_detached_subtrees_and_walk_ancestors() {
    use super::selector::CssSelector;
    let tree = HtmlTree::from_str(
        "<body><section class='outer'><div class='inner'><i></i><b class='hit'></b></div></section></body>",
    )
    .unwrap();
    let section = tree.body().unwrap().first_element_child().unwrap();
    let inner = section.first_element_child().unwrap();
    let hit = inner.last_element_child().unwrap();
    section.remove();
    assert!(section.parent_element().is_none());
    assert!(section.matches("section.outer").unwrap());
    assert_eq!(section.closest("section").unwrap(), Some(section.clone()));
    assert!(hit.matches("section.outer > div.inner > i + b.hit:last-child").unwrap());
    assert_eq!(hit.closest(".inner, .outer").unwrap(), Some(inner.clone()));
    assert_eq!(hit.closest(".hit, .inner").unwrap(), Some(hit.clone()));
    assert!(hit.closest("body").unwrap().is_none());
    assert_eq!(section.query_selector_all(".hit, b").unwrap().len(), 1);
    assert_eq!(section.query_selector("section, b").unwrap(), Some(hit.clone()));
    let selector = CssSelector::new(".hit").unwrap();
    assert!(selector.matches(&hit).unwrap());
    hit.set_class_name("other");
    assert!(!selector.matches(&hit).unwrap());
    assert!(selector.query_selector(&section.as_noderef()).unwrap().is_none());

    let standalone = tree.document().unwrap().create_element("aside").unwrap();
    assert!(standalone.matches("aside").unwrap());
    assert_eq!(standalone.closest("aside").unwrap(), Some(standalone.clone()));
    assert!(!standalone.matches("aside > b").unwrap());
    assert!(standalone.matches("[").is_err());
    assert!(standalone.closest("[").is_err());
}

#[test]
fn compiled_selectors_own_memory_and_are_reusable_across_documents() {
    use super::selector::CssSelector;
    let selector = CssSelector::new("b, .hit").unwrap();
    for _ in 0..3 {
        let tree = HtmlTree::from_str("<body><b class='hit'></b><i class='hit'></i></body>").unwrap();
        let body = tree.body().unwrap();
        assert_eq!(selector.query_selector_all(&body.as_noderef()).unwrap().len(), 2);
        assert_eq!(selector.query_selector(&body.as_noderef()).unwrap(), body.first_element_child());
        assert!(selector.matches(&body.first_element_child().unwrap()).unwrap());
        // Creating, failing to parse, and dropping other selectors cannot invalidate this one.
        assert!(CssSelector::new("[").is_err());
        assert!(CssSelector::new("").is_err());
        let other = CssSelector::new("i").unwrap();
        assert_eq!(other.query_selector_all(&body.as_noderef()).unwrap().len(), 1);
        drop(other);
        assert_eq!(selector.query_selector_all(&body.as_noderef()).unwrap().len(), 2);
    }
}

#[test]
fn collection_selectors_deduplicate_in_root_then_tree_order() {
    use super::selector::CssSelector;
    let tree =
        HtmlTree::from_str("<body><main><b id='a'></b><section><b id='b'></b></section><b id='c'></b></main></body>")
            .unwrap();
    let main = tree.body().unwrap().first_element_child().unwrap();
    let section = main.query_selector("section").unwrap().unwrap();
    let other_tree = HtmlTree::from_str("<body><b id='d'></b></body>").unwrap();
    let other_body = other_tree.body().unwrap();
    let roots = ElementNodeList::from(vec![section.clone(), main.clone(), section, other_body]);
    let selector = CssSelector::new("b, [id]").unwrap();
    let result = roots.query_selector_all_compiled(&selector).unwrap();
    assert_eq!(result.iter().map(|e| e.id().unwrap()).collect::<Vec<_>>(), ["b", "a", "c", "d"]);
    assert_eq!(roots.query_selector_all("b, [id]").unwrap().values(), result.values());
    assert_eq!(roots.query_selector_compiled(&selector).unwrap().as_ref(), result.item(0));
    assert!(roots.matches("main").unwrap());
    assert!(roots.matches_compiled(&CssSelector::new("section").unwrap()).unwrap());
    main.remove();
    assert_eq!(roots.query_selector_all_compiled(&selector).unwrap().values(), result.values());
    let empty = ElementNodeList::default();
    assert!(empty.query_selector("[").is_err());
    assert!(empty.query_selector_all("[").is_err());
    assert!(empty.matches("[").is_err());
    assert!(empty.query_selector_all_compiled(&selector).unwrap().is_empty());
    assert!(!empty.matches_compiled(&selector).unwrap());
}
