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

/// Check a single element against the main-content heuristics at the given depth.
fn main_content(html: &str, depth: usize, opts: &ExtractOptions) -> bool {
    let tree = HtmlTree::parse(html).unwrap();
    let node = tree.body().unwrap().first_element_child().unwrap().as_node();
    is_main_content(&node, depth, opts)
}

#[test]
fn boilerplate_attributes_and_landmarks() {
    let opts = ExtractOptions::default();
    for html in [
        "<nav>Navigation</nav>",
        "<aside>Aside</aside>",
        "<audio>Audio</audio>",
        "<video>Video</video>",
        "<time>Date</time>",
        "<p hidden>Hidden</p>",
        "<p rel=author>Author</p>",
        "<p itemprop=datePublished>Date</p>",
        "<p aria-hidden=true>Hidden</p>",
        "<p aria-expanded=false>Collapsed</p>",
        "<div id=global-footer>Footer</div>",
        "<div id=neutral class=site-header>Header</div>",
        "<div id=neutral class=cookie>Consent</div>",
        "<div id=neutral class=navigation>Menu</div>",
        "<div id=neutral class=related>Related</div>",
        "<div id=neutral class=search>Search</div>",
        "<div id=neutral class=sidebar>Sidebar</div>",
        "<div id=neutral class=modal>Modal</div>",
        "<div id=neutral class=gallery>Gallery</div>",
        "<div id=neutral class=print-only>Print</div>",
        "<a id=neutral class=skip>Skip</a>",
        "<p id=neutral class=login>Login</p>",
        "<p id=neutral class=social>Social</p>",
        "<p id=neutral class=logo>Logo</p>",
        "<p id=neutral class=sponsored>Ad</p>",
        "<p id=neutral class=spinner>Loading</p>",
        "<p id=neutral class=widget data-ad>Ad</p>",
        "<div id=neutral class=widget role=dialog>Dialog</div>",
        "<div id=neutral class=hidden>Hidden</div>",
        "<div id=neutral class=widget style='display:none'>Hidden</div>",
    ] {
        for variant in [html.to_owned(), html.replace("id=neutral ", "")] {
            assert!(!main_content(&variant, 3, &opts), "{variant}");
        }
    }
    for html in [
        "<main hidden>Main</main>",
        "<h1>Title</h1>",
        "<p>Story</p>",
        "<span id=neutral class=widget>Inline</span>",
        "<div id=neutral class=widget rel=main>Main</div>",
        "<div id=neutral class=article>Story</div>",
        "<div id=neutral class=widget>Story</div>",
    ] {
        assert!(main_content(html, 3, &opts), "{html}");
    }
}

#[test]
fn optional_hidden_comments_and_metadata() {
    let mut opts = ExtractOptions::default();
    for html in [
        "<div id=neutral class=comments>Discussion</div>",
        "<p id=neutral class=post-meta>Author</p>",
    ] {
        assert!(main_content(html, 3, &opts));
    }
    opts.comments = false;
    opts.post_meta = false;
    opts.hidden_elements = true;
    assert!(!main_content("<div id=neutral class=comments>Discussion</div>", 3, &opts));
    assert!(!main_content("<p id=neutral class=post-meta>Author</p>", 3, &opts));
    assert!(main_content("<div id=neutral class=hidden>Hidden</div>", 3, &opts));
    assert!(!main_content("<div id=neutral class=hidden hidden>Hidden</div>", 3, &opts));
    assert!(!main_content("<div id=neutral class=widget style='visibility:hidden'>Hidden</div>", 3, &opts));
    // Metadata and sign-in controls are only discarded below the top two levels.
    assert!(main_content("<p id=neutral class=post-meta>Author</p>", 2, &opts));
    assert!(main_content("<p id=neutral class=login>Login</p>", 2, &opts));
}

#[test]
fn link_density_and_depth_limits() {
    let opts = ExtractOptions::default();
    assert!(!main_content("<ul><li>Text</li></ul>", 3, &opts));
    assert!(main_content("<ul><li>Text</li></ul>", 4, &opts));
    for (tag, depth) in [("ul", 4), ("article", 3), ("div", 6)] {
        let html = format!("<{tag}><a href='/'>Linked text</a></{tag}>");
        assert!(!main_content(&html, depth, &opts), "{html}");
    }
    assert!(main_content("<article><a>Link</a></article>", 2, &opts));
    assert!(main_content("<div><a>Link</a></div>", 5, &opts));
    assert!(!main_content("<div id=neutral class=widget><a>Link</a></div>", 3, &opts));
    assert!(main_content("<div id=neutral class=widget> </div>", 6, &opts));
    // Long article bodies are retained even if most of their text is linked.
    assert!(main_content(&format!("<article><a>{}</a></article>", "x".repeat(501)), 3, &opts));
    let tree = HtmlTree::parse("<p>alpha <a>beta</a></p>").unwrap();
    let node = tree.body().unwrap().first_element_child().unwrap().as_node();
    assert!(link_cluster(&node, 0.39, 10));
    assert!(!link_cluster(&node, 0.4, 10));
    assert!(!link_cluster(&node, 0.39, 9));
    assert!(!link_cluster(&node.first_child().unwrap(), 0.0, 0));
}

#[test]
fn footer_position_and_text_symbols() {
    let opts = ExtractOptions::default();
    assert!(!main_content("<footer>Copyright</footer>", 2, &opts));
    assert!(!main_content("<footer>Copyright</footer>", 3, &opts));
    let nested_footer = |html: &str| {
        let tree = HtmlTree::parse(&format!("<div><section>{html}</section></div>")).unwrap();
        let footer = tree
            .body()
            .unwrap()
            .query_selector("footer")
            .unwrap()
            .unwrap()
            .as_node();
        is_main_content(&footer, 3, &opts)
    };
    assert!(nested_footer("<footer>Article footer</footer><p>More story</p>"));
    assert!(nested_footer("<footer>Article footer</footer> text <p>More story</p>"));
    assert!(!nested_footer("<footer>Copyright</footer> trailing text"));
    assert!(!nested_footer("<footer><a>Link</a></footer><p>More</p>"));
    let tree = HtmlTree::parse("<p>¶</p><p>\u{e123}</p><p>abc</p><!-- comment -->").unwrap();
    let body = tree.body().unwrap();
    let elements = body.query_selector_all("p").unwrap();
    for (index, keep) in [false, false, true].into_iter().enumerate() {
        assert_eq!(is_main_content(&elements.item(index).unwrap().first_child().unwrap(), 1, &opts), keep);
    }
    assert!(is_main_content(&body.last_child().unwrap(), 1, &opts));
}

#[test]
fn article_pattern_requires_content_marker() {
    for value in [
        "",
        " ",
        "_",
        "-",
        "widget ",
        "site-header ",
        "widget  sidebar",
        "widget--sidebar",
        "articleish",
    ] {
        assert!(!ARTICLE_CLS_REGEX.is_match(value), "{value:?}");
    }
    for value in [
        "article",
        "ARTICLE",
        "entry",
        "post",
        "story",
        "single-post",
        "main-content",
        "body",
        "text",
        "page",
        "widget article",
        "article-body",
    ] {
        assert!(ARTICLE_CLS_REGEX.is_match(value), "{value:?}");
    }
}

#[test]
fn explicit_boilerplate_classes_still_match() {
    let opts = ExtractOptions::default();
    for class in [
        "navigation",
        "navbar",
        "nav-bar",
        "main-menu",
        "links-list",
        "sidebar",
        "search",
        "search-box",
        "gallery",
        "carousel",
        "modal",
        "modal-window",
        "popup",
        "lightbox",
        "site-footer",
        "footer-wrapper",
        "copyright",
        "cookie-banner",
        "related-posts",
        "recommended",
    ] {
        let html = format!("<div class='{class}'>Boilerplate</div>");
        assert!(!main_content(&html, 3, &opts), "{class}");
    }
    assert!(!main_content("<article><a href='/'>Related article</a></article>", 3, &opts));
}
