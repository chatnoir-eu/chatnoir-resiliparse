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

import distutils.ccompiler
from distutils.dir_util import copy_tree
from setuptools import find_packages, setup, Extension


# 1. BOILERPLATE -------------------------------------------------------

VERSION = '0.10.6'
ROOT_DIRECTORY = os.path.abspath(os.path.dirname(__file__))
CXX = distutils.ccompiler.get_default_compiler()

TRACE = bool(int(os.getenv('TRACE', 0)))
DEBUG = bool(int(os.getenv('DEBUG', 0))) or TRACE
ASAN = bool(int(os.getenv('ASAN', 0)))

USE_CYTHON = False
ext = 'cpp'
cython_args = {}
try:
    from Cython.Build import cythonize
    import Cython.Compiler.Options

    ext = 'pyx'
    cython_args = dict(
        annotate=DEBUG,
        language_level='3',
        compiler_directives=dict(
            linetrace=TRACE,
            initializedcheck=DEBUG,
            boundscheck=DEBUG,
            cdivision=True
        )
    )
    USE_CYTHON = True
except ModuleNotFoundError as e:
    pass

cpp_args = {}
if TRACE:
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
    if ASAN:
        cpp_args['extra_compile_args'].append('-fsanitize=address')
        cpp_args['extra_link_args'].append('-fsanitize=address')

elif CXX == 'msvc':
    cpp_args.update(dict(
        extra_compile_args=['/std:c++latest'],
        extra_link_args=[]
    ))

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


# 2. RESILIPARSE SETUP -------------------------------------------------------

fast_warc_src = os.path.abspath(os.path.join(ROOT_DIRECTORY, '..', 'fastwarc'))
if os.path.isdir(fast_warc_src):
    sys.path.insert(0, fast_warc_src)

resiliparse_extensions = [
    Extension('resiliparse.extract.html2text',
              sources=[f'resiliparse/extract/html2text.{ext}'], libraries=['lexbor', 're2'], **cpp_args),
    Extension('resiliparse.parse.encoding',
              sources=[f'resiliparse/parse/encoding.{ext}'], libraries=['uchardet', 'lexbor'], **cpp_args),
    Extension('resiliparse.parse.html',
              sources=[f'resiliparse/parse/html.{ext}'], libraries=['lexbor'], **cpp_args),
    Extension('resiliparse.parse.http',
              sources=[f'resiliparse/parse/http.{ext}'], **cpp_args),
    Extension('resiliparse.parse.lang',
              sources=[f'resiliparse/parse/lang.{ext}'], **cpp_args)
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
    long_description=open(os.path.join(ROOT_DIRECTORY, 'README.md')).read(),
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
    ],
    extras_require={
        "CLI": [
            'joblib'
        ],
    },
    entry_points={
        'console_scripts': ['resiliparse=resiliparse.cli:main[CLI]']
    }
)
