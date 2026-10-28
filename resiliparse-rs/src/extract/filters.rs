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

use super::html2text::*;
use crate::parse::html::dom::node::Node;
use crate::parse::html::dom::traits::*;
use regex::{Regex, RegexBuilder};
use std::sync::LazyLock;

/// Class or ID names suggesting article or main-content containers.
static ARTICLE_CLS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(
        r"(?:^|[\s_-])(?:article|entry|post|story|single[_-]?post|(?:main[_-])?content|body|text|page)?(?:$|[\s_-])",
    )
    .case_insensitive(true)
    .unicode(false)
    .build()
    .expect("valid extraction pattern")
});

/// Class or ID names suggesting navigation menus, breadcrumbs, or link collections.
static NAV_CLS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(concat!(
        r"(?:^|\s)(?:(?:l|m|wp|main|site|page|sub|article|global|sticky|main)[_-]*)?",
        r"(?:nav(?:igation)?|menu(?:[_-]item)?|drop[_-]?down|bread[_-]?crumbs?)|(?:links?[_-]?",
        r"(?:bar|box|list|container|section|wrapp(?:er))?)(?:$|[\s_-])",
    ))
    .case_insensitive(true)
    .unicode(false)
    .build()
    .expect("valid extraction pattern")
});

/// Class or ID names suggesting recommended, related, or popular content.
static RECOMMENDED_CLS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(concat!(
        r"(?:^|[\s_-])(?:trends|trending|recommended|featured|popular|editors?[_-]picks",
        r"|related|read-next|(?:related|more|other)[_-]?(?:links|articles|posts|guides|stories))(?:$|[\s_-])",
    ))
    .case_insensitive(true)
    .unicode(false)
    .build()
    .expect("valid extraction pattern")
});

/// IDs identifying global page landmarks such as headers, footers, and side columns.
static LANDMARK_ID_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(r"^(?:(?:l|wp|global|page|site|full|sticky)[_-]*)?(?:(?:head|foot)(?:er)?|right)$")
        .case_insensitive(true)
        .unicode(false)
        .build()
        .expect("valid extraction pattern")
});

/// Class or ID names identifying page header containers.
static HEADER_CLS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(r"(?:^|\s)(?:l|m|wp|global|page|site|full|sticky)[_-]*header(?:[_-]?wrap(?:per)?|bar)?(?:$|\s)")
        .case_insensitive(true)
        .unicode(false)
        .build()
        .expect("valid extraction pattern")
});

/// Class or ID names identifying footers, copyright notices, and consent content.
static FOOTER_CLS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(
        r"(?:^|[\s_-])(?:global|page|site|copyright)?(?:footer|copyright|cookie|consent|legal|fcontainer)(?:$|[\s_-])",
    )
    .case_insensitive(true)
    .unicode(false)
    .build()
    .expect("valid extraction pattern")
});

/// Class names identifying article metadata such as authors, dates, tags, and categories.
static POST_META_CLS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(
        concat!(
            r"(?:^|[\s_-])(?:(?:post|entry|article(?:page)?|content|story|section)[_-]*(?:text[_-]*)?",
            r"(?:footer|teaser|meta(?:[_-]?data)?|subline|sidebar|author(?:name)?|published|timestamp|date|posted[_-]?on|info",
            r"|labels?|tags?|keywords|category)|by[_-]?line|date[_-]?line|author-date|submitted(?:-by)?)",
            r"|meta[_-]?data(?:$|[\s_-])",
        ),
    )
    .case_insensitive(true)
    .unicode(false)
    .build()
    .expect("valid extraction pattern")
});

/// Class or ID names identifying sidebars and narrow side columns.
static SIDEBAR_CLS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(concat!(
        r"(?:^|\s)(?:(?:l|wp|right|left|global|sticky)[_-]*)?(?:(?:side|sticky)[_-]?(?:bars?|box)",
        r"|one-third)(?:$|[\s_-])",
    ))
    .case_insensitive(true)
    .unicode(false)
    .build()
    .expect("valid extraction pattern")
});

/// Class or ID names identifying search controls.
static SEARCH_CLS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(r"(?:^|[\s_-])search(?:[_-]?(?:bar|facility|box))?(?:$|\s)")
        .case_insensitive(true)
        .unicode(false)
        .build()
        .expect("valid extraction pattern")
});

/// Class or ID names identifying skip links, pagination, and scrolling controls.
static SKIP_LINK_CLS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(concat!(
        r"(?:^|\s)(?:link[_-]?)?(?:skip(?:[_-]?(?:to|link))?|scroll[_-]?(?:up|down)|next|prev(?:ious)?",
        r"|permalink|pagination|skip-to-(?:main-)?content)(?:$|\s|[_-]?(?:post|article))",
    ))
    .case_insensitive(true)
    .unicode(false)
    .build()
    .expect("valid extraction pattern")
});

/// Class names indicating hidden, collapsed, or otherwise invisible content.
static DISPLAY_CLS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(concat!(
        r"(?:^|\s)(?:(?:is|visually)[_-])?",
        r"(?:display-none|hidden|invisible|collapsed|h-0|nocontent|expandable)(?:-xs|-sm|-lg|-2?xl)?(?:$|\s)",
    ))
    .case_insensitive(true)
    .unicode(false)
    .build()
    .expect("valid extraction pattern")
});

/// Inline CSS declarations setting display to none or visibility to hidden.
static DISPLAY_CSS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(r"(?:^|;\s*)(?:display\s?:\s?none|visibility\s?:\s?hidden)(?:$|\s?;)")
        .case_insensitive(true)
        .unicode(false)
        .build()
        .expect("valid extraction pattern")
});

/// Class or ID names identifying modal dialogs, popups, and lightboxes.
static MODAL_CLS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(r"(?:^|\s)(?:wp-|p-|-l)?(?:modal|popup|lightbox)(?:[_-]*(?:window|pane|box))?(?:$|[\s_-])")
        .case_insensitive(true)
        .unicode(false)
        .build()
        .expect("valid extraction pattern")
});

/// Class or ID names identifying image galleries and carousels.
static GALLERY_CLS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(r"(?:^|[\s_-])(?:gallery|carousel)(?:$|[\s_-])")
        .case_insensitive(true)
        .unicode(false)
        .build()
        .expect("valid extraction pattern")
});

/// Class names identifying login, registration, and account controls.
static SIGNIN_CLS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(concat!(
        r"(?:^|[\s_-])(?:(?:log[_-]?in|sign[_-]?(?:in|up)|account)|user[_-]",
        r"(?:info|profile|settings|actions))(?:$|[\s_-])",
    ))
    .case_insensitive(true)
    .unicode(false)
    .build()
    .expect("valid extraction pattern")
});

/// Class or ID names identifying advertisements, sponsorships, and donation banners.
static ADS_CLS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(concat!(
        r"(?:^|\s)(?:(?:google|wide)[_-]?ads?|ad(?:vert|vertise(?:ment|link)?|$|_[a-f0-9]+)",
        r"|sponsor(?:ed)?|promoted|paid|(?:wide)?banner|donate)(?:$|[\s_-])",
    ))
    .case_insensitive(true)
    .unicode(false)
    .build()
    .expect("valid extraction pattern")
});

/// Class names identifying sharing, subscription, and social engagement controls.
static SOCIAL_CLS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(concat!(
        r"(?:^|\s|__|--|mobile-|desktop-|l-|m-|c-)(?:social(?:media|search)?|share(?:daddy)?",
        r"|syndication|newsletter|sharing|follow|email|likes?|(?:give[_-]?)?feedback|(?:brand[_-])?engagement",
        r"|facebook|twitter|subscribe|wa|jp|aptf-follow)(?:[_-]?(?:post|links?|section|icons?",
        r"|btn|buttons?|target))?(?:$|[\s_-])",
    ))
    .case_insensitive(true)
    .unicode(false)
    .build()
    .expect("valid extraction pattern")
});

/// Class or ID names identifying comment sections and reply controls.
static COMMENTS_CLS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(
        r"(?:^|[\s_-])(?:(?:article|user|post)[_-]*)?(?:(?:no[_-]?)?comments?|comment[_-]?list|reply)(?:$|[\s_-])",
    )
    .case_insensitive(true)
    .unicode(false)
    .build()
    .expect("valid extraction pattern")
});

/// Class or ID names identifying logos and branding.
static LOGO_CLS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(r"(?:brand(?:ing)?[_-]*)?logo(?:$|\s)")
        .case_insensitive(true)
        .unicode(false)
        .build()
        .expect("valid extraction pattern")
});

/// Class or ID names identifying print-related content.
static PRINT_CLS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(r"(?:^|\s)print[_-]")
        .case_insensitive(true)
        .unicode(false)
        .build()
        .expect("valid extraction pattern")
});

/// Class names identifying view counters and loading spinners.
static OTHER_JUNK_CLS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new(r"(?:^|\s)short-view-count|spinner(?:$|[\s_-])")
        .case_insensitive(false)
        .unicode(false)
        .build()
        .expect("valid extraction pattern")
});

/// Search a nonempty string for a matching pattern.
fn matches(s: &str, regex: &Regex) -> bool {
    !s.is_empty() && regex.is_match(s)
}

/// Whether an element contains more link text than the given ratio permits.
///
/// Measure lengths in UTF-8 bytes after collapsing white space. A nonzero
/// `max_length` excludes longer text from consideration; zero disables that limit.
/// Empty text and non-element nodes are not link clusters.
fn link_cluster(node: &Node, ratio: f64, max_length: usize) -> bool {
    let text = collapse(&node.text_content().unwrap_or_default());
    if (max_length > 0 && text.len() > max_length) || text.is_empty() {
        return false;
    }
    let Some(parent) = node.as_element() else {
        return false;
    };
    let links: usize = parent
        .get_elements_by_tag_name("a")
        .iter()
        .map(|n| collapse(&n.text_content().unwrap_or_default()).len())
        .sum();
    links > 0 && links as f64 / text.len() as f64 > ratio
}

/// Determine heuristically whether a DOM node is part of the main content.
///
/// `depth` is the node depth relative to the document body. Use tag names,
/// attributes, class and ID patterns, and link density to reject boilerplate.
/// The caller skips the entire subtree when this function returns false.
/// Comment sections, article metadata, and hidden classes honor the corresponding
/// options; HTML comment nodes are handled separately by the traversal.
pub(super) fn is_main_content(node: &Node, depth: usize, opts: &ExtractOptions) -> bool {
    if node.node_type() == Some(NodeType::Text) {
        let text = node.node_value().unwrap_or_default();
        let text = trim(&text);
        if text == "¶" {
            return false;
        }
        // Match the legacy extractor's byte-level test. It interprets three
        // UTF-8 bytes as a little-endian integer, so this intentionally
        // preserves its historical classification, including some symbols
        // outside the private-use area.
        if let [a, b, c] = text.as_bytes() {
            let code = u32::from_le_bytes([*a, *b, *c, 0]);
            return !(0x8080ee..=0xbfa3ef).contains(&code);
        }
        return true;
    }
    if node.as_element().is_none() {
        return true;
    }
    let name = tag(node);
    match name.as_str() {
        "body" | "main" | "h1" => return true,
        "footer" => {
            if depth < 3 || link_cluster(node, 0.2, 0) {
                return false;
            }
            let mut parent = node.clone();
            while let Some(next_parent) = parent.parent_node() {
                if tag(&next_parent) == "body" {
                    break;
                }
                if let Some(next) = parent.next_sibling() {
                    if next.node_type() != Some(NodeType::Text) || next.next_sibling().is_some() {
                        return true;
                    }
                }
                parent = next_parent;
            }
            return false;
        }
        "ul" if depth < 4 || link_cluster(node, 0.2, 0) => return false,
        "article" if depth > 2 && link_cluster(node, 0.2, 500) => return false,
        "nav" | "aside" | "audio" | "video" | "time" => return false,
        _ => {}
    }
    if has_attr(node, "hidden")
        || matches!(trim(&attr(node, "rel")), "author" | "icon" | "search" | "prev" | "next" | "tag")
        || matches!(trim(&attr(node, "itemprop")), "datePublished" | "author" | "url")
        || trim(&attr(node, "aria-hidden")) == "true"
        || trim(&attr(node, "aria-expanded")) == "false"
    {
        return false;
    }
    let class = attr(node, "class");
    let id = attr(node, "id");
    if class.is_empty() && id.is_empty() {
        return name != "div" || depth <= 5 || !link_cluster(node, 0.6, 800);
    }
    let combined = format!("{}{}{}", class, if class.is_empty() { "" } else { " " }, id);
    if (!opts.hidden_elements && matches(&class, &DISPLAY_CLS_REGEX))
        || matches(&attr(node, "style"), &DISPLAY_CSS_REGEX)
    {
        return false;
    }
    if matches!(name.as_str(), "a" | "div" | "li") && matches(&combined, &SKIP_LINK_CLS_REGEX) {
        return false;
    }
    if depth > 2
        && (matches(&class, &SIGNIN_CLS_REGEX)
            || (!opts.post_meta && matches(&class, &POST_META_CLS_REGEX))
            || matches(&class, &SOCIAL_CLS_REGEX))
    {
        return false;
    }
    if matches(&combined, &LOGO_CLS_REGEX)
        || matches(&combined, &ADS_CLS_REGEX)
        || ["data-ad", "data-advertisement", "data-text-ad"]
            .iter()
            .any(|a| has_attr(node, a))
        || matches(&class, &OTHER_JUNK_CLS_REGEX)
    {
        return false;
    }
    if !is_block(&name) && name != "td" {
        return true;
    }
    if trim(&attr(node, "rel")) == "main" {
        return true;
    }
    if matches!(
        trim(&attr(node, "role")),
        "alert"
            | "banner"
            | "checkbox"
            | "comment"
            | "complementary"
            | "contentinfo"
            | "dialog"
            | "img"
            | "menu"
            | "menubar"
            | "menuitem"
            | "navigation"
            | "presentation"
            | "radio"
            | "search"
            | "searchbox"
            | "separator"
            | "tab"
            | "toolbar"
            | "tooltip"
    ) {
        return false;
    }
    if matches(&combined, &ARTICLE_CLS_REGEX) {
        return true;
    }
    if matches(&id, &LANDMARK_ID_REGEX)
        || matches(&combined, &HEADER_CLS_REGEX)
        || matches(&combined, &FOOTER_CLS_REGEX)
        || matches(&combined, &NAV_CLS_REGEX)
        || matches(&combined, &RECOMMENDED_CLS_REGEX)
        || (!opts.comments && matches(&combined, &COMMENTS_CLS_REGEX))
        || matches(&combined, &SEARCH_CLS_REGEX)
        || matches(&combined, &SIDEBAR_CLS_REGEX)
        || matches(&combined, &MODAL_CLS_REGEX)
        || matches(&combined, &GALLERY_CLS_REGEX)
        || matches(&combined, &PRINT_CLS_REGEX)
    {
        return false;
    }
    !(depth > 2 && name == "div" && link_cluster(node, 0.6, 1500))
}
