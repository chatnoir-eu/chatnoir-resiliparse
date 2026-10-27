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

use crate::parse::html::exception::{CSSParserExceptionPy, HTMLParserExceptionPy};
use crate::parse::html::tree::HTMLTreePy;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use resiliparse::extract::html2text::{self, ExtractOptions, Formatting};
use resiliparse::parse::html::tree::HtmlTree;

/// Python values accepted by the preserve_formatting option.
#[derive(FromPyObject)]
pub enum FormattingArg {
    /// Enable basic formatting with true or disable formatting with false.
    Bool(bool),
    /// Select minimal HTML with "minimal_html"; empty strings disable formatting,
    /// while other strings select basic formatting.
    Text(String),
}

/// Perform a simple plain-text extraction from an HTML string or parsed DOM tree.
///
/// Extracts all visible text (excluding script/style elements, comment nodes etc.)
/// and collapses consecutive white space characters.
///
/// If ``preserve_formatting`` is ``True``, line breaks, paragraphs, other block-level elements,
/// list elements, and pre-formatted text will be preserved. Use the special value ``'minimal_html'`` to
/// add reduced HTML markup to the formatted output, preserving headings (``<h1-6>``), paragraphs (``<p>``),
/// lists (``<ul>``, ``<ol>``), ``<pre>`` text, ``<br>`` line breaks, and links (``<a>``, if ``links=True``).
///
/// Extraction of particular elements and attributes such as links, alt texts, or form fields
/// can be configured individually by setting the corresponding parameter to ``True``.
/// Alternative text and list bullets are enabled by default; links, form fields, and noscript content are disabled.
///
/// :param html: HTML as DOM tree or Unicode string
/// :type html: HTMLTree or str
/// :param preserve_formatting: preserve basic block-level formatting (use ``'minimal_html'`` for minimal HTML
///                             markup in output)
/// :type preserve_formatting: bool or t.Literal['minimal_html']
/// :param main_content: apply simple heuristics for extracting only "main-content" elements
/// :type main_content: bool
/// :param list_bullets: insert bullets / numbers for list items
/// :type list_bullets: bool
/// :param alt_texts: preserve alternative text descriptions
/// :type alt_texts: bool
/// :param links: extract link target URLs
/// :type links: bool
/// :param form_fields: extract form fields and their values
/// :type form_fields: bool
/// :param noscript: extract contents of ``<noscript>`` elements
/// :type noscript: bool
/// :param comments: treat comment sections as main content
/// :type comments: bool
/// :param post_meta: preserve blog post / article meta data in main content extract
/// :type post_meta: bool
/// :param hidden_elements: keep elements with hidden class names during main-content extraction;
///                         explicit hidden attributes and inline CSS remain subject to filtering
/// :type hidden_elements: bool
/// :param skip_elements: list of CSS selectors for elements to skip
/// :type skip_elements: t.Iterable[str] or None
/// :return: extracted plain text
/// :rtype: str
/// :raises TypeError: if html is neither a Unicode string nor an HTMLTree, or an option has an invalid type
/// :raises HTMLParserException: if parsing the HTML string fails
/// :raises CSSParserException: if a skip selector cannot be parsed
#[pyfunction]
#[pyo3(signature = (html, preserve_formatting=FormattingArg::Bool(true), main_content=false,
    list_bullets=true, alt_texts=true, links=false, form_fields=false, noscript=false,
    comments=true, post_meta=true, hidden_elements=false, skip_elements=None))]
#[allow(clippy::too_many_arguments)]
pub fn extract_plain_text(
    html: &Bound<'_, PyAny>,
    preserve_formatting: FormattingArg,
    main_content: bool,
    list_bullets: bool,
    alt_texts: bool,
    links: bool,
    form_fields: bool,
    noscript: bool,
    comments: bool,
    post_meta: bool,
    hidden_elements: bool,
    skip_elements: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    let formatting = match preserve_formatting {
        FormattingArg::Text(s) if s == "minimal_html" => Formatting::MinimalHtml,
        FormattingArg::Bool(false) => Formatting::Off,
        FormattingArg::Text(s) if s.is_empty() => Formatting::Off,
        _ => Formatting::Basic,
    };
    let skip_elements = match skip_elements {
        Some(items) => items
            .try_iter()?
            .map(|item| item?.extract::<String>())
            .collect::<PyResult<Vec<_>>>()?,
        None => Vec::new(),
    };
    let options = ExtractOptions {
        formatting,
        main_content,
        list_bullets,
        alt_texts,
        links,
        form_fields,
        noscript,
        comments,
        post_meta,
        hidden_elements,
        skip_elements,
    };
    let extract = |tree: &HtmlTree| {
        html2text::extract_plain_text(tree, &options).map_err(|err| CSSParserExceptionPy::new_err(err.to_string()))
    };
    if let Ok(document) = html.extract::<String>() {
        let tree = HtmlTree::parse(&document).map_err(|err| HTMLParserExceptionPy::new_err(err.to_string()))?;
        return extract(&tree);
    }
    let tree = html
        .extract::<PyRef<'_, HTMLTreePy>>()
        .map_err(|_| PyTypeError::new_err("Parameter \"html\" is neither string nor HTMLTree."))?;
    extract(&tree.tree)
}
