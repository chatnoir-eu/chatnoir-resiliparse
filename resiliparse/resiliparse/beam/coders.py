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

from apache_beam.coders import *
from apache_beam.coders import StrUtf8Coder as _StrUtf8Coder
from resiliparse.parse.encoding import bytes_to_str


class StrUtf8Coder(_StrUtf8Coder):
    """
    More resilient version of :class:`apache_beam.coders.StrUtf8Coder`, which can handle encoding errors.

    Uses the :class:`~resiliparse.parse.encoding.bytes_to_str` encoding helpers for encoding and decoding text.
    """

    def decode(self, value):
        return bytes_to_str(value, encoding='utf-8')
