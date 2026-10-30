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
use crate::stream_io::bufread::LimitedBufReader;
use crate::stream_io::traits::{IntoWarcReader, WarcRead, WarcWrite};
use std::cell::RefCell;
use std::io;
use std::io::{BufRead, Cursor, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::rc::Rc;

// ===========================================================
// Test helpers.
// ===========================================================

/// Test helper simulating an unreliable writer.
pub struct ErrorWriter {
    fail_on_write: bool,
    fail_on_flush: bool,
}

// ===========================================================
// Helper types.
// ===========================================================

impl Write for ErrorWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.fail_on_write && !buf.is_empty() {
            Err(io::Error::other("injected write failure"))
        } else {
            Ok(buf.len())
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.fail_on_flush {
            Err(io::Error::other("injected flush failure"))
        } else {
            Ok(())
        }
    }
}

/// Test helper for testing writer Drop implementations with a shared Vec buffer.
#[derive(Clone, Default)]
pub struct SharedVecWriter {
    data: Rc<RefCell<Vec<u8>>>,
}

impl SharedVecWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn data(&self) -> Rc<RefCell<Vec<u8>>> {
        Rc::clone(&self.data)
    }
}

impl Write for SharedVecWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.data.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// ===========================================================
// Test fixtures.
// ===========================================================

// noinspection DuplicatedCode
/// Helper for getting path to external test fixtures.
pub fn get_fixture_path(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../test-data");
    path.push(name);
    path
}

pub fn sample_data() -> Vec<u8> {
    b"The quick brown fox jumps over the lazy dog.\n".repeat(128)
}

// ===========================================================
// Module tests.
// ===========================================================

#[test]
fn limited_buf_read_seek_read_line() -> std::io::Result<()> {
    let mut inner: Vec<u8> = b"abcd\nefghij\r\n\n".to_vec();
    inner.extend_from_slice(b"a".repeat(128).as_slice());
    let reader_limit = 130;
    let mut reader = LimitedBufReader::new(io::Cursor::new(inner), Some(reader_limit));

    let mut line = Vec::new();
    assert_eq!(reader.read_line(&mut line, 64)?, 5);
    assert_eq!(line, b"abcd\n");
    assert_eq!(reader.stream_position()?, 5);

    // Append behavior matches BufRead::read_line: return only newly read bytes.
    line.clear();
    line.extend_from_slice(b"prefix");
    assert_eq!(reader.read_line(&mut line, 64)?, 8);
    assert_eq!(line, b"prefixefghij\r\n");
    assert_eq!(reader.stream_position()?, 13);

    // Line with CRLF
    line.clear();
    assert_eq!(reader.read_line(&mut line, 64)?, 1);
    assert_eq!(line, b"\n");
    assert_eq!(reader.stream_position()?, 14);

    // Last line too long
    line.clear();
    assert_eq!(reader.read_line(&mut line, 64)?, 64);
    assert_eq!(line, b"a".repeat(64).as_slice());
    assert_eq!(reader.stream_position()?, 78);

    // Read rest (truncated by reader limit)
    line.clear();
    assert_eq!(reader.read_line(&mut line, 64)?, 52);
    assert_eq!(line, b"a".repeat(52).as_slice());
    assert_eq!(reader.stream_position()?, 130);

    // EOF
    line.clear();
    assert_eq!(reader.read_line(&mut line, 64)?, 0);
    assert!(line.is_empty());
    assert_eq!(reader.stream_position()?, 130);

    Ok(())
}

// ===========================================================
// Test generics.
// ===========================================================

pub fn test_compress_decompress_roundtrip_validation<C, D>(compress_fn: C, decompress_fn: D) -> io::Result<()>
where
    C: Fn(&[u8]) -> io::Result<Vec<u8>>,
    D: Fn(&[u8], usize) -> io::Result<Vec<u8>>,
{
    let data = b"ABC".repeat(200);
    let compressed = compress_fn(&data)?;
    assert_ne!(data, compressed);
    assert_eq!(data, decompress_fn(&compressed, data.len())?);
    Ok(())
}

pub fn test_reader_new_read_seek_and_stream_position<C, R, S>(compress_fn: C, reader_new_fn: R) -> io::Result<()>
where
    C: Fn(&[u8]) -> io::Result<Vec<u8>>,
    R: Fn(Cursor<Vec<u8>>) -> S,
    S: WarcRead + BufReadSeek,
{
    let plain = sample_data();
    let compressed = compress_fn(&plain)?;
    let mut reader = reader_new_fn(Cursor::new(compressed));

    let mut prefix = [0; 17];
    assert_eq!(reader.read(&mut prefix)?, prefix.len());
    assert_eq!(prefix, plain[..prefix.len()]);
    assert_eq!(reader.stream_position()?, prefix.len() as u64);

    assert_eq!(reader.seek(SeekFrom::Current(11))?, 28);
    assert_eq!(reader.stream_position()?, 28);

    let mut next = [0; 9];
    reader.read_exact(&mut next)?;
    assert_eq!(next, plain[28..37]);

    assert_eq!(reader.seek(SeekFrom::Start(50))?, 50);
    let mut tail = [0; 12];
    reader.read_exact(&mut tail)?;
    assert_eq!(tail, plain[50..62]);

    // Backward seeking is intentionally unsupported because decompression only advances.
    let err = reader.seek(SeekFrom::Current(-1)).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::Unsupported);

    // Seeking from the end would require scanning the full compressed stream first.
    let err = reader.seek(SeekFrom::End(0)).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::Unsupported);

    Ok(())
}

pub fn test_reader_with_capacity_fill_buf_consume<C, R, S>(compress_fn: C, reader_with_capacity_fn: R) -> io::Result<()>
where
    C: Fn(&[u8]) -> io::Result<Vec<u8>>,
    R: Fn(Cursor<Vec<u8>>, usize) -> S,
    S: WarcRead + BufReadSeek,
{
    let plain = sample_data();
    let compressed = compress_fn(&plain)?;
    let mut reader = reader_with_capacity_fn(Cursor::new(compressed.clone()), 128);

    let buf = reader.fill_buf()?.to_vec();
    assert!(!buf.is_empty());
    assert_eq!(buf, plain[..buf.len()]);

    reader.consume(3);
    assert_eq!(reader.stream_position()?, 3);

    let buf = reader.fill_buf()?.to_vec();
    assert_eq!(buf, plain[3..3 + buf.len()]);

    Ok(())
}

pub fn test_reader_inner_stream_position_and_into_inner<C, R, S, I>(
    compress_fn: C,
    reader_with_capacity_fn: R,
    into_inner_fn: I,
) -> io::Result<()>
where
    C: Fn(&[u8]) -> io::Result<Vec<u8>>,
    R: Fn(Cursor<Vec<u8>>, usize) -> S,
    S: WarcRead + BufReadSeek,
    I: Fn(S) -> Cursor<Vec<u8>>,
{
    let plain = sample_data();
    let compressed = compress_fn(&plain)?;

    // Test inner_stream_position()
    let mut reader = reader_with_capacity_fn(Cursor::new(compressed.clone()), 128);
    assert_eq!(reader.stream_position()?, 0);
    assert_eq!(reader.inner_stream_position()?, 0);
    let _ = reader.read(&mut [0; 5])?;
    assert_eq!(reader.stream_position()?, 5);
    assert!(reader.inner_stream_position()? >= 5);
    assert_eq!(reader.inner_seek(SeekFrom::Start(0))?, 0);
    assert_eq!(reader.inner_stream_position()?, 0);

    let inner_pos = reader.inner_stream_position()?;
    let mut inner = into_inner_fn(reader);
    assert!(inner.stream_position()? >= inner_pos);

    let mut compressed_roundtrip = Vec::new();
    inner.rewind()?;
    inner.read_to_end(&mut compressed_roundtrip)?;
    assert_eq!(compressed_roundtrip, compressed);

    Ok(())
}

pub fn test_reader_with_tiny_capacity_handles_many_refills<C, R, S>(
    compress_fn: C,
    reader_with_capacity_fn: R,
) -> io::Result<()>
where
    C: Fn(&[u8]) -> io::Result<Vec<u8>>,
    R: Fn(Cursor<Vec<u8>>, usize) -> S,
    S: WarcRead + BufReadSeek,
{
    let plain = sample_data();
    let compressed = compress_fn(&plain)?;
    let mut reader = reader_with_capacity_fn(Cursor::new(compressed), 8);
    let mut out = Vec::with_capacity(plain.len());

    loop {
        let buf = reader.fill_buf()?;
        if buf.is_empty() {
            break;
        }

        // Consume tiny chunks to force repeated refills and cross-buffer bookkeeping.
        let chunk_len = buf.len().min(5);
        out.extend_from_slice(&buf[..chunk_len]);
        reader.consume(chunk_len);
    }

    assert_eq!(out, plain);
    assert_eq!(reader.stream_position()?, plain.len() as u64);
    assert_eq!(reader.read(&mut [0; 1])?, 0);

    Ok(())
}

pub fn test_reader_reads_to_eof_after_external_compression<C, R, S>(compress_fn: C, reader_new_fn: R) -> io::Result<()>
where
    C: Fn(&[u8]) -> io::Result<Vec<u8>>,
    R: Fn(Cursor<Vec<u8>>) -> S,
    S: WarcRead + BufRead,
{
    let plain = sample_data();
    let compressed = compress_fn(&plain)?;
    let mut reader = reader_new_fn(Cursor::new(compressed));

    let mut out = Vec::new();
    reader.read_to_end(&mut out)?;
    assert_eq!(out, plain);
    assert_eq!(reader.read(&mut [0; 1])?, 0);
    assert_eq!(reader.fill_buf()?, b"");

    Ok(())
}

pub fn test_reader_inner_seek_inner_stream_position_and_member_tracking<C, R, S>(
    compress_fn: C,
    reader_new_fn: R,
) -> io::Result<()>
where
    C: Fn(&[u8]) -> io::Result<Vec<u8>>,
    R: Fn(Cursor<Vec<u8>>) -> S,
    S: WarcRead + BufRead,
{
    let first_plain = b"first member data\n".repeat(32);
    let second_plain = b"second member payload\n".repeat(24);

    let first_member = compress_fn(&first_plain)?;
    let second_member = compress_fn(&second_plain)?;

    let frames_supported = reader_new_fn(Cursor::new(Vec::new())).frame_start_position()?.is_some();

    let mut combined;
    if frames_supported {
        combined = first_member.clone();
        combined.extend_from_slice(&second_member);
    } else {
        combined = first_member.clone();
    }
    let combined_len = combined.len();

    // Read everything but the last byte of the first member.
    let mut reader = reader_new_fn(Cursor::new(combined.clone()));
    let mut first_out = vec![0; first_plain.len() - 1];
    reader.read_exact(&mut first_out)?;
    assert_eq!(first_out, first_plain[..first_plain.len() - 1]);
    if frames_supported {
        assert_eq!(reader.frame_start_position()?.unwrap(), 0);
    }
    assert_eq!(reader.stream_position()?, first_out.len() as u64);
    let inner_pos_first = reader.inner_stream_position()?;
    assert!(inner_pos_first > 0);

    // Read remaining bytes and test that we're still in the first member.
    let mut one_more_byte = [0; 1];
    reader.read_exact(&mut one_more_byte)?;
    assert_eq!(one_more_byte, first_plain[first_plain.len() - 1..]);
    if frames_supported {
        assert_eq!(reader.frame_start_position()?.unwrap(), 0);
    }
    assert_eq!(reader.stream_position()?, (first_out.len() + 1) as u64);
    assert!(reader.inner_stream_position()? >= inner_pos_first);

    if !frames_supported {
        return Ok(());
    }

    // Read another byte. The member offset should jump once the second member starts, and
    // stream_position() keeps counting up beyond member boundaries.
    let mut first_byte_second_member = [0; 1];
    reader.read_exact(&mut first_byte_second_member)?;
    assert_eq!(first_byte_second_member[0], second_plain[0]);
    assert_eq!(reader.frame_start_position()?.unwrap(), first_member.len() as u64);
    assert_eq!(reader.stream_position()?, (first_out.len() + 2) as u64);
    assert!(reader.inner_stream_position()? >= first_member.len() as u64);

    // Reset the inner stream to the beginning of the second member and decompress again.
    // This should decompress without errors and reset stream_position() to 0.
    assert_eq!(reader.inner_seek(SeekFrom::Start(first_member.len() as u64))?, first_member.len() as u64);
    assert_eq!(reader.frame_start_position()?.unwrap(), first_member.len() as u64);
    assert_eq!(reader.stream_position()?, 0);
    let mut first_two_bytes_second_member = [0; 2];
    reader.read_exact(&mut first_two_bytes_second_member)?;
    assert_eq!(first_two_bytes_second_member, second_plain[..2]);
    assert_eq!(reader.frame_start_position()?.unwrap(), first_member.len() as u64);
    assert_eq!(reader.stream_position()?, 2);
    assert!(reader.inner_stream_position()? >= first_member.len() as u64);

    // Read the rest (member_start_position() should not increase at EOF).
    let mut rest = Vec::with_capacity(second_plain.len() - 2);
    reader.read_to_end(&mut rest)?;
    assert_eq!(rest, second_plain[2..]);
    assert_eq!(reader.frame_start_position()?.unwrap(), first_member.len() as u64);
    assert_eq!(reader.stream_position()?, second_plain.len() as u64);
    assert_eq!(reader.inner_stream_position()?, combined_len as u64);

    Ok(())
}

pub fn test_nested_warc_read<C, R>(compress_fn: C, reader_new_fn: R) -> io::Result<()>
where
    C: Fn(&[u8]) -> io::Result<Vec<u8>>,
    R: Fn(Box<dyn WarcRead>) -> Box<dyn WarcRead>,
{
    let payload = sample_data();
    let compressed = compress_fn(&compress_fn(&payload)?)?;

    let mut reader = reader_new_fn(reader_new_fn(Box::new(Cursor::new(compressed)).into_warc_reader()));
    let mut out = vec![0; payload.len()];

    reader.read_exact(&mut out[..7])?;
    assert_eq!(out[..7], payload[..7]);
    assert_eq!(reader.stream_position()?, 7);
    // Nested decompressor, cannot seek backwards
    assert_eq!(reader.seek(SeekFrom::Start(0)).unwrap_err().to_string(), "Backward seeking not supported");
    assert_eq!(reader.seek(SeekFrom::Start(8))?, 8);
    reader.read_exact(&mut out[8..])?;
    assert_eq!(&out[8..], &payload[8..]);

    Ok(())
}

pub fn test_frame_start_position_in_sync<C, R, S>(
    compress_fn: C,
    reader_new_fn: R,
    frame_end_offset: usize,
) -> io::Result<()>
where
    C: Fn(&[u8]) -> io::Result<Vec<u8>>,
    R: Fn(Cursor<Vec<u8>>) -> S,
    S: WarcRead + BufReadSeek,
{
    const COUNT: usize = 20;
    let data = b"abcdefgh".repeat(200);
    // let mut w = GzipWriter::new(io::Cursor::new(data.clone()));
    // w.write_all(&data)?;
    // let compressed = w.into_inner()?.into_inner();
    let compressed = compress_fn(&data)?;
    let mut expected_offsets = Vec::with_capacity(COUNT);
    let mut buf = Vec::with_capacity(300);
    for _ in 0..COUNT {
        expected_offsets.push(buf.len() as u64);
        buf.extend_from_slice(&compressed);
    }
    assert_eq!(buf.len(), compressed.len() * COUNT);

    let mut reader = reader_new_fn(io::Cursor::new(buf));
    let mut count = 0;
    let mut last_frame_pos = 0;
    let mut n_read = 0;
    loop {
        let mut out_buf = [0u8; 200];
        let pos = reader.inner_stream_position()?;
        let fpos = reader.frame_start_position()?.unwrap();
        assert!(pos >= fpos);

        let n = reader.read(&mut out_buf)?;
        if n == 0 {
            break;
        }
        n_read += n;
        if n_read == data.len() {
            // frame_start_position() should always be start of frame.
            if count != 0 {
                assert_eq!(fpos, last_frame_pos + compressed.len() as u64);
            } else {
                assert_eq!(fpos, 0);
            }
            assert_eq!(fpos, expected_offsets[count]);

            // stream_position() should be end of frame (minus frame terminator padding)
            assert_eq!(pos as usize, fpos as usize + compressed.len() - frame_end_offset);

            count += 1;
            n_read = 0;
            last_frame_pos = fpos;
        }
    }
    assert_eq!(count, COUNT);
    Ok(())
}

pub fn test_writer_new_write_and_into_inner_roundtrip<D, W, S, I>(
    decompress_fn: D,
    writer_new_fn: W,
    into_inner_fn: I,
) -> io::Result<()>
where
    D: Fn(&[u8], usize) -> io::Result<Vec<u8>>,
    W: Fn(Vec<u8>) -> S,
    S: WarcWrite,
    I: Fn(S) -> io::Result<Vec<u8>>,
{
    let plain = sample_data();
    let mut writer = writer_new_fn(Vec::new());

    assert_eq!(writer.write(&plain[..23])?, 23);
    writer.write_all(&plain[23..])?;

    let compressed = into_inner_fn(writer)?;
    assert_eq!(decompress_fn(&compressed, plain.len())?, plain);

    Ok(())
}

pub fn test_writer_drop_finishes_and_flushes_stream<D, W, S>(
    decompress_fn: D,
    writer_with_capacity_fn: W,
) -> io::Result<()>
where
    D: Fn(&[u8], usize) -> io::Result<Vec<u8>>,
    W: Fn(SharedVecWriter, usize) -> S,
    S: WarcWrite,
{
    let plain = sample_data();
    let inner = SharedVecWriter::new();
    let shared_data = inner.data();

    {
        let mut writer = writer_with_capacity_fn(inner.clone(), 11);
        writer.write_all(&plain)?;
    }

    // Drop should behave like finish() + flush() when the writer still owns an inner stream.
    assert!(!shared_data.borrow().is_empty());
    assert_eq!(decompress_fn(&shared_data.borrow(), plain.len())?, plain);

    Ok(())
}

pub fn test_writer_propagates_inner_flush_errors<W, S>(writer_with_capacity_fn: W) -> io::Result<()>
where
    W: Fn(ErrorWriter, usize) -> S,
    S: WarcWrite,
{
    let plain = sample_data();
    let mut writer = writer_with_capacity_fn(
        ErrorWriter {
            fail_on_write: false,
            fail_on_flush: true,
        },
        64,
    );

    writer.write_all(&plain)?;

    // flush() must forward the inner stream failure after draining buffered output.
    let err = writer.flush().unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::Other);
    assert!(err.to_string().contains("injected flush failure"));

    Ok(())
}

pub fn test_writer_propagates_inner_write_errors<W, S>(writer_with_capacity_fn: W) -> io::Result<()>
where
    W: Fn(ErrorWriter, usize) -> S,
    S: WarcWrite,
{
    let plain = sample_data();
    let mut writer = writer_with_capacity_fn(
        ErrorWriter {
            fail_on_write: true,
            fail_on_flush: false,
        },
        8,
    );

    let mut res = writer.write_all(&plain);
    if res.is_ok() {
        res = writer.finish();
    }
    if res.is_ok() {
        res = writer.flush();
    }
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::Other);
    assert!(err.to_string().contains("injected write failure"));
    Ok(())
}
