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
fn callback_errors_stop_and_release_matcher() {
    let tree = HtmlTree::parse("<p>One</p><p>Two</p>").unwrap();
    let body = tree.body().unwrap();
    let selector = CssSelector::new("p").unwrap();
    let mut visited = 0;
    let error = selector
        .match_elements(
            &body.as_noderef(),
            |_, _, count| {
                *count += 1;
                TraverseAction::Err
            },
            &mut visited,
        )
        .unwrap_err();
    assert_eq!(visited, 1);
    assert!(error.to_string().starts_with("CSS parser error:"));
    // A failed callback must leave the compiled selector reusable.
    assert_eq!(selector.query_selector_all(&body.as_noderef()).unwrap().len(), 2);
}

#[test]
fn callback_panics_resume_in_rust() {
    // The callback must catch the panic and resume it only after C returns.
    let tree = HtmlTree::parse("<p>One</p><p>Two</p>").unwrap();
    let body = tree.body().unwrap();
    let selector = CssSelector::new("p").unwrap();
    let mut visited = 0;
    let panic = catch_unwind(AssertUnwindSafe(|| {
        selector
            .match_elements(
                &body.as_noderef(),
                |_, _, count| {
                    *count += 1;
                    panic!("callback panic");
                },
                &mut visited,
            )
            .unwrap();
    }))
    .unwrap_err();
    assert_eq!(panic.downcast_ref::<&str>(), Some(&"callback panic"));
    assert_eq!(visited, 1);
    assert_eq!(selector.query_selector_all(&body.as_noderef()).unwrap().len(), 2);
    // Both native matcher resources and the document lock must have been released.
    body.set_attribute("data-after-panic", "ok");
    assert_eq!(body.attribute("data-after-panic").as_deref(), Some("ok"));
}

#[test]
fn reverse_matching_ignores_non_elements() {
    let tree = HtmlTree::parse("<p>Text</p>").unwrap();
    let paragraph = tree.body().unwrap().first_element_child().unwrap();
    let text = paragraph.first_child().unwrap();
    let selector = CssSelector::new("*").unwrap();
    // Even the universal selector applies only to elements, not their text nodes.
    let mut visited = 0;
    selector
        .match_elements_reverse(
            &text.as_noderef(),
            |_, _, count| {
                *count += 1;
                TraverseAction::Ok
            },
            &mut visited,
        )
        .unwrap();
    assert_eq!(visited, 0);
}

#[test]
fn invalid_selector_errors_are_informative() {
    for input in ["", "[", "div >"] {
        let error = match CssSelector::new(input) {
            Ok(_) => panic!("invalid selector accepted: {input:?}"),
            Err(error) => error,
        };
        assert!(error.to_string().starts_with("CSS parser error:"));
        assert!(!error.msg.is_empty());
    }
}

#[test]
fn decomposed_roots_have_no_matches() {
    let tree = HtmlTree::parse("<div><p>Text</p></div>").unwrap();
    let root = tree.body().unwrap().first_element_child().unwrap();
    let selector = CssSelector::new("*").unwrap();
    root.decompose();
    // Queries on an invalidated handle must return safely without touching freed nodes.
    assert!(selector.query_selector(&root.as_noderef()).unwrap().is_none());
    assert!(selector.query_selector_all(&root.as_noderef()).unwrap().is_empty());
    assert!(!selector.matches(&root).unwrap());
}
