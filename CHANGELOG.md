# Changelog

## 2.0.1 (2026-11-04)

### Resiliparse

- Fix a crash in Lexbor triggered by empty attribute values.

## 2.0.0 (2026-10-31)

- Switch source builds to Maturin.
- FastWARC: Remove the legacy Cython implementations and compatibility shims.

### Resiliparse

- Expose the full processing API in Rust, with Python bindings to the same core.
- Add Rust text extraction, encoding and language detection, HTTP utilities, iterator helpers, and process guards.
- Process guards remain unavailable on Windows.
- Preserve spaces between adjacent table cells during text extraction.
- Fix invisible-element exclusion and boilerplate filtering.
- Fix an unwanted newline when serialising without `noscript`.
- Fix language-profile CLI commands treating unrelated files as language directories.
- Rust: Rename the HTML `css` module to `selectors`.
- Rust: Standardise type names to PascalCase, for example `HTMLTree` to `HtmlTree`.
- Python: Replace `DOMNode` and `DOMCollection` with typed nodes and collections, such as `ElementNodeList`.
- Python: Return tuples for collection slices.
- Python: Make `matches(selector)` test elements themselves rather than their descendants.
- Python: Use `query_selector(selector) is not None` for the previous descendant-search behaviour.

## 1.1.0 (2026-10-26)

- Add Python 3.15 wheels.

### Resiliparse

- Fix a memory-guard startup race.
- Rust: Add reusable, precompiled CSS selectors.
- Rust: Distinguish snapshot `NodeList` collections from live `LiveCollection` views.
- Rust: Fix DOM identity checks, traversal during mutation, node movement, and insertion cycles.
- Rust: Fix Unicode character indexing.
- Rust: Separate the `node_value` and `text_content` setters.

### FastWARC

- Fix parsing ClueWeb22 `warcinfo` records and detaching HTTP payload decoders during in-place iteration.
- Migrate the CLI to the Rust-backed API.

### FastWARC-gRPC

- Initial release of a streaming gRPC server for (remote) WARC parsing, record filtering, and payload streaming.

## 1.0.9 (2026-07-20)

### FastWARC

- Fix slicing errors when accessing empty or whitespace-only header values.
- Python: Improve conversion of headers to dictionaries and tuples.

## 1.0.8 (2026-06-15)

### FastWARC

- Improve record filtering performance, including native dispatch for built-in Python filter predicates.
- Rust: Add the `WarcHeader` enum for standard header names.
- Rust: Require `WarcRead` implementations to be `Send`.
- Python: Fix installation from source distributions and builds from the source tree.

## 1.0.7 (2026-06-11)

### FastWARC

- Preserve original header bytes when writing unmodified records, allowing byte-identical serialisation.
- Expand the Rust and Python guides for writing WARC files and calculating checksums.
- Rust: Make `WarcRecord::init_headers` take a `WarcRecordType` directly instead of an `Option`.
- Rust: Change the `Unknown` record-type flag from 512 to 32768.

## 1.0.6 (2026-06-10)

### FastWARC

- Use a default buffer size of 1 MiB when iterating local files.
- Add a Rust usage guide and document freezing records in Python.
- Restore missing method and property documentation.
- Rust: Move `SharedWarcRecord` to the `warc::record` module.

## 1.0.5 (2026-06-09)

### FastWARC

- Python: Emit deprecation warnings when legacy functionality is used rather than imported.

## 1.0.4 (2026-06-08)

### FastWARC

- Improve header parsing and payload-skipping performance, with copy-on-write parsed headers.
- Add a configurable maximum WARC/HTTP header length with a default of 32 KiB.
- Support LF-only headers through quirks mode.
- Fix header parsing when the end-of-header marker crosses a buffer boundary.
- Fix record offsets in incorrectly compressed archives.
- Rust: Add an in-place archive iterator that reuses the current record for faster processing.
- Python: Deprecate legacy stream shims and API aliases.
- Python: Fix `init_headers(record_urn=...)` adding a duplicate `urn:` prefix.

## 1.0.3 (2026-05-30)

### FastWARC

- Improve reading and seeking with larger buffers, cached positions, and buffered forward seeks.
- Improve Gzip reading performance.
- Rust: Simplify using files, buffered readers, and cursors as WARC input.
- Rust: Add file conversions for writer adapters.
- Python: Fix forwarding seeks to the underlying native stream.

## 1.0.2 (2026-05-28)

### FastWARC

- Fix a severe performance regression caused by repeatedly wrapping stream readers.
- Python: Require Python 3.10 or newer.
- Python: Add `WarcRecord.from_bytes` and `WarcRecord.from_reader` constructors.
- Python: Add equality comparisons for frozen records.
- Python: Fix pickling deadlocks and preserve the payload reader position when pickling frozen records.
- Python: Fix writer file-opening modes and expose Zstd compression-level selection.
- Python: Include S3 support when installing the `fsspec` extra and fix handling of `fsspec_args=False`.

## 1.0.1 (2026-05-26)

### FastWARC

- Fix compression auto-detection for Zstd archives with embedded dictionaries.
- Reset the Zstd decoder correctly when supplying a dictionary.
- Avoid panics when constructing readers over failing input streams.
- Python: Fix top-level API exports and improve type stubs.

## 1.0.0 (2026-05-24)

### FastWARC

- Replace the default Cython backend with a Rust implementation and PyO3 Python bindings.
- Keep the previous implementation under `fastwarc.legacy` for compatibility.
- Add Zstd-compressed WARC support, including embedded dictionaries and dictionary training.
- Support HTTP transfer/content decoding in the Rust API, including chunked transfer encoding and Brotli compression.
- Python: Add thread-safe iteration and composable record filters to the Rust-backed API.
- Python: Support freezing and pickling records with the Rust backend.

## 0.16.0 (2026-04-02)

- Restore module-level enum exports when building with Cython 3.1.

### Resiliparse

- Introduce an experimental Rust HTML5 parsing and DOM API backed by Lexbor.
- Support CSS selection, DOM mutation, traversal, and serialisation in Rust.

### FastWARC

- Add optional `fsspec` support for remote archives, including S3.
- Accept paths directly in `ArchiveIterator` and stream constructors.

## 0.15.2 (2025-03-27)

- Restore installation of the `fastwarc` and `resiliparse` CLI commands.
- Remove the deprecated `distutils` dependency from source builds.

### Resiliparse

- Add the `post_meta` extraction option to control whether article and blog metadata is retained.
- Add `hidden_elements` to retain content hidden by class names during main-content extraction.
- Improve boilerplate filtering and omit empty tags in minimal HTML output.

## 0.15.1 (2024-12-05)

- Fix macOS cross-compilation when building wheels.

## 0.15.0 (2024-12-04)

### Resiliparse

- Add minimal HTML output with `preserve_formatting='minimal_html'`.
- Retain headings, paragraphs, lists, preformatted text, line breaks, and optional links in HTML output.

## 0.14.9 (2024-08-13)

- Include `py.typed` markers and correct missing or inaccurate type annotations.

## 0.14.8 (2024-08-09)

### FastWARC

- Add Python type stubs for the public API.

## 0.14.7 (2024-04-29)

### FastWARC

- Improve buffered reading performance and reduce allocation overhead.

## 0.14.6 (2024-04-04)

- Add Linux ARM64 wheels for Python 3.10 and newer.
- Update native dependencies and Cython compatibility for source builds.

## 0.14.5 (2023-08-11)

- Update source builds for newer Cython and RE2 versions.

## 0.14.4 (2023-08-08)

### Resiliparse

- Fix iterator helpers raising `RuntimeError` instead of ending normally when an input iterator is exhausted.

### FastWARC

- Handle missing `Content-Length` headers in ClueWeb22 archives.

## 0.14.3 (2023-03-22)

### Resiliparse

- Fix a memory leak when using CSS selectors.

## 0.14.2 (2023-03-07)

- Migrate package metadata and build configuration to `pyproject.toml`.

## 0.14.1 (2023-03-06)

- Fix Apple Silicon builds and compilation with the updated Lexbor API.

## 0.14.0 (2023-03-06)

- Support Python 3.11 and provide Apple Silicon wheels.

### Resiliparse

- Allow plain-text extraction directly from HTML strings.
- Add `EncodingDetector.reset()` and increase the default encoding-detection sample size from 4 KiB to 128 KiB.
- Improve text extraction and DOM matching performance by reducing GIL locking and string conversions.

### FastWARC

- Expose stream methods directly to Python callers, including reading, writing, seeking, and retrieving buffer contents.
- Fix the CLI index command failing when determining the final record's length.

## 0.13.7 (2022-11-01)

### Resiliparse

- Improve plain-text extraction performance by reducing GIL locking.

## 0.13.6 (2022-11-01)

No user-facing changes.

## 0.13.5 (2022-09-15)

### FastWARC

- Add `WarcRecord.reason_phrase` and parse HTTP status codes even when the reason phrase is missing.

## 0.13.4 (2022-09-15)

### FastWARC

- Decode HTTP deflate content with or without a zlib header.
- Rename the `GZipStream` parameter `deflate` to `zlib`.
- Accept the non-standard `Content-Encoding: none` without a warning.

## 0.13.3 (2022-09-14)

### FastWARC

- Fix deflate decoding and ensure freezing a record refreshes its payload buffer.

## 0.13.2 (2022-09-14)

### FastWARC

- Fix `MemoryError` when pickling records after HTTP content decoding.

## 0.13.1 (2022-09-14)

### FastWARC

- Allow HTTP transfer decoding and content decoding to be enabled independently.

## 0.13.0 (2022-09-14)

### FastWARC

- Add automatic HTTP content and transfer decoding for Gzip, deflate, and Brotli payloads.

## 0.12.2 (2022-06-10)

### FastWARC

- Add `WarcRecord.http_last_modified` for the HTTP `Last-Modified` header.

## 0.12.1 (2022-05-04)

### FastWARC

- Fix `ArchiveIterator.min_content_length` filtering.

## 0.12.0 (2022-03-18)

### Resiliparse

- Make iterator helpers available on Windows.
- Move `progress_loop` from `resiliparse.itertools` to `resiliparse.process_guard`, retaining a deprecated alias.

## 0.11.2 (2022-03-16)

### Resiliparse

- Add the `cli-benchmark` extra and report missing optional CLI dependencies explicitly.

### FastWARC

- Support hexadecimal WARC digests alongside Base32 digests.

## 0.11.1 (2022-03-16)

### Resiliparse

- Fix invalid memory access when extracting link URLs as plain text.

## 0.11.0 (2022-03-15)

- Add the `all` extra to install all optional dependencies.

### Resiliparse

- Add Apache Beam utilities for reading WARC and text files, file matching, and Elasticsearch integration.

### FastWARC

- Add `WarcRecord.http_date` for the HTTP `Date` header and improve charset parsing.

## 0.10.6 (2022-03-11)

### FastWARC

- Add `WarcRecord.record_date` for the `WARC-Date` header.

## 0.10.5 (2022-02-28)

### Resiliparse

- Update Lexbor to fix a use-after-free bug in HTML parsing.

## 0.10.4 (2022-01-27)

### FastWARC

- Add `strict_mode=False` to allow LF-only line endings in WARC and HTTP headers.

## 0.10.3 (2022-01-27)

### FastWARC

- Accept additional WARC 0.x and 1.x version strings.

## 0.10.2 (2021-12-17)

### FastWARC

- Fix record stream positions when reading from an initial offset or after compression auto-detection.

## 0.10.1 (2021-12-16)

### Resiliparse

- Fix extraction incorrectly restricting the content root when `main_content=False`.
- Improve whitespace-collapsing performance and update Lexbor to fix HTML parser crashes.

## 0.10.0 (2021-12-09)

### Resiliparse

- Improve main-content extraction precision and speed.
- Improve recognition of article containers and WordPress boilerplate.
- Improve text rendering and whitespace handling.
