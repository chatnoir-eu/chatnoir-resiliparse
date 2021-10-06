.. _fastwarc-cli:

FastWARC CLI
============

Besides the :ref:`Python API <fastwarc-manual>`, FastWARC also provides a command line interface via the ``fastwarc`` command, which enables working with WARC files on the console.

Run ``fastwarc [COMMAND] --help`` for detailed help listings.

------------

Top-Level Commands
------------------

In the following is a short listing of the top-level commands:

.. click:: fastwarc.cli:main
   :prog: fastwarc
   :nested: short

------------

Full Command Listing
--------------------

Below is a full description of all available commands:


.. click:: fastwarc.cli:main
   :prog: fastwarc
   :nested: full
