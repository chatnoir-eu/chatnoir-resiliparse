// Copyright 2026 Kristian Rickert
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

#[test]
fn empty_read_does_not_wait_for_input() {
    let (_tx, rx) = mpsc::channel(1);
    let mut reader = ChannelReader::new(rx);
    assert_eq!(reader.read(&mut []).unwrap(), 0);
}

#[test]
fn fragmented_stream_head_is_coalesced_for_detection() {
    let (tx, rx) = mpsc::channel(3);
    tx.try_send(Bytes::from_static(b"\x1f")).unwrap();
    tx.try_send(Bytes::from_static(b"\x8b\x08")).unwrap();
    tx.try_send(Bytes::from_static(b"\x00payload")).unwrap();
    drop(tx);

    let mut reader = ChannelReader::new(rx);
    assert_eq!(reader.fill_buf().unwrap(), b"\x1f\x8b\x08\x00payload");
}

#[test]
fn short_stream_returns_available_prefix_at_eof() {
    let (tx, rx) = mpsc::channel(1);
    tx.try_send(Bytes::from_static(b"WA")).unwrap();
    drop(tx);

    let mut reader = ChannelReader::new(rx);
    assert_eq!(reader.fill_buf().unwrap(), b"WA");
}

#[test]
fn seek_accepts_current_position_only() {
    let (tx, rx) = mpsc::channel(1);
    tx.try_send(Bytes::from_static(b"WARC")).unwrap();
    drop(tx);

    let mut reader = ChannelReader::new(rx);
    assert_eq!(reader.stream_position().unwrap(), 0);
    assert_eq!(reader.seek(SeekFrom::Start(0)).unwrap(), 0);

    let mut prefix = [0; 2];
    reader.read_exact(&mut prefix).unwrap();
    assert_eq!(&prefix, b"WA");
    assert_eq!(reader.stream_position().unwrap(), 2);
    assert_eq!(reader.seek(SeekFrom::Start(2)).unwrap(), 2);
    assert_eq!(reader.seek(SeekFrom::Start(0)).unwrap_err().kind(), io::ErrorKind::Unsupported);
    assert_eq!(reader.seek(SeekFrom::Current(1)).unwrap_err().kind(), io::ErrorKind::Unsupported);
    assert_eq!(reader.seek(SeekFrom::End(0)).unwrap_err().kind(), io::ErrorKind::Unsupported);
}
