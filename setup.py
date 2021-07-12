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
import platform
from setuptools import setup, Extension
import warnings
import sys

VERSION = '0.3.9'
THIS_DIRECTORY = os.path.abspath(os.path.dirname(__file__))
USE_CYTHON = True
try:
    from Cython.Build import cythonize
    import Cython.Compiler.Options

    Cython.Compiler.Options.annotate = bool(os.getenv('DEBUG'))
    ext = 'pyx'
    cython_args = dict(annotate=Cython.Compiler.Options.annotate,
                       language_level='3')

except ModuleNotFoundError:
    USE_CYTHON = False
    ext = 'cpp'
    cython_args = {}

cpp_args = dict(
    extra_compile_args=['-std=c++17', '-O3', '-Wno-deprecated-declarations',
                        '-Wno-unreachable-code', '-Wno-unused-function'],
    extra_link_args=['-std=c++17'])

BUILD_PACKAGES = ['fastwarc', 'resiliparse']
if os.environ.get('BUILD_PACKAGES'):
    BUILD_PACKAGES = os.environ.get('BUILD_PACKAGES').split(' ')

data_ext = ['*.pxd', '*.md']
inc_module = []
inc_module_data = {}
if 'sdist' in sys.argv:
    # Include resiliparse_inc module and *.pyx only in source distribution
    data_ext.extend(['*.pyx', '*.pxi'])
    inc_module.append('resiliparse_inc')
    inc_module_data['resiliparse_inc'] = data_ext


# ------------------------------------------
# Resiliparse
# ------------------------------------------

if 'resiliparse' in BUILD_PACKAGES and os.path.isdir('resiliparse'):
    resiliparse_extensions = [
        Extension('resiliparse.parse', sources=[f'resiliparse/parse.{ext}'],
                  libraries=['uchardet'], **cpp_args)
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
        long_description=open(os.path.join(THIS_DIRECTORY, 'resiliparse/README.md')).read(),
        long_description_content_type='text/markdown',
        author='Janek Bevendorff',
        url='https://github.com/chatnoir-eu/chatnoir-resiliparse',
        license='Apache License 2.0',
        packages=['resiliparse', *inc_module],
        package_data={
            'resiliparse': data_ext,
            **inc_module_data
        },
        install_requires=[
            'selectolax'
        ],
        setup_requires=[
            'setuptools>=18.0'
        ],
        ext_modules=resiliparse_extensions
    )


# ------------------------------------------
# FastWARC
# ------------------------------------------

if 'fastwarc' in BUILD_PACKAGES and os.path.isdir('fastwarc'):
    fastwarc_extensions = [
        Extension('fastwarc.warc', sources=[f'fastwarc/warc.{ext}'], **cpp_args),
        Extension('fastwarc.stream_io', sources=[f'fastwarc/stream_io.{ext}'], libraries=['z', 'lz4'], **cpp_args),
        Extension('fastwarc.tools', sources=[f'fastwarc/tools.{ext}'], **cpp_args)
    ]
    if USE_CYTHON:
        fastwarc_extensions = cythonize(fastwarc_extensions, **cython_args)

    setup(
        name='FastWARC',
        version=VERSION,
        description='A high-performance WARC parsing library for Python written in C++/Cython.',
        long_description=open(os.path.join(THIS_DIRECTORY, 'fastwarc/README.md')).read(),
        long_description_content_type='text/markdown',
        author='Janek Bevendorff',
        url='https://github.com/chatnoir-eu/chatnoir-resiliparse',
        license='Apache License 2.0',
        packages=['fastwarc', *inc_module],
        package_data={
            'fastwarc': data_ext,
            **inc_module_data
        },
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
