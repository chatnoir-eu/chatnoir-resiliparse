# ChatNoir Resiliparse

A collection of robust and fast processing tools for parsing and analyzing web archive data written in Rust with
bindings for Python.

Resiliparse is a part of the [ChatNoir web analytics toolkit](https://github.com/chatnoir-eu/).

## Installing Resiliparse

Pre-built Resiliparse binaries can be installed from PyPi:

```bash
pip install resiliparse
```

## Building Resiliparse From Source

To build Resiliparse from sources, you need to install all required build-time dependencies listed in
`vcpkg.json`. It's possible to install them globally via your package manager, but the easiest and most consistent way
is to use [vcpkg](https://vcpkg.io/en/):

```bash
# Install vcpkg itself (skip if you have a working vcpkg installation already)
git clone https://github.com/Microsoft/vcpkg
./vcpkg/bootstrap-vcpkg.sh

# Install dependencies to vcpkg_installed (must be run from sources root)
./vcpkg/vcpkg install --triplet=x64-linux
```

Replace the triplet value with one suitable for your platform. Valid values are: `x64-windows`, `x64-osx`, `arm64-osx`,
`arm64-linux` (or any of the vcpkg default triplets).

By default, Resiliparse uses `vcpkg` to build and install native dependencies. You can set
`RESILIPARSE_USE_SYSTEM_LIBS=1` to skip `vcpkg` and use dependencies already present on your system.

After installing the dependencies, you can build the actual Python packages:

```bash
# Create a fresh venv first (recommended)
python3 -m venv venv && source venv/bin/activate

# Option 1: Build and install in editable mode (best for development)
python3 -m pip install -e ./resiliparse-py

# Option 2 (alternative): Build and install wheels in separate steps (best for redistribution)
python3 -m pip wheel -w build ./resiliparse-py
ls ./build/*.whl | xargs python3 -m pip install
```

In most cases, the build routine should be smart enough to detect the location of the installed vcpkg dependencies.
However, in some cases you may be getting errors about missing header files or undefined symbols. This can happen if you
don't build from the source repository, use Python's new `build` module, or run `pip wheel` with `--isolated`. To work
around that, set the `RESILIPARSE_VCPKG_PATH` environment variable to the absolute path of the vcpkg installation
directory:

```bash
export RESILIPARSE_VCPKG_PATH="$(pwd)/vcpkg_installed"
```

Native vcpkg builds link libraries statically. Linux manylinux wheel builds use libraries from the build image and
bundle them into the wheel. Installed wheels therefore do not need the build-time `vcpkg_installed` directory.
Custom builds using `RESILIPARSE_USE_SYSTEM_LIBS=1` must provide those libraries at runtime.

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
