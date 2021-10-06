.. _fastwarc-cli:

FastWARC CLI
============

Besides the :ref:`Python API <fastwarc-manual>`, FastWARC also provides a command line interface with the ``fastwarc`` command:

.. code-block:: bash

  $ fastwarc --help
  Usage: fastwarc [OPTIONS] COMMAND [ARGS]...

    FastWARC Command Line Interface.

  Options:
    -h, --help  Show this message and exit.

  Commands:
    benchmark   Benchmark FastWARC performance.
    check       Verify WARC consistency by checking all digests.
    extract     Extract WARC record by offset.
    index       Index WARC records as CDXJ.
    recompress  Recompress a WARC file with different settings.

Check Digests
-------------
You can verify all block and payload digests in the given WARC file and print a summary of all corrupted and (optionally) all intact records with

.. code-block:: bash

  fastwarc check INFILE

The command will exit with a non-zero exit code if at least one record fails verification.

Run ``fastwarc check --help`` for a full help listing.

Recompress WARC
---------------
If your WARC is uncompressed or not compressed properly at the record-level or you want to recompress a GZip WARC as LZ4 or vice versa, you can do that with

.. code-block:: bash

  fastwarc recompress INFILE OUTFILE


Run ``fastwarc recompress --help`` for a full help listing.

Extract Records by Offset
-------------------------
You can extract individual records at a given byte offset with either just headers, payload, or both:

.. code-block:: bash

  fastwarc extract [--headers] [--payload] [--output] INFILE OFFSET

Run ``fastwarc extract --help`` for a full help listing.

Index Records as CDXJ
---------------------
WARC files can be indexed to the `CDXJ <https://github.com/webrecorder/cdxj-indexer>`_ format with a configurable set of fields:

.. code-block::

  fastwarc index [--fields FIELDS] [--preserve-multi-header] [--output] [INFILES]...

Run ``fastwarc index --help`` for a full help listing.

Benchmark FastWARC vs. WARCIO
-----------------------------
The FastWARC CLI comes with a benchmarking tool that allows you to test record decompression and parsing speeds and compare them with WARCIO. Depending on your CPU, your storage speed, and the used compression algorithm, you can typically expect speedups between 1.3x and 6.5x over WARCIO.

Here are example runs on five `Common Crawl <https://commoncrawl.org/>`_ WARCs (run on an AMD Ryzen Threadripper 2920X with NVMe SSD):

.. code-block:: bash

  # Uncompressed WARC
  $ fastwarc benchmark read CC-MAIN-*.warc --bench-warcio
  Benchmarking read performance from 5 input path(s)...
  FastWARC: 630,245 records read in 5.81 seconds (108,487.93 records/s).
  WARCIO:   630,245 records read in 37.19 seconds (16,945.51 records/s).
  Time difference: -31.38 seconds, speedup: 6.40

  # GZip WARC
  $ fastwarc benchmark read CC-MAIN-*.warc.gz --bench-warcio
  Benchmarking read performance from 5 input path(s)...
  FastWARC: 630,245 records read in 60.52 seconds (10,413.38 records/s).
  WARCIO:   630,245 records read in 97.56 seconds (6,460.06 records/s).
  Time difference: -37.04 seconds, speedup: 1.61

  # LZ4 WARC (direct comparison not possible, since WARCIO does not support LZ4)
  $ fastwarc benchmark read CC-MAIN-*.warc.lz4
  Benchmarking read performance from 5 input path(s)...
  FastWARC: 630,245 records read in 12.65 seconds (49,825.44 records/s).

The benchmarking tool has additional options, such as reading WARCs directly from a remote S3 data source using `Boto3 <https://boto3.amazonaws.com/v1/documentation/api/latest/index.html>`_. Run ``fastwarc benchmark --help`` for more information.
