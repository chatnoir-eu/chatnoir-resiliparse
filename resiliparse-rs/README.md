# ChatNoir Resiliparse

A collection of robust and fast processing tools for parsing and analyzing web archive data, written in Rust with
bindings for Python. Resiliparse provides HTML parsing and DOM manipulation, text extraction, character encoding
detection, HTTP parsing, and language detection.

Resiliparse is a part of the [ChatNoir web analytics toolkit](https://github.com/chatnoir-eu/).

## Building Resiliparse

You need Rust and Cargo, a C/C++ compiler, CMake, and libclang to build Resiliparse. Install the Rust toolchain with
[rustup](https://rustup.rs/) if necessary:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

From a checkout of this repository, install [vcpkg](https://vcpkg.io/) and add its executable to your `PATH`:

```bash
git clone https://github.com/chatnoir-eu/chatnoir-resiliparse.git
cd chatnoir-resiliparse
git clone https://github.com/microsoft/vcpkg.git
./vcpkg/bootstrap-vcpkg.sh
export PATH="$PWD/vcpkg:$PATH"
```

Build the crate from the repository root:

```bash
cargo build -p resiliparse
```

The crate includes its native build manifests and overlays. The build script installs native dependencies under Cargo's
build output directory through vcpkg and links them statically. Set `RESILIPARSE_VCPKG_PATH` to
use an existing vcpkg installation directory containing the platform triplet directories. Alternatively, set
`RESILIPARSE_USE_SYSTEM_LIBS=1` to use system headers and shared libraries. Those libraries must also be available at
runtime.

Set `RESILIPARSE_STATIC_LIBS=1` as well to link system libraries statically instead. Their static archives must be
available to rustc; use `RUSTFLAGS="-L native=/path/to/lib"` for non-standard library directories. The C++ runtime remains
dynamically linked on Linux and macOS. The Docker image enables static system libraries by default.

## Using Resiliparse as a Library

Add Resiliparse to your project's `Cargo.toml`:

```toml
[dependencies]
resiliparse = "*" # Specify the correct version here
```

Then build your project with `cargo build`. The native build prerequisites described above also apply when using
Resiliparse as a dependency.

### Building Python Bindings From Source

The Python bindings use PyO3 and are built with maturin. Pip installs the build backend automatically. From the
repository root, after setting up the native build prerequisites:

```bash
python3 -m venv venv && source venv/bin/activate

# Build and install both packages in editable mode for development
python3 -m pip install -e ./fastwarc-py -e ./resiliparse-py

# Or build wheels and install from the resulting directory
python3 -m pip wheel -w build ./fastwarc-py ./resiliparse-py
python3 -m pip install --no-index --find-links build resiliparse
```

## Usage Instructions

For the Rust API, see the [crate documentation](https://docs.rs/resiliparse/). For Python usage, see the
[Resiliparse User Manual](https://resiliparse.chatnoir.eu/en/latest/).

### Updating docs.rs Bindings

Since docs.rs cannot download vcpkg dependencies to generate FFI bindings, we need to ship a set of pre-generated
bindings. The following script can be used to re-generate the bindings:

```bash
python3 ./tools/regenerate-docs-bindings.py
```

The pre-generated bindings are used only for compiling the docs. Actual crate builds will still generate the bindings
directly from the native headers.

## Cite Us

If you use ChatNoir or Resiliparse, please consider citing
our [ECIR 2018 demo paper](https://webis.de/downloads/publications/papers/bevendorff_2018.pdf):

```bibtex
@InProceedings{bevendorff:2018,
  address =             {Berlin Heidelberg New York},
  author =              {Janek Bevendorff and Benno Stein and Matthias Hagen and Martin Potthast},
  booktitle =           {Advances in Information Retrieval. 40th European Conference on IR Research (ECIR 2018)},
  editor =              {Leif Azzopardi and Allan Hanbury and Gabriella Pasi and Benjamin Piwowarski},
  month =               mar,
  publisher =           {Springer},
  series =              {Lecture Notes in Computer Science},
  site =                {Grenoble, France},
  title =               {{Elastic ChatNoir: Search Engine for the ClueWeb and the Common Crawl}},
  year =                2018
}
```

If you use FastWARC, you can also cite our [OSSYM 2021 abstract paper](https://arxiv.org/abs/2112.03103):

```bibtex
@InProceedings{bevendorff:2021,
  author =                {Janek Bevendorff and Martin Potthast and Benno Stein},
  booktitle =             {3rd International Symposium on Open Search Technology (OSSYM 2021)},
  editor =                {Andreas Wagner and Christian Guetl and Michael Granitzer and Stefan Voigt},
  month =                 oct,
  publisher =             {International Open Search Symposium},
  site =                  {CERN, Geneva, Switzerland},
  title =                 {{FastWARC: Optimizing Large-Scale Web Archive Analytics}},
  year =                  2021
}
```
