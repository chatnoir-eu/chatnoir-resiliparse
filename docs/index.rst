.. _resiliparse-docs-index:

++++++++++++++++++++++++++++++++++++++++++++
ChatNoir Resiliparse |release| Documentation
++++++++++++++++++++++++++++++++++++++++++++

ChatNoir Resiliparse is a collection of robust and fast processing tools for parsing and analyzing web archive data,
written in Rust with bindings for Python. Resiliparse is part of the
`ChatNoir web analytics toolkit <https://github.com/chatnoir-eu/>`__.


Table of Contents
=================

This is the full table of contents for this documentation:

.. toctree::
   :maxdepth: 1
   :caption: User Manual (Python)

   man/installation
   man/parse
   man/extract
   man/process-guard
   man/itertools
   man/beam
   man/fastwarc


.. toctree::
   :maxdepth: 1
   :caption: User Manual (Rust)

   Resiliparse <https://docs.rs/resiliparse/latest/resiliparse/>
   FastWARC <https://docs.rs/fastwarc/latest/fastwarc/>
   FastWARC-gRPC <https://docs.rs/fastwarc-grpc/latest/fastwarc_grpc/>


.. toctree::
   :maxdepth: 1
   :caption: API Documentation (Python)

   api/parse
   api/extract
   api/process-guard
   api/itertools
   api/beam
   api/fastwarc


.. toctree::
   :maxdepth: 1
   :caption: API Documentation (Rust)

   Resiliparse <https://docs.rs/resiliparse/latest/resiliparse/all.html>
   FastWARC <https://docs.rs/fastwarc/latest/fastwarc/all.html>
   FastWARC-gRPC <https://docs.rs/fastwarc-grpc/latest/fastwarc_grpc/all.html>


.. toctree::
   :maxdepth: 1
   :caption: CLI Documentation

   cli/resiliparse
   cli/fastwarc


Indices and Tables
------------------

* :ref:`genindex`
* :ref:`modindex`


Resiliparse Module Summary
==========================

As of version |release|, the Resiliparse collection encompasses the following two modules:

1. Resiliparse
--------------
The Resiliparse main module comes with the following subcomponents:

Parsing Utilities
^^^^^^^^^^^^^^^^^
The Resiliparse Parsing Utilities are the largest submodule and provide an extensive (and growing) collection of
efficient tools for dealing with encodings and raw protocol payloads, parsing HTML web pages, and preparing them for
further processing by extracting structural or semantic information.

Main documentation: :ref:`parse-manual`.

Extraction Utilities
^^^^^^^^^^^^^^^^^^^^
The Resiliparse Extraction Utilities extract structural and semantic information from noisy web data, including
main-content extraction, boilerplate removal, and schema extraction.

Main documentation: :ref:`extract-manual`.

Process Guard
^^^^^^^^^^^^^
The Resiliparse Process Guard module is a set of decorators and context managers for guarding a processing context to
stay within pre-defined limits for execution time and memory usage. Process Guards help to ensure the (partially)
successful completion of batch processing jobs in which individual tasks may time out or use abnormal amounts of memory,
but in which the success of the whole job is not threatened by (a few) individual failures. A guarded processing context
will be interrupted upon exceeding its resource limits so that the task can be skipped or rescheduled.

Main documentation: :ref:`process-guard-manual`.

Itertools
^^^^^^^^^
Resiliparse Itertools are a collection of convenient and robust helper functions for iterating over data from unreliable
sources using other tools from the Resiliparse toolkit.

Main documentation: :ref:`itertools-manual`.

2. FastWARC
-----------
FastWARC is a high-performance WARC parsing library written in Rust with bindings for Python. The API is inspired in
large parts by WARCIO, but does not aim at being a drop-in replacement. FastWARC supports compressed and uncompressed
WARC/1.0 and WARC/1.1 streams. Supported compression algorithms are Gzip, Zstandard, and LZ4.

FastWARC provides both a Python API and a command line interface (CLI).

Main documentation: :ref:`fastwarc-manual` and :ref:`fastwarc-cli`

FastWARC also provides a standalone gRPC server, allowing clients written in other languages to use it without native
bindings. See the `FastWARC-gRPC documentation <https://docs.rs/fastwarc-grpc/>`__ for server usage and client examples.


Installation (Rust)
===================

To use Resiliparse and FastWARC as libraries in your Rust project, add the packages you need to your ``Cargo.toml``:

.. code-block:: toml

    [dependencies]
    resiliparse = "2"
    fastwarc = "2"

Then build your project with ``cargo build``. Resiliparse requires native build dependencies; see
:ref:`resiliparse-installation` for the prerequisites and source-build instructions.


Installation (Python)
=====================

Pre-built wheels are available for Linux, macOS, and Windows on PyPI:

.. code-block:: bash

    pip install resiliparse
    pip install fastwarc

Building Bindings From Source
-----------------------------

Both libraries use Rust with PyO3 bindings, built with maturin. For build instructions and native dependencies, see
:ref:`resiliparse-installation` and :ref:`fastwarc-installation`.


About ChatNoir
==============

`ChatNoir <https://chatnoir.eu>`__ is a web search engine developed developed by the `Webis Group <https://webis.de>`__
based on the `ClueWeb09 <https://lemurproject.org/clueweb09/>`__, `ClueWeb12 <https://lemurproject.org/clueweb12/>`__,
and `Common Crawl <https://commoncrawl.org/>`__ datasets with the goal to make large-scale web retrieval research
accessible to the wider research community.

Cite Us
-------

If you use ChatNoir or any of its tools for a publication, you can make us happy by citing our `ECIR demo paper
<https://webis.de/downloads/publications/papers/bevendorff_2018.pdf>`__:

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


If you use FastWARC, you can also cite our `OSSYM 2021 abstract paper <https://arxiv.org/abs/2112.03103>`__:

.. code-block:: bibtex

    @InProceedings{bevendorff:2021c,
      author =                {Janek Bevendorff and Martin Potthast and Benno Stein},
      booktitle =             {3rd International Symposium on Open Search Technology (OSSYM 2021)},
      editor =                {Andreas Wagner and Christian Guetl and Michael Granitzer and Stefan Voigt},
      month =                 oct,
      publisher =             {International Open Search Symposium},
      site =                  {CERN, Geneva, Switzerland},
      title =                 {{FastWARC: Optimizing Large-Scale Web Archive Analytics}},
      year =                  2021
    }
