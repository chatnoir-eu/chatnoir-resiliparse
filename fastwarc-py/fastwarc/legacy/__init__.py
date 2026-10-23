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

import importlib

# Load Cython exports only when requested. Importing the Rust-backed package's
# compatibility shims must not require the legacy extensions.

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


def __getattr__(name):
    if name not in __all__:
        raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
    module = '.warc' if name in ('ArchiveIterator', 'WarcRecord', 'WarcRecordType') else '.stream_io'
    value = getattr(importlib.import_module(module, __name__), name)
    globals()[name] = value
    return value


def __dir__():
    return sorted(set(globals()) | set(__all__))
