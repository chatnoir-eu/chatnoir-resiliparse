# Copyright 2023 Janek Bevendorff
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
import sys

from Cython.Build import cythonize
from Cython.Distutils.build_ext import new_build_ext as build_ext
import distutils.ccompiler
from distutils.dir_util import copy_tree
from setuptools import Extension, setup

TRACE = bool(int(os.getenv('TRACE', 0)))
DEBUG = bool(int(os.getenv('DEBUG', 0))) or TRACE
ASAN = bool(int(os.getenv('ASAN', 0)))

ROOT_DIR = os.path.abspath(os.path.dirname(__file__))
CXX = distutils.ccompiler.get_default_compiler()

# Construct vcpkg lib and include paths
def _vcpkg_path():
    osname = platform.system().lower().replace('darwin', 'osx')
    arch = platform.machine().lower().replace('x86_64', 'x64').replace('amd64', 'x64')
    if osname == 'linux' and arch == 'arm64':
        arch = 'aarch64'
    triplet = f'{arch}-{osname}'

    if os.environ.get('RESILIPARSE_VCPKG_PATH'):
        return os.path.join(os.environ['RESILIPARSE_VCPKG_PATH'], triplet)
    return os.path.join(os.path.dirname(ROOT_DIR), 'vcpkg_installed', triplet)

INCLUDE_PATH = os.path.join(_vcpkg_path(), 'include')
LIBRARY_PATH = os.path.join(_vcpkg_path(), 'lib')


def get_cpp_args():
    cpp_args = {}

    if TRACE:
        cpp_args.update(dict(define_macros=[('CYTHON_TRACE_NOGIL', '1')]))

    if CXX == 'unix':
        cpp_args.update(dict(
            extra_compile_args=['-std=c++17',
                                f'-O{0 if DEBUG else 3}',
                                f'-I{INCLUDE_PATH}',
                                '-Wall',
                                '-Wno-deprecated-declarations',
                                '-Wno-unreachable-code',
                                '-Wno-unused-function'],
            extra_link_args=['-std=c++17', f'-L{LIBRARY_PATH}']
        ))
        if DEBUG:
            cpp_args['extra_compile_args'].append('-Werror')
        if ASAN:
            cpp_args['extra_compile_args'].append('-fsanitize=address')
            cpp_args['extra_link_args'].append('-fsanitize=address')

    elif CXX == 'msvc':
        cpp_args.update(dict(
            extra_compile_args=['/std:c++latest',
                                '/W3',
                                f'/O{"d" if DEBUG else 2}',
                                f'/I{INCLUDE_PATH}'],
            extra_link_args=[f'/LIBPATH:{LIBRARY_PATH}']
        ))
        if DEBUG:
            cpp_args['extra_compile_args'].append('/WX')
            cpp_args['extra_link_args'].append('/WX')

    return cpp_args


def get_cython_args():
    return dict(
        annotate=DEBUG,
        gdb_debug=DEBUG,
        nthreads=os.cpu_count(),
        compiler_directives=dict(
            language_level='3',
            linetrace=TRACE,
            initializedcheck=DEBUG,
            boundscheck=DEBUG,
            cdivision=True
        )
    )


def get_ext_modules():
    cpp_args = get_cpp_args()

    fastwarc_extensions = [
        Extension('fastwarc.warc', sources=['fastwarc/warc.pyx'], **cpp_args),
        Extension('fastwarc.stream_io', sources=['fastwarc/stream_io.pyx'],
                  libraries=['zlib' if CXX == 'msvc' else 'z', 'lz4'], **cpp_args),
        Extension('fastwarc.tools', sources=['fastwarc/tools.pyx'], **cpp_args)
    ]

    return cythonize(fastwarc_extensions, **get_cython_args())


# Copy Resiliparse header files
if os.path.isdir(os.path.join(ROOT_DIR, '..', 'resiliparse', 'resiliparse_inc')):
    copy_tree(os.path.join(ROOT_DIR, '..', 'resiliparse', 'resiliparse_inc'),
              os.path.join(ROOT_DIR, 'resiliparse_inc'), update=1)
    copy_tree(os.path.join(ROOT_DIR, '..', 'resiliparse', 'resiliparse_common'),
              os.path.join(ROOT_DIR, 'resiliparse_common'), update=1)

setup(
    ext_modules=get_ext_modules(),
    cmdclass=dict(build_ext=build_ext),
    exclude_package_data={
        '': [] if 'sdist' in sys.argv else ['*.pxd', '*.pxi', '*.pyx', '*.h', '*.cpp']
    }
)
