# ChatNoir Resiliparse

A collection of robust and fast processing tools for parsing and analyzing web archive data.

Resiliparse is part of the [ChatNoir](https://chatnoir.eu/) toolkit. If you use ChatNoir or any of its tools for a publication, you can make us happy by citing our ECIR demo paper:
```bibtex
@InProceedings{bevendorff:2018,
  address =             {Berlin Heidelberg New York},
  author =              {Janek Bevendorff and Benno Stein and Matthias Hagen and Martin Potthast},
  booktitle =           {Advances in Information Retrieval. 40th European Conference on IR Research (ECIR 2018)},
  editor =              {Leif Azzopardi and Allan Hanbury and Gabriella Pasi and Benjamin Piwowarski},
  ids =                 {potthast:2018c,stein:2018c},
  month =               mar,
  publisher =           {Springer},
  series =              {Lecture Notes in Computer Science},
  site =                {Grenoble, France},
  title =               {{Elastic ChatNoir: Search Engine for the ClueWeb and the Common Crawl}},
  year =                2018
}
```

## Modules

The Resiliparse collection encompasses the following modules at the moment:

### Resiliparse ProcessGuard
The Resiliparse ProcessGuard module is a set of decorators and context managers for guarding a processing context to stay within pre-defined limits for execution time and memory usage. ProcessGuard helps to ensure the (partially) successful completion of batch processing jobs in which individual tasks may time out or use abnormal amounts of memory, but in which the success of the whole job is not threatened by (a few) individual failures. A guarded processing context will be interrupted upon exceeding its resource limits so that the task can be skipped or rescheduled.

For more information, see the [ProcessGuard documentation](resiliparse/README.md#ProcessGuard)


### FastWARC
FastWARC is a high-performance WARC parsing library for Python written in C++/Cython. The API is inspired in large parts by [WARCIO](https://github.com/webrecorder/warcio), but does not aim at being a drop-in replacement.  FastWARC supports compressed and uncompressed WARC/1.0 and WARC/1.1 streams. Supported compression algorithms are GZip and LZ4.

For more information, see the [FastWARC documentation](fastwarc/README.md)
