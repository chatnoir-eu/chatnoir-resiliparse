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

use super::*;
use std::io::Cursor;

#[test]
fn binary_chunks_and_extensions_preserve_payload_bytes() {
    let mut reader = Cursor::new(b"3;extension=x\r\n\0\xff\x80\r\n1\r\nx\r\n0\r\n\r\ntrailer");
    let chunks = iterate_http_chunks(&mut reader)
        .collect::<io::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(chunks, [vec![0, 255, 128], b"x".to_vec()]);
    assert_eq!(&reader.get_ref()[reader.position() as usize..], b"trailer");
}

#[test]
fn short_chunk_and_invalid_header_stop_without_fabricating_bytes() {
    assert_eq!(read_http_chunk(&mut Cursor::new(b"5\r\nabc")).unwrap(), b"abc");
    let mut reader = Cursor::new(b"invalid\r\nbody");
    assert!(read_http_chunk(&mut reader).unwrap().is_empty());
    assert_eq!(&reader.get_ref()[reader.position() as usize..], b"dy");
}
