from resiliparse.parse.encoding import *


def test_encoding_detection():
    det = EncodingDetector()

    det.update(b'\xff\xfeH\x00e\x00l\x00l\x00o\x00 \x00W\x00o\x00r\x00l\x00d\x00')
    assert det.encoding() == 'utf-16-le'
    det.update(b'\xff\xfeH\x00e\x00l\x00l\x00o\x00 \x00W\x00o\x00r\x00l\x00d\x00')
    assert det.encoding(html5_compatible=False) == 'utf-16'
    det.update(b'Autres temps, autres m\x9curs.')
    assert det.encoding() == 'cp1252'

    assert detect_encoding(b'\xc3\xa4\xc3\xb6\xc3\xbc') == 'utf-8'
    assert detect_encoding(b'Hello World') == 'cp1252'
    assert detect_encoding(b'Hello World', html5_compatible=False) == 'ascii'
    assert detect_encoding(b'Potrzeba jest matk\xb1 wynalazk\xf3w.') == 'iso8859-2'

    html = b"""<!doctype html><meta charset="iso-8859-1"><title>Foo</title><body></body>"""
    assert detect_encoding(html, html5_compatible=True) == 'cp1252'
    assert detect_encoding(html, html5_compatible=False) == 'ascii'

    html = b"""<!doctype html><meta charset="iso-8859-1"><title>\xc3\xa4\xc3\xb6\xc3\xbc</title><body></body>"""
    assert detect_encoding(html, from_html_meta=False) == 'utf-8'
    assert detect_encoding(html, from_html_meta=True) == 'cp1252'


def test_whatwg_encoding_mapping():
    assert map_encoding_to_html5('ascii') == 'cp1252'
    assert map_encoding_to_html5('iso-8859-1') == 'cp1252'
    assert map_encoding_to_html5('csisolatin9') == 'iso8859-15'
    assert map_encoding_to_html5('utf-7') == 'utf-8'
    assert map_encoding_to_html5('utf-8') == 'utf-8'
    assert map_encoding_to_html5('utf-16') == 'utf-16-le'
    assert map_encoding_to_html5('oops') == 'utf-8'


def test_bytes_to_str():
    bytestr = b'\xc3\x9cbung macht den Meister'
    assert bytes_to_str(bytestr, 'ascii') == 'Übung macht den Meister'
    assert bytes_to_str(bytestr, 'cp1252') == 'Ãœbung macht den Meister'
    assert bytes_to_str(bytestr, detect_encoding(bytestr)) == 'Übung macht den Meister'

    assert bytes_to_str(b'+Condensed', 'utf-7') == '+Condensed'

    # Erroneous but best-effort decoding without thrown exceptions
    assert bytes_to_str(b'+Condensed', 'utf-7', fallback_encodings=[]) == 'ઉ笞'
