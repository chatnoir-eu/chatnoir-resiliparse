from typing import Optional, Sequence


class EncodingDetector:
    def update(self, data: bytes) -> None: ...
    def encoding(self, html5_compatible: bool = True) -> Optional[str]: ...
    def reset(self) -> None: ...


def detect_encoding(
    data: bytes,
    max_len: int = 131072,
    html5_compatible: bool = True,
    from_html_meta: bool = False,
) -> str: ...

def map_encoding_to_html5(encoding: str, fallback_utf8: bool = True) -> str: ...

def bytes_to_str(
    data: bytes,
    encoding: str = "utf-8",
    errors: str = "ignore",
    fallback_encodings: Optional[Sequence[str]] = None,
    strip_bom: bool = True,
) -> str: ...

def detect_mime(data: bytes, max_unprintable: float = 0.05) -> str: ...
