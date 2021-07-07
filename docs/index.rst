.. _resiliparse-index:

++++++++++++++++++++++++++++++++++++++++++++
ChatNoir Resiliparse |release| Documentation
++++++++++++++++++++++++++++++++++++++++++++

ChatNoir Resiliparse is a collection of robust and fast processing tools for parsing and analyzing web archive data. Resiliparse is part of the `ChatNoir web analytics toolkit <https://github.com/chatnoir-eu/>`_.


Table of Contents
=================

This is the full table of contents for this documentation:

.. toctree::
   :maxdepth: 4

   man/index
   api/index

Indices and Tables
------------------

* :ref:`genindex`
* :ref:`modindex`
* :ref:`search`


Resiliparse Module Summary
==========================

As of version |release|, the Resiliparse collection encompasses the following two modules:

1. Resiliparse
--------------
The Resiliparse main module with the following subcomponents:

Process Guard
^^^^^^^^^^^^^
The Resiliparse Process Guard module is a set of decorators and context managers for guarding a processing context to stay within pre-defined limits for execution time and memory usage. Process Guards help to ensure the (partially) successful completion of batch processing jobs in which individual tasks may time out or use abnormal amounts of memory, but in which the success of the whole job is not threatened by (a few) individual failures. A guarded processing context will be interrupted upon exceeding its resource limits so that the task can be skipped or rescheduled.

For more information, see :ref:`process-guard-manual`.

Itertools
^^^^^^^^^
Resiliparse Itertools are a collection of convenient and robust helper functions for iterating over data from unreliable sources using other tools from the Resiliparse toolkit.

For more information, see :ref:`itertools-manual`.

2. FastWARC
-----------
FastWARC is a high-performance WARC parsing library for Python written in C++/Cython. The API is inspired in large parts by WARCIO, but does not aim at being a drop-in replacement. FastWARC supports compressed and uncompressed WARC/1.0 and WARC/1.1 streams. Supported compression algorithms are GZip and LZ4.

FastWARC provides both a Python API and a command line interface (CLI).

For more information, see: :ref:`fastwarc-manual` and :ref:`fastwarc-cli`.


About ChatNoir
==============

`ChatNoir <https://chatnoir.eu>`_ is a web search engine developed developed by the `Webis Group <https://webis.de>`_ based on the `ClueWeb09 <https://lemurproject.org/clueweb09/>`_, `ClueWeb12 <https://lemurproject.org/clueweb12/>`_, and `Common Crawl <https://commoncrawl.org/>`_ datasets with the goal to make large-scale web retrieval research accessible to the wider research community.

If you use ChatNoir or any of its tools for a publication, you can make us happy by citing our ECIR demo paper:

.. code-block:: bibtex

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
