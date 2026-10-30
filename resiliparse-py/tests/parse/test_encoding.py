import pytest

from resiliparse.parse.encoding import bytes_to_str, detect_encoding, map_encoding_to_html5


@pytest.mark.parametrize(('label', 'expected'), [
    *[(f'windows-{code}', f'cp{code}') for code in range(1250, 1259)],
    *[(f'iso-8859-{code}', f'iso8859-{code}') for code in [2, 3, 4, 5, 6, 7, 8, 10, 11, 13, 14, 15, 16]],
    ('ibm866', 'cp866'),
    ('euc-jp', 'euc_jp'),
    ('euc-kr', 'euc_kr'),
    ('iso-2022-jp', 'iso2022_jp'),
    ('macintosh', 'mac-roman'),
    ('utf-16le', 'utf-16-le'),
    ('utf-16be', 'utf-16-be'),
    ('shift_jis', 'shift_jis'),
    ('utf-8', 'utf-8'),
])
def test_python_encoding_names(label, expected):
    assert map_encoding_to_html5(label, fallback_utf8=False) == expected
    html = f'<meta charset="{label}">'.encode('ascii')
    assert detect_encoding(html, from_html_meta=True) == expected


def test_bytes_to_str_python_codec_fallback_behavior():
    data = b'\xc3\x9cbung macht den Meister'
    assert bytes_to_str(data, 'ascii') == 'Übung macht den Meister'
    assert bytes_to_str(data, 'cp1252') == 'Ãœbung macht den Meister'

    # Unsupported data is decoded with the caller-provided fallback list.
    assert bytes_to_str(b'+Condensed', 'utf-7', fallback_encodings=[]) == 'ઉ笞'
