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

use crate::parse::html::dom::traits::*;
use crate::parse::html::tree::HtmlTree;

#[test]
fn visible_text_skips_non_visible_content() {
    // Formatting excludes script/style contents that remain present in raw text_content.
    let tree =
        HtmlTree::parse("<div>one\t two<br>three<!-- ignored --><script>script</script><style>style</style></div>")
            .unwrap();
    let element = tree.body().unwrap().first_element_child().unwrap();
    assert_eq!(element.inner_text(), "one two\nthree");
    assert!(element.text_content().unwrap().contains("script"));
}

#[test]
fn visible_text_preserves_preformatted_content() {
    // Preserve indentation and embedded line breaks instead of collapsing preformatted text.
    let tree = HtmlTree::parse("<div><pre>  one\n    two</pre></div>").unwrap();
    let element = tree.body().unwrap().first_element_child().unwrap();
    assert_eq!(element.inner_text(), "  one\n    two\n");
}

#[test]
fn markup_escapes_text_and_attributes() {
    let tree = HtmlTree::parse("<div></div>").unwrap();
    let element = tree.body().unwrap().first_element_child().unwrap();
    element.set_attribute("title", "A\"&B");
    element.set_text_content("<tag> & text");
    assert_eq!(element.outer_html(), "<div title=\"A&quot;&amp;B\">&lt;tag&gt; &amp; text</div>");
}

#[test]
fn skipped_noscript_does_not_change_visible_text() {
    // Given the existing skip policy, noscript must contribute neither text nor a block break.
    for (html, expected) in [
        ("<div>Visible<noscript>Fallback</noscript></div>", "Visible"),
        ("<div><noscript>Fallback</noscript>Visible</div>", "Visible"),
        ("<div>Before<noscript>Fallback</noscript>After</div>", "BeforeAfter"),
        ("<div><noscript>Fallback</noscript></div>", ""),
        // Skipping an element must also leave existing whitespace untouched.
        ("<div>Visible <noscript>Fallback</noscript></div>", "Visible "),
        ("<div>Visible<br><noscript>Fallback</noscript></div>", "Visible\n"),
    ] {
        let tree = HtmlTree::parse(html).unwrap();
        let element = tree.body().unwrap().first_element_child().unwrap();
        assert_eq!(element.inner_text(), expected, "{html}");
    }
}
