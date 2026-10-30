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
