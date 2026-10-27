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


# Legacy shim for exporting beam module under resiliparse.legacy.

import importlib
from pathlib import Path

from resiliparse import beam as _beam

__all__ = _beam.__all__
__path__ = [str(Path(_beam.__file__).parent)]


def __getattr__(name):
    if name not in __all__:
        raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
    value = importlib.import_module(f"{__name__}.{name}")
    globals()[name] = value
    return value
