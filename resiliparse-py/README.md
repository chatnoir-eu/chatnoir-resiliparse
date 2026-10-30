# Resiliparse (Python)

A collection of robust and fast processing tools for parsing and analyzing web archive data written in Rust with
bindings for Python.

This package contains the Python bindings for Resiliparse, including HTML parsing and DOM manipulation, text extraction,
encoding and language detection, HTTP parsing, process guards, and Apache Beam utilities.

Resiliparse is a part of the [ChatNoir web analytics toolkit](https://github.com/chatnoir-eu/).

## Installing Resiliparse

Pre-built Resiliparse binaries for Python can be installed from [PyPI](https://pypi.org/project/Resiliparse/):

```bash
pip install resiliparse
```

## Building Python Bindings From Source

You need the Rust toolchain, a C/C++ compiler, CMake, and libclang. The bindings use PyO3 and are built with maturin;
pip installs the build backend automatically. To build directly from the GitHub repository:

```bash
# Clone repository
git clone https://github.com/chatnoir-eu/chatnoir-resiliparse.git
cd chatnoir-resiliparse

# Install Rust toolchain (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install vcpkg (if not already available on PATH)
git clone https://github.com/microsoft/vcpkg.git
./vcpkg/bootstrap-vcpkg.sh
export PATH="$PWD/vcpkg:$PATH"
```

Resiliparse's build script installs Lexbor and uchardet through vcpkg when they are missing.

From the repository root, build and install the Python packages. Building FastWARC alongside Resiliparse ensures the
matching dependency is available locally:

```bash
# Create a fresh venv first (recommended)
python3 -m venv venv && source venv/bin/activate

# Option 1: Build and install in editable mode (best for development)
python3 -m pip install -e ./fastwarc-py -e ./resiliparse-py

# Option 2 (alternative): Build and install wheels in separate steps (best for redistribution)
python3 -m pip wheel -w build ./fastwarc-py ./resiliparse-py
python3 -m pip install --no-index --find-links build resiliparse
```

To use an existing vcpkg installation, set `RESILIPARSE_VCPKG_PATH` to the directory containing its platform triplet
directories:

```bash
export RESILIPARSE_VCPKG_PATH="$(pwd)/vcpkg_installed"
```

Native vcpkg builds link libraries statically. Linux manylinux wheel builds use libraries from the build image and
bundle them into the wheel. Installed wheels therefore do not need the build-time `vcpkg_installed` directory.
Set `RESILIPARSE_USE_SYSTEM_LIBS=1` to skip vcpkg and use system headers and shared libraries. Those libraries must be
available at runtime.

## Usage Instructions

For detailed usage instructions, please consult
the [Resiliparse User Manual](https://resiliparse.chatnoir.eu/en/latest/).

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
