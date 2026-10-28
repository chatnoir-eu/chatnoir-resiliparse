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

import importlib
import sys

_native = importlib.import_module('._resiliparse', __name__)

__all__ = [*_native.__all__]


def __getattr__(name):
    # Lazily bind submodules already initialized and registered in sys.modules.
    try:
        value = sys.modules[f"{__name__}.{name}"]
    except KeyError:
        raise AttributeError(f'module {__name__!r} has no attribute {name!r}') from None
    else:
        globals()[name] = value
        return value


# Lazy sub package loader.
def _load_native_package(name, namespace):
    native_package = getattr(_native, f'_load_{name}')()
    namespace.update({
        key: value
        for key, value in vars(native_package).items()
        if key not in {'__name__', '__loader__', '__package__', '__spec__', '__file__'}
    })
