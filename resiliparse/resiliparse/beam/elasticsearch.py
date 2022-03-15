# Copyright 2021 Janek Bevendorff
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

import logging
import time

import apache_beam as beam
from apache_beam.utils.windowed_value import WindowedValue
import apache_beam.typehints as t
from elasticsearch import exceptions as es_exc, Elasticsearch
from elasticsearch.helpers import BulkIndexError, streaming_bulk


logger = logging.getLogger()


class ElasticsearchBulkIndex(beam.PTransform):
    """
    Bulk-index documents to Elasticsearch.

    Expects bulk index actions that can be passed to :func:`elasticsearch.helpers.streaming_bulk`.
    Helpers for creating such actions are given with :func:`index_action`, :func:`update_action`,
    and :func:`delete_action`.

    If the records are KV pairs, the key will be used as the document ``_id`` and the value is
    expected to be the document that is to be indexed as a dict. In this case, fields starting with
    underscores will be discarded and ``default_index`` must be set.

    Returns the document IDs of successfully indexed documents.

    :param es_args: Elasticsearch client arguments
    :param default_index: default index name if none is given in the index action
    :param update: do a bulk UPDATE instead of a bulk INDEX (requires documents to exist)
    :param parallelism: reshuffle to achieve the desired level of parallelism
    :param buffer_size: internal buffer size
    :param chunk_size: indexing chunk size
    :param max_retries: maximum number of retries on recoverable failures
    :param initial_backoff: initial retry backoff
    :param max_backoff: maximum retry backoff
    :param request_timeout: Elasticsearch request timeout
    :param ignore_400: ignore HTTP 400 errors (skip unindexable documents)
    :param dry_run: discard documents and do not actually index them
    """

    def __init__(self,
                 es_args: t.Dict[str, t.Any],
                 default_index: t.Optional[str] = None,
                 update: bool = False,
                 parallelism: int = None,
                 buffer_size: int = 3200,
                 chunk_size: int = 800,
                 max_retries: int = 10,
                 initial_backoff: float = 2,
                 max_backoff: float = 600,
                 request_timeout: int = 240,
                 ignore_400: bool = True,
                 dry_run: bool = False):

        super().__init__()
        self._pardo = _ElasticsearchBulkIndex(es_args=es_args,
                                              default_index=default_index,
                                              update=update,
                                              buffer_size=buffer_size,
                                              chunk_size=chunk_size,
                                              max_retries=max_retries,
                                              initial_backoff=initial_backoff,
                                              max_backoff=max_backoff,
                                              request_timeout=request_timeout,
                                              ignore_400=ignore_400,
                                              dry_run=dry_run)
        self._parallelism = parallelism

    def expand(self, pcoll):
        if self._parallelism:
            pcoll |= beam.Reshuffle(self._parallelism)
        return pcoll | beam.ParDo(self._pardo)


# noinspection PyAbstractClass
class _ElasticsearchBulkIndex(beam.DoFn):
    def __init__(self,
                 es_args,
                 default_index,
                 update,
                 buffer_size,
                 chunk_size,
                 max_retries,
                 initial_backoff,
                 max_backoff,
                 request_timeout,
                 ignore_400,
                 dry_run):
        super().__init__()

        self.buffer_size = buffer_size
        self.buffer = []

        self.es_args = es_args
        self.default_index = default_index
        self.update = update
        self.client = None
        self.bulk_args = dict(
            chunk_size=chunk_size,
            # We handle errors ourselves, so don't raise and don't let Elasticsearch retry
            # HTTP 429 errors, since that would mess with the result order.
            raise_on_exception=False,
            raise_on_error=False,
            max_retries=0,
            request_timeout=request_timeout,
            yield_ok=True
        )
        self.max_retries = max_retries
        self.initial_backoff = initial_backoff
        self.max_backoff = max_backoff
        self.ignore_400 = ignore_400
        self.dry_run = dry_run

    def setup(self):
        self.client = Elasticsearch(**self.es_args)

    def teardown(self):
        assert len(self.buffer) == 0
        if self.client:
            self.client.transport.close()

    # noinspection PyIncorrectDocstring
    def process(self,
                element: t.Union[t.KV[str, t.Dict[str, t.Any]], t.Dict[str, t.Any]],
                timestamp=beam.DoFn.TimestampParam,
                window=beam.DoFn.WindowParam):
        """
        Add element to index buffer.

        Yields the document IDs of successfully indexed document on buffer flush.

        :param element: input index action or KV pair containing the document ID and the document values.
        :return: iterable of indexed document IDs
        """

        if type(element) is tuple and len(element) == 2:
            if not self.default_index:
                raise RuntimeError('Got a KV pair, but default index is not set.')
            doc_id, element = element
            element = index_action(doc_id, self.default_index, element)

        if self.dry_run:
            yield element.get('_id')
            return

        self.buffer.append((element, timestamp, window))
        if len(self.buffer) >= self.buffer_size:
            yield from self._flush_buffer()

    def finish_bundle(self):
        if len(self.buffer) > 0:
            yield from self._flush_buffer(window_values=True)

    def _flush_buffer(self, window_values=False):
        if not self.buffer:
            return

        retry_count = 0
        errors = []
        self.buffer.sort(key=lambda x: x[0].get('_id', ''))

        try:
            while retry_count < self.max_retries + 1:
                try:
                    errors = []
                    to_retry = []

                    buf_gen = (e[0] for e in self.buffer)
                    for i, (ok, info) in enumerate(streaming_bulk(self.client, buf_gen, **self.bulk_args)):
                        info_val = list(info.values())[0]
                        if not ok:
                            errors.append(info)
                            if info_val.get('status') != 400 or not self.ignore_400:
                                # 400 errors are persistent, no point in retrying them
                                to_retry.append(self.buffer[i])
                            logger.error('Failed to index document (attempt %i/%i): %s',
                                         retry_count + 1, self.max_retries + 1, info)
                        else:
                            if window_values:
                                yield WindowedValue(info_val['_id'], self.buffer[i][1], [self.buffer[i][2]])
                            else:
                                yield info_val['_id']

                    if not to_retry:
                        return

                    self.buffer = to_retry

                except es_exc.TransportError as e:
                    logger.error('Elasticsearch transport error (attempt %i/%i): %s',
                                 retry_count + 1, self.max_retries + 1, e.message)
                    if retry_count >= self.max_retries:
                        raise e

                if retry_count >= self.max_retries:
                    break
                logger.error('Retrying with exponential backoff in %i seconds...',
                             self.initial_backoff * (2 ** retry_count))
                time.sleep(min(self.max_backoff, self.initial_backoff * (2 ** retry_count)))
                retry_count += 1
        finally:
            self.buffer.clear()

        raise BulkIndexError("%i document(s) failed to index." % len(errors), errors)


def index_action(doc_id: str, index: t.Optional[str], doc: t.Dict[str, t.Any]):
    """
    Build an index bulk action.

    :param doc_id: document ID (``None`` for using autogenerated IDs)
    :param index: index name
    :param doc: document as dict
    :return: index action dict
    """
    d = {
        '_op_type': 'index',
        '_index': index,
        '_id': doc_id,
        **{k: v for k, v in doc.items() if not k.startswith('_')}
    }
    if d['_id'] is None:
        del d['_id']
    return d


def update_action(doc_id: str, index: str, partial_doc: t.Dict[str, t.Any]):
    """
    Build an update bulk action.

    :param doc_id: document ID
    :param index: index name
    :param partial_doc: partial update document as dict
    :return: index action dict
    """
    return {
        '_op_type': 'update',
        '_index': index,
        '_id': doc_id,
        'doc': {k: v for k, v in partial_doc.items() if not k.startswith('_')}
    }


def delete_action(doc_id: str, index: str):
    """
    Build a delete bulk action.

    :param doc_id: document ID
    :param index: index name
    :return: index action dict
    """
    return {
        '_op_type': 'delete',
        '_index': index,
        '_id': doc_id
    }


def ensure_index(client: Elasticsearch,
                 index_name: str,
                 index_settings: t.Dict[str, str] = None,
                 mapping: t.Dict[str, str] = None):     # pragma: no cover
    """
    Helper function to ensure an index exists.

    If the index does not exist, it will be created with the given mapping and settings.

    :param client: Elasticsearch client instance
    :param index_name: index name
    :param index_settings: index settings dict
    :param mapping: index mapping dict
    """
    index_settings = index_settings or {}
    mapping = mapping or {}

    if not client.indices.exists(index=index_name):
        client.indices.create(index=index_name, body=dict(
            settings=index_settings,
            mappings=mapping
        ))
