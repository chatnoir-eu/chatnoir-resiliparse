from collections.abc import Iterator
from typing import BinaryIO

__all__ = ["iterate_http_chunks", "read_http_chunk"]

def read_http_chunk(reader: BinaryIO) -> bytes: ...
def iterate_http_chunks(reader: BinaryIO) -> Iterator[bytes]: ...
