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

import io
import logging

import apache_beam as beam
import apache_beam.typehints as t
from apache_beam.io.aws.s3io import S3IO, S3Downloader
from apache_beam.io.aws.clients.s3 import boto3_client
from apache_beam.io.filesystem import CompressionTypes
from apache_beam.io.filesystemio import DownloaderStream
from apache_beam.io.filesystems import FileSystems
from apache_beam.io.restriction_trackers import OffsetRange, OffsetRestrictionTracker
from apache_beam.options.value_provider import RuntimeValueProvider
import apache_beam.transforms.window as window
import boto3
import botocore.client as boto_client

from fastwarc.warc import ArchiveIterator
from resiliparse.beam.fileio import MatchFiles
from resiliparse.itertools import warc_retry

logger = logging.getLogger()


class ReadWarcs(beam.PTransform):
    """
    Read WARC records from files matching a glob pattern.

    :param file_pattern: input file glob pattern
    :param warc_args: arguments to pass to :class:`fastwarc.warc.ArchiveIterator`
    :param with_filename: keep the input filename as a key (otherwise return only the record)
    :param freeze: freeze returned records (required if the records are not consumed immediately)
    :param always_keep_meta: always return record metadata, even if they exceed ``max_content_length``
                             (from ``warc_args``), but strip them of their payload
    :param shuffle_files: shuffle matched file names (useful if Beam runner does not support automatic
                          shuffling of input source splits)
    """

    def __init__(self,
                 file_pattern: str,
                 warc_args: t.Dict[str, t.Any] = None,
                 with_filename: bool = True,
                 freeze: bool = True,
                 always_keep_meta: bool = False,
                 shuffle_files: bool = False):
        super().__init__()
        self._transform = MatchFiles(file_pattern, shuffle=shuffle_files)
        self._transform |= beam.ParDo(_ReadWarc(warc_args=warc_args,
                                                with_filename=with_filename,
                                                freeze=freeze,
                                                always_keep_meta=always_keep_meta))

    def expand(self, pcoll):
        return pcoll | self._transform


class ReadAllWarcs(beam.PTransform):
    """
    Read WARC records from a given :class:`~apache_beam.pvalue.PCollection` of
    :class:`~apache_beam.io.filesystem.FileMetadata` objects.

    :param warc_args: arguments to pass to :class:`fastwarc.warc.ArchiveIterator`
    :param with_filename: keep the input filename as a key (otherwise return only the record)
    :param freeze: freeze returned records (required if returned records are not consumed immediately)
    :param always_keep_meta: always return record metadata, even if they exceed ``max_content_length``
                             (from ``warc_args``), but strip them of their payload
    """
    def __init__(self,
                 warc_args: t.Dict[str, t.Any] = None,
                 with_filename: bool = True,
                 freeze: bool = True,
                 always_keep_meta: bool = False):
        super().__init__()
        self._transform = beam.ParDo(_ReadWarc(warc_args=warc_args,
                                               with_filename=with_filename,
                                               freeze=freeze,
                                               always_keep_meta=always_keep_meta))

    def expand(self, pcoll):
        return pcoll | self._transform


class _WarcRestrictionProvider(beam.transforms.core.RestrictionProvider):
    def initial_restriction(self, file_meta):
        return OffsetRange(0, file_meta.size_in_bytes)

    def create_tracker(self, restriction):
        return OffsetRestrictionTracker(restriction)

    def restriction_size(self, file_meta, restriction):
        return min(file_meta.size_in_bytes, restriction.stop - restriction.start)


# noinspection PyAbstractClass
class _ReadWarc(beam.DoFn):
    """
    WARC file input source.
    """

    def __init__(self, warc_args, with_filename, freeze, always_keep_meta):
        super().__init__()
        self.warc_args = warc_args or {}
        self.with_filename = with_filename
        self.freeze = freeze
        self.max_content_length = None
        if always_keep_meta and 'max_content_length' in self.warc_args:
            self.max_content_length = self.warc_args['max_content_length']
            del self.warc_args['max_content_length']

    # noinspection PyMethodOverriding
    def process(self, file_meta, tracker=beam.DoFn.RestrictionParam(_WarcRestrictionProvider())):
        restriction = tracker.current_restriction()

        def stream_factory():
            return self._open_file(file_meta.path)

        record = None
        stream = None
        try:
            logger.info('Starting WARC file %s', file_meta.path)
            stream = stream_factory()
            for record in warc_retry(ArchiveIterator(stream, **self.warc_args), stream_factory):
                logger.debug('Reading WARC record %s', record.record_id)
                if not tracker.try_claim(record.stream_pos):
                    break

                if self.max_content_length is not None and record.content_length > self.max_content_length:
                    # Max length exceeded, but we still want to return a metadata record
                    logger.debug("Stripping long record %s (%i bytes) of its payload.",
                                 record.record_id, record.content_length)
                    record.reader.consume()

                if self.freeze:
                    record.freeze()

                if self.with_filename:
                    yield window.TimestampedValue((file_meta.path, record), record.record_date.timestamp())
                else:
                    yield window.TimestampedValue(record, record.record_date.timestamp())
            else:
                tracker.try_claim(restriction.stop)
            logger.info('Completed WARC file %s', file_meta.path)
        except Exception as e:
            if record:
                logger.error('WARC reader failed in %s past record %s (pos: %i).',
                             file_meta.path, record.record_id, record.stream_pos)
            else:
                logger.error('WARC reader failed in %s', file_meta.path)
            logger.exception(e)
        finally:
            if stream and not stream.closed:
                stream.close()

    def _open_file(self, file_name):
        """Get input file stream."""
        if file_name.startswith('s3://'):
            stream = self._open_s3_stream(file_name)
        else:
            stream = FileSystems.open(file_name, 'application/octet-stream',
                                      compression_type=CompressionTypes.UNCOMPRESSED)

        return stream

    # noinspection PyProtectedMember
    def _open_s3_stream(self, file_name, buffer_size=65536):
        """Open S3 streams with custom Boto3 client."""

        options = FileSystems._pipeline_options or RuntimeValueProvider.runtime_options
        s3_client = _Boto3Client(options=options)
        s3io = S3IO(client=s3_client, options=options)

        downloader = S3Downloader(s3io.client, file_name, buffer_size=buffer_size)
        return io.BufferedReader(DownloaderStream(downloader, mode='rb'), buffer_size=buffer_size)


class _Boto3Client(boto3_client.Client):    # pragma: no cover
    """Boto3 client with custom settings."""

    # noinspection PyMissingConstructor
    def __init__(self, options, connect_timeout=60, read_timeout=240):
        super().__init__(options)

        options = options.get_all_options()
        session = boto3.session.Session()
        self.client = session.client(
            service_name='s3',
            region_name=options.get('s3_region_name'),
            api_version=options.get('s3_api_version'),
            use_ssl=not options.get('s3_disable_ssl', False),
            verify=options.get('s3_verify'),
            endpoint_url=options.get('s3_endpoint_url'),
            aws_access_key_id=options.get('s3_access_key_id'),
            aws_secret_access_key=options.get('s3_secret_access_key'),
            aws_session_token=options.get('s3_session_token'),
            config=boto_client.Config(
                connect_timeout=connect_timeout,
                read_timeout=read_timeout
            )
        )
