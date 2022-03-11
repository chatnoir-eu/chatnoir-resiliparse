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
from apache_beam.io import fileio as beam_fio


class MatchFiles(beam.PTransform):
    """
    Match a file pattern using :meth:`apache_beam.io.filesystems.FileSystems.match`.

    Unlike the original Beam implementation, this file matcher enforces a fusion break
    by reshuffling the matched file names. This circumvents limitations in certain
    Beam runners that do not automatically distribute splits, such as the FlinkRunner.

    :param file_pattern: file glob
    :param empty_match_treatment: what to do with empty glob matches
    :param shuffle: shuffle matches to break fusion (setting this to ``False`` effectively
                    falls back to the original Beam implementation)
    """

    def __init__(self,
                 file_pattern: str,
                 empty_match_treatment: beam_fio.EmptyMatchTreatment = beam_fio.EmptyMatchTreatment.ALLOW_IF_WILDCARD,
                 shuffle: bool = True):
        super().__init__()
        self._transform = beam_fio.MatchFiles(file_pattern, empty_match_treatment)
        if shuffle:
            self._transform |= beam.Reshuffle()

    def expand(self, pcoll):
        return pcoll | self._transform
