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

import platform
import os
import warnings
import sys

from setuptools import find_packages, setup, Extension

sys.path.append(os.path.join(os.path.dirname(__file__), '..'))
from setup_base import *

resiliparse_extensions = [
    Extension('resiliparse.parse.encoding',
              sources=[f'resiliparse/parse/encoding.{ext}'], libraries=['uchardet', 'lexbor'], **cpp_args),
    Extension('resiliparse.parse.html',
              sources=[f'resiliparse/parse/html.{ext}'], libraries=['uchardet', 'lexbor'], **cpp_args),
    Extension('resiliparse.parse.http',
              sources=[f'resiliparse/parse/http.{ext}'], libraries=['uchardet'], **cpp_args)
]
if os.name == 'posix':
    resiliparse_extensions.extend([
        Extension('resiliparse.process_guard', sources=[f'resiliparse/process_guard.{ext}'],
                  libraries=['pthread'], **cpp_args),
        Extension('resiliparse.itertools', sources=[f'resiliparse/itertools.{ext}'], **cpp_args)
    ])
else:
    warnings.warn(f"Unsupported platform '{platform.system()}': Building without ProcessGuard extension.")

if USE_CYTHON:
    resiliparse_extensions = cythonize(resiliparse_extensions, **cython_args)

setup(
    name='Resiliparse',
    version=VERSION,
    description='A collection of robust and fast processing tools for parsing and '
                'analyzing (not only) web archive data.',
    long_description=open(os.path.join(ROOT_DIRECTORY, 'resiliparse/README.md')).read(),
    long_description_content_type='text/markdown',
    author='Janek Bevendorff',
    url='https://github.com/chatnoir-eu/chatnoir-resiliparse',
    license='Apache License 2.0',
    packages=[*find_packages(), *inc_package],
    package_data={'': data_ext},
    ext_modules=resiliparse_extensions,
    install_requires=[
        'click',
        'fastwarc',
        'tqdm'
    ],
    setup_requires=['setuptools>=18.0'],
    tests_require=[
        'pytest',
        'pytest-cov'
    ]
)
