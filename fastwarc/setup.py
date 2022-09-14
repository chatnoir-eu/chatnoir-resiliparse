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

from itertools import chain
import os
import sys

import distutils.ccompiler
from distutils.dir_util import copy_tree
from setuptools import find_packages, setup, Extension


# 1. BOILERPLATE -------------------------------------------------------

VERSION = '0.13.0'
ROOT_DIRECTORY = os.path.abspath(os.path.dirname(__file__))
CXX = distutils.ccompiler.get_default_compiler()

TRACE = bool(int(os.getenv('TRACE', 0)))
DEBUG = bool(int(os.getenv('DEBUG', 0))) or TRACE
ASAN = bool(int(os.getenv('ASAN', 0)))

cpp_args = {}
try:
    from Cython.Build import cythonize
    import Cython.Compiler.Options

    cpp_ext = 'pyx'
    cython_args = dict(
        annotate=DEBUG,
        gdb_debug=DEBUG,
        compiler_directives=dict(
            language_level='3',
            linetrace=TRACE,
            initializedcheck=DEBUG,
            boundscheck=DEBUG,
            cdivision=True
        )
    )
    USE_CYTHON = True
except ModuleNotFoundError as e:
    cpp_ext = 'cpp'
    cython_args = {}
    USE_CYTHON = False

if TRACE:
    cpp_args.update(dict(define_macros=[('CYTHON_TRACE_NOGIL', '1')]))

if CXX == 'unix':
    cpp_args.update(dict(
        extra_compile_args=['-std=c++17',
                            f'-O{0 if DEBUG else 3}',
                            '-Wall',
                            '-Wno-deprecated-declarations',
                            '-Wno-unreachable-code',
                            '-Wno-unused-function'],
        extra_link_args=['-std=c++17']
    ))
    if DEBUG:
        cpp_args['extra_compile_args'].append('-Werror')
    if ASAN:
        cpp_args['extra_compile_args'].append('-fsanitize=address')
        cpp_args['extra_link_args'].append('-fsanitize=address')
elif CXX == 'msvc':
    cpp_args.update(dict(
        extra_compile_args=['/std:c++latest', '/W3'],
        extra_link_args=[]
    ))
    if DEBUG:
        cpp_args['extra_compile_args'].append('/WX')
        cpp_args['extra_link_args'].append('/WX')

if os.path.isdir(os.path.join(ROOT_DIRECTORY, '..', 'resiliparse_inc')):
    copy_tree(os.path.join(ROOT_DIRECTORY, '..', 'resiliparse_inc'),
              os.path.join(ROOT_DIRECTORY, 'resiliparse_inc'), update=1)
    copy_tree(os.path.join(ROOT_DIRECTORY, '..', 'resiliparse_common'),
              os.path.join(ROOT_DIRECTORY, 'resiliparse_common'), update=1)

data_ext = ['*.pxd', '*.h']
inc_package = []
if 'sdist' in sys.argv:
    # Include resiliparse_inc module and Cython src files only in source distribution
    data_ext.extend(['*.pyx', '*.pxi'])
    inc_package.append('resiliparse_inc')
    inc_package.append('resiliparse_common')


# 2. FASTWARC SETUP -------------------------------------------------------

fastwarc_extensions = [
    Extension('fastwarc.warc', sources=[f'fastwarc/warc.{cpp_ext}'], **cpp_args),
    Extension('fastwarc.stream_io', sources=[f'fastwarc/stream_io.{cpp_ext}'],
              libraries=['zlib' if CXX == 'msvc' else 'z', 'lz4'], **cpp_args),
    Extension('fastwarc.tools', sources=[f'fastwarc/tools.{cpp_ext}'], **cpp_args)
]
if USE_CYTHON:
    fastwarc_extensions = cythonize(fastwarc_extensions, **cython_args)

extras_require = {}
extras_require.update({
    'all': list(chain(*extras_require.values())),   # All except "test"
})

tests_require = [
    'pytest',
    'pytest-cov',
    'lz4'
]
extras_require['test'] = tests_require
setup(
    name='FastWARC',
    version=VERSION,
    description='A high-performance WARC parsing library for Python written in C++/Cython.',
    long_description=open(os.path.join(ROOT_DIRECTORY, 'README.md')).read(),
    long_description_content_type='text/markdown',
    author='Janek Bevendorff',
    url='https://github.com/chatnoir-eu/chatnoir-resiliparse',
    license='Apache License 2.0',
    packages=[*find_packages(), *inc_package],
    package_data={'': data_ext},
    ext_modules=fastwarc_extensions,
    install_requires=[
        'brotli',
        'click',
        'tqdm'
    ],
    setup_requires=['setuptools>=18.0'],
    tests_require=tests_require,
    extras_require=extras_require,
    entry_points={
        'console_scripts': ['fastwarc=fastwarc.cli:main']
    }
)
