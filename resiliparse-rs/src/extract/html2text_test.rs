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

fn extract(html: &str, opts: ExtractOptions) -> String {
    let tree = HtmlTree::parse(html).unwrap();
    extract_plain_text(&tree, &opts).unwrap()
}

#[test]
fn formatting_links_forms_and_alternative_text() {
    let html = "<p>Hello <a href='/there'>world</a></p><img alt='picture'><input value='name'>";
    let mut opts = ExtractOptions::default();
    opts.links = true;
    opts.form_fields = true;
    assert_eq!(extract(html, opts), "Hello world (/there)\n\npicture[ name ]");
}

#[test]
fn compact_formatting_and_skip_selectors() {
    let html = "<div> one   two </div><nav>skip me</nav><p>three</p>";
    let mut opts = ExtractOptions::default();
    opts.formatting = Formatting::Off;
    opts.skip_elements = vec!["nav".into()];
    assert_eq!(extract(html, opts), "one two three");
}

#[test]
fn minimal_html_and_main_content() {
    let html =
        "<body><header>Header</header><article><h1>Title</h1><p>Story</p></article><footer>Footer</footer></body>";
    let mut opts = ExtractOptions::default();
    opts.main_content = true;
    opts.formatting = Formatting::MinimalHtml;
    assert_eq!(extract(html, opts), "Header\n\n<h1>Title</h1>\n\n<p>Story</p>");
}

#[test]
fn nested_lists_and_numbering() {
    let html = "<ul><li>A</li><li>B<ol><li>C</li><li>D</li></ol></li></ul><ol><li>E</li></ol>";
    assert_eq!(extract(html, ExtractOptions::default()), "  • A\n  • B\n    1. C\n    2. D\n  1. E");
    let mut opts = ExtractOptions::default();
    opts.formatting = Formatting::MinimalHtml;
    assert_eq!(
        extract(html, opts.clone()),
        "<ul>\n  <li>A</li>\n  <li>B\n  <ol>\n    <li>C</li>\n    <li>D</li>\n  </ol></li>\n</ul>\n<ol>\n  <li>E</li>\n</ol>"
    );
    opts.list_bullets = false;
    assert_eq!(extract(html, opts), "A\nB\nC\nD\nE");
    // A standalone list item still has an implicit list level.
    assert_eq!(extract("<li>A</li><li>B</li>", ExtractOptions::default()), "  • A\n  • B");
}

#[test]
fn preformatted_text_and_escaping() {
    let html = "<pre> A\n  &lt;&amp;&quot;&gt;\n</pre><p><a href=' /?a=1&amp;b=&quot;x&quot; '> &lt;&amp;&gt; </a></p>";
    let mut opts = ExtractOptions::default();
    opts.formatting = Formatting::MinimalHtml;
    opts.links = true;
    assert_eq!(
        extract(html, opts.clone()),
        "<pre> A\n  &lt;&amp;&quot;&gt;\n</pre>\n\n<p><a href=\"/?a=1&amp;b=&quot;x&quot;\"> &lt;&amp;&gt; </a></p>"
    );
    opts.formatting = Formatting::Off;
    assert_eq!(extract(html, opts), "A <&\"> <&> (/?a=1&b=\"x\")");
}

#[test]
fn form_values_and_excluded_input_types() {
    let html = "<textarea>one\n two</textarea><button>Go</button><input value=' value ' placeholder='ignored'>\
                <input placeholder=' hint '><input value=' '><input type=checkbox value=ignored>\
                <input type=color value=ignored><input type=file value=ignored><input type=hidden value=ignored>\
                <input type=radio value=ignored><input type=reset value=ignored>";
    let mut opts = ExtractOptions::default();
    opts.form_fields = true;
    opts.formatting = Formatting::Off;
    assert_eq!(extract(html, opts), "[ one two ] [ Go ] [ value ] [ hint ]");
    assert_eq!(extract(html, ExtractOptions::default()), "");
}

#[test]
fn skipped_subtrees_and_invalid_selectors() {
    let html = "<p>Visible</p><!-- ignored --><script>script</script><style>style</style>\
                <iframe>frame</iframe><template>template</template><noscript>fallback</noscript>\
                <figure>caption<img alt='image'></figure><svg><text>vector</text></svg>";
    let mut opts = ExtractOptions::default();
    opts.alt_texts = false;
    opts.noscript = true;
    opts.formatting = Formatting::Off;
    assert_eq!(extract(html, opts.clone()), "Visible fallback");
    opts.skip_elements = vec!["[".into()];
    assert!(extract_plain_text(&HtmlTree::parse(html).unwrap(), &opts).is_err());
}

#[test]
fn main_content_candidates() {
    let mut opts = ExtractOptions::default();
    opts.main_content = true;
    opts.formatting = Formatting::Off;
    assert_eq!(extract("<p>Outside</p><div class=article-body>Inside</div>", opts.clone()), "Inside");
    assert_eq!(
        extract("<p>Outside</p><div class=article-body>One</div><div class=post-content>Two</div>", opts.clone()),
        "Outside One Two"
    );
    // A node matching two candidate selectors keeps the legacy whole-body fallback.
    assert_eq!(extract("<p>Outside</p><div class=main-content role=main>Inside</div>", opts), "Outside Inside");
}

#[test]
fn line_breaks_tables_and_empty_blocks() {
    assert_eq!(extract("<p>A<br>B<br><br>C</p><p></p><div>D</div>", ExtractOptions::default()), "A\nB\n\nC\n\nD");
    let mut opts = ExtractOptions::default();
    opts.links = true;
    assert_eq!(extract("<table><tr><td><a href='/'>A</a></td><td>B</td></tr></table>", opts), "A (/)\t\tB");
}

#[test]
fn minimal_html_line_breaks_and_unbulleted_lists() {
    let html = "<p>A<br>B<br><br></p><p>C</p>";
    let mut opts = ExtractOptions::default();
    opts.formatting = Formatting::MinimalHtml;
    assert_eq!(extract(html, opts), "<p>A<br>\nB<br><br></p>\n\n<p>C</p>");
    let mut opts = ExtractOptions::default();
    opts.list_bullets = false;
    assert_eq!(extract("<ul><li>A</li><li>B</li></ul>", opts), "  A\n  B");
}

#[test]
fn multiline_form_fields() {
    let mut opts = ExtractOptions::default();
    opts.form_fields = true;
    assert_eq!(extract("<textarea>one\n two</textarea>", opts.clone()), "[ one\n two ]");
    opts.formatting = Formatting::MinimalHtml;
    assert_eq!(extract("<textarea>&lt;&amp;</textarea>", opts), "[ &lt;&amp; ]");
}
