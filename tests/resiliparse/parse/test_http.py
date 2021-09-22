from io import BytesIO
from resiliparse.parse.http import *


chunked_http = b'''c\r\n\
Resiliparse \r\n\
6\r\n\
is an \r\n\
8\r\n\
awesome \r\n\
5\r\n\
tool.\r\n\
0\r\n\
\r\n'''


def test_chunked_http():
    reader = BytesIO(chunked_http)
    decoded = b''
    while True:
        chunk = read_http_chunk(reader)
        if not chunk:
            break
        decoded += chunk

    assert decoded == b'Resiliparse is an awesome tool.'
    assert b''.join(iterate_http_chunks(BytesIO(chunked_http))) == b'Resiliparse is an awesome tool.'
