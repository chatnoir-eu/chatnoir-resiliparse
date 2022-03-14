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

import apache_beam as beam
import apache_beam.typehints as t
from apache_beam.io import fileio as beam_fio
from apache_beam.io.filesystems import FileSystems
from apache_beam.io.filesystem import CompressionTypes, FileMetadata, CompressedFile
from apache_beam.io.restriction_trackers import OffsetRange, OffsetRestrictionTracker

from resiliparse.beam import coders
from resiliparse.beam.fileio import MatchFiles


DEFAULT_DESIRED_SPLIT_SIZE = 64 * 1024 * 1024   # 64 MiB
DEFAULT_MIN_SPLIT_SIZE = 1024 * 1024            # 1 MiB


class ReadFromText(beam.PTransform):
    """
    Read lines from text files in parallel.

    Can be used to parallelize the processing of large individual text files with newline-delimited
    records. Unlike :class:`apache_beam.io.textio.ReadFromText`, :class:`ReadFromText` can prevent
    fusion of the file splits by opportunistically generating splits and shuffling them before
    actually reading the file contents. This prevents input bottlenecks on certain runners that do not
    automatically distribute splits for parallel processing such as the FlinkRunner.

    The only supported newline separator at the moment is ``\\n``.

    :param file_pattern: input file glob
    :param empty_match_treatment: what to do with empty glob matches
    :param shuffle_splits: shuffle input splits to break fusion
    :param shuffle_names: shuffle matched file names before deriving splits (adds one more fusion
                          break, should be enabled if ``file_pattern`` matches many files or if
                          input files are compressed and cannot be split)
    :param desired_split_size: desired file split size in bytes
    :param min_split_size: minimum file split size in bytes
    :param compression_type: file compression type, will determine whether individual files are splittable
    :param coder: coder for decoding file contents
    """

    def __init__(self,
                 file_pattern: str,
                 empty_match_treatment: beam_fio.EmptyMatchTreatment = beam_fio.EmptyMatchTreatment.ALLOW_IF_WILDCARD,
                 shuffle_splits: bool = True,
                 shuffle_names: bool = False,
                 desired_split_size: int = DEFAULT_DESIRED_SPLIT_SIZE,
                 min_split_size: int = DEFAULT_MIN_SPLIT_SIZE,
                 compression_type: CompressionTypes = CompressionTypes.AUTO,
                 coder: coders.Coder = coders.StrUtf8Coder()):
        super().__init__()
        self._transform = MatchFiles(file_pattern, empty_match_treatment, shuffle_names)
        self._transform |= beam.ParDo(_GenSplits(desired_split_size, min_split_size, compression_type))
        if shuffle_splits:
            self._transform |= beam.Reshuffle()
        self._transform |= beam.ParDo(_ReadSplits(compression_type, coder))

    def expand(self, pcoll):
        return pcoll | self._transform


class ReadAllFromText(beam.PTransform):
    """
    Read lines from a given :class:`~apache_beam.pvalue.PCollection` of
    :class:`~apache_beam.io.filesystem.FileMetadata` objects in parallel.

    See :class:`ReadFormText` for more information.

    :param shuffle_splits: shuffle input splits to break fusion
    :param shuffle_names: shuffle matched file names before deriving splits (adds one more fusion
                          break, should be enabled if ``file_pattern`` matches many files or if
                          input files are compressed and cannot be split)
    :param desired_split_size: desired file split size in bytes
    :param min_split_size: minimum file split size in bytes
    :param compression_type: file compression type, will determine whether individual files are splittable
    :param coder: coder for decoding file contents
    """

    def __init__(self,
                 shuffle_splits: bool = True,
                 shuffle_names: bool = False,
                 desired_split_size: int = DEFAULT_DESIRED_SPLIT_SIZE,
                 min_split_size: int = DEFAULT_MIN_SPLIT_SIZE,
                 compression_type: CompressionTypes = CompressionTypes.AUTO,
                 coder: coders.Coder = coders.StrUtf8Coder()):
        super().__init__()
        self._transform = beam.ParDo(_GenSplits(desired_split_size, min_split_size, compression_type))
        if shuffle_names:
            self._transform = beam.Reshuffle() | self._transform
        if shuffle_splits:
            self._transform |= beam.Reshuffle()
        self._transform |= beam.ParDo(_ReadSplits(compression_type, coder))

    def expand(self, pcoll):
        return pcoll | self._transform


# noinspection PyAbstractClass
class _GenSplits(beam.DoFn):
    """Create initial splits based on the text file size."""

    def __init__(self, desired_split_size, min_split_size, compression_type):
        super().__init__()
        self.desired_split_size = max(min_split_size, desired_split_size)
        self.min_split_size = min_split_size
        self.compression_type = compression_type

    # noinspection PyMethodOverriding
    def process(self, element: FileMetadata):
        comp_type = self.compression_type
        if self.compression_type == CompressionTypes.AUTO:
            comp_type = CompressionTypes.detect_compression_type(element.path)

        if comp_type != CompressionTypes.UNCOMPRESSED:
            yield element, (0, element.size_in_bytes)
            return

        for start_offset in range(0, element.size_in_bytes, self.desired_split_size):
            end_offset = min(start_offset + self.desired_split_size, element.size_in_bytes)
            if element.size_in_bytes - end_offset < self.min_split_size:
                yield element, (start_offset, element.size_in_bytes)
                break
            yield element, (start_offset, end_offset)


class _FileSplitRestrictionProvider(beam.transforms.core.RestrictionProvider):
    def initial_restriction(self, file_split):
        return OffsetRange(file_split[1][0], file_split[1][1])

    def create_tracker(self, restriction):
        return OffsetRestrictionTracker(restriction)

    def restriction_size(self, file_split, restriction):
        return file_split[1][1] - file_split[1][0]


# noinspection PyAbstractClass
class _ReadSplits(beam.DoFn):
    """Read text lines from splits."""

    def __init__(self, compression_type, coder):
        super().__init__()
        self.compression_type = compression_type
        self.coder = coder

    # noinspection PyMethodOverriding
    def process(self,
                element: t.KV[FileMetadata, t.Tuple[int, int]],
                tracker=beam.DoFn.RestrictionParam(_FileSplitRestrictionProvider())):
        file_meta, _ = element

        restriction = tracker.current_restriction()
        pos = restriction.start

        with FileSystems.open(file_meta.path,
                              mime_type=CompressionTypes.mime_type(self.compression_type),
                              compression_type=self.compression_type) as file:

            if isinstance(file, CompressedFile) and tracker.try_claim(pos):
                # If file is compressed, we need to check the position in the compressed stream
                if pos > 0:
                    file.seek(pos)
                while file._file.tell() <= restriction.stop:
                    line = file.readline()
                    if not line:
                        break
                    yield line[:-1]     # Strip trailing newline character

                tracker.try_claim(file._file.tell())
                return

            if pos > 0:
                # Seek to one character before the starting position and discard the first line.
                # This avoids skipping lines if files are split exactly at newlines.
                file.seek(pos - 1)
                file.readline()

            while tracker.try_claim(file.tell()):
                line = file.readline()
                if not line:
                    raise IOError(f'Unexpected EOF at {file.tell()}')
                line = line[:-1]     # Strip trailing newline character
                if self.coder:
                    line = self.coder.decode(line)
                yield line
