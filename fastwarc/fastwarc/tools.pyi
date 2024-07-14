from enum import IntFlag
from typing import Union, Type, Iterator, Tuple

from .stream_io import IOStream
from .warc import WarcRecord


class CompressionAlg(IntFlag):
    gzip = 0
    lz4 = 1
    uncompressed = 2
    auto = 3


def detect_compression_algorithm(file: str) -> CompressionAlg: ...


def wrap_warc_stream(
    file: Union[str, Type[IOStream]],
    mode: str,
    comp_alg: CompressionAlg = CompressionAlg.auto,
    **comp_args
) -> Type[IOStream]: ...


def recompress_warc_interactive(
    warc_in: Union[str, Type[IOStream]],
    warc_out: Union[str, Type[IOStream]],
    comp_alg_in: CompressionAlg = CompressionAlg.auto,
    comp_alg_out: CompressionAlg = CompressionAlg.auto,
    **comp_args
) -> Iterator[Tuple[WarcRecord, int]]: ...


def recompress_warc(
    warc_in: Union[str, Type[IOStream]],
    warc_out: Union[str, Type[IOStream]],
    comp_alg_in: CompressionAlg = CompressionAlg.auto,
    comp_alg_out: CompressionAlg = CompressionAlg.auto,
    **comp_args
) -> Iterator[Tuple[WarcRecord, int]]: ...


def verify_digests(
    warc_in: Union[str, Type[IOStream]],
    verify_payloads: bool = False,
    comp_alg: CompressionAlg = CompressionAlg.auto,
) -> bool: ...
