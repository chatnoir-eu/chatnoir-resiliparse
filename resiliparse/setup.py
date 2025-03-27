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

import glob
import os
import platform
import shutil
import sys

from Cython.Build import cythonize
from Cython.Distutils.build_ext import new_build_ext as build_ext
from setuptools import Extension, setup

TRACE = bool(int(os.getenv('TRACE', 0)))
DEBUG = bool(int(os.getenv('DEBUG', 0))) or TRACE
ASAN = bool(int(os.getenv('ASAN', 0)))

ROOT_DIR = os.path.abspath(os.path.dirname(__file__))


class resiliparse_build_ext(build_ext):
    def build_extension(self, ext):
        for k, v in self.get_cpp_args().items():
            setattr(ext, k, v)
        return super().build_extension(ext)

    def get_vcpkg_path(self):
        osname = platform.system().lower().replace('darwin', 'osx')
        arch = platform.machine().lower()
        if os.environ.get('_PYTHON_HOST_PLATFORM', '').startswith('macosx-'):
            arch = os.environ['_PYTHON_HOST_PLATFORM'].split('-')[-1]
        elif osname == 'linux' and arch == 'arm64':
            arch = 'aarch64'
        arch = arch.replace('x86_64', 'x64').replace('amd64', 'x64')
        triplet = f'{arch}-{osname}'

        if os.environ.get('RESILIPARSE_VCPKG_PATH'):
            return os.path.join(os.environ['RESILIPARSE_VCPKG_PATH'], triplet)
        return os.path.join(os.path.dirname(ROOT_DIR), 'vcpkg_installed', triplet)

    def get_cpp_args(self):
        include_path = os.path.join(self.get_vcpkg_path(), 'include')
        library_path = os.path.join(self.get_vcpkg_path(), 'lib')
        cpp_args = {}

        if TRACE:
            cpp_args.update(dict(define_macros=[('CYTHON_TRACE_NOGIL', '1')]))

        if self.compiler.compiler_type == 'unix':
            cpp_args.update(dict(
                extra_compile_args=['-std=c++17',
                                    f'-O{0 if DEBUG else 3}',
                                    f'-I{include_path}',
                                    '-Wall',
                                    '-Wno-deprecated-declarations',
                                    '-Wno-unreachable-code',
                                    '-Wno-unused-function'],
                extra_link_args=['-std=c++17', f'-L{library_path}', f'-Wl,-rpath,{library_path}']
            ))
            if DEBUG:
                cpp_args['extra_compile_args'].append('-Werror')
            if ASAN:
                cpp_args['extra_compile_args'].append('-fsanitize=address')
                cpp_args['extra_link_args'].append('-fsanitize=address')
            if platform.system() == 'Darwin':
                cpp_args['extra_link_args'].append('-headerpad_max_install_names')

        elif self.compiler.compiler_type == 'msvc':
            cpp_args.update(dict(
                extra_compile_args=['/std:c++latest',
                                    '/W3',
                                    f'/O{"d" if DEBUG else 2}',
                                    f'/I{include_path}'],
                extra_link_args=[f'/LIBPATH:{library_path}']
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
    resiliparse_extensions = [
        Extension('resiliparse.itertools',
                  sources=[f'resiliparse/itertools.pyx']),
        Extension('resiliparse.extract.html2text',
                  sources=[f'resiliparse/extract/html2text.pyx'], libraries=['lexbor', 're2']),
        Extension('resiliparse.parse.encoding',
                  sources=[f'resiliparse/parse/encoding.pyx'], libraries=['uchardet', 'lexbor']),
        Extension('resiliparse.parse.html',
                  sources=[f'resiliparse/parse/html.pyx'], libraries=['lexbor']),
        Extension('resiliparse.parse.http',
                  sources=[f'resiliparse/parse/http.pyx']),
        Extension('resiliparse.parse.lang',
                  sources=[f'resiliparse/parse/lang.pyx']),
    ]
    if os.name == 'posix':
        # Process Guards are unsupported on Windows
        resiliparse_extensions.append(
            Extension('resiliparse.process_guard', sources=[f'resiliparse/process_guard.pyx'],
                      libraries=['pthread'])
        )

    return cythonize(resiliparse_extensions, **get_cython_args())


# Copy FastWARC headers
fastwarc_headers = glob.glob(os.path.join(ROOT_DIR, '..', 'fastwarc', 'fastwarc', "*.pxd"))
if fastwarc_headers:
    os.makedirs(os.path.join(ROOT_DIR, 'fastwarc'), exist_ok=True)
    [shutil.copy2(f, os.path.join(ROOT_DIR, 'fastwarc')) for f in fastwarc_headers]

setup(
    ext_modules=get_ext_modules(),
    cmdclass=dict(build_ext=resiliparse_build_ext),
    exclude_package_data={
        '': [] if 'sdist' in sys.argv else ['*.pxd', '*.pxi', '*.pyx', '*.h', '*.cpp']
    }
)
