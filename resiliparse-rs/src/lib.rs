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

//! Resiliparse is a collection of robust and fast tools for parsing and analyzing web data, written in Rust with
//! bindings for Python. It provides HTML parsing and DOM manipulation, character encoding and MIME detection,
//! language detection, HTTP chunk decoding, and text extraction with optional boilerplate removal.
//!
//! Resiliparse belongs to the [ChatNoir Resiliparse toolkit](https://resiliparse.chatnoir.eu/).
//!
//! > **Note:** This documentation covers the Resiliparse Rust crate. For the Python bindings, see the
//! > [Python documentation](https://resiliparse.chatnoir.eu/en/latest/).
//!
//! # Installing Resiliparse
//!
//! Add Resiliparse to your project's `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! resiliparse = "*"    # Use correct version here
//! ```
//!
//! Building requires Rust and Cargo, a C/C++ compiler, CMake, libclang for binding generation, and `vcpkg` on `PATH`.
//! The build script installs Lexbor and uchardet through vcpkg when the selected triplet is missing and links them
//! statically. Set `RESILIPARSE_VCPKG_PATH` to use an existing vcpkg installation containing platform triplet directories.
//! Alternatively, `RESILIPARSE_USE_SYSTEM_LIBS=1` uses system headers and shared libraries, which must also be available
//! at runtime. See the [build instructions](https://resiliparse.chatnoir.eu/en/latest/man/installation.html) for details.
//!
//! From a checkout of the repository, run `cargo build -p resiliparse` to build the Rust crate.
//!
//! ## Resiliparse for Python
//!
//! Pre-built Python wheels with PyO3 bindings are available from [PyPI](https://pypi.org/project/Resiliparse/):
//!
//! ```bash
//! pip install resiliparse
//! ```
//!
//! The [Python manual](https://resiliparse.chatnoir.eu/en/latest/) also covers the CLI, process guards, iteration helpers,
//! and Apache Beam utilities. The Rust crate exposes the parsing and extraction modules described below.
//!
//! # Character Encoding
//!
//! ## Detecting Encodings
//!
//! [`EncodingDetector`](parse::encoding::EncodingDetector) wraps uchardet's universal charset detector. Feed it byte
//! blocks with `update()` and retrieve the predicted label with `encoding()`. Retrieving a label resets the detector,
//! so one instance can be reused for multiple documents. To discard an unfinished document, call `reset()`.
//!
//! Unlike the Python wrapper, the Rust detector returns the raw uchardet label, or an empty string if detection fails.
//! Use [`map_encoding_to_html5`](parse::encoding::map_encoding_to_html5) to normalize that label for web content:
//!
//! ```
//! use resiliparse::parse::encoding::{EncodingDetector, map_encoding_to_html5};
//!
//! let mut detector = EncodingDetector::new().expect("allocate charset detector");
//! detector.update(b"\xff\xfeH\x00e\x00l\x00l\x00o\x00");
//! let label = detector.encoding();
//! assert_eq!(map_encoding_to_html5(&label, true), Some("utf-16-le"));
//! ```
//!
//! For a complete byte buffer, [`detect_encoding`](parse::encoding::detect_encoding) is a convenient shortcut.
//! Its `max_len` argument limits the sample to the beginning and end of large inputs; `0` samples the entire input.
//! An allocation failure returns `None`, while unsuccessful detection returns `Some("")`.
//!
//! ```
//! use resiliparse::parse::encoding::{detect_encoding, map_encoding_to_html5};
//!
//! let label = detect_encoding(b"Hello world", 128 * 1024).expect("allocate charset detector");
//! let encoding = map_encoding_to_html5(&label, true).unwrap();
//! assert_eq!(encoding, "cp1252");
//! ```
//!
//! Detection is heuristic: a predicted label does not guarantee that every input byte can be decoded with that encoding.
//!
//! ## Mapping Labels and Decoding Text
//!
//! The WHATWG mapping accounts for common web encoding labels and mislabelling practices. For example, ISO-8859-1 is
//! mapped to Windows-1252. The returned labels are the same codec names used by the Python bindings:
//!
//! ```
//! use resiliparse::parse::encoding::map_encoding_to_html5;
//!
//! assert_eq!(map_encoding_to_html5("iso-8859-1", true), Some("cp1252"));
//! assert_eq!(map_encoding_to_html5("csisolatin9", true), Some("iso8859-15"));
//! assert_eq!(map_encoding_to_html5("unknown", true), Some("utf-8"));
//! assert_eq!(map_encoding_to_html5("unknown", false), None);
//! ```
//!
//! [`html_meta_label`](parse::encoding::html_meta_label) extracts the first encoding declaration from HTML's initial
//! 1024 bytes. Normalize an extracted HTTP charset or HTML meta label before passing it to a decoder.
//! The Rust crate does not expose the Python `bytes_to_str` helper: decode non-UTF-8 bytes with a suitable Rust codec
//! library before parsing. For known UTF-8 input, `std::str::from_utf8` validates bytes without allocating a new string;
//! `String::from_utf8_lossy` replaces invalid UTF-8 sequences when that behavior is appropriate.
//!
//! ## Detecting MIME Types
//!
//! [`detect_mime`](parse::encoding::detect_mime) checks magic bytes and the proportion of unprintable bytes.
//! Its second argument is the maximum unprintable-byte ratio allowed for a plain-text result:
//!
//! ```
//! use resiliparse::parse::encoding::detect_mime;
//!
//! assert_eq!(detect_mime(b"%PDF-1.5\n%\xd0\xd4\xc5\xd8", 0.05), "application/pdf");
//! assert_eq!(detect_mime(b"Hello world", 0.05), "text/plain");
//! assert_eq!(detect_mime(b"Hello \x00\x0f\x00world", 0.05), "application/octet-stream");
//! ```
//!
//! This is a quick content check rather than comprehensive file-format validation.
//!
//! # HTML Parsing
//!
//! [`HtmlTree`](parse::html::tree::HtmlTree) parses HTML with Lexbor and provides document, head, body, and title access.
//! Input strings must already be decoded to Unicode. Import the DOM traits to make their methods available on node handles:
//!
//! ```
//! use resiliparse::parse::html::{dom::traits::*, tree::HtmlTree};
//!
//! let tree = HtmlTree::parse(r#"<!doctype html>
//!     <html><head><title>Example page</title></head><body>
//!     <main id="foo">
//!         <p id="a">Hello <span class="bar">world</span>!</p>
//!         <p id="b" class="dom">Hello <a href="/example">DOM</a>!</p>
//!     </main></body></html>"#).unwrap();
//!
//! assert_eq!(tree.title().as_deref(), Some("Example page"));
//! let body = tree.body().unwrap();
//! assert_eq!(body.local_name().as_deref(), Some("body"));
//! let paragraph = tree.document().unwrap().get_element_by_id("a").unwrap();
//! assert_eq!(paragraph.text_content().as_deref(), Some("Hello world!"));
//! assert_eq!(paragraph.outer_html(), r#"<p id="a">Hello <span class="bar">world</span>!</p>"#);
//! ```
//!
//! The parser repairs malformed HTML according to HTML parsing rules. Parser allocation or processing failures return
//! [`HtmlParserError`](parse::html::tree::HtmlParserError). Document anchors and selectors return `Option` for missing
//! nodes. Selector parsing adds a separate `Result`: `Ok(None)` means no match, whereas `Err` means an invalid selector.
//!
//! ## Selecting Elements
//!
//! Use ID, tag, class, attribute, or CSS selectors to find elements. Prefer a single-match query when you only need the
//! first element. `matches()` checks the element itself; `query_selector()` searches its descendants.
//! ID lookup with `get_element_by_id()` is available on the document; use a CSS ID selector for a subtree query.
//!
//! ```
//! use resiliparse::parse::html::{dom::traits::*, tree::HtmlTree};
//!
//! let tree = HtmlTree::parse("<main><p class='item'>One</p><p class='item'>Two</p></main>").unwrap();
//! let body = tree.body().unwrap();
//! let main = body.query_selector("main").unwrap().unwrap();
//! assert!(main.matches("main").unwrap());
//! assert!(!main.matches(".item").unwrap());
//! assert!(main.query_selector(".item").unwrap().is_some());
//!
//! let paragraphs = main.query_selector_all("p.item").unwrap();
//! assert_eq!(paragraphs.len(), 2);
//! assert_eq!(paragraphs.item(0).unwrap().text_content().as_deref(), Some("One"));
//! for paragraph in paragraphs.iter() {
//!     println!("{}", paragraph.outer_html());
//! }
//! ```
//!
//! [`ElementNodeList`](parse::html::dom::coll::ElementNodeList) is a snapshot of matching node handles. It supports
//! `len()`, `item()`, `iter()`, and `as_slice()`, as well as queries within the selected elements' subtrees.
//! By contrast, `children()` and `child_nodes()` return live collections that reflect subsequent document mutations.
//! Use their `snapshot()` method when you need a fixed collection before editing the tree.
//!
//! Repeated queries can reuse a compiled [`CssSelector`](parse::html::selector::CssSelector):
//!
//! ```
//! use resiliparse::parse::html::{dom::traits::*, selector::CssSelector, tree::HtmlTree};
//!
//! let selector = CssSelector::new("a[href]").unwrap();
//! for html in ["<a href='/one'>One</a>", "<a href='/two'>Two</a>"] {
//!     let tree = HtmlTree::parse(html).unwrap();
//!     let body = tree.body().unwrap();
//!     let link = selector.query_selector(&body.as_noderef()).unwrap().unwrap();
//!     println!("{}", link.attribute("href").unwrap());
//! }
//! ```
//!
//! ## Attributes and Serialization
//!
//! The [`Element`](parse::html::dom::traits::Element) trait supplies `attribute()`, `set_attribute()`, `remove_attribute()`,
//! `id()`, `class_name()`, and `class_list()`. Missing attributes return `None`. `attribute_names()` lists attribute names,
//! and `attributes()` provides a map-like view of attribute nodes.
//!
//! ```
//! use resiliparse::parse::html::{dom::traits::*, tree::HtmlTree};
//!
//! let tree = HtmlTree::parse("<p id='message'>Hello <b>world</b>!</p>").unwrap();
//! let paragraph = tree.document().unwrap().get_element_by_id("message").unwrap();
//! paragraph.set_attribute("data-source", "example");
//! paragraph.set_class_name("greeting highlighted");
//! assert_eq!(paragraph.attribute("data-source").as_deref(), Some("example"));
//! assert_eq!(paragraph.class_name().as_deref(), Some("greeting highlighted"));
//! assert_eq!(paragraph.inner_html(), "Hello <b>world</b>!");
//! assert_eq!(paragraph.text_content().as_deref(), Some("Hello world!"));
//! ```
//!
//! `outer_html()` includes the node's own markup, while `inner_html()` serializes only its children.
//! `text_content()` concatenates descendant text without main-content filtering or block formatting. For readable output
//! with whitespace normalization and boilerplate filtering, use [HTML text extraction](#html-text-extraction).
//!
//! ## DOM Traversal
//!
//! `iter()` traverses a subtree in pre-order, including its root. `iter_elements()` visits only element nodes.
//! Use `child_nodes()` or `children()` to visit immediate children instead. Parent and sibling navigation is available
//! through `parent_node()`, `first_child()`, `last_child()`, `previous_sibling()`, and `next_sibling()`.
//!
//! ```
//! use resiliparse::parse::html::{dom::traits::*, tree::HtmlTree};
//!
//! let tree = HtmlTree::parse("<p>Hello <b>DOM</b></p>").unwrap();
//! let body = tree.body().unwrap();
//! let names = body.iter_elements()
//!     .map(|result| result.map(|element| element.local_name().unwrap()))
//!     .collect::<Result<Vec<_>, _>>()?;
//! assert_eq!(names, ["body", "p", "b"]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Traversal items are `Result`s. Mutating the document during lazy traversal invalidates the iterator; it returns
//! [`TraversalError::TreeModified`](parse::html::dom::iter::TraversalError::TreeModified) once and then stops.
//! Collect the nodes first if you need to mutate them in a traversal-driven operation.
//!
//! ## DOM Manipulation
//!
//! Create new nodes through the [`Document`](parse::html::dom::traits::Document) trait, then attach them to the document.
//! `append_child()`, `insert_before()`, `replace_child()`, and `remove_child()` operate on borrowed node views obtained
//! with `as_noderef()`. These operations return `None` when the requested operation is invalid.
//!
//! ```
//! use resiliparse::parse::html::{dom::traits::*, tree::HtmlTree};
//!
//! let tree = HtmlTree::parse("<main></main>").unwrap();
//! let document = tree.document().unwrap();
//! let main = tree.body().unwrap().query_selector("main").unwrap().unwrap();
//! let paragraph = document.create_element("p").unwrap();
//! let text = document.create_text_node("Hello Resiliparse!").unwrap();
//! paragraph.append_child(&text.as_noderef()).unwrap();
//! main.append_child(&paragraph.as_noderef()).unwrap();
//! assert_eq!(main.inner_html(), "<p>Hello Resiliparse!</p>");
//!
//! // Replacing inner text escapes markup; replacing inner HTML parses new nodes.
//! paragraph.set_inner_text("<b>Text</b>");
//! assert_eq!(paragraph.inner_html(), "&lt;b&gt;Text&lt;/b&gt;");
//! paragraph.set_inner_html("<b>Markup</b>");
//! assert_eq!(paragraph.inner_html(), "<b>Markup</b>");
//! main.remove_child(&paragraph.as_noderef()).unwrap();
//! assert!(main.children().is_empty());
//! ```
//!
//! Node handles retain their document, and mutations use interior mutability, so these methods take shared references.
//! Removing a child detaches it without destroying it; `decompose()` destroys a node and its subtree, invalidating handles
//! to the destroyed nodes. Prefer detaching nodes if you intend to reinsert or inspect them later.
//!
//! # Language Detection
//!
//! [`detect_fast`](parse::lang::detect_fast) uses character n-gram profiles for 101 languages. It returns a vector of
//! [`LanguageMatch`](parse::lang::LanguageMatch) values, each containing a language code and integer out-of-place rank.
//! Lower ranks indicate closer matches, not probabilities. `cutoff` is the maximum acceptable rank (`1200` is a typical
//! starting point), and `n_results` limits how many matches are returned. An empty vector means no candidates met the
//! requested settings. Longer texts generally produce more reliable results than short snippets; NFC-normalized text
//! is recommended, and normalization is not applied automatically.
//!
//! ```
//! use resiliparse::parse::lang::{detect_fast, supported_langs};
//!
//! let text = "This is a synthetic English sentence about language detection.";
//! let matches = detect_fast(text, 1200, 3, Some(&["en", "de", "fr"]));
//! assert_eq!(matches[0].language, "en");
//! for candidate in matches {
//!     println!("{}: {}", candidate.language, candidate.rank);
//! }
//! assert!(supported_langs().contains(&"en"));
//! ```
//!
//! Use `langs=None` to compare all built-in profiles. Restricting the candidate languages reduces comparisons and may
//! improve precision when you know which languages can occur. An empty candidate slice permits no languages.
//!
//! [`language_vector`](parse::lang::language_vector) generates feature vectors, and
//! [`train_language_examples`](parse::lang::train_language_examples) averages examples into a custom profile.
//! These helpers do not replace or modify the built-in profiles:
//!
//! ```
//! use resiliparse::parse::lang::{language_vector, train_language_examples};
//!
//! let examples = ["This is an English training example.", "Another sentence about the web."];
//! let profile = train_language_examples(examples, 256).unwrap();
//! let vector = language_vector("A new sentence to compare with the profile.", 256).unwrap();
//! assert_eq!(profile.len(), vector.len());
//! ```
//!
//! # HTTP Tools
//!
//! FastWARC preserves HTTP payload bytes and does not automatically decode chunked transfer encoding. When the payload
//! is still chunked, [`read_http_chunk`](parse::http::read_http_chunk) accepts any `std::io::BufRead` positioned at the
//! first chunk-size line. It returns one chunk per call; an empty vector signals termination. I/O errors propagate.
//!
//! ```
//! use std::io::Cursor;
//! use resiliparse::parse::http::read_http_chunk;
//!
//! let mut reader = Cursor::new(b"c\r\nResiliparse \r\n6\r\nis an \r\n8\r\nawesome \r\n5\r\ntool.\r\n0\r\n\r\n");
//! let mut decoded = Vec::new();
//! loop {
//!     let chunk = read_http_chunk(&mut reader)?;
//!     if chunk.is_empty() {
//!         break;
//!     }
//!     decoded.extend(chunk);
//! }
//! assert_eq!(decoded, b"Resiliparse is an awesome tool.");
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! [`iterate_http_chunks`](parse::http::iterate_http_chunks) provides a lazy iterator over the same chunk reader:
//!
//! ```
//! use std::io::{self, Cursor};
//! use resiliparse::parse::http::iterate_http_chunks;
//!
//! let reader = Cursor::new(b"4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n");
//! let chunks = iterate_http_chunks(reader).collect::<io::Result<Vec<_>>>()?;
//! assert_eq!(chunks.concat(), b"Wikipedia");
//! # Ok::<(), io::Error>(())
//! ```
//!
//! These helpers preserve the Python helper's permissive framing behavior. They are not strict HTTP validators:
//! truncated chunks can return partial payloads, an invalid size parsed as zero ends iteration, and trailer headers are
//! not processed. They do not parse response headers or decompress content encodings such as gzip. Consuming chunks
//! incrementally avoids collecting the entire response, though each individual chunk is returned as an allocated vector.
//!
//! # HTML Text Extraction
//!
//! [`extract_plain_text`](extract::html2text::extract_plain_text) extracts visible body text from an already parsed
//! [`HtmlTree`](parse::html::tree::HtmlTree). It skips script/style elements and HTML comments, normalizes whitespace,
//! and can preserve basic block formatting. Extraction does not mutate the tree, so you can reuse a parsed document.
//!
//! ```
//! use resiliparse::{
//!     extract::html2text::{extract_plain_text, ExtractOptions},
//!     parse::html::tree::HtmlTree,
//! };
//!
//! let tree = HtmlTree::parse("<p>Hello <b>world</b>!</p><script>alert('ignored')</script>").unwrap();
//! let text = extract_plain_text(&tree, &ExtractOptions::default()).unwrap();
//! assert_eq!(text, "Hello world!");
//! ```
//!
//! ## Formatting and Content Options
//!
//! [`ExtractOptions`](extract::html2text::ExtractOptions) controls formatting and content selection. The default
//! [`Formatting::Basic`](extract::html2text::Formatting::Basic) preserves paragraphs, line breaks, lists, and preformatted
//! text. Use `Formatting::Off` to collapse block content into a compact string, or `Formatting::MinimalHtml` to retain
//! reduced markup for headings, paragraphs, lists, and links:
//!
//! ```
//! use resiliparse::{
//!     extract::html2text::{extract_plain_text, ExtractOptions, Formatting},
//!     parse::html::tree::HtmlTree,
//! };
//!
//! let tree = HtmlTree::parse("<h1>Title</h1><p>See <a href='/example'>this page</a>.</p>").unwrap();
//! let compact = ExtractOptions { formatting: Formatting::Off, ..Default::default() };
//! assert_eq!(extract_plain_text(&tree, &compact).unwrap(), "Title See this page.");
//!
//! let minimal = ExtractOptions {
//!     formatting: Formatting::MinimalHtml,
//!     links: true,
//!     ..Default::default()
//! };
//! let html = extract_plain_text(&tree, &minimal).unwrap();
//! assert!(html.contains("<h1>Title</h1>"));
//! assert!(html.contains("<a href=\"/example\">this page</a>"));
//! ```
//!
//! Other options control list bullets (`list_bullets`), alternative text (`alt_texts`), link targets (`links`),
//! form values (`form_fields`), and `<noscript>` content (`noscript`). Default options enable list bullets and alternative
//! text but disable link targets, form fields, and noscript content.
//!
//! ## Main Content Extraction
//!
//! Set `main_content=true` to apply heuristics for skipping navigation, sidebars, footers, advertising, and hidden elements.
//! You can also supply CSS selectors in `skip_elements` to exclude specific subtrees. Invalid selectors return a
//! [`CssParserError`](parse::html::selector::CssParserError).
//!
//! ```
//! use resiliparse::{
//!     extract::html2text::{extract_plain_text, ExtractOptions},
//!     parse::html::tree::HtmlTree,
//! };
//!
//! let tree = HtmlTree::parse("<nav>Menu</nav><main><p>Article text.</p></main><footer>Copyright</footer>").unwrap();
//! let options = ExtractOptions {
//!     main_content: true,
//!     skip_elements: vec!["nav".into(), "footer".into()],
//!     ..Default::default()
//! };
//! let text = extract_plain_text(&tree, &options).unwrap();
//! assert!(!text.contains("Menu"));
//! assert!(!text.contains("Copyright"));
//! ```
//!
//! `comments` controls comment sections, `post_meta` controls article metadata, and `hidden_elements` controls whether
//! class-name-based hidden-content filters are applied. Main-content detection is heuristic, and the extractor does not
//! run JavaScript or compute a browser layout. Reduced HTML output is intended for text presentation and is not a general
//! HTML sanitization API.

mod third_party;

pub mod extract;
pub mod parse;
