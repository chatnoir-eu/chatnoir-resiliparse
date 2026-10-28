from collections.abc import Sequence
from typing import Any, Optional


def detect_fast(
    text: str,
    cutoff: int = 1200,
    n_results: int = 1,
    langs: Optional[Sequence[str]] = None,
) -> list[tuple[str, float]]: ...

def supported_langs() -> list[str]: ...
def train_language_examples(examples: Any, vec_len: int = ...) -> Any: ...
