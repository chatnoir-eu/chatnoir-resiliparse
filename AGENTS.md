# Agent Instructions

## Project Scope

FastWARC and Resiliparse are high-performance data processing libraries for large amounts of web data.

- The libraries are available on Linux, macOS, and Windows. All functions must compile and work on all three platforms.
- If some functionality is platform-specific, the exception must be documented. The code and corresponding tests must
  then be platform-guarded.
- The projects' primary language is Rust, but all functionality is available in Python as well. The Rust and Python APIs
  must be feature-equivalent, but may deviate slightly in usage patterns to make them idiomatic in each language.
- All functions must have polynomial runtimes and bounded memory footprints, even when processing very large objects.
- Error conditions must be well-defined, no unrecoverable crashes or exceptions must happen due to malformed input.
- Processed inputs are considered untrusted, full of errors, and potentially malicious.
- Input streams are unreliable and can be interrupted or break off intermittently. Readers and parsers must handle that
  and must be able to recover or resume gracefully.
- When not provided directly by the processing function, resume, retry, or backoff mechanisms should be implemented
  as easy-to-use helpers. Users should not need to implement them on their own.
- Processing should happen close to the metal. External dependencies should be used only when actually needed, not for
  trivial functionality.

## Project Structure

Resiliparse is the main web processing library. FastWARC is part of the suite, but shipped as a standalone library.
FastWARC-gRPC builds on top of FastWARC, and it is also shipped separately.

### Source Tree

- `.github/workflows`: GitHub Actions workflows
- `.vcpkg`: vcpkg overlay ports and triplets
- `benchmarks`: benchmark suites (should not be used for temporary performance tests)
- `docs`: main project documentation
    - `docs/man`: general usage documentation (Python)
    - `docs/api`: Python API docs
- `fastwarc-grpc`: gRPC wrapper around FastWARC
- `fastwarc-py`: FastWARC PyO3 bindings
    - `fastwarc-py/python`: Python package and type stubs
    - `fastwarc-py/src`: Rust binding implementations
    - `fastwarc-py/tests`: Python interface tests
- `fastwarc-rs`: Main FastWARC Rust crate
    - `fastwarc-rs/tests`: Additional integration tests and fixtures
- `resiliparse-py`: Resiliparse PyO3 bindings
    - `resiliparse-py/python`: Python package and type stubs
    - `resiliparse-py/src`: Rust binding implementations
    - `resiliparse-py/tests`: Python interface tests
- `resiliparse-rs`: Main Resiliparse Rust crate
    - `resiliparse-rs/tests`: Additional integration tests and fixtures
- `test-data`: global test data used in multiple packages / crates

### Build Instructions

- The Rust projects are built with `cargo`. The Python bindings use PyO3 and are built with `maturin` via
  `pyproject.toml`.
- Native dependencies are installed via `vcpkg`. Rust installs its dependencies to the target dir with `vcpkg`. The
  `vcpkg` binary must be available on the `PATH`. If `RESILIPARSE_USE_SYSTEM_LIBS=1` is set, it uses libraries from the
  system instead.
- `vcpkg install` can install the native dependencies to a `vcpkg_installed` directory in the project root. Library
  search paths should include `vcpg_installed/{host-triplet}/lib` for runtime tests.

### Rust Module Structure

- Resiliparse and FastWARC are structured into submodules that group functionality semantically.
- Each submodule lives in a module directory with a `mod.rs`. The `mod.rs` file contains no actual code, only imports
  and documentation.
- The actual functionality is implemented in separate modules next to the `mod.rs` file. Tests for that module do not go
  into the same file. They go into another file called `modulename_tests.rs`, which is included at the end of the module
  file like so:
  ```
  // ===========================================================
  // Tests
  // ===========================================================
  
  #[cfg(test)]
  #[path = "modulename_test.rs"]
  mod modulename_test;
  ```
- Additional integration tests and fixtures / assets go into the `tests` directory next to `src`.

### Python Bindings Structure

- The Python bindings are mixed native and Python modules. The structure resembles the Rust project, but since the
  modules are mostly just PyO3 shims, the structure may be collapsed where it makes sense.
- Certain non-performance-critical Python-only functionality (such as the CLI or helpers for other Python libraries
  like Apache Beam) can be implemented in pure Python.
- There are no Rust tests in the bindings directory. The Python bindings are tested via `pytest`.
- The `pytest` test files are in the `tests` subdirectory of the respective bindings root. The file names must be
  globally unique to allow running `pytest` from the repository root.
- All Python modules must have complete `.pyi` stub files which must pass a `mypy.stubtest` run from within the
  respective bindings root with `--mypy-config-file pyproject.toml` and `--allowlist=stubtest-allowlist.txt`.
- The Python module import behaviour should be similar to real Python modules. I.e., `import resiliparse.foo.bar` must
  be possible without importing `resiliparse` and `resiliparse.foo` first. This is achieved by registering the modules
  in `lib.rs` and in the `#[pymodule_init]` functions of the respective submodules.
- Python source and stub files must be able to discover their native counterparts, so `__path__` must be set. In
  Resiliparse, `set_package_path` from `lib.rs` can be used for that.
- The top-level Resiliparse modules must be loaded lazily via `native_package_loader!(...)` from an `__init__.py`.
- PyO3 modules contain one top-level `#[pymodule]` (either inline or in `mod.rs`). The Rust name of the `#[pymodule]` is
  the same as the containing Rust module, but starting with an `_` underscore (e.g., `crate::foo::_foo`).
- The actual member implementations are on the Rust module level and the Python export plumbing is done via
  `#[pymodule_export] pub use super::member_name` directives in the `#[pymodule]`.
- Submodules are registered in the  `#[pymodule]`'s `#[pymodule_init]`.

## Tests

### Running Tests

- Rust tests can be run with `cargo test --workspace` for testing everything or `cargo test --package PKGNAME` for
  individual crates.
- Python tests can be run with `pytest` from the root or with `pytest subdirectory` for individual packages.
- Running Python tests needs a venv. Unless specified otherwise in your harness, try to use an existing venv, which is
  probably located at `.venv` or `venv` in the project root.
- Builds using vcpkg link its libraries statically. System-library builds may need native library paths at runtime.
- Run the whole test suite and stubtest before committing anything.

## Writing Tests

- All code paths must have test coverage.
- Functionality tests should be done in Rust tests.
- Keep tests short and focussed. Every test must have a purpose.
- Test names should be to the point, but not verbose or overly specific.
- Python tests must cover the full call patch from Python to the core Rust library. These tests must be complete, but
  actual functionality tests must be added only for Python-specific behaviour.
- Add short comments inside the tests where the purpose of the tested condition isn't obvious. Try not to overexplain.

## Coding Style

- Sources files must start with an Apache-2.0 license header.
- Rust source files must be formatted with `rustfmt`.
- Names with all-caps abbreviations use pure `PascalCase` in Rust, but `CAPSCase` in Python.
- PyO3 binding structs and enums have the suffix `Py` and set their Python-visible names (without `Py`) via
  `#[pyclass(name = "...", module = "...")]`. The `module` path must be present and complete.
- PyO3 modules use an `_` underscore at the beginning of the name instead of the `Py` suffix and set their
  Python-visible names via `#[pymodule(name = "...", module = "...")]`.
- Lines in Markdown and ReST files should wrap after 120 characters.
- RustDoc comment lines should also not exceed 120 characters.
- Code examples within RustDoc should use sensible formatting to avoid horizontal scrolling on the rendered HTML page.

## Documentation

- General usage documentation is maintained in `docs/man` as Sphinx documentation.
- The Sphinx docs can be built using `make html` with dependencies from `docs/requirements.txt`.
- The documentation in `docs/man` should give a good overview of and introduction to the library with a focus on the
  Python interface.
- An equivalent Rust-specific manual is maintained as large module-level RustDoc blocks in the `lib.rs` files of the
  respective crates. The `docs.rs` links to these manuals are linked in the Sphinx doc sidebar.
- Python API documentation is included via `autodoc` in `docs/api`. New Python modules must be registered there.
- All public members must be documented with informative (but not verbose) RustDoc comments. The minimal structure is:
  ```
  /// Short one-liner summary.
  ///
  /// More detailed multi-line description here.
  ///
  /// # Arguments
  ///
  /// * `param1` - Short description
  /// * `param2` - Short description
  ///
  /// # Returns
  /// 
  /// Short return value description.
  ```
- Types are inferred from the signature and should not be mentioned in the comment. RustDoc doc test code snippets can
  be added where it makes sense.
- Avoid implementation-specific notes in the doc blocks unless they are of interested to the user. Such remarks should
  be made as comments inside the code (try not to over comment, however).
- Private members should be documented as well, but the description can be brief and arguments and return values do not
  need to be documented.
- PyO3 exports must have equivalent doc strings, but using ReST syntax:
  ```
  /// Short one-liner summary.
  ///
  /// More detailed multi-line description here.
  ///
  /// :param param1: Short description
  /// :param param2: Short description
  /// :return: Short return value description.
  ```
  Python-specific behaviour should be documented, but no references to the Rust implementation should be made.
