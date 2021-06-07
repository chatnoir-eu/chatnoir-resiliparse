# distutils: language = c++

import io
import zlib

cdef int BUFF_SIZE = 16384


class GZipStream(io.BufferedReader):
    def __init__(self, raw, buffer_size=io.DEFAULT_BUFFER_SIZE):
        super().__init__(raw, buffer_size)
        self.decomp_obj = None
        self._unused_data = None
        self._buf = b''

        self._init_decomp_obj()

    def _init_decomp_obj(self):
        self._unused_data = b''
        self.decomp_obj = zlib.decompressobj(16 + zlib.MAX_WBITS)

    def _fill_buffer(self, size):
        if not self._unused_data:
            self._buf = super().read(size)
        elif self._unused_data:
            self._buf = self._unused_data
            self._init_decomp_obj()
        else:
            self._buf = b''

    def read(self, size=BUFF_SIZE):
        self._fill_buffer(size)
        decomp = self.decomp_obj.decompress(self._buf)
        self._unused_data = self.decomp_obj.unused_data
        return decomp
