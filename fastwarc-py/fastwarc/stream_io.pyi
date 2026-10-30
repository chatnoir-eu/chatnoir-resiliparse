# Copyright 2026 Janek Bevendorff
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

from os import PathLike
from types import TracebackType
from typing import BinaryIO, ContextManager, Protocol, Type, Self
from typing_extensions import disjoint_base

__all__ = [
    'WarcReader',
    'WarcWriter',
    'GzipReader',
    'GzipWriter',
    'ZstdReader',
    'ZstdWriter',
    'Lz4Reader',
    'Lz4Writer',
    'BrotliReader',
    'BrotliWriter',
    'ChunkedReader',
    'ChunkedWriter',
    'zstd_train_dictionary_from_continuous',
    'zstd_train_dictionary_from_files',
    'zstd_train_dictionary_from_samples',

]


class _GenericReader(Protocol):
    def read(self, size: int = ...) -> bytes: ...

    def seek(self, offset: int, whence: int = ...) -> int: ...

    def tell(self) -> int: ...

    def close(self) -> None: ...


class _GenericWriter(Protocol):
    def write(self, data: bytes) -> int: ...

    def flush(self) -> None: ...

    def close(self) -> None: ...


@disjoint_base
class WarcReader(ContextManager["WarcReader"]):
    def __new__(cls) -> Self: ...

    def read(self, size: int = ...) -> bytes: ...

    def seek(self, offset: int, whence: int = 0) -> int: ...

    def tell(self) -> int: ...

    def close(self) -> None: ...

    def __enter__(self) -> Self: ...

    def __exit__(
            self,
            exc_type: Type[BaseException] | None,
            exc: BaseException | None,
            traceback: TracebackType | None
    ) -> None: ...

    def inner_seek(self, offset: int, whence: int = 0) -> int: ...

    def inner_tell(self) -> int: ...

    def frame_start_position(self) -> int | None: ...


@disjoint_base
class WarcWriter(ContextManager["WarcWriter"]):
    def __new__(cls) -> Self: ...

    def write(self, data: bytes) -> int: ...

    def flush(self) -> None: ...

    def close(self) -> None: ...

    def __enter__(self) -> Self: ...

    def __exit__(
            self,
            exc_type: Type[BaseException] | None,
            exc: BaseException | None,
            traceback: TracebackType | None
    ) -> None: ...

    def finish(self) -> None: ...


_ReaderInput = WarcReader | BinaryIO | _GenericReader | PathLike[str] | str
_WriterInput = WarcWriter | BinaryIO | _GenericWriter | PathLike[str] | str


@disjoint_base
class GzipReader(WarcReader):
    def __new__(cls, inner: _ReaderInput, buffer_size: int = 64 << 10, zlib: bool = False,
                fsspec_args=None) -> Self: ...


@disjoint_base
class GzipWriter(WarcWriter):
    def __new__(cls, inner: _WriterInput, compression_level: int = 9, buffer_size: int = 8192, zlib=False,
                fsspec_args=None) -> Self: ...


@disjoint_base
class ZstdReader(WarcReader):
    def __new__(cls, inner: _ReaderInput, buffer_size: int = 64 << 10, fsspec_args=None,
                dictionary: bytes | None = None) -> Self: ...


@disjoint_base
class ZstdWriter(WarcWriter):
    def __new__(cls, inner: _WriterInput, buffer_size: int = 8192, compression_level: int = 3, fsspec_args=None,
                dictionary: bytes | None = None, compress_dictionary_frame=False) -> Self: ...


@disjoint_base
class Lz4Reader(WarcReader):
    def __new__(cls, inner: _ReaderInput, buffer_size: int = 64 << 10, fsspec_args=None) -> Self: ...


@disjoint_base
class Lz4Writer(WarcWriter):
    def __new__(cls, inner: _WriterInput, buffer_size: int = 8192, fsspec_args=None) -> Self: ...


@disjoint_base
class BrotliReader(WarcReader):
    def __new__(cls, inner: _ReaderInput, buffer_size: int = 64 << 10, fsspec_args=None) -> Self: ...


@disjoint_base
class BrotliWriter(WarcWriter):
    def __new__(cls, inner: _WriterInput, buffer_size: int = 8192, fsspec_args=None) -> Self: ...


@disjoint_base
class ChunkedReader(WarcReader):
    def __new__(cls, inner: _ReaderInput, buffer_size: int = 4096, fsspec_args=None) -> Self: ...


@disjoint_base
class ChunkedWriter(WarcWriter):
    def __new__(cls, inner: _WriterInput, min_chunk_size: int = 1024, fsspec_args=None) -> Self: ...


def zstd_train_dictionary_from_continuous(sample_data: bytes, sample_sizes: list[int], max_size: int) -> bytes: ...


def zstd_train_dictionary_from_samples(samples: list[bytes], max_size: int) -> bytes: ...


def zstd_train_dictionary_from_files(filenames: list[str], max_size: int) -> bytes: ...
