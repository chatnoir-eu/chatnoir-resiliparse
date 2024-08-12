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

from .stream_io import FileStream, GZipStream, LZ4Stream
from .stream_io import FastWARCError, StreamError
from .warc import ArchiveIterator, WarcRecord, WarcRecordType

# Exposing symbols for legacy compatibility, please prefer explicit imports from submodules

__all__ = [
    "FileStream",
    "GZipStream",
    "LZ4Stream",
    "FastWARCError",
    "StreamError",
    "ArchiveIterator",
    "WarcRecord",
    "WarcRecordType"
]
