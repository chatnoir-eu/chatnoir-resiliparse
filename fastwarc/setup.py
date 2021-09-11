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

import os
import sys

from setuptools import find_packages, setup, Extension

sys.path.append(os.path.join(os.path.dirname(__file__), '..'))
from setup_base import *

fastwarc_extensions = [
    Extension('fastwarc.warc', sources=[f'fastwarc/warc.{ext}'], **cpp_args),
    Extension('fastwarc.stream_io', sources=[f'fastwarc/stream_io.{ext}'],
              libraries=['zlib' if CXX == 'msvc' else 'z', 'lz4'], **cpp_args),
    Extension('fastwarc.tools', sources=[f'fastwarc/tools.{ext}'], **cpp_args)
]
if USE_CYTHON:
    fastwarc_extensions = cythonize(fastwarc_extensions, **cython_args)

setup(
    name='FastWARC',
    version=VERSION,
    description='A high-performance WARC parsing library for Python written in C++/Cython.',
    long_description=open(os.path.join(ROOT_DIRECTORY, 'fastwarc/README.md')).read(),
    long_description_content_type='text/markdown',
    author='Janek Bevendorff',
    url='https://github.com/chatnoir-eu/chatnoir-resiliparse',
    license='Apache License 2.0',
    packages=[*find_packages(), *inc_package],
    package_data={'': data_ext},
    ext_modules=fastwarc_extensions,
    install_requires=[
        'click',
        'tqdm'
    ],
    setup_requires=['setuptools>=18.0'],
    tests_require=[
        'pytest',
        'pytest-cov',
        'lz4'
    ],
    entry_points={
        'console_scripts': ['fastwarc=fastwarc.cli:main']
    }
)
