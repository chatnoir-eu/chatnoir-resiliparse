# ChatNoir Resiliparse

[![Build Wheels](https://github.com/chatnoir-eu/chatnoir-resiliparse/actions/workflows/build-wheels.yml/badge.svg)](https://github.com/chatnoir-eu/chatnoir-resiliparse/actions/workflows/build-wheels.yml)
[![Codecov](https://codecov.io/gh/chatnoir-eu/chatnoir-resiliparse/branch/develop/graph/badge.svg?token=VA51APYHU5)](https://codecov.io/gh/chatnoir-eu/chatnoir-resiliparse)
[![Documentation Status](https://readthedocs.org/projects/chatnoir-resiliparse/badge/?version=latest)](https://resiliparse.chatnoir.eu/en/latest/?badge=latest)

A collection of robust and fast processing tools for parsing and analyzing web archive data.

## Usage Instructions
For detailed information about the build process, dependencies, APIs, or usage instructions, please read the [Resiliparse Documentation](https://resiliparse.chatnoir.eu/en/latest/index.html)

## Resiliparse Module Summary
The Resiliparse collection encompasses the following two modules at the moment:

### 1. Resiliparse
The Resiliparse main module with the following subcomponents:

#### Parsing Utilities
The Resiliparse Parsing Utilities are highly optimized tools for dealing with encodings, detecting content types of raw protocol payloads, parsing HTML web pages, performing language detection, and more.

Main documentation: [Resiliparse Parsing Utilities](https://resiliparse.chatnoir.eu/en/latest/man/parse.html)

#### Extraction Utilities
The Resiliparse Extraction Utilities are a set of performance-optimized and highly efficient tools for extracting structural or semantic information from noisy raw web data for further processing, such as (main) content extraction / boilerplate removal, schema extraction, general web data cleansing, and more.

Main documentation: [Resiliparse Extraction Utilities](https://resiliparse.chatnoir.eu/en/latest/man/extract.html)

#### Process Guards
The Resiliparse Process Guard module is a set of decorators and context managers for guarding a processing context to stay within pre-defined limits for execution time and memory usage. Process Guards help to ensure the (partially) successful completion of batch processing jobs in which individual tasks may time out or use abnormal amounts of memory, but in which the success of the whole job is not threatened by (a few) individual failures. A guarded processing context will be interrupted upon exceeding its resource limits so that the task can be skipped or rescheduled.

Main documentation: [Resiliparse Process Guards](https://resiliparse.chatnoir.eu/en/latest/man/process-guard.html)

#### Itertools
Resiliparse Itertools are a collection of convenient and robust helper functions for iterating over data from unreliable sources using other tools from the Resiliparse toolkit.

Main documentation: [Resiliparse Itertools](https://resiliparse.chatnoir.eu/en/latest/man/itertools.html)

### 2. FastWARC
FastWARC is a high-performance WARC parsing library for Python written in C++/Cython. The API is inspired in large parts by [WARCIO](https://github.com/webrecorder/warcio), but does not aim at being a drop-in replacement.  FastWARC supports compressed and uncompressed WARC/1.0 and WARC/1.1 streams. Supported compression algorithms are GZip and LZ4.

Main documentation: [FastWARC](https://resiliparse.chatnoir.eu/en/latest/man/fastwarc.html) and [FastWARC CLI](https://resiliparse.chatnoir.eu/en/latest/man/fastwarc-cli.html)

## Installation
The main Resiliparse package can be installed from PyPi as follows:
```bash
pip install resiliparse
```
FastWARC is being distributed as its own package and can be installed like so:
```bash
pip install fastwarc
```
## Building From Source
To build Resiliparse and FastWARC from sources, you need to install all required build-time dependencies listed in `vcpkg.json`. It's possible to install them globally via your package manager, but the easiest and most consistent way is to use [vcpkg](https://vcpkg.io/en/):
```bash
# Install vcpkg itself (skip if you have a working vcpkg installation already)
git clone https://github.com/Microsoft/vcpkg
./vcpkg/bootstrap-vcpkg.sh

# Install dependencies to vcpkg_installed (must be run from sources root)
./vcpkg/vcpkg install --triplet=x64-linux
```
Replace the triplet value with one suitable for your platform. Valid values are: `x64-windows`, `x64-osx`, `arm64-osx`, `aarch64-linux` (or any of the vcpkg default triplets).

After installing the dependencies, you can build the actual Python packages:
```bash
# Create a fresh venv first (recommended)
python3 -m venv venv && source venv/bin/activate

# Option 1: Build and install in editable mode (best for development)
python3 -m pip install -e ./fastwarc ./resiliparse

# Option 2 (alternative): Build and install wheels in separate steps (best for redistribution)
python3 -m pip wheel -w build ./fastwarc ./resiliparse
ls ./build/*.whl | xargs python3 -m pip install
```
In most cases, the build routine should be smart enough to detect the location of the installed vcpkg dependencies. However, in some cases you may be getting errors about missing header files or undefined symbols. This can happen if you don't build from the source repository, use Python's new `build` module, or run `pip wheel` with `--isolated`. To work around that, set the `RESILIPARSE_VCPKG_PATH` environment variable to the absolute path of the vcpkg installation directory:
```bash
export RESILIPARSE_VCPKG_PATH="$(pwd)/vcpkg_installed"
```

**NOTE:** Unless you fix up the wheels to embed the linked shared libraries (via [auditwheel](https://github.com/pypa/auditwheel) on Linux, [delocate-wheel](https://github.com/matthew-brett/delocate) on macOS, or [delvewheel](https://github.com/adang1345/delvewheel) on Windows), you will have to add the vcpkg library directory (`vcpkg_installed/TRIPLET/lib`) to your library search path to use them. On Linux, add the directory path to the `LD_LIBRARY_PATH` environment variable, on macOS to `DYLD_LIBRARY_PATH`. On Windows, you have to add the directory to the `Path` environment variable.

Here's an example of how to use `auditwheel` on Linux to fix up the build wheels:
```bash
LD_LIBRARY_PATH=$(pwd)/vcpkg_installed/x64-linux/lib \
  auditwheel repair --plat linux_x86_64 build/Resiliparse*.whl build/FastWARC*.whl
```
(Please note that `linux_x86_64` platform wheels are [not suitable for general redistribution](https://packaging.python.org/en/latest/specifications/platform-compatibility-tags/#platform-tag).)


## Cite Us

Resiliparse is part of the [ChatNoir](https://chatnoir.eu/) web analytics toolkit. If you use ChatNoir or any of its tools for a publication, you can make us happy by citing our [ECIR 2018 demo paper](https://webis.de/downloads/publications/papers/bevendorff_2018.pdf):
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
