// Copyright 2026 Janek Bevendorff
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::stream_io::{DecompressingStream, ReadSeek};
use std::io;
use zlib_rs::Deflate;

pub struct GZipReader<T> {
    inner: T,
    deflate: Deflate,
}

impl<T: ReadSeek> GZipReader<T> {
    fn new_with_params(input_stream: T, comp_level: i32, window_bits: u8) -> Self {
        Self {
            inner: input_stream,
            deflate: Deflate::new(comp_level, false, window_bits),
        }
    }
}

impl<T: ReadSeek + Sized> DecompressingStream for GZipReader<T> {
    type Input = T;

    fn new(input_stream: T) -> Self {
        todo!()
    }
}

impl<T: ReadSeek> io::Read for GZipReader<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        todo!()
    }
}

impl<T: ReadSeek> io::Seek for GZipReader<T> {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }

    fn stream_position(&mut self) -> io::Result<u64> {
        self.inner.stream_position()
    }
}
