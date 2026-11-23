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
use crate::stream_io::mod_test::*;
use crate::stream_io::traits::IntoWarcReader;
use std::io::{Cursor, Read};

// Fixed chunk size for testing.
const CHUNK_SIZE: usize = 20;

#[test]
fn chunk_extensions_and_trailers() -> io::Result<()> {
    let input = b"3;foo=\"a;b\"\r\nabc\r\n0;end\r\nX-Test: value\r\n\r\n";
    let mut reader = ChunkedReader::new(Cursor::new(input));
    let mut output = Vec::new();
    reader.read_to_end(&mut output)?;
    assert_eq!(output, b"abc");
    assert_eq!(reader.read(&mut [0])?, 0);
    reader.inner_seek(SeekFrom::Start(0))?;
    output.clear();
    reader.read_to_end(&mut output)?;
    assert_eq!(output, b"abc");
    Ok(())
}

#[test]
fn chunk_framing_limit() {
    // Repeated tiny chunks cannot accumulate unbounded extension overhead.
    let chunk = format!("1;{}\r\nx\r\n", "a".repeat(1024));
    let input = format!("{}0\r\n\r\n", chunk.repeat(100));
    let mut reader = ChunkedReader::new(Cursor::new(input.into_bytes()));
    assert_eq!(reader.read_to_end(&mut Vec::new()).unwrap_err().kind(), io::ErrorKind::InvalidData);
}

#[test]
fn chunk_line_limit() {
    let input = format!("1;{}\r\nx\r\n0\r\n\r\n", "a".repeat(32 * 1024));
    let mut reader = ChunkedReader::new(Cursor::new(input.into_bytes()));
    assert_eq!(reader.read_to_end(&mut Vec::new()).unwrap_err().kind(), io::ErrorKind::InvalidData);
}

#[test]
fn chunk_inner_seek_clears_buffer() -> io::Result<()> {
    let mut reader = ChunkedReader::new(Cursor::new(b"3\r\nabc\r\n0\r\n\r\n".to_vec()));
    assert_eq!(reader.read(&mut [0])?, 1);
    reader.inner_seek(SeekFrom::Start(0))?;
    let mut output = Vec::new();
    reader.read_to_end(&mut output)?;
    assert_eq!(output, b"abc");
    Ok(())
}

#[test]
fn truncated_chunk_trailer() {
    let mut reader = ChunkedReader::new(Cursor::new(b"0\r\nX-Test: value\r\n".to_vec()));
    assert_eq!(reader.read_to_end(&mut Vec::new()).unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
}

fn chunk(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut reader = Cursor::new(data);
    let mut writer = Vec::new();
    loop {
        let mut buf = vec![0u8; CHUNK_SIZE];
        let n = reader.read(&mut buf)?;
        if n == 0 {
            writer.write_all(b"0\r\n\r\n")?;
            return Ok(writer);
        }
        let header = format!("{:x}\r\n", n).into_bytes();
        writer.write_all(&header)?;
        writer.write_all(&buf[..n])?;
        writer.write_all(b"\r\n")?;
    }
}

fn dechunk(data: &[u8], expected_len: usize) -> io::Result<Vec<u8>> {
    let mut reader = Cursor::new(data);
    let mut out_buf = Vec::with_capacity(expected_len);
    loop {
        let mut header = Vec::with_capacity(8);
        let n = reader.read_until(b'\n', &mut header)?;
        if n == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        let header = String::from_utf8_lossy(&header);
        let header = header.trim_ascii();
        let chunk_size = usize::from_str_radix(header, 16)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, format!("Invalid chunk header: '{}'", header)))?;
        if chunk_size == 0 {
            reader.read_until(b'\n', &mut Vec::new())?;
            return Ok(out_buf);
        }
        let old_len = out_buf.len();
        out_buf.resize(out_buf.len() + chunk_size, 0);
        reader
            .read_exact(&mut out_buf[old_len..])
            .map_err(|_| io::ErrorKind::UnexpectedEof)?;
        reader.read_until(b'\n', &mut Vec::new())?;
    }
}

#[test]
fn chunked_compress_decompress_roundtrip_validation() -> io::Result<()> {
    test_compress_decompress_roundtrip_validation(chunk, dechunk)
}

// ===========================================================
// Generic tests.
// ===========================================================

#[test]
fn chunked_reader_new_read_seek_and_stream_position() -> io::Result<()> {
    test_reader_new_read_seek_and_stream_position(chunk, ChunkedReader::new)
}

#[test]
fn chunked_reader_with_capacity_fill_buf_consume() -> io::Result<()> {
    test_reader_with_capacity_fill_buf_consume(chunk, ChunkedReader::with_capacity)
}

#[test]
fn chunked_reader_inner_stream_position_and_into_inner() -> io::Result<()> {
    test_reader_inner_stream_position_and_into_inner(chunk, ChunkedReader::with_capacity, ChunkedReader::into_inner)
}

#[test]
fn chunked_reader_with_tiny_capacity_handles_many_refills() -> io::Result<()> {
    test_reader_with_tiny_capacity_handles_many_refills(chunk, ChunkedReader::with_capacity)
}

#[test]
fn chunked_reader_reads_to_eof_after_external_compression() -> io::Result<()> {
    test_reader_reads_to_eof_after_external_compression(chunk, ChunkedReader::new)
}

#[test]
fn brotli_reader_inner_seek_inner_stream_position_and_member_tracking() -> io::Result<()> {
    test_reader_inner_seek_inner_stream_position_and_member_tracking(chunk, ChunkedReader::new)
}

#[test]
fn chunked_nested_warc_read() -> io::Result<()> {
    test_nested_warc_read(chunk, |r| ChunkedReader::new(r).into_warc_reader())
}

#[test]
fn chunked_writer_new_write_and_into_inner_roundtrip() -> io::Result<()> {
    test_writer_new_write_and_into_inner_roundtrip(dechunk, ChunkedWriter::new, ChunkedWriter::into_inner)
}

#[test]
fn chunked_writer_drop_finishes_and_flushes_stream() -> io::Result<()> {
    test_writer_drop_finishes_and_flushes_stream(dechunk, ChunkedWriter::with_chunk_size)
}

#[test]
fn chunked_writer_propagates_inner_flush_errors() -> io::Result<()> {
    test_writer_propagates_inner_flush_errors(ChunkedWriter::with_chunk_size)
}

#[test]
fn chunked_writer_propagates_inner_write_errors() -> io::Result<()> {
    test_writer_propagates_inner_write_errors(ChunkedWriter::with_chunk_size)
}
