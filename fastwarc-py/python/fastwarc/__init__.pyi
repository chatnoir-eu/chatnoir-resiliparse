# Copyright 2026 Janek Bevendorff
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

from .warc import ArchiveIterator, WarcHeader, WarcRecord, WarcRecordType
from .stream_io import (
    GzipReader,
    GzipWriter,
    ZstdReader,
    ZstdWriter,
    Lz4Reader,
    Lz4Writer,

)

__all__ = [
    'ArchiveIterator',
    'WarcHeader',
    'WarcRecord',
    'WarcRecordType',
    'GzipReader',
    'GzipWriter',
    'ZstdReader',
    'ZstdWriter',
    'Lz4Reader',
    'Lz4Writer',

]
