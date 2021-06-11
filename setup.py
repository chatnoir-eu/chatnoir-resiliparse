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


from setuptools import find_packages, setup, Extension
from Cython.Build import cythonize

import Cython.Compiler.Options
Cython.Compiler.Options.annotate = True

cpp_args = dict(
      extra_compile_args=["-std=c++14"],
      extra_link_args=["-std=c++14"])

extensions = [
      Extension('resiliparse.warc', sources=['resiliparse/warc.pyx'], **cpp_args),
      Extension('resiliparse.stream_io', sources=['resiliparse/stream_io.pyx'], **cpp_args)
]

setup(
      name='ResiliParse',
      version='1.0',
      description='Resilient web archive parsing library with fixed memory footprint and execution time.',
      author='Janek Bevendorff',
      author_email='janek.bevendorff@uni-weimar.de',
      url='https://webis.de',
      license='Apache License 2.0',
      packages=find_packages(),
      setup_requires=[
            'cython',
            'setuptools>=18.0'
      ],
      ext_modules=cythonize(extensions, annotate=True, language_level='3')
)
