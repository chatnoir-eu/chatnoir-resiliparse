from apache_beam.testing.test_pipeline import TestPipeline as TestPipelineBeam
from apache_beam.options.pipeline_options import PipelineOptions, PortableOptions


# Prevent issues with persistently running StopOnExitJobServer
class TestPipeline(TestPipelineBeam):
    def __init__(self, options=None, **kwargs):
        if not options:
            options = PipelineOptions()
        options = options.view_as(PortableOptions)
        options.job_endpoint = 'embed'
        super().__init__(options=options, **kwargs)
