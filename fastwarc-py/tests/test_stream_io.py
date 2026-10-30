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

import io
import builtins
from pathlib import Path
import sys
import pytest
from fastwarc.stream_io import *


def get_fixtures_path():
    return Path(__file__).resolve().parents[2] / "fastwarc-rs" / "tests" / "fixtures"


TQBF = b"The quick brown fox jumps over the lazy dog."


# noinspection PyNoneFunctionAssignment
def test_stream_reader_writer_base_classes():
    reader = WarcReader()
    assert reader.read() is NotImplemented
    assert reader.seek(0) is NotImplemented
    assert reader.tell() is NotImplemented
    assert reader.inner_seek(0) is NotImplemented
    assert reader.inner_tell() is NotImplemented
    assert reader.frame_start_position() is None
    assert reader.close() is None
    with reader as r:
        assert r.read() is NotImplemented

    reader = WarcWriter()
    assert reader.write(b'payload') is NotImplemented
    assert reader.flush() is None
    assert reader.finish() is None

    assert reader.close() is None
    with reader as r:
        assert r.write(b'payload') is NotImplemented


@pytest.mark.parametrize(
    ("reader_cls", "writer_cls", "reader_supports_frame_position"),
    [
        pytest.param(GzipReader, GzipWriter, True, id="gzip"),
        pytest.param(BrotliReader, BrotliWriter, False, id="brotli"),
        pytest.param(Lz4Reader, Lz4Writer, True, id="lz4"),
        pytest.param(ZstdReader, ZstdWriter, True, id="zstd"),
        pytest.param(ChunkedReader, ChunkedWriter, False, id="chunked"),
    ],
)
def test_stream_reader_writer(reader_cls, writer_cls, reader_supports_frame_position):
    payload = b"alpha\nbeta\n"
    raw = io.BytesIO()

    writer = writer_cls(raw)
    assert writer.write(payload) == len(payload)
    writer.flush()
    writer.finish()
    writer.close()

    encoded = raw.getvalue()
    assert encoded

    with reader_cls(io.BytesIO(encoded)) as reader:
        assert reader.tell() == 0
        assert reader.seek(0) == 0
        assert reader.read(6) == b"alpha\n"
        assert reader.tell() == 6
        assert reader.read() == b"beta\n"
        assert isinstance(reader.inner_tell(), int)
        if reader_supports_frame_position:
            assert reader.frame_start_position() == 0
        else:
            assert reader.frame_start_position() is None
        assert reader.inner_seek(0) == 0


def test_chunked_reader_writer():
    payload = b"alpha\nbeta\n"
    raw = io.BytesIO()

    with ChunkedWriter(raw, min_chunk_size=1) as writer:
        assert writer.write(payload[:5]) == 5
        assert writer.write(payload[5:]) == len(payload) - 5
        writer.flush()
        writer.finish()

    encoded = raw.getvalue()
    assert encoded.startswith(b"5\r\nalpha\r\n")

    with ChunkedReader(io.BytesIO(encoded)) as reader:
        assert reader.seek(0) == 0
        assert reader.read(6) == b"alpha\n"
        assert reader.tell() == 6
        assert reader.inner_seek(0) == 0
        assert isinstance(reader.inner_tell(), int)


def test_stream_io_context_manager():
    payload = b"context manager payload"
    raw = io.BytesIO()

    with GzipWriter(raw) as writer:
        assert writer.write(payload) == len(payload)
        writer.flush()
        writer.finish()

    with GzipReader(io.BytesIO(raw.getvalue())) as reader:
        assert reader.read() == payload


def test_stream_reader_seek_with_whence():
    payload = b"alpha\nbeta\ngamma"
    raw = io.BytesIO()

    with GzipWriter(raw) as writer:
        assert writer.write(payload) == len(payload)
        writer.finish()

    with GzipReader(io.BytesIO(raw.getvalue())) as reader:
        assert reader.seek(6, 0) == 6
        assert reader.seek(2, 1) == 8
        assert reader.read(2) == b"ta"
        with pytest.raises(OSError) as e:
            assert reader.seek(-5, 2)
        assert 'Seeking from end not supported' == e.value.args[0]
        assert reader.read() == b"\ngamma"


class _ForwardingReader:
    """
    Wrapper type for BytesIO that triggers the PythonReaderAdapter paths
    and can return bytes and bytearray.
    """

    def __init__(self, data, return_bytearray=False):
        self._raw = io.BytesIO(data)
        self._return_bytearray = return_bytearray

    def read(self, size=-1):
        data = self._raw.read(size)
        return bytearray(data) if self._return_bytearray else data

    def seek(self, offset, whence=0):
        return self._raw.seek(offset, whence)

    def tell(self):
        return self._raw.tell()

    def close(self):
        self._raw.close()


class _ForwardingWriter:
    """
    Wrapper type for BytesIO that triggers the PythonReaderAdapter
    and keeps track of the number of flush calls.
    """

    def __init__(self):
        self._raw = io.BytesIO()
        self.flush_count = 0
        self.closed = False

    def write(self, data):
        return self._raw.write(data)

    def flush(self):
        self.flush_count += 1

    def close(self):
        self.closed = True

    def getvalue(self):
        return self._raw.getvalue()


@pytest.mark.parametrize(
    ("reader_cls", "writer_cls"),
    [
        pytest.param(GzipReader, GzipWriter, id="gzip"),
        pytest.param(ZstdReader, ZstdWriter, id="zstd"),
        pytest.param(Lz4Reader, Lz4Writer, id="lz4"),
    ],
)
def test_native_stream_adapter(reader_cls, writer_cls):
    payload = b"native adapter payload\n" * 8
    raw = io.BytesIO()

    with writer_cls(writer_cls(raw, buffer_size=128), buffer_size=64) as writer:
        assert writer.write(payload) == len(payload)
        writer.flush()
        writer.finish()

    with reader_cls(reader_cls(io.BytesIO(raw.getvalue()), buffer_size=97), buffer_size=31) as reader:
        assert reader.read(7) == payload[:7]
        assert reader.tell() == 7
        with pytest.raises(OSError) as e:
            reader.seek(0)
        assert 'Backward seeking not supported' == e.value.args[0]
        assert reader.read() == payload[7:]
        assert isinstance(reader.inner_tell(), int)


@pytest.mark.parametrize("return_bytearray", [False, True])
def test_forwarding_stream_adapter_paths(return_bytearray):
    payload = b"forwarded payload"
    writer_raw = _ForwardingWriter()
    with GzipWriter(writer_raw) as writer:
        assert writer.write(payload) == len(payload)
        writer.flush()
        writer.finish()
    assert writer_raw.flush_count > 0

    with GzipReader(_ForwardingReader(writer_raw.getvalue(), return_bytearray=return_bytearray),
                    buffer_size=17) as reader:
        assert reader.tell() == 0
        assert reader.read(5) == payload[:5]
        assert reader.tell() == 5
        with pytest.raises(OSError) as e:
            reader.seek(0)
        assert 'Backward seeking not supported' == e.value.args[0]
        assert reader.inner_seek(0) == 0
        assert reader.read() == payload


def test_stream_io_fsspec_memory_write():
    pytest.importorskip("fsspec")

    url = "memory://fastwarc-pytest-stream.gz"
    payload = b"fsspec payload"

    with GzipWriter(url) as writer:
        assert writer.write(payload) == len(payload)
        writer.flush()
        writer.finish()

    with GzipReader(url) as reader:
        assert reader.read() == payload


def test_stream_io_fsspec_with_args_dict(tmp_path: Path):
    pytest.importorskip("fsspec")

    payload = b"fsspec args payload"
    nested_path = tmp_path / "nested" / "stream.gz"
    url = nested_path.resolve().as_uri()

    with GzipWriter(url, fsspec_args={"auto_mkdir": True}) as writer:
        assert writer.write(payload) == len(payload)
        writer.finish()

    assert nested_path.exists()
    with GzipReader(url, fsspec_args={"auto_mkdir": True}) as reader:
        assert reader.read() == payload


def test_stream_io_fsspec_read_from_file():
    pytest.importorskip("fsspec")

    fixture = get_fixtures_path() / "warcfile.warc.zst"
    url = fixture.resolve().as_uri()
    assert url.startswith("file://")

    with ZstdReader(url) as reader:
        prefix = reader.read(5)
        assert prefix == b"WARC/"
        assert reader.read()


def test_stream_io_fsspec_false_to_disable():
    with pytest.raises(OSError):
        GzipReader("memory://fastwarc-pytest-missing.gz", fsspec_args=False)


def test_stream_io_fsspec_import_error_is_propagated(monkeypatch):
    real_import = builtins.__import__

    def fake_import(name, *args, **kwargs):
        if name == "fsspec":
            raise RuntimeError("boom")
        return real_import(name, *args, **kwargs)

    monkeypatch.delitem(sys.modules, "fsspec", raising=False)
    monkeypatch.setattr(builtins, "__import__", fake_import)

    with pytest.raises(RuntimeError, match="boom"):
        GzipReader("memory://fastwarc-pytest-import-error.gz")


@pytest.mark.parametrize(
    ("dict_train_data", "train_dictionary",),
    [
        pytest.param(None, lambda _: None, id="no-dictionary"),
        pytest.param(
            TQBF * 20,
            lambda r: zstd_train_dictionary_from_continuous(r, [len(TQBF)] * 20, 100000),
            id="continuous",
        ),
        pytest.param(
            [TQBF] * 20,
            lambda r: zstd_train_dictionary_from_samples(r, 100000),
            id="samples",
        ),
        pytest.param(
            get_fixtures_path() / "tqbf.txt",
            lambda r: zstd_train_dictionary_from_files([str(r)] * 8, 100000),
            id="files",
        ),
    ],
)
def test_zstd_dictionary_roundtrip(dict_train_data, train_dictionary):
    raw_bytes = TQBF * 200

    out = io.BytesIO()
    d = train_dictionary(dict_train_data)
    with ZstdWriter(out, dictionary=d) as w:
        w.write(raw_bytes)
    out.seek(0)

    # Test dictionary frame loading
    r = ZstdReader(out)
    assert r.read() == raw_bytes

    out.seek(0)
    # Test explicit dictionary
    r = ZstdReader(out, dictionary=d)
    assert r.read() == raw_bytes
