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
    # The Rust core tests chunk parsing, framing, and termination. This checks
    # that the Python iterator wrapper exposes the same payload bytes.
    assert b''.join(iterate_http_chunks(BytesIO(chunked_http))) == b'Resiliparse is an awesome tool.'
