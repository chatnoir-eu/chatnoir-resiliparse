# FastWARC

FastWARC is a high-performance WARC parsing library for Python written in C++/Cython. The API is inspired in large parts by [WARCIO](https://github.com/webrecorder/warcio), but does not aim at being a drop-in replacement.  FastWARC supports compressed and uncompressed WARC/1.0 and WARC/1.1 streams. Supported compression algorithms are GZip and LZ4.

FastWARC belongs to the [ChatNoir Resiliparse toolkit](https://github.com/chatnoir-eu/chatnoir-resiliparse/) for fast and robust web data processing.

## Installing FastWARC
Pre-built FastWARC binaries for most Linux platforms can be installed from PyPi:
```bash
pip install fastwarc
```
**However:** the Linux binaries are provided *solely for your convenience*. Since they are built on the very old `manylinux` base system for better compatibility, their performance isn't optimal (though still better than WARCIO). For best performance, see the next section on how to build FastWARC yourself.

## Building FastWARC From Source
You can compile FastWARC either from the PyPi source package or directly from this repository, though in any case, you need to install all required build-time dependencies first. On Ubuntu, this is done as follows:
```bash
sudo apt install build-essential python3-dev zlib1g-dev liblz4-dev
```
To build and install FastWARC from PyPi, run
```bash
pip install --no-binary fastwarc fastwarc
```
That's it. If you prefer to build and install directly from this repository instead, run:
```bash
pip install -e fastwarc
```
To build the wheels without installing them, run:
```bash
pip wheel -e fastwarc

# Or:
pip install build && python -m build --wheel fastwarc
```

## Usage Instructions
For detailed usage instructions, please consult the [FastWARC User Manual](https://resiliparse.chatnoir.eu/en/latest/man/fastwarc.html).

## Cite Us
If you use FastWARC, please consider citing our [OSSYM 2021 abstract paper](https://arxiv.org/abs/2112.03103):
```bibtex
@InProceedings{bevendorff:2021,
  author =                {Janek Bevendorff and Martin Potthast and Benno Stein},
  booktitle =             {3nd International Symposium on Open Search Technology (OSSYM 2021)},
  editor =                {Andreas Wagner and Christian Guetl and Michael Granitzer and Stefan Voigt},
  month =                 oct,
  publisher =             {International Open Search Symposium},
  site =                  {CERN, Geneva, Switzerland},
  title =                 {{FastWARC: Optimizing Large-Scale Web Archive Analytics}},
  year =                  2021
}
```
