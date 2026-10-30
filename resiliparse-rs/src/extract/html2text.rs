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

//! Convert parsed HTML into readable text or reduced HTML markup.
//!
//! [`extract_plain_text`] reads an existing [`HtmlTree`] without modifying or reparsing it.
//! [`ExtractOptions`] controls whitespace and block formatting, list bullets, alternative text,
//! link targets, and form fields. Enable `main_content` to filter likely boilerplate, or supply
//! CSS selectors in `skip_elements` to exclude specific subtrees.
//!
//! Main-content filtering uses heuristics rather than browser layout. [`Formatting::MinimalHtml`]
//! preserves selected markup for presentation and does not provide general HTML sanitization.

use crate::parse::html::dom::node::Node;
use crate::parse::html::dom::traits::*;
use crate::parse::html::selector::CssParserError;
use crate::parse::html::tree::HtmlTree;
use std::collections::HashSet;

/// Output formatting policy.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Formatting {
    /// Collapse consecutive white space and separate blocks with single spaces.
    Off,
    /// Preserve line breaks, paragraphs, lists, and pre-formatted text without HTML markup.
    Basic,
    /// Preserve basic formatting with reduced HTML markup for headings, paragraphs, lists, and links.
    MinimalHtml,
}

/// Text selection and formatting options.
#[derive(Clone, Debug)]
pub struct ExtractOptions {
    /// Output formatting policy; defaults to basic block-level formatting.
    pub formatting: Formatting,
    /// Apply simple heuristics for extracting only main-content elements; defaults to false.
    pub main_content: bool,
    /// Insert bullets or numbers for list items, or retain list markup in minimal HTML; defaults to true.
    pub list_bullets: bool,
    /// Preserve alternative text descriptions and media fallback text; defaults to true.
    pub alt_texts: bool,
    /// Extract link target URLs; defaults to false.
    pub links: bool,
    /// Extract form fields and their values; defaults to false.
    pub form_fields: bool,
    /// Extract contents of `<noscript>` elements; defaults to false.
    pub noscript: bool,
    /// Treat comment sections as main content; defaults to true. Does not include HTML comment nodes.
    pub comments: bool,
    /// Preserve blog post or article metadata in main-content extraction; defaults to true.
    pub post_meta: bool,
    /// Keep elements whose class names indicate hidden content during main-content extraction.
    /// Defaults to false; explicit hidden attributes and inline CSS remain subject to filtering.
    pub hidden_elements: bool,
    /// CSS selectors for elements whose entire subtrees should be skipped; empty by default.
    pub skip_elements: Vec<String>,
}

impl Default for ExtractOptions {
    /// Enable basic formatting, list bullets, alternative text, comments, and post metadata.
    fn default() -> Self {
        Self {
            formatting: Formatting::Basic,
            main_content: false,
            list_bullets: true,
            alt_texts: true,
            links: false,
            form_fields: false,
            noscript: false,
            comments: true,
            post_meta: true,
            hidden_elements: false,
            skip_elements: Vec::new(),
        }
    }
}

/// Whether a character is ASCII white space for extraction normalization.
pub(super) fn is_space(c: char) -> bool {
    // Include vertical tab characters to maintain compatibility with the
    // previous Cython implementation.
    c.is_ascii_whitespace() || c == '\u{000b}'
}

/// Remove leading and trailing ASCII white space without allocating.
pub(super) fn trim(s: &str) -> &str {
    s.trim_matches(is_space)
}

/// Remove trailing ASCII white space from a string in place.
fn rstrip(s: &mut String) {
    s.truncate(s.trim_end_matches(is_space).len());
}

/// Collapse newlines and consecutive ASCII white space in a string to single spaces.
pub(super) fn collapse(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = false;
    for c in s.chars() {
        if is_space(c) {
            if !space {
                out.push(' ');
            }
            space = true;
        } else {
            out.push(c);
            space = false;
        }
    }
    out
}

/// Return the local tag name of an element, or an empty string for other nodes.
pub(super) fn tag(n: &Node) -> String {
    n.as_element().and_then(|n| n.local_name()).unwrap_or_default()
}

/// Return an element attribute value, or an empty string if the attribute or element is absent.
pub(super) fn attr(n: &Node, name: &str) -> String {
    n.as_element().map(|n| n.attribute_or_default(name)).unwrap_or_default()
}

/// Whether a node is an element with the named attribute.
pub(super) fn has_attr(n: &Node, name: &str) -> bool {
    n.as_element().is_some_and(|n| n.has_attribute(name))
}

/// Whether a tag introduces a block boundary in extracted text.
pub(super) fn is_block(tag: &str) -> bool {
    matches!(
        tag,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "br"
            | "center"
            | "details"
            | "dd"
            | "dt"
            | "div"
            | "dl"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "hgroup"
            | "hr"
            | "li"
            | "main"
            | "nav"
            | "ol"
            | "p"
            | "pre"
            | "section"
            | "table"
            | "tr"
            | "ul"
    )
}

/// Escape ampersands, double quotes, and angle brackets for minimal HTML output.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

/// Collected traversal event and text fragment awaiting output formatting.
struct ExtractNode {
    /// Local tag name associated with this event; empty for non-element nodes.
    tag: String,
    /// Whether the source node has children, used to decide which HTML tags to emit.
    has_children: bool,
    /// DOM depth relative to the extraction root.
    depth: usize,
    /// Nesting depth of pre-formatted elements after processing this event.
    pre_depth: usize,
    /// Whether adjacent block margins collapse instead of accumulating as explicit line breaks.
    collapse_margins: bool,
    /// Whether this event requests a block boundary.
    block: bool,
    /// Whether this block requests a blank line, as paragraphs and major headings do.
    big_block: bool,
    /// Whether the event closes an element rather than entering it.
    end: bool,
    /// Whether the collected text still needs HTML escaping during serialization.
    escape: bool,
    /// Text accumulated for this event, including generated link or form-field text.
    text: Option<String>,
}

/// Collect one DOM traversal event, merging inline text into the current extraction node.
///
/// `depth` is relative to the extraction root; `end` marks an element exit.
/// Block boundaries, links, and text areas create separate extraction nodes.
fn collect(nodes: &mut Vec<ExtractNode>, node: &Node, depth: usize, end: bool, opts: &ExtractOptions) {
    let name = tag(node);
    let block = is_block(&name);
    let last = nodes.last();
    if last.is_none_or(|last| block || depth < last.depth) || (opts.links && name == "a") || name == "textarea" {
        let mut pre_depth = last.map_or(0, |n| n.pre_depth);
        if matches!(name.as_str(), "pre" | "textarea") {
            pre_depth = if end {
                pre_depth.saturating_sub(1)
            } else {
                pre_depth + 1
            };
        }
        nodes.push(ExtractNode {
            tag: name.clone(),
            has_children: node.has_child_nodes(),
            depth,
            pre_depth,
            collapse_margins: true,
            block,
            big_block: matches!(name.as_str(), "p" | "h1" | "h2" | "h3" | "h4"),
            end,
            escape: opts.formatting == Formatting::MinimalHtml,
            text: None,
        });
    }
    let current = nodes.last_mut().unwrap();
    let mut text = None;
    if node.node_type() == Some(NodeType::Text) {
        let value = node.node_value().unwrap_or_default();
        text = Some(if current.tag == "a" && opts.formatting == Formatting::MinimalHtml {
            escape(&value)
        } else {
            value
        });
    } else if node.as_element().is_none() {
        return;
    } else if name == "br" && opts.formatting == Formatting::Basic {
        current.collapse_margins = false;
        text = Some(String::new());
    } else if opts.links && name == "a" {
        let href = attr(node, "href");
        current.block = false;
        text = Some(if opts.formatting == Formatting::MinimalHtml {
            current.escape = false;
            if end {
                "</a>".to_owned()
            } else {
                format!("<a href=\"{}\">", escape(trim(&href)))
            }
        } else if end {
            format!(" ({})", trim(&href))
        } else {
            String::new()
        });
    } else if opts.alt_texts && matches!(name.as_str(), "img" | "area") {
        text = Some(attr(node, "alt"));
    } else if opts.form_fields && matches!(name.as_str(), "textarea" | "button") {
        text = Some(if end { " ] " } else { "[ " }.to_owned());
    } else if opts.form_fields
        && name == "input"
        && !matches!(trim(&attr(node, "type")), "checkbox" | "color" | "file" | "hidden" | "radio" | "reset")
    {
        let value = attr(node, "value");
        let value = if trim(&value).is_empty() {
            attr(node, "placeholder")
        } else {
            value
        };
        if !trim(&value).is_empty() {
            text = Some(format!("[ {} ] ", trim(&value)));
        }
    }
    if let Some(text) = text {
        current.text.get_or_insert_with(String::new).push_str(&text);
    }
}

/// Flush the pending block margin and reset its size.
///
/// Use a single space when formatting is disabled; otherwise emit the requested
/// line breaks, preserving trailing white space inside pre-formatted content.
fn margin(out: &mut String, size: &mut usize, n: &ExtractNode, opts: &ExtractOptions) {
    if *size == 0 {
        return;
    }
    if n.pre_depth == 0 || opts.formatting == Formatting::Off {
        rstrip(out);
    }
    if !out.is_empty() {
        if opts.formatting == Formatting::Off {
            out.push(' ');
        } else {
            out.extend(std::iter::repeat_n('\n', *size));
        }
    }
    *size = 0;
}

/// Append two spaces per list nesting level.
fn indent(out: &mut String, depth: usize) {
    out.extend(std::iter::repeat_n(' ', depth * 2));
}

/// Serialize collected extraction nodes according to the requested formatting policy.
///
/// Collapse block margins, maintain list numbering and indentation, preserve
/// pre-formatted text, and optionally emit reduced HTML markup.
fn serialize(nodes: Vec<ExtractNode>, opts: &ExtractOptions) -> String {
    let mut out = String::new();
    let mut prefix = String::new();
    let mut bullet = false;
    let mut depth: usize = 0;
    let mut margins: usize = 0;
    let mut uncollapsed = 0;
    let mut numbering: Vec<usize> = Vec::new();
    for mut n in nodes {
        if opts.formatting != Formatting::Off {
            if n.block && !n.collapse_margins {
                uncollapsed += 1;
            }
            if matches!(n.tag.as_str(), "ul" | "ol") || (n.tag == "li" && depth == 0) {
                if n.end {
                    depth = depth.saturating_sub(1);
                    numbering.pop();
                    bullet = false;
                    prefix.clear();
                } else {
                    depth += 1;
                    numbering.push(usize::from(n.tag == "ol"));
                }
            }
            if opts.list_bullets && n.tag == "li" {
                if opts.formatting == Formatting::Basic {
                    let number = numbering.last_mut().expect("list item has a list level");
                    prefix = if *number == 0 {
                        "• ".to_owned()
                    } else {
                        let prefix = format!("{}. ", number);
                        if !n.end {
                            *number += 1;
                        }
                        prefix
                    };
                    bullet = !n.end;
                } else {
                    margin(&mut out, &mut margins, &n, opts);
                    if !n.end {
                        indent(&mut out, depth);
                        out.push_str("<li>");
                        margins = 0;
                        n.block = false;
                    } else {
                        if n.pre_depth == 0 {
                            rstrip(&mut out);
                        }
                        out.push_str("</li>\n");
                    }
                }
            }
        }
        if opts.formatting == Formatting::MinimalHtml {
            if n.tag == "pre" {
                if !n.end {
                    margin(&mut out, &mut margins, &n, opts);
                }
                out.push_str(if n.end { "</pre>" } else { "<pre>" });
                margins = 0;
            }
            if n.pre_depth > 0 {
                n.block = false;
            }
            if n.tag == "br" {
                out.push_str("<br>");
            }
            let list = matches!(n.tag.as_str(), "ul" | "ol");
            if n.has_children
                && (matches!(n.tag.as_str(), "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p")
                    || (list && opts.list_bullets))
            {
                if (!n.end && n.pre_depth == 0) || (uncollapsed > 0 && n.collapse_margins) {
                    let size = usize::from(n.block) + usize::from(n.big_block);
                    if n.collapse_margins {
                        margins = margins.max(size);
                    } else {
                        margins += size;
                    }
                    margin(&mut out, &mut margins, &n, opts);
                    n.block = false;
                    uncollapsed = 0;
                }
                if opts.list_bullets {
                    indent(&mut out, depth.saturating_sub(usize::from(list && !n.end)));
                }
                out.push('<');
                if n.end {
                    out.push('/');
                }
                out.push_str(&n.tag);
                out.push('>');
                if list && !n.end && n.pre_depth == 0 {
                    out.push('\n');
                }
            }
        }
        if n.block {
            if n.collapse_margins {
                margins = margins.max(if n.big_block && n.pre_depth == 0 { 2 } else { 1 });
            } else {
                margins += if n.big_block { 2 } else { 1 };
            }
        }
        let Some(mut text) = n.text.take() else {
            continue;
        };
        if n.pre_depth == 0 || opts.formatting == Formatting::Off {
            text = collapse(&text);
            if n.block || out.chars().last().is_some_and(is_space) {
                text = text.trim_start_matches(is_space).to_owned();
            }
        }
        if text.is_empty() {
            continue;
        }
        if n.escape {
            text = escape(&text);
        }
        margin(&mut out, &mut margins, &n, opts);
        uncollapsed = 0;
        if depth > 0 && opts.formatting == Formatting::Basic {
            indent(&mut out, depth + usize::from(opts.list_bullets && !bullet));
            bullet = false;
        }
        if opts.formatting != Formatting::Off
            && matches!(n.tag.as_str(), "td" | "th")
            && !out.is_empty()
            && !out.ends_with('\n')
        {
            out.push_str("\t\t");
        }
        out.push_str(&prefix);
        prefix.clear();
        out.push_str(&text);
    }
    rstrip(&mut out);
    out
}

/// Extract text from the document body without modifying or reparsing the tree.
///
/// Extract visible text, excluding script/style elements and HTML comment nodes,
/// and collapse consecutive white space outside pre-formatted content. Options
/// control block formatting, reduced HTML markup, alternative text, links, form
/// fields, and heuristic main-content filtering.
///
/// Return an empty string if the document has no body. When main-content
/// filtering identifies exactly one candidate container, extract from that
/// subtree instead of the entire body.
///
/// # Errors
///
/// Return a [`CssParserError`] if a selector in [`ExtractOptions::skip_elements`]
/// cannot be parsed.
pub fn extract_plain_text(tree: &HtmlTree, opts: &ExtractOptions) -> Result<String, CssParserError> {
    let Some(body) = tree.body() else {
        return Ok(String::new());
    };
    let owner = body.tree();
    let _guard = owner.doc_ptr();
    let mut root = body.as_node();
    if opts.main_content {
        // The legacy selector-list query counts an element once for every
        // selector it matches. Query selectors separately to preserve that
        // behavior when one element matches, for example, both `.main-content`
        // and `[role=main]`.
        let selectors = [
            ".article-body",
            ".articleBody",
            ".contentBody",
            ".article-text",
            ".main-content",
            ".postcontent",
            ".post-content",
            ".single-post",
            "[role=main]",
        ];
        let mut candidate = None;
        let mut candidate_count = 0;
        for selector in selectors {
            let matches = body.query_selector_all(selector)?;
            if matches.is_empty() {
                continue;
            }
            candidate_count += matches.len();
            if candidate_count > 1 {
                break;
            }
            candidate = Some(matches.item(0).unwrap().as_node());
        }
        if candidate_count == 1 {
            root = candidate.unwrap();
        }
    }
    let mut selectors = opts.skip_elements.clone();
    selectors.extend(["script", "style", "iframe", "frame", "template"].map(str::to_owned));
    if !opts.alt_texts {
        selectors.extend(
            [
                "object",
                "video",
                "audio",
                "embed",
                "img",
                "area",
                "svg",
                "figcaption",
                "figure",
            ]
            .map(str::to_owned),
        );
    }
    if !opts.noscript {
        selectors.push("noscript".to_owned());
    }
    if !opts.form_fields {
        selectors.extend(["textarea", "input", "button", "select", "option", "label"].map(str::to_owned));
    }
    let blacklist: HashSet<_> = root
        .as_parent()
        .unwrap()
        .query_selector_all(&selectors.join(","))?
        .iter()
        .filter_map(|n| n.identity())
        .collect();
    let mut base_depth = 0;
    let mut parent = root.clone();
    while tag(&parent) != "body" {
        let Some(next) = parent.parent_node() else {
            break;
        };
        parent = next;
        base_depth += 1;
    }
    let mut nodes = Vec::new();
    let mut current = Some(root.clone());
    let mut depth = 0;
    let mut end = false;
    while let Some(node) = current {
        if !matches!(node.node_type(), Some(NodeType::Element | NodeType::Text))
            || node.identity().is_some_and(|id| blacklist.contains(&id))
            || (opts.main_content && !super::filters::is_main_content(&node, depth + base_depth, opts))
        {
            end = true;
        } else {
            collect(&mut nodes, &node, depth, end, opts);
        }
        if !end && let Some(child) = node.first_child() {
            depth += 1;
            current = Some(child);
        } else if node == root {
            current = None;
        } else if let Some(next) = node.next_sibling() {
            end = false;
            current = Some(next);
        } else {
            end = true;
            depth = depth.saturating_sub(1);
            current = node.parent_node();
        }
    }
    Ok(serialize(nodes, opts))
}

// ===========================================================
// Tests
// ===========================================================

#[cfg(test)]
#[path = "html2text_test.rs"]
mod html2text_test;
