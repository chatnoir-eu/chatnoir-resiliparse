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

# Legacy shims for making the new Rust extension work with the old type names

import sys
import warnings

if sys.version_info >= (3, 13):
    from warnings import deprecated
else:
    from typing_extensions import deprecated

__all__ = [
    'wrap_stream',
    'BufferedReader',
    'IOStream',
    'PythonIOStreamAdapter',
    'CompressingStream',
    'FileStream',
    'BytesIOStream',
    'GZipStream',
    'LZ4Stream',
    'BrotliStream',
    'FastWARCError',
    'StreamError',
    'ReaderStaleError',
]


@deprecated("Use OSError instead.", category=FutureWarning)
class FastWARCError(OSError): ...


@deprecated("Use OSError instead.", category=FutureWarning)
class StreamError(OSError): ...


@deprecated("Use OSError instead.", category=FutureWarning)
class ReaderStaleError(OSError): ...


@deprecated("wrap_stream is deprecated and will be removed in a future version.", category=FutureWarning)
def wrap_stream(raw_stream, mode='rb', fsspec_args=None):
    if isinstance(raw_stream, str):
        if fsspec_args is not False and '://' in raw_stream:
            try:
                import fsspec
                return fsspec.open(raw_stream, mode, **(fsspec_args or {})).open()
            except ModuleNotFoundError:
                pass
        return open(raw_stream, mode)

    return raw_stream


@deprecated("BufferedReader is deprecated and will be removed in a future version.", category=FutureWarning)
class BufferedReader:
    def __init__(self, stream, *args, **kwargs):
        self._stream = stream

    def close(self):
        return self._stream.close()

    def consume(self, size: int = -1):
        return self._stream.consume(size)

    def read(self, size: int = -1):
        return self._stream.read(size)

    def readline(self, crlf=True, max_line_len=8192):
        return self._stream.readLine(max_line_len)

    def tell(self):
        return self._stream.tell()


class IOStream:
    __deprecated__ = "Use the new Reader and Writer classes from stream_io instead."

    def __new__(cls, *args, **kwargs):
        if cls is IOStream:
            warnings.warn(cls.__deprecated__, DeprecationWarning, stacklevel=2)
        return super().__new__(cls)

    def __init__(self, *_, _reader_factory=None, _writer_factory=None, **__):
        self._reader_factory = _reader_factory
        self._writer_factory = _writer_factory
        self._reader = None
        self._writer = None

    def _get_stream(self):
        if self._reader is not None:
            return self._reader
        if self._writer is not None:
            return self._writer
        stream = self._get_reader()
        if stream is None:
            stream = self._get_writer()
        return stream

    def _get_reader(self):
        if self._reader is None:
            self._reader = self._reader_factory()
        return self._reader

    def _get_writer(self):
        if self._writer is None:
            self._writer = self._writer_factory()
        return self._writer

    def read(self, size: int) -> bytes:
        return self._get_reader().read(size)

    def write(self, data: bytes) -> int:
        return self._get_writer().write(data)

    def close(self):
        self._get_stream().close()

    def flush(self):
        self._get_stream().flush()

    def seek(self, offset: int):
        self._get_stream().seek(offset)

    def tell(self) -> int:
        return self._get_stream().tell()

    def __enter__(self):
        s = self._get_stream()
        if hasattr(s, '__enter__'):
            return self._get_stream().__enter__()
        return self

    def __exit__(self, exc_type, exc, traceback):
        s = self._get_stream()
        if hasattr(s, '__exit__'):
            self._get_stream().__exit__(exc_type, exc, traceback)
        s.close()


@deprecated("Use the new Reader and Writer classes from stream_io instead.", category=FutureWarning)
class CompressingStream(IOStream):
    def begin_member(self):
        pass

    def end_member(self):
        self._get_writer().finish()
        self._writer = None


@deprecated("Use the plain Python stream object without any wrapper.", category=FutureWarning)
class PythonIOStreamAdapter:
    def __new__(cls, py_stream):
        return IOStream(_reader_factory=lambda: py_stream,
                        _writer_factory=lambda: py_stream)


@deprecated("Use plain file paths instead.", category=FutureWarning)
class FileStream:
    def __new__(cls, filename: str, mode: str = 'rb'):
        if 'b' not in mode:
            mode += 'b'
        f = open(filename, mode)
        return IOStream(_reader_factory=lambda: f, _writer_factory=lambda: f)


@deprecated("Use io.BytesIO instead.", category=FutureWarning)
class BytesIOStream:
    def __new__(cls, initial_data=None):
        import io
        bio = io.BytesIO(initial_data if initial_data is not None else b'')
        return IOStream(_reader_factory=lambda: bio, _writer_factory=lambda: bio)


@deprecated("Use the new GzipReader and GzipWriter classes from stream_io instead.", category=FutureWarning)
class GZipStream:
    def __new__(cls, raw_stream, compression_level=9, zlib=False, fsspec_args=None):
        from ..stream_io import GzipReader, GzipWriter
        stream = CompressingStream(
            _reader_factory=lambda: GzipReader(raw_stream, zlib=zlib, fsspec_args=fsspec_args),
            _writer_factory=lambda: GzipWriter(raw_stream, compression_level=compression_level, zlib=zlib,
                                               fsspec_args=fsspec_args))
        stream._raw_stream = raw_stream
        return stream


@deprecated("Use the new Lz4Reader and Lz4Writer classes from stream_io instead.", category=FutureWarning)
class LZ4Stream:
    def __new__(cls, raw_stream, mode='r', compression_level=12,
                favor_dec_speed=True, fsspec_args=None):
        from ..stream_io import Lz4Reader, Lz4Writer
        stream = CompressingStream(
            _reader_factory=lambda: Lz4Reader(raw_stream, fsspec_args=fsspec_args),
            _writer_factory=lambda: Lz4Writer(raw_stream, fsspec_args=fsspec_args))
        stream._raw_stream = raw_stream
        return stream


@deprecated("Use the new BrotliReader and BrotliWriter classes from stream_io instead.", category=FutureWarning)
class BrotliStream:
    def __new__(cls, raw_stream, quality=11, lgwin=22, lgblock=0, fsspec_args=None):
        from ..stream_io import BrotliReader, BrotliWriter
        stream = CompressingStream(
            _reader_factory=lambda: BrotliReader(raw_stream, fsspec_args=fsspec_args),
            _writer_factory=lambda: BrotliWriter(raw_stream, fsspec_args=fsspec_args))
        stream._raw_stream = raw_stream
        return stream
