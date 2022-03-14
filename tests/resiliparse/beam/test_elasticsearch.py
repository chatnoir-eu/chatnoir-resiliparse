import json
import pytest
import os
from unittest.mock import patch

import apache_beam as beam
from apache_beam.testing.test_pipeline import TestPipeline
from apache_beam.testing.util import assert_that, equal_to
from elasticsearch.helpers import BulkIndexError
from resiliparse.beam import elasticsearch as es


DATA_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', 'data'))


MOCK_RETURN_CODE = 200


def mock_bulk(body, **_):
    items = []
    for line in body.strip().split('\n'):
        doc = json.loads(line)
        key = list(doc.keys())[0]
        val = doc[key]
        if not type(val) is dict or not val.get('_index'):
            # Payload line
            continue
        items.append({
            key: {
                '_index': val['_index'],
                '_id': val['_id'],
                'version': 1,
                'status': MOCK_RETURN_CODE
            }
        })
    return {
        'took': 100,
        'errors': False,
        'items': items
    }


@patch('elasticsearch.Elasticsearch')
def mock_client(es):
    es.bulk.side_effect = mock_bulk
    es.transport.serializer.dumps.side_effect = json.dumps
    return es


def es_setup(self):
    self.client = mock_client()


# Use mock client
es._ElasticsearchBulkIndex.setup = es_setup


INDEX_DOCS = [
    es.index_action('doc1', 'index_name', {'field': 'value'}),
    es.index_action('doc2', 'index_name', {'field': 'value'}),
    es.update_action('doc1', 'index_name', {'field': 'value'}),
    es.delete_action('doc2', 'index_name'),
]
EXPECTED_INDEX_ACTIONS = [
    {'_op_type': 'index', '_id': 'doc1', '_index': 'index_name', 'field': 'value'},
    {'_op_type': 'index', '_id': 'doc2', '_index': 'index_name', 'field': 'value'},
    {'_op_type': 'update', '_id': 'doc1', '_index': 'index_name', 'doc': {'field': 'value'}},
    {'_op_type': 'delete', '_id': 'doc2', '_index': 'index_name'}
]


def bulk_index_with_args(docs, **args):
    with TestPipeline() as pipeline:
        ids = (pipeline
               | beam.Create(docs)
               | es.ElasticsearchBulkIndex({}, **args))

    assert_that(ids, equal_to([d['_id'] for d in docs]))


def test_bulk_actions():
    for d, a in zip(INDEX_DOCS, EXPECTED_INDEX_ACTIONS):
        assert d == a

    bulk_index_with_args(INDEX_DOCS)

    # Smaller buffer
    bulk_index_with_args(INDEX_DOCS, buffer_size=2)

    # With reshuffle
    bulk_index_with_args(INDEX_DOCS, parallelism=2)

    # Dry run
    bulk_index_with_args(INDEX_DOCS, dry_run=True)


def test_bulk_index_with_error():
    # With client error
    global MOCK_RETURN_CODE
    MOCK_RETURN_CODE = 400

    # Raise
    with pytest.raises(BulkIndexError):
        with TestPipeline() as pipeline:
            _ = (pipeline
                 | beam.Create(INDEX_DOCS)
                 | es.ElasticsearchBulkIndex({}, ignore_400=False, max_retries=1))

    # Ignore
    with TestPipeline() as pipeline:
        ids = (pipeline
               | beam.Create(INDEX_DOCS)
               | es.ElasticsearchBulkIndex({}, ignore_400=True, max_retries=1))

    assert_that(ids, equal_to([]))

    # Retry non-client error
    MOCK_RETURN_CODE = 500
    with pytest.raises(BulkIndexError):
        with TestPipeline() as pipeline:
            ids = (pipeline
                   | beam.Create(INDEX_DOCS)
                   | es.ElasticsearchBulkIndex({}, ignore_400=True, max_retries=2, initial_backoff=0.01))

    assert_that(ids, equal_to([]))

    MOCK_RETURN_CODE = 200


def test_bulk_kv_pairs():
        docs = [
            ('doc1', {'field': 'value'}),
            ('doc2', {'field': 'value'})
        ]

        # Default index not set
        with pytest.raises(RuntimeError):
            with TestPipeline() as pipeline:
                _ = (pipeline
                     | beam.Create(docs)
                     | es.ElasticsearchBulkIndex({}))

        with TestPipeline() as pipeline:
            ids = (pipeline
                   | beam.Create(docs)
                   | es.ElasticsearchBulkIndex({}, default_index='index_name'))
        assert_that(ids, equal_to([d[0] for d in docs]))
