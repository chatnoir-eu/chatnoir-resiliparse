import gzip
import io
import os
import tempfile

import brotli
import lz4.frame
import pytest

import fastwarc.stream_io as sio

DATA_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', 'data'))


# noinspection PyProtectedMember
def validate_stream_io(stream):
    data = b'Hello World'
    size = sio._io_stream_py_test_write(stream, data)
    assert size == len(data)

    sio._io_stream_py_test_flush(stream)
    assert sio._io_stream_py_test_tell(stream) == len(data)
    sio._io_stream_py_test_seek(stream, 0)
    assert sio._io_stream_py_test_tell(stream) == 0

    data_read, size = sio._io_stream_py_test_read(stream, len(data))
    assert data_read == data
    assert size == len(data)

    data_read, size = sio._io_stream_py_test_read(stream, 1)
    assert data_read == b''
    assert size == 0

    sio._io_stream_py_test_seek(stream, data.index(b' ') + 1)
    assert sio._io_stream_py_test_tell(stream) == data.index(b' ') + 1
    data_read, size = sio._io_stream_py_test_read(stream, 1000)
    assert data_read == b'World'
    assert size == 5

    sio._io_stream_py_test_close(stream)
    with pytest.raises(ValueError):
        sio._io_stream_py_test_write(stream, b'abc')

    with pytest.raises(ValueError):
        sio._io_stream_py_test_seek(stream, 0)

    with pytest.raises(ValueError):
        sio._io_stream_py_test_tell(stream)


def test_file_stream():
    stream = sio.FileStream(os.path.join(DATA_DIR, 'warcfile.warc'), 'rb')
    data, size = sio._io_stream_py_test_read(stream, 5)
    assert size == 5
    assert data == b'WARC/'

    data, size = sio._io_stream_py_test_read(stream, 3)
    assert size == 3
    assert data == b'1.0'

    sio._io_stream_py_test_close(stream)
    with pytest.raises(ValueError):
        sio._io_stream_py_test_read(stream, 1)

    with tempfile.TemporaryDirectory() as tmpdir:
        validate_stream_io(sio.FileStream(os.path.join(tmpdir, 'testfile.txt'), 'w+'))


def test_bytes_io_stream():
    validate_stream_io(sio.BytesIOStream(b''))


def test_python_io_stream_adapter():
    validate_stream_io(sio.PythonIOStreamAdapter(io.BytesIO()))


# noinspection PyProtectedMember
def validate_compressing_stream(raw_stream, comp_stream_cls, comp_val_func, decomp_val_func, raises=True):
    # Compression
    in_value = b'Hello World'
    comp_stream = comp_stream_cls(raw_stream)
    sio._io_stream_py_test_write(comp_stream, in_value)
    sio._io_stream_py_test_flush(comp_stream)
    sio._io_stream_py_test_close(comp_stream)

    # Decompression
    sio._io_stream_py_test_seek(raw_stream, 0)
    out_value, out_size = sio._io_stream_py_test_read(raw_stream, 1024)
    assert out_size != len(in_value)
    assert out_value != in_value
    assert decomp_val_func(out_value) == in_value

    sio._io_stream_py_test_seek(raw_stream, 0)
    comp_stream = comp_stream_cls(raw_stream)
    out_value, out_size = sio._io_stream_py_test_read(comp_stream, 1024)
    assert out_size == len(in_value)
    assert out_value == in_value

    # Data compressed by external compressor
    comp_stream = comp_stream_cls(sio.BytesIOStream(comp_val_func(in_value)))
    out_value, out_size = sio._io_stream_py_test_read(comp_stream, 1024)
    assert out_size == len(in_value)
    assert out_value == in_value

    # Invalid stream
    if raises:
        sio._io_stream_py_test_seek(raw_stream, 0)
        sio._io_stream_py_test_write(raw_stream, b'\x00\x00\x00\x00\x00\x00\x00')
        with pytest.raises(sio.StreamError):
            comp_stream = comp_stream_cls(raw_stream)
            sio._io_stream_py_test_read(comp_stream, 1024)


def test_gzip_stream():
    validate_compressing_stream(sio.BytesIOStream(b''),
                                sio.GZipStream,
                                gzip.compress,
                                gzip.decompress)


def test_lz4_stream():
    validate_compressing_stream(sio.BytesIOStream(b''),
                                sio.LZ4Stream,
                                lz4.frame.compress,
                                lz4.frame.decompress)


def test_brotli_stream():
    validate_compressing_stream(sio.BytesIOStream(b''),
                                sio.BrotliStream,
                                brotli.compress,
                                brotli.decompress,
                                raises=False)


# noinspection PyProtectedMember
def validate_buf_reader_on_warc(reader, uncompressed):
    # Lines and byte blocks
    sio._buf_reader_py_test_detect_stream_type(reader)
    assert reader.tell() == 0
    assert reader.readline() == b'WARC/1.0\r\n'
    if uncompressed:
        assert reader.tell() == 10
    assert reader.read(9) == b'WARC-Type'
    if uncompressed:
        assert reader.tell() == 19

    # Limited reader
    sio._buf_reader_py_test_set_limit(reader, 512)
    assert len(reader.read(1024)) == 512
    assert reader.read(1024) == b''
    pos = reader.tell()
    assert reader.read() == b''
    assert reader.tell() == pos
    sio._buf_reader_py_test_reset_limit(reader)
    assert len(reader.read(1024)) == 1024

    # Stream consumption
    pos = reader.tell()
    reader.consume(8)
    if uncompressed:
        assert reader.tell() == pos + 8
    assert len(reader.read(10)) == 10
    reader.consume()
    assert reader.read() == b''
    assert reader.read(1024) == b''
    assert reader.tell() > pos

    reader.close()


def test_buffered_reader():
    stream = sio.FileStream(os.path.join(DATA_DIR, 'warcfile.warc'), 'rb')
    validate_buf_reader_on_warc(sio.BufferedReader(stream), True)

    # Test stream negotiation
    stream = sio.FileStream(os.path.join(DATA_DIR, 'warcfile.warc.gz'), 'rb')
    reader = sio.BufferedReader(stream)
    sio._buf_reader_py_test_detect_stream_type(reader)
    validate_buf_reader_on_warc(reader, False)

    stream = sio.FileStream(os.path.join(DATA_DIR, 'warcfile.warc.lz4'), 'rb')
    reader = sio.BufferedReader(stream)
    validate_buf_reader_on_warc(reader, False)

    # Invalid stream
    with pytest.raises(sio.StreamError):
        stream = sio.BytesIOStream(b'\x00\x00\x00\x00\x00\x00\x00\x00')
        reader = sio.BufferedReader(stream)
        sio._buf_reader_py_test_detect_stream_type(reader)
