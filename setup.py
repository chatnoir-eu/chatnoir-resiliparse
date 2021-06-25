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
from setuptools import setup, Extension

USE_CYTHON = True
try:
      from Cython.Build import cythonize
      import Cython.Compiler.Options

      Cython.Compiler.Options.annotate = bool(os.getenv('DEBUG'))
      ext = 'pyx'
except ModuleNotFoundError:
      USE_CYTHON = False
      ext = 'cpp'

this_directory = os.path.abspath(os.path.dirname(__file__))
cpp_args = dict(
      extra_compile_args=['-std=c++17', '-O3', '-Wno-deprecated-declarations',
                          '-Wno-unreachable-code', '-Wno-unused-function'],
      extra_link_args=['-std=c++17', '-lz', '-llz4'])

# setup(
#       name='ResiliParse',
#       version='1.0',
#       description='Optimized and resilient web archive parsing library with fixed memory and execution time ceiling.',
#       author='Janek Bevendorff',
#       url='https://github.com/chatnoir-eu/chatnoir-resiliparse',
#       license='Apache License 2.0',
#       packages=[],
# )

fastwarc_extensions = [
      Extension('fastwarc.warc', sources=[f'fastwarc/warc.{ext}'], **cpp_args),
      Extension('fastwarc.stream_io', sources=[f'fastwarc/stream_io.{ext}'], **cpp_args),
      Extension('fastwarc.tools', sources=[f'fastwarc/tools.{ext}'], **cpp_args)
]
if USE_CYTHON:
      fastwarc_extensions=cythonize(fastwarc_extensions, annotate=Cython.Compiler.Options.annotate, language_level='3')

with open(os.path.join(this_directory, 'README.md'), encoding='utf-8') as f:
      fastwarc_long_description = f.read()

setup(
      name='FastWARC',
      version='0.2.3',
      description='A high-performance WARC parsing library for Python written in C++/Cython.',
      long_description=fastwarc_long_description,
      long_description_content_type='text/markdown',
      author='Janek Bevendorff',
      url='https://github.com/chatnoir-eu/chatnoir-resiliparse',
      license='Apache License 2.0',
      packages=['fastwarc'],
      install_requires=[
            'click',
            'tqdm'
      ],
      setup_requires=[
            'setuptools>=18.0'
      ],
      ext_modules=fastwarc_extensions,
      entry_points={
            'console_scripts': ['fastwarc=fastwarc.cli:main']
      }
)
