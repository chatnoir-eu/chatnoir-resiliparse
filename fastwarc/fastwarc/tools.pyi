from enum import IntFlag
from typing import Union, Iterator, Tuple, Protocol

from .stream_io import IOStream, _GenericIOStream
from .warc import WarcRecord


class CompressionAlg(IntFlag):
    gzip = 0
    lz4 = 1
    uncompressed = 2
    auto = 3


def detect_compression_algorithm(file: str) -> CompressionAlg: ...


def wrap_warc_stream(
    file: Union[str, IOStream, _GenericIOStream],
    mode: str,
    comp_alg: CompressionAlg = CompressionAlg.auto,
    **comp_args
) -> IOStream: ...


def recompress_warc_interactive(
    warc_in: Union[str, IOStream, _GenericIOStream],
    warc_out: Union[str, IOStream, _GenericIOStream],
    comp_alg_in: CompressionAlg = CompressionAlg.auto,
    comp_alg_out: CompressionAlg = CompressionAlg.auto,
    **comp_args
) -> Iterator[Tuple[WarcRecord, int]]: ...


def recompress_warc(
    warc_in: Union[str, IOStream, _GenericIOStream],
    warc_out: Union[str, IOStream, _GenericIOStream],
    comp_alg_in: CompressionAlg = CompressionAlg.auto,
    comp_alg_out: CompressionAlg = CompressionAlg.auto,
    **comp_args
) -> Iterator[Tuple[WarcRecord, int]]: ...


def verify_digests(
    warc_in: Union[str, IOStream],
    verify_payloads: bool = False,
    comp_alg: CompressionAlg = CompressionAlg.auto,
) -> bool: ...
