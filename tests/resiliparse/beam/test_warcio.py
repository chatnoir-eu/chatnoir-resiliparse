import datetime
import os

import apache_beam as beam
from apache_beam.io.filesystem import FileMetadata
from apache_beam.io.aws.clients.s3 import boto3_client, messages
from apache_beam.testing.test_pipeline import TestPipeline
from apache_beam.testing.util import assert_that, equal_to
from resiliparse.beam import warcio


DATA_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', 'data'))
FILE_PATH = os.path.join(DATA_DIR, 'warcfile.warc')
FILE_META = FileMetadata(FILE_PATH, os.path.getsize(FILE_PATH))


def test_readwarcs():
    # Test iteration
    with TestPipeline() as pipeline:
        count = (pipeline
                 | warcio.ReadWarcs(os.path.join(DATA_DIR, 'warcfile.warc'))
                 | beam.combiners.Count.Globally())
        assert_that(count, equal_to([50]))

    # With shuffle
    with TestPipeline() as pipeline:
        count = (pipeline
                 | warcio.ReadWarcs(os.path.join(DATA_DIR, 'warcfile.warc'), shuffle_files=True)
                 | beam.combiners.Count.Globally())
        assert_that(count, equal_to([50]))


def test_readallwarcs():
    # Test iteration
    with TestPipeline() as pipeline:
        count = (pipeline
                 | beam.Create([FILE_META])
                 | warcio.ReadAllWarcs()
                 | beam.combiners.Count.Globally())
        assert_that(count, equal_to([50]))


def test_max_content_length():
    def assert_content_length(e):
        assert e.content_length <= 500

    # Test Content-Length filtering
    with TestPipeline() as pipeline:
        count = (pipeline
                 | warcio.ReadWarcs(
                    os.path.join(DATA_DIR, 'warcfile.warc'),
                    with_filename=False,
                    warc_args=dict(max_content_length=500)
                 )
                 | beam.Map(assert_content_length)
                 | beam.combiners.Count.Globally())

        assert_that(count, equal_to([33]))

    def assert_content_length_keep_meta(e):
        if e.content_length > 500:
            assert not e.reader.read()

    # Test Content-Length filtering (always keep meta)
    with TestPipeline() as pipeline:
        count = (pipeline
                 | warcio.ReadWarcs(
                    os.path.join(DATA_DIR, 'warcfile.warc'),
                    with_filename=False,
                    always_keep_meta=True,
                    warc_args=dict(max_content_length=500)
                 )
                 | beam.Map(assert_content_length_keep_meta)
                 | beam.combiners.Count.Globally())

        assert_that(count, equal_to([50]))


class _StubBoto3Client:    # pragma: no cover
    """Stub Boto3 client."""
    def __init__(self, *args, **kwargs):
        pass

    def get_object_metadata(self, request):
        return messages.Item(
                etag=request.object,
                key=request.object,
                last_modified=datetime.datetime.fromtimestamp(os.path.getmtime(FILE_PATH)),
                size=os.path.getsize(FILE_PATH))

    def list(self, request):
        item = self.get_object_metadata(messages.ListRequest(request.bucket, os.path.basename(FILE_PATH)))
        return messages.ListResponse([item], None)

    def get_range(self, _, start, end):
        with open(FILE_PATH, 'rb') as f:
            f.seek(start)
            return f.read(end - start)


boto3_client.Client = _StubBoto3Client
warcio._Boto3Client = _StubBoto3Client


def test_s3_read():
    with TestPipeline() as pipeline:
        count = (pipeline
                 | warcio.ReadWarcs('s3://test/warcfile.warc')
                 | beam.combiners.Count.Globally())
        assert_that(count, equal_to([50]))
