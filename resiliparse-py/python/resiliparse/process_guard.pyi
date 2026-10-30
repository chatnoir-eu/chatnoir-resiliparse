from typing import Any, Callable, Iterator, Optional, TypeVar
from typing_extensions import final

__all__ = [
    "mem_guard",
    "progress",
    "progress_loop",
    "time_guard",
    "ExecutionTimeout",
    "ResiliparseGuardException",
    "MemGuard",
    "MemoryLimitExceeded",
    "TimeGuard",
    "InterruptType",
]

@final
class InterruptType:
    exception: InterruptType
    signal: InterruptType
    exception_then_signal: InterruptType

class ResiliparseGuardException(BaseException): ...
class ExecutionTimeout(ResiliparseGuardException): ...
class MemoryLimitExceeded(ResiliparseGuardException): ...

_F = TypeVar("_F", bound=Callable[..., Any])

class TimeGuard:
    def progress(self) -> None: ...
    def __call__(self, func: _F) -> _F: ...
    def __enter__(self) -> TimeGuard: ...
    def __exit__(self, exc_type: Any, exc_value: Any, traceback: Any) -> bool: ...

class MemGuard:
    def __call__(self, func: _F) -> _F: ...
    def __enter__(self) -> MemGuard: ...
    def __exit__(self, exc_type: Any, exc_value: Any, traceback: Any) -> bool: ...

def time_guard(
    timeout: Optional[int] = ..., timeout_ms: Optional[int] = ...,
    grace_period: Optional[int] = ..., grace_period_ms: Optional[int] = ...,
    interrupt_type: InterruptType = ..., send_kill: bool = ..., check_interval: int = ...,
) -> TimeGuard: ...

def mem_guard(
    max_memory: int, absolute: bool = ..., grace_period: int = ...,
    grace_period_ms: Optional[int] = ..., secondary_grace_period: int = ...,
    secondary_grace_period_ms: Optional[int] = ..., interrupt_type: InterruptType = ...,
    send_kill: bool = ..., check_interval: int = ...,
) -> MemGuard: ...

def progress(ctx: Any = ...) -> None: ...
def progress_loop(it: Any, ctx: Any = ...) -> Iterator[Any]: ...
