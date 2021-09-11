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

import distutils.ccompiler
from distutils.dir_util import copy_tree

VERSION = '0.4.0'
ROOT_DIRECTORY = os.path.abspath(os.path.dirname(__file__))
CXX = distutils.ccompiler.get_default_compiler()

DEBUG = bool(os.getenv('DEBUG'))
USE_CYTHON = False
ext = 'cpp'
cython_args = {}
try:
    from Cython.Build import cythonize
    import Cython.Compiler.Options

    ext = 'pyx'
    Cython.Compiler.Options.annotate = DEBUG
    cython_args = dict(
        annotate=Cython.Compiler.Options.annotate,
        language_level='3',
        compiler_directives=dict(
            linetrace=DEBUG
        )
    )
    USE_CYTHON = True
except ModuleNotFoundError as e:
    pass

cpp_args = {}
if DEBUG:
    cpp_args.update(dict(define_macros=[('CYTHON_TRACE_NOGIL', '1')]))

if CXX == 'unix':
    cpp_args.update(dict(
        extra_compile_args=['-std=c++17', '-O3', '-Wno-deprecated-declarations',
                            '-Wno-unreachable-code', '-Wno-unused-function',
                            # Temporary flags until https://github.com/lexbor/lexbor/pull/125 and
                            # https://github.com/lexbor/lexbor/pull/135 are released
                            '-fpermissive', '-Wno-c++11-narrowing'],
        extra_link_args=['-std=c++17']
    ))
elif CXX == 'msvc':
    cpp_args.update(dict(
        extra_compile_args=['/std:c++latest'],
        extra_link_args=[]
    ))

data_ext = []
inc_package = []
if 'sdist' in sys.argv:
    # Include resiliparse_inc module and Cython files only in source distribution
    data_ext.extend(['*.pxd', '*.pyx', '*.pxi'])
    inc_package.append('resiliparse_inc')
copy_tree(os.path.join(ROOT_DIRECTORY, 'resiliparse_inc'), 'resiliparse_inc', update=1)
