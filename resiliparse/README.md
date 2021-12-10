# ChatNoir Resiliparse

A collection of robust and fast processing tools for parsing and analyzing (not only) web archive data.

Resiliparse is a part of the [ChatNoir web analytics toolkit](https://github.com/chatnoir-eu/).

## Installing Resiliparse
Pre-built Resiliparse binaries can be installed from PyPi:
```bash
pip install resiliparse
```

## Building Resiliparse From Source
You can compile Resiliparse either from the PyPi source package or directly from this repository, though in any case, you need to install all required build-time dependencies first. On Ubuntu, this is done as follows:
```bash
# Add Lexbor repository
curl -L https://lexbor.com/keys/lexbor_signing.key | sudo apt-key add -
echo "deb https://packages.lexbor.com/ubuntu/ $(lsb_release -sc) liblexbor" | \
    sudo tee /etc/apt/sources.list.d/lexbor.list

# Install build dependencies
sudo apt update
sudo apt install build-essential python3-dev libuchardet-dev liblexbor-dev libre2-dev
```
To build and install Resiliparse from PyPi, run
```bash
pip install --no-binary resiliparse resiliparse
```
That's it. If you prefer to build and install directly from this repository instead, run:
```bash
pip install -e resiliparse
```
To build the wheels without installing them, run:
```bash
pip wheel -e resiliparse

# Or:
pip install build && python -m build --wheel resiliparse
```

## Usage Instructions
For detailed usage instructions, please consult the [Resiliparse User Manual](https://resiliparse.chatnoir.eu/en/latest/).

## Cite Us

If you use ChatNoir or Resiliparse, please consider citing our [ECIR 2018 demo paper](https://webis.de/downloads/publications/papers/bevendorff_2018.pdf):

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
