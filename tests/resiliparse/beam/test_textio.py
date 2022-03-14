import gzip
import os

import apache_beam as beam
from apache_beam.testing.test_pipeline import TestPipeline
from apache_beam.testing.util import assert_that, equal_to
from resiliparse.beam import textio


DATA_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', 'data'))


def test_read_lines():
    with open(os.path.join(DATA_DIR, 'warcfile.warc'), 'rb') as f:
        num_lines = len(f.readlines())

    with TestPipeline() as pipeline:
        count = (pipeline
                 | textio.ReadFromText(os.path.join(DATA_DIR, 'warcfile.warc'))
                 | beam.combiners.Count.Globally())
        assert_that(count, equal_to([num_lines]))

    # Smaller splits
    with TestPipeline() as pipeline:
        count = (pipeline
                 | textio.ReadFromText(os.path.join(DATA_DIR, 'warcfile.warc'),
                                       desired_split_size=64, min_split_size=16)
                 | beam.combiners.Count.Globally())
        assert_that(count, equal_to([num_lines]))


def test_read_lines_compressed():
    with gzip.GzipFile(os.path.join(DATA_DIR, 'warcfile.warc.gz'), 'rb') as f:
        num_lines = len(f.readlines())

    with TestPipeline() as pipeline:
        count = (pipeline
                 | textio.ReadFromText(os.path.join(DATA_DIR, 'warcfile.warc.gz'))
                 | beam.combiners.Count.Globally())

        assert_that(count, equal_to([num_lines]))

    # Smaller splits (should not do anything if file is compressed)
    with TestPipeline() as pipeline:
        count = (pipeline
                 | textio.ReadFromText(os.path.join(DATA_DIR, 'warcfile.warc.gz'),
                                       desired_split_size=64, min_split_size=16)
                 | beam.combiners.Count.Globally())
        assert_that(count, equal_to([num_lines]))
