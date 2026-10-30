from typing import Any, Callable, Iterable, Iterator, Optional, Tuple, TypeVar

__all__ = ["exc_loop", "warc_retry"]

_T = TypeVar("_T")

def exc_loop(it: Iterable[_T]) -> Iterator[Tuple[Optional[_T], Optional[BaseException]]]: ...
def warc_retry(
    archive_iterator: Any,
    stream_factory: Callable[..., Any],
    retry_count: Optional[int] = ...,
    seek: Optional[bool] = ...,
) -> Iterator[Any]: ...

