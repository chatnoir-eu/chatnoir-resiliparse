import os
import pytest

import apache_beam as beam
from apache_beam.testing.test_pipeline import TestPipeline
from apache_beam.testing.util import assert_that, equal_to
from resiliparse.beam import fileio


DATA_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', 'data'))


@pytest.mark.slow
def test_matchfiles():
    # Match without shuffle
    with TestPipeline() as pipeline:
        count = (pipeline
                 | fileio.MatchFiles(os.path.join(DATA_DIR, 'warcfile*'), shuffle=False)
                 | beam.combiners.Count.Globally())
        assert_that(count, equal_to([3]))

    # Match with shuffle
    with TestPipeline() as pipeline:
        count = (pipeline
                 | fileio.MatchFiles(os.path.join(DATA_DIR, 'warcfile*'), shuffle=True)
                 | beam.combiners.Count.Globally())
        assert_that(count, equal_to([3]))
