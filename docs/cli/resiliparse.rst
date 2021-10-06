.. _resiliparse-cli:

Resiliparse CLI
===============

The ``resiliparse`` command line utility provides tools for maintaining and benchmarking Resiliparse. At the moment, these tools are aimed primarily at developers of Resiliparse. General-purpose tools geared towards users of the library may be added later.

Run ``resiliparse [COMMAND] --help`` for detailed help listings.

------------

Top-Level Commands
------------------

In the following is a short listing of the top-level commands:

.. click:: resiliparse.cli:main
   :prog: resiliparse
   :nested: short

------------

Full Command Listing
--------------------

Below is a full description of all available commands:

.. click:: resiliparse.cli:main
   :prog: resiliparse
   :nested: full
