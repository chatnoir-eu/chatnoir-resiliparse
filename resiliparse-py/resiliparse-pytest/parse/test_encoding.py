from resiliparse.parse.encoding import bytes_to_str


def test_bytes_to_str_python_codec_fallback_behavior():
    data = b'\xc3\x9cbung macht den Meister'
    assert bytes_to_str(data, 'ascii') == 'Übung macht den Meister'
    assert bytes_to_str(data, 'cp1252') == 'Ãœbung macht den Meister'

    # Unsupported data is decoded with the caller-provided fallback list.
    assert bytes_to_str(b'+Condensed', 'utf-7', fallback_encodings=[]) == 'ઉ笞'
