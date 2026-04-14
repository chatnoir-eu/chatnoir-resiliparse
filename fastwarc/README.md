# FastWARC

**WORK IN PROGRESS!** This crate is not production-ready yet. It is only here to reserve the name already.
Please use the Python version for now.

FastWARC is a high-performance WARC parsing library for Python written in C++/Cython. The API is inspired in large parts
by [WARCIO](https://github.com/webrecorder/warcio), but does not aim at being a drop-in replacement. FastWARC supports
compressed and uncompressed WARC/1.0 and WARC/1.1 streams. Supported compression algorithms are GZip and LZ4.

FastWARC belongs to the [ChatNoir Resiliparse toolkit](https://github.com/chatnoir-eu/chatnoir-resiliparse/) for fast
and robust web data processing.

## Usage Instructions

For detailed usage instructions, please consult
the [FastWARC User Manual](https://resiliparse.chatnoir.eu/en/latest/man/fastwarc.html).

## Cite Us

If you use FastWARC, please consider citing our [OSSYM 2021 abstract paper](https://arxiv.org/abs/2112.03103):

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
