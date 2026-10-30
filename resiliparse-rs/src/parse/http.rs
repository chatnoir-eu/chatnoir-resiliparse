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

//! Helpers for consuming HTTP payloads with chunked transfer encoding.
//!
//! Call [`read_http_chunk`] repeatedly and concatenate its nonempty results, or
//! use [`iterate_http_chunks`] to consume chunks lazily. Both accept buffered
//! byte readers and preserve payload bytes without decoding them as text.
//! Parsing follows the permissive legacy helper rather than strictly validating
//! HTTP chunk framing. These functions do not parse HTTP response headers or
//! apply content decompression.

use std::io::{self, BufRead, Read};

/// Parse a chunk size using the legacy `strtol(..., 16)` prefix semantics.
///
/// The input is expected to be a complete size line ending in CRLF. Its final
/// two bytes are omitted without checking their value; inputs shorter than two
/// bytes are used as supplied. Leading ASCII whitespace, an optional sign, and
/// an optional `0x` prefix are accepted. Parsing stops at the first non-hexadecimal
/// byte, which permits chunk extensions such as `;name=value`.
///
/// A line with no parsable hexadecimal digits is interpreted as size zero. Values outside
/// the platform's C `long` range are clamped before conversion to `usize`;
/// negative sizes use the legacy unsigned conversion and can produce large
/// values. This function does not reject malformed or negative size lines.
///
/// # Arguments
///
/// * `header` - The raw chunk size line, normally including its trailing CRLF.
///
/// # Examples
///
/// ```
/// use resiliparse::parse::http::chunk_size;
///
/// assert_eq!(chunk_size(b"c;name=value\r\n"), 12);
/// assert_eq!(chunk_size(b"0xA\r\n"), 10);
/// assert_eq!(chunk_size(b"invalid\r\n"), 0);
/// ```
pub fn chunk_size(header: &[u8]) -> usize {
    let header = &header[..header.len().checked_sub(2).unwrap_or(header.len())];
    let mut pos = 0;
    while header
        .get(pos)
        .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))
    {
        pos += 1;
    }
    let negative = header.get(pos) == Some(&b'-');
    if matches!(header.get(pos), Some(b'+' | b'-')) {
        pos += 1;
    }
    if header.get(pos) == Some(&b'0') && matches!(header.get(pos + 1), Some(b'x' | b'X')) {
        pos += 2;
    }
    let limit = std::ffi::c_long::MAX as usize + usize::from(negative);
    let mut value = 0usize;
    while let Some(&b) = header.get(pos) {
        let digit = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            _ => break,
        };
        value = value.saturating_mul(16).saturating_add(digit as usize).min(limit);
        pos += 1;
    }
    if negative { value.wrapping_neg() } else { value }
}

/// Read the next chunk from an HTTP payload with chunked transfer encoding.
///
/// Reads a size line through its next LF and interprets it with [`chunk_size`].
/// It then reads up to the declared size plus two delimiter bytes, returning at
/// most the declared payload length. The delimiter is discarded without checking
/// for CRLF. If EOF truncates the chunk, any available payload bytes are returned
/// rather than reporting an incomplete-frame error.
///
/// An empty vector signals a zero-sized chunk, an invalid size parsed as zero,
/// or EOF before any payload bytes can be read. Callers should stop on that
/// result. Two bytes after a zero-sized header are still consumed if available;
/// HTTP trailer headers are not parsed or skipped as a block.
///
/// # Arguments
///
/// * `reader` - A buffered reader positioned at the next chunk size line.
///   Each call advances it past the bytes read for that chunk and its delimiter.
///
/// # Errors
///
/// Propagates errors from the underlying reader. Malformed framing alone is not
/// reported as an I/O error.
///
/// # Examples
///
/// ```
/// use std::io::Cursor;
/// use resiliparse::parse::http::read_http_chunk;
///
/// let mut reader = Cursor::new(b"3\r\nabc\r\n0\r\n\r\n");
/// assert_eq!(read_http_chunk(&mut reader)?, b"abc");
/// assert!(read_http_chunk(&mut reader)?.is_empty());
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn read_http_chunk<R: BufRead>(reader: &mut R) -> io::Result<Vec<u8>> {
    let mut header = Vec::new();
    reader.read_until(b'\n', &mut header)?;
    let size = chunk_size(&header);
    let mut data = Vec::new();
    reader.take(size.wrapping_add(2) as u64).read_to_end(&mut data)?;
    data.truncate(size);
    Ok(data)
}

/// Lazily iterate over the chunks of an HTTP payload with chunked transfer encoding.
///
/// Wraps [`read_http_chunk`] and yields each nonempty payload as `Ok(Vec<u8>)`.
/// Construction does not read from the stream. The first empty chunk terminates
/// iteration and is not yielded. An I/O error is yielded once as `Err`, after
/// which the iterator stays exhausted. Empty-chunk and error termination both
/// prevent subsequent attempts to read more bytes.
///
/// The same permissive size parsing, delimiter handling, and truncated-payload
/// behavior as [`read_http_chunk`] apply. Trailer headers are not processed.
///
/// # Arguments
///
/// * `reader` - A buffered reader positioned at the first chunk size line. Pass
///   `&mut reader` to retain access to the stream and its position after iteration.
///
/// # Examples
///
/// ```
/// use std::io::{self, Cursor};
/// use resiliparse::parse::http::iterate_http_chunks;
///
/// let mut reader = Cursor::new(b"4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n");
/// let chunks = iterate_http_chunks(&mut reader).collect::<io::Result<Vec<_>>>()?;
/// assert_eq!(chunks.concat(), b"Wikipedia");
/// # Ok::<(), io::Error>(())
/// ```
pub fn iterate_http_chunks<R: BufRead>(mut reader: R) -> impl Iterator<Item = io::Result<Vec<u8>>> {
    let mut done = false;
    std::iter::from_fn(move || {
        if done {
            return None;
        }
        match read_http_chunk(&mut reader) {
            Ok(chunk) if chunk.is_empty() => {
                done = true;
                None
            }
            Ok(chunk) => Some(Ok(chunk)),
            Err(error) => {
                done = true;
                Some(Err(error))
            }
        }
    })
}

// ===========================================================
// Tests
// ===========================================================

#[cfg(test)]
#[path = "http_test.rs"]
mod http_test;
