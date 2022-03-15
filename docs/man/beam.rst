.. _beam-manual:

Resiliparse Beam Transforms
===========================

Resiliparse offers a variety of :class:`~apache_beam.transforms.PTransform` utility classes for processing web archive data with `Apache Beam <https://beam.apache.org/>`__. The transform library provides input formats for processing WARC files, bulk-indexing data to Elasticsearch, as well as a few convenience transforms that work around shortcomings in certain Beam runners.

Installing Resiliparse Beam Transforms
--------------------------------------

The Resiliparse Beam transforms come bundled with Resiliparse by default, but in order to install the required dependencies (``apache_beam``, ``boto3``, etc.), you need to install Resiliparse with the ``beam`` extra (or alternatively: ``all``):

.. code-block:: console

    pip install 'resiliparse[beam]'

Reading WARC Files
------------------

Reading WARC files in Apache Beam is made easy with :class:`~resiliparse.beam.warcio.ReadWarcs` and :class:`~resiliparse.beam.warcio.ReadAllWarcs`, which accept either a file glob or a  :class:`~apache_beam.pvalue.PCollection` of file metadata objects:

.. code-block:: python

    import apache_beam as beam
    from resiliparse.beam.fileio import MatchFiles
    from resiliparse.beam.warcio import ReadWarcs, ReadAllWarcs

    # Count WARC records from local file system
    with beam.Pipeline() as pipeline:
        record_count = (pipeline
            | ReadWarcs('/path/to/warcs/*.warc.gz')
            | beam.combiners.Count.Globally())

    # Count WARC records from S3 storage (requires a proper S3 client config)
    with beam.Pipeline() as pipeline:
        record_count = (pipeline
            | ReadWarcs('s3://mybucket/warcs/*.warc.gz')
            | beam.combiners.Count.Globally())

    # Count WARC records from pre-matched file names
    with beam.Pipeline() as pipeline:
        record_count = (pipeline
            | MatchFiles('/path/to/warc/*.warc.gz')
            | ReadAllWarcs()
            | beam.combiners.Count.Globally())

The WARC files are read with FastWARC. If you want to customise the WARC iterator, you can pass an additional dict of parameters to ``warc_args``, which will be used for instantiating the :class:`~fastwarc.warc.ArchiveIterator`.


.. note::

    :class:`resiliparse.beam.fileio.MatchFiles` is a replacement for Beam's own :class:`~apache_beam.io.fileio.MatchFiles` that forces a fusion break by adding a ``Reshuffle`` transform. This is meant primarily for fixing a major shortcoming of the FlinkRunner, which does not support distributing splits on its own at the moment. If you don't need (or don't want) to reshuffle the matched file names (e.g., because you use the DataflowRunner), either set the ``shuffle`` parameter to ``False`` or use the original Beam implementation.


Reading Text Files
------------------

For reading text files line by line, Resiliparse offers :class:`~resiliparse.beam.textio.ReadFromText` and :class:`~resiliparse.beam.textio.ReadAllFromText`. The two transform behave similar to Beam's own implementations :class:`apache_beam.io.textio.ReadFromText` and :class:`apache_beam.io.textio.ReadAllFromText`, but enforce fusion breaks for ensuring that splittable text files are processed in parallel (similar to what :class:`~resiliparse.beam.fileio.MatchFiles` does). If a text file is uncompressed, it will be split into chunks of up to ``desired_split_size`` bytes. The splits are reshuffled and then processes in parallel. This is to avoid input bottlenecks on Beam runners that do not (yet) support runner-initiated splits, such as the FlinkRunner.

.. code-block:: python

    import apache_beam as beam
    from resiliparse.beam.fileio import MatchFiles
    from resiliparse.beam.textio import ReadFromText, ReadAllFromText

    # Count lines from text files
    with beam.Pipeline() as pipeline:
        line_count = (pipeline
            | ReadFromText(
                '/path/to/text/files/*.txt',
                 shuffle_splits=True  # shuffle textfile splits (if splittable)
                 shuffle_names=True   # shuffle matched filenames (use if you have lots of files)
                 desired_split_size=64*1024*1024  # desired split size in bytes (default: 64 MB)
                 min_split_size=1024*1024         # minimum split size in bytes (default: 1 MB)
              )
            | beam.combiners.Count.Globally())

    # Count lines from pre-matched file names
    with beam.Pipeline() as pipeline:
        line_count = (pipeline
            | MatchFiles('/path/to/text/files/*.txt')
            | ReadAllFromText(shuffle_splits=True)      # No need to set shuffle_names here
            | beam.combiners.Count.Globally())

.. note::

    :class:`~resiliparse.beam.textio.ReadFromText` and :class:`~resiliparse.beam.textio.ReadAllFromText` support only ``\n``-separated files at the moment, but can be used for reading lines with binary content as well if ``coder`` is set to ``None``. By default ``coder`` is set to :class:`~resiliparse.beam.coders.StrUtf8Coder`, which uses Resiliparse's :class:`~resiliparse.parse.encoding.bytes_to_str` for decoding lines to handle broken encodings better (the default Beam implementation would raise an exception here).


Bulk-indexing to Elasticsearch
------------------------------

Resiliparse provides a transform for bulk-indexing documents to an Elasticsearch cluster. It expects either `Elasticsearch bulk index actions <https://www.elastic.co/guide/en/elasticsearch/reference/current/docs-bulk.html>`__ (also see `Bulk helpers <https://elasticsearch-py.readthedocs.io/en/latest/helpers.html>`__ for more information) or key/value pairs of document IDs and document dicts. The former can also be used for upserts and deletes, the latter only for indexing new documents.

.. code-block:: python

    from resiliparse.beam import elasticsearch as es

    # Arguments for creating an Elasticsearch Python client
    es_client_args = {hosts=['localhost:9200'], use_ssl=True}

    # Index documents to Elasticsearch cluster
    # (returns the IDs of successfully index documents)
    with beam.Pipeline() as pipeline:
        _ = (pipeline
             | beam.Create([
                   # index_action() is a helper for creating a valid index action from
                   # a document ID, an index name, and a document dict.
                   es.index_action('doc1', 'index_name', {'field': 'value'}),
                   es.index_action('doc2', 'index_name', {'field': 'value'}),
                   es.index_action('doc3', 'index_name', {'field': 'value'}),
                   es.index_action(None, 'index_name', {'field': 'value'}),   # Auto ID
                   es.delete_action('doc1', 'index_name'),                    # Delete
               ])
             | es.ElasticsearchBulkIndex(es_client_args))


    # You can also pass KV pairs (requires you to define a default index)
    with beam.Pipeline() as pipeline:
        _ = (pipeline
             | beam.Create([
                   ('doc1', {'field': 'value'}),
                   ('doc2', {'field': 'value'}),
                   ('doc3', {'field': 'value'}),
               ])
             | es.ElasticsearchBulkIndex(es_client_args, default_index='index_name'))
