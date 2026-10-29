"""Representative synthetic inputs with fixed legacy UTF-8 output bytes."""
import inspect

import pytest
from resiliparse.parse import encoding


@pytest.mark.parametrize(('function', 'data', 'options', 'expected'), [
    pytest.param(
        encoding.map_encoding_to_html5, ' \tASCII\n', {'fallback_utf8': False}, b'cp1252',
        id='label-normalization',
    ),
    pytest.param(
        encoding.map_encoding_to_html5, 'unknown-codec', {'fallback_utf8': False}, None,
        id='unknown-label-without-fallback',
    ),
    pytest.param(
        encoding.detect_encoding, b'<meta charset="ISO-8859-1">\xc3\xa4',
        {'from_html_meta': True, 'html5_compatible': False}, b'cp1252',
        id='meta-label-always-remapped',
    ),
    pytest.param(
        encoding.detect_encoding, b'prefix' + b'\xc3\xa4' * 30 + b'suffix',
        {'max_len': 3, 'html5_compatible': False}, b'ascii',
        id='odd-limit-samples-both-ends',
    ),
    pytest.param(
        encoding.bytes_to_str, b'\xfe\xff\0H\0i',
        {'encoding': 'utf-16-le', 'strip_bom': True}, b'Hi',
        id='bom-overrides-byte-order',
    ),
    pytest.param(
        encoding.bytes_to_str, b'\xef\xbb\xbfhello', {'strip_bom': False}, b'\xef\xbb\xbfhello',
        id='retain-bom',
    ),
    pytest.param(
        encoding.bytes_to_str, b'a\xffb', {'errors': 'replace', 'fallback_encodings': []},
        b'a\xef\xbf\xbdb', id='replace-invalid-bytes-without-fallbacks',
    ),
    pytest.param(
        encoding.detect_mime, b'RIFF....WAVE', {}, b'image/webp',
        id='mime-signature-precedence',
    ),
    pytest.param(
        encoding.detect_mime, b'a' * 19 + b'\0', {'max_unprintable': 0.05}, b'text/plain',
        id='binary-ratio-at-threshold',
    ),
    pytest.param(
        encoding.detect_mime, b'a' * 18 + b'\0\xff', {'max_unprintable': 0.05}, b'application/octet-stream',
        id='binary-ratio-above-threshold',
    ),
])
def test_synthetic_encoding_compatibility(function, data, options, expected):
    result = function(data, **options)
    assert (None if result is None else result.encode('utf-8')) == expected


def test_native_encoding_module():
    assert encoding.EncodingDetector.__module__ == 'resiliparse.parse.encoding'
    assert encoding.__all__ == [
        'EncodingDetector', 'bytes_to_str', 'detect_encoding', 'detect_mime', 'map_encoding_to_html5',
    ]
    for name in encoding.__all__[1:]:
        assert inspect.isbuiltin(getattr(encoding, name))


def test_incremental_detector_updates_and_resets():
    detector = encoding.EncodingDetector()
    assert detector.encoding().encode('utf-8') == b'utf-8'
    assert detector.encoding(html5_compatible=False) is None

    # Feed a UTF-8 character split across updates, then start a fresh stream.
    detector.update(b'\xc3')
    detector.update(b'\xa4\xc3\xb6')
    detector.update(b'\xc3\xbc')
    assert detector.encoding().encode('utf-8') == b'utf-8'
    assert detector.encoding(html5_compatible=False) is None

    detector.update(b'\xff\xfeH\0i\0')
    detector.reset()
    detector.update(b'Hello')
    assert detector.encoding(html5_compatible=False).encode('utf-8') == b'ascii'


def test_detector_supports_python_subclasses():
    class Detector(encoding.EncodingDetector):
        pass

    detector = Detector()
    detector.update(b'Hello world')
    assert detector.encoding().encode('utf-8') == b'cp1252'


def test_decode_consumes_fallback_iterables_before_primary_attempt():
    def fallbacks():
        yield 'utf-8'
        raise RuntimeError('synthetic iterable failure')

    with pytest.raises(RuntimeError, match='synthetic iterable failure'):
        encoding.bytes_to_str(b'valid UTF-8', fallback_encodings=fallbacks())

    assert encoding.bytes_to_str(b'\xff', fallback_encodings=iter(['utf-8', 'cp1252'])).encode('utf-8') == b'\xc3\xbf'
