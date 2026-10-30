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

//! FastWARC is a high-performance WARC parsing library written in Rust with bindings available for Python.
//! FastWARC's design goals are high speed, a low and fixed memory footprint, ease of use, and byte-level content
//! preservation. Supported are compressed and uncompressed WARC/1.0 and WARC/1.1 streams. Available compression
//! algorithms are Gzip, Zstd, and LZ4.
//!
//! > **Note:** FastWARC belongs to the [ChatNoir Resiliparse toolkit](https://resiliparse.chatnoir.eu/en/stable/index.html)
//! > for fast and robust web data processing.
//!
//! This documentation covers the FastWARC Rust crate. For the Python bindings, see the
//! [Python documentation](https://resiliparse.chatnoir.eu/en/latest/man/fastwarc.html).
//!
//! # Installing FastWARC
//! To use FastWARC in your Rust project, add it as a dependency to your `Cargo.toml` to install it directly
//! from [crates.io](https://crates.io/crates/fastwarc):
//!
//! ```toml
//! [dependencies]
//! fastwarc = "*"    # Use correct version here
//! ```
//!
//! To build FastWARC as a standalone library, run `cargo build` from the root of this repository.
//!
//! ## FastWARC for Python
//! Pre-build wheels with PyO3 bindings are also available for Python. See
//! [Python docs](https://resiliparse.chatnoir.eu/en/stable/man/fastwarc.html) for usage instructions and API
//! documentation. You can install the wheels from [PyPi](https://pypi.org/project/FastWARC/) using `pip`:
//!
//! ```bash
//! pip install fastwarc
//! ```
//!
//! # Iterating WARC Files
//!
//! The central class for stream-processing WARC files is [`warc::iter::ArchiveIterator`].
//! ```no_run
//! use fastwarc::warc::iter::ArchiveIterator;
//! use std::fs::File;
//! use std::io::BufReader;
//!
//! let in_file = BufReader::new(File::open("warcfile.warc.gz").expect("File error"));
//! for record in ArchiveIterator::new(in_file) {
//!     match record {
//!         Ok(r) => println!("Record ID: {}", r.borrow().record_id().expect("No record ID")),
//!         Err(e) => println!("Error: {}", e),
//!     }
//! }
//! ```
//! This will iterate over all records in the file and print out their IDs. The [`ArchiveIterator`](warc::iter::ArchiveIterator)
//! automatically tries to detect whether the input is a compressed Gzip-, Zstd-, or LZ4 stream. If you need, you can also
//! make the stream type explicit by using a reader wrapper from [`stream_io`] (e.g.,
//! [`GzipReader`](stream_io::gzip::GzipReader) or [`ZstdReader`](stream_io::zstd::ZstdReader)).
//!
//! ```no_run
//! use fastwarc::stream_io::gzip::GzipReader;
//! # use std::fs::File;
//! # use std::io::BufReader;
//!
//! // Wrap an existing reader:
//! let in_file = GzipReader::new(BufReader::new(
//!     File::open("warcfile.warc.gz").expect("File error")));
//!
//! // Or construct a GzipReader from a path (PathBuf or str):
//! let in_file = GzipReader::from_path("warcfile.warc.gz");
//! ```
//!
//! In general, any reader that implements [`std::io::BufRead`] and [`std::io::Seek`] can be used as an input stream, as
//! long as it also implements [`stream_io::traits::IntoWarcReader`]. For some common reader sources, this is already
//! covered by a blanket implementation.
//!
//! > **Note:** If you plan to implement a custom reader, you should better implement [`stream_io::traits::WarcRead`]
//! > directly instead of [`stream_io::traits::IntoWarcReader`]. That ensures that correct stream offsets are reported
//! > even for compressed streams, which is essential if you want to index a WARC file.
//!
//! You can also construct an [`ArchiveIterator`](warc::iter::ArchiveIterator) directly from a file path.
//! ```no_run
//! # use fastwarc::warc;
//! use fastwarc::warc::iter::ArchiveIterator;
//! for record in ArchiveIterator::from_path("warcfile.warc.gz").expect("File error") {
//!     // ...
//! }
//! ```
//! The compression type of the file is determined automatically based on the file extension in this case.
//!
//! Since calling `borrow()` on the record can get tedious (and archive iterators can also return
//! [other types of shared record references](warc::record::SharedWarcRecord), such as when using
//! [`warc::iter::ArchiveIteratorThreadSafe`]), you can use
//! [`SharedWarcRecord::with_mut()`](`warc::record::SharedWarcRecord::with_mut()) to clean up the code a bit:
//!
//! ```
//! # use fastwarc::warc::iter::ArchiveIterator;
//! # use fastwarc::warc::record::WarcRecord;
//! use fastwarc::warc::record::SharedWarcRecord;
//!
//! # let in_file = std::io::Cursor::new(Vec::new());
//! for record in ArchiveIterator::new(in_file) {
//!     let r = record.expect("Error reading record");
//!     r.with_mut(|r| println!("Record ID: {}", r.record_id().unwrap()));
//! }
//! ```
//!
//! # Filtering Records
//!
//! FastWARC provides several ways in which you can filter and efficiently skip records you are not interested in.
//! These filters are checked very early in the parsing process, right after the WARC header block has been read.
//! Multiple types of filters can be combined. [`ArchiveIterator::with_filter()`](warc::iter::ArchiveIterator::with_filter)
//! accepts any predicate function that takes a mutable [`warc::record::WarcRecord`] reference and returns a `bool`.
//! FastWARC already provides several predicate functions out of the box:
//!
//! ## Record Type Filter
//! If you want only records of a certain type, you can skip all other records efficiently by specifying the desired
//! record type or a bitmask of multiple types:
//!
//! ```
//! # use fastwarc::warc::iter::ArchiveIterator;
//! use fastwarc::warc::record::WarcRecordType;
//! use fastwarc::warc::iter::filter::has_record_type;
//!
//! // has_record_type(...) takes a WarcRecordType or a bitmask combination of them.
//! let type_filter = has_record_type(WarcRecordType::Request | WarcRecordType::Response);
//! # let in_file = std::io::Cursor::new(Vec::new());
//! for record in ArchiveIterator::new(in_file).with_filter(type_filter) {
//!     // ...
//! }
//! ```
//!
//! This will skip all records with a `WARC-Type` other than `"request"` or `"response"`.
//!
//! ## Content-Length Filter
//! You can automatically skip any records whose `Content-Length` exceeds or is lower than a certain value:
//!
//! ```
//! # use fastwarc::warc::iter::ArchiveIterator;
//! use fastwarc::warc::iter::filter::has_content_length_lte;
//!
//! // has_content_length_lte(...) and has_content_length_gte(...) can be used to filter
//! // for maximum or minimum record lengths, in bytes.
//! let len_filter = has_content_length_lte(4096 << 10);
//! # let in_file = std::io::Cursor::new(Vec::new());
//! for record in ArchiveIterator::new(in_file).with_filter(len_filter) {
//!     // ...
//! }
//! ```
//!
//! This will skip all records larger than 4 MiB.
//!
//! ## Content Filters
//! Several filters for the type of content exist:
//!
//! ```
//! # use fastwarc::warc::iter::ArchiveIterator;
//! use fastwarc::warc::iter::filter::*;
//!
//! // Iterate only records with an HTTP payload.
//! # let in_file = std::io::Cursor::new(Vec::new());
//! for record in ArchiveIterator::new(in_file).with_filter(is_http) {
//!     // ...
//! }
//!
//! // Iterate only WARC/1.1 records.
//! # let in_file = std::io::Cursor::new(Vec::new());
//! for record in ArchiveIterator::new(in_file).with_filter(is_warc_11) {
//!     // ...
//! }
//!
//! // Iterate only records that have a WARC-Block-Digest header (without verifying it).
//! # let in_file = std::io::Cursor::new(Vec::new());
//! for record in ArchiveIterator::new(in_file).with_filter(has_block_digest) {
//!     // ...
//! }
//! ```
//!
//! ## Digest Filters
//! You can skip all records with an invalid `WARC-Block-Digest` or `WARC-Payload-Digest` header. Unlike all the other
//! filter predicates, [`has_valid_block_digest`](warc::iter::filter::has_valid_block_digest) and
//! [`has_valid_payload_digest`](warc::iter::filter::has_valid_payload_digest) are executed only *after* the content is
//! available, which is needed for the digest calculation. These filters will skip records without or with an invalid
//! block or payload digest. You can also use [`ArchiveIterator::with_verify_digests()`](warc::iter::ArchiveIterator::with_verify_digests())
//! as a shorthand for specifying explicit digest filters for both types of digests.
//
//! ```
//! # use fastwarc::warc::iter::ArchiveIterator;
//! // Iterate only records with valid digests.
//! # let in_file = std::io::Cursor::new(Vec::new());
//! for record in ArchiveIterator::new(in_file).with_verify_digests(true) {
//!     // ...
//! }
//! ```
//! > **Warning:** These are the most expensive filters of all, as they will create an in-memory copy of the whole record and
//! > calculate a digest of it. See [Verifying Record Digests](#verifying-record-digests) for more information on how
//! > digest verification works.
//
//! ## Other Filters
//! The full list of pre-defined filter predicates is:
//!
//! * [`filter::is_warc_10`](warc::iter::filter::is_warc_10)
//! * [`filter::is_warc_11`](warc::iter::filter::is_warc_11)
//! * [`filter::has_block_digest`](warc::iter::filter::has_block_digest)
//! * [`filter::has_valid_block_digest`](warc::iter::filter::has_valid_block_digest)
//! * [`filter::has_payload_digest`](warc::iter::filter::has_payload_digest)
//! * [`filter::has_valid_payload_digest`](warc::iter::filter::has_valid_payload_digest)
//! * [`filter::is_http`](warc::iter::filter::is_http)
//! * [`filter::is_concurrent`](warc::iter::filter::is_concurrent)
//! * [`filter::has_record_type(...)`](warc::iter::filter::has_record_type)
//! * [`filter::has_content_length_lte(...)`](warc::iter::filter::has_content_length_lte)
//! * [`filter::has_content_length_gte(...)`](warc::iter::filter::has_content_length_gte)
//!
//! Besides these, you can use any function that accepts a mutable [`warc::record::WarcRecord`] reference and returns
//! a `bool`.
//!
//! ## Combining Filters
//! Multiple filters can be combined with a custom closure:
//!
//! ```
//! # use fastwarc::warc::iter::ArchiveIterator;
//! # use fastwarc::warc::iter::filter::*;
//! # let in_file = std::io::Cursor::new(Vec::new());
//! // Filter for records with a block AND a payload digest.
//! for record in ArchiveIterator::new(in_file)
//!     .with_filter(|r| has_block_digest(r) && has_payload_digest(r)) {
//!     // ...
//! }
//! ```
//!
//! Chaining [`ArchiveIterator::with_filter()`](warc::iter::ArchiveIterator::with_filter) to create conjunctions is also
//! possible, but not recommended, since it will create multiple indirection layers.
//!
//!
//! # Record Properties
//!
//! An [`ArchiveIterator`](warc::iter::ArchiveIterator) returns shared references to [`WarcRecord`](warc::record::WarcRecord)
//! instances, which have various properties exposed via getters (and setters if you want to mutate a record):
//!
//! ```
//! # use fastwarc::warc::iter::ArchiveIterator;
//! # use fastwarc::warc::record::WarcRecord;
//! # let in_file = io::Cursor::new(Vec::new());
//! use fastwarc::warc::record::SharedWarcRecord;
//! use std::io::{self, Read};
//!
//! let mut buf = vec![0u8; 1024];
//!
//! for record in ArchiveIterator::new(in_file) {
//!     let record = match record {
//!         Ok(r) => r,
//!         Err(e) => panic!("Error reading record: {}", e),
//!     };
//!
//!     let body = record.with_mut(|r| -> io::Result<&[u8]> {
//!         r.headers();            // Map-like object containing the WARC headers.
//!         r.headers_mut();        // Mutable reference to the WARC headers.
//!         r.record_id();          // Shorthand for record.headers().get(WarcHeader::WarcRecordId).
//!         r.record_type();        // Shorthand for record.headers().get(WarcHeader::WarcType).
//!         r.record_date();        // Parsed record.headers().get(WarcHeader::WarcDate).
//!         r.content_length();     // Effective record payload length.
//!         r.stream_pos();         // Record start offset in the (uncompressed) stream.
//!         r.is_http();            // Boolean indicating whether record is an HTTP record.
//!         r.is_http_parsed();     // Boolean indicating whether the record has been parsed as HTTP.
//!         r.http_headers();       // Map-like object containing the parsed HTTP headers.
//!         r.http_headers_mut();   // Mutable reference to the parsed HTTP headers.
//!         r.http_content_type();  // Plain HTTP Content-Type without charset.
//!         r.http_charset();       // HTTP charset from the Content-Type header (if any).
//!         r.reader_mut();         // A BufReader for the record content.
//!
//!         // Read up to 1024 bytes from the record stream.
//!         let n = r.reader_mut().unwrap().read(&mut buf)?;
//!
//!         // Read the remaining record bytes.
//!         buf.truncate(n);
//!         if buf.capacity() < r.content_length() as usize {
//!             buf.reserve(r.content_length() as usize - buf.capacity());
//!         }
//!         r.reader_mut().unwrap().read_to_end(&mut buf)?;
//!
//!         // Or: consume the rest of stream without allocating a buffer for it (i.e., skip over).
//!         r.consume()?;
//!
//!         Ok(&buf)
//!     });
//!
//!     if let Err(e) = body {
//!         panic!("Error reading record payload: {}", e);
//!     }
//! }
//! ```
//!
//! HTTP request and response records are parsed automatically for convenience. If not needed (or wanted), you can disable
//! this behaviour by using [`ArchiveIterator::with_parse_http(false)`](warc::iter::ArchiveIterator::with_parse_http)
//! (or [`ArchiveIterator::set_parse_http(false)`](warc::iter::ArchiveIterator::set_parse_http)) to avoid unnecessary
//! processing. [`WarcRecord::reader_mut()`](warc::record::WarcRecord::reader_mut) will then start at the beginning of
//! the HTTP header block instead of the HTTP body. You can parse HTTP headers later on a per-record basis by calling
//! [`WarcRecord::parse_http()`](warc::record::WarcRecord::parse_http) as long as the reader hasn't been consumed at
//! that point.
//!
//! # Freezing Records
//!
//! By default, [`WarcRecord::reader_mut()`](warc::record::WarcRecord::reader_mut) returns a live reader on the WARC
//! input stream. This means that when the iterator returns the [`WarcRecord`](warc::record::WarcRecord) instance, no
//! content bytes have been consumed yet and no buffer has been allocated for them. This is very efficient, and it also
//! prevents large records inadvertently consuming too much memory. However, that also means that once the content
//! bytes have been read or once the iterator moves to the next record, there is no way to read them again.
//! Also, if the input stream is compressed, you cannot seek backwards in the stream, even if you still hold a reference
//! to the [`WarcRecord`](warc::record::WarcRecord) instance.
//!
//! To work around this, you can freeze a record by calling [`WarcRecord::freeze()`](warc::record::WarcRecord::freeze).
//! This will consume the record payload reader, store the bytes in an internal buffer, and detach the record from
//! the input stream. The reader returned by [`WarcRecord::reader_mut()`](warc::record::WarcRecord::reader_mut) then
//! no longer points to the input stream, but to the frozen byte buffer. Frozen records can be used indefinitely,
//! and seeking on their payload reader is supported both in the forward and in the backward direction.
//!
//!
//! # Verifying Record Digests
//!
//! If a record has digest headers, you can verify the consistency of the record contents and/or its HTTP payload:
//! ```
//! # use fastwarc::warc::iter::ArchiveIterator;
//! # use fastwarc::warc::record::WarcRecord;
//! use fastwarc::warc::record::{SharedWarcRecord, DigestError};
//! use std::io::{self, Read};
//! use fastwarc::warc::header::WarcHeader;
//!
//! # let in_file = io::Cursor::new(Vec::new());
//! for record in ArchiveIterator::new(in_file).with_parse_http(false) {
//!     record.expect("Error reading record").with_mut(|r| -> Result<(), DigestError> {
//!
//!         if r.headers().contains_key(WarcHeader::WarcBlockDigest) {
//!             // consume = false means do not consume record body (record will be frozen).
//!             println!("Block digest OK: {}", r.verify_block_digest(false)?);
//!         }
//!
//!         if r.headers().contains_key(WarcHeader::WarcPayloadDigest) {
//!             // It's safe to call this even if the record has no HTTP payload.
//!             r.parse_http();
//!             println!("Payload digest OK: {}", r.verify_payload_digest(false)?);
//!         }
//!
//!         Ok(())
//!     })
//!     .expect("Error verifying record payload digest");
//! }
//! ```
//!
//! Both [`verify_block_digest`](warc::record::WarcRecord::verify_block_digest) and
//! [`verify_payload_digest`](warc::record::WarcRecord::verify_payload_digest) return a [`DigestError`](warc::record::DigestError)
//! if the headers do not exist, contain unsupported digest types, are incorrectly formatted, or are otherwise unsupported.
//! Keep in mind that the block verification will fail if the reader has been (partially) consumed, so automatic
//! HTTP parsing has to be turned off for this to work.
//!
//! > **Warning:** Calling either of these two methods will create an in-memory copy of the remaining record stream to
//! > preserve its contents for further processing (if `consume` is set to `false`). That's why verifying the HTTP
//! > payload digest after verifying the block digest worked in the first place. If your records are very large, you
//! > need to ensure that they fit into memory entirely (e.g., by checking
//! > [`WarcRecord::content_length()`](warc::record::WarcRecord::content_length) first.
//!
//! If you do not need to preserve the stream contents, you can set `consume = true`. This will avoid the allocation of
//! a payload buffer and fully consume the rest of the record instead. However, that also means that the payload is
//! lost after verifying the digests.
//!
//!
//! # Writing WARC Files
//!
//! FastWARC supports creating new WARC records from scratch or from existing byte buffers. The created records can then
//! be written out using any [`std::io::Write`] writer. For writing compressed WARCs, you should use the writers
//! provided in [`stream_io`].
//!
//! ## Create and Serialise Records
//! Here's how you can create a new record and populate it with headers and a payload:
//!
//! ```
//! use fastwarc::warc::record::{WarcRecord, WarcRecordType};
//! use std::fs::File;
//! use fastwarc::warc::header::WarcHeader;
//!
//! let mut record = WarcRecord::new();
//!
//! // Initialize mandatory headers.
//! record.init_headers(WarcRecordType::Response, Some(b"uuid:record-uuid"));
//!
//! // Alternative: initialize mandatory headers with an auto-generated random ID.
//! record.init_headers(WarcRecordType::Response, None);
//!
//! // Set the target ID header.
//! record.headers_mut().append_bytes(WarcHeader::WarcRecordId, b"https://example.com/index.html");
//!
//! // Set the payload bytes (automatically adjusts the Content-Length header).
//! let payload = b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
//!               <!DOCTYPE html><html><body>Hello, world!</body></html>";
//! record.set_bytes_payload(payload.to_vec());
//!
//! // Mark the record as an HTTP record by setting the correct Content-Type.
//! record.set_is_http(true);
//!
//! // Write out the record.
//! let mut out_file = File::create("out.warc").expect("Error creating output file");
//! record.write(&mut out_file).expect("Write error");
//! ```
//!
//! Output file:
//!
//! ```warc
//! WARC/1.1
//! WARC-Type: response
//! WARC-Date: 2026-06-11T11:07:04.191283Z
//! WARC-Record-ID: <urn:uuid:94719fe2-63ee-4060-b322-fc4ddbdc0834>
//! Content-Length: 98
//! WARC-Target-URI: https://example.com/index.html
//! Content-Type: application/http; msgtype=response
//!
//! HTTP/1.1 200 OK
//! Content-Type: text/html
//!
//! <!DOCTYPE html><html><body>Hello, world!</body></html>
//!
//!
//! ```
//!
//! Instead of constructing the record manually, you can also parse an existing byte string:
//!
//! ```no_run
//! # use fastwarc::warc::record::WarcRecord;
//! # let in_buf = Vec::new();
//! let mut record = match WarcRecord::from_bytes(in_buf.clone()) {
//!     Ok(r) => r,
//!     Err(e) => panic!("Error reading record: {}", e),
//! };
//!
//! // Write out the record.
//! let mut out_buf = Vec::with_capacity(record.content_length() as usize);
//! record.write(&mut out_buf).expect("Write error");
//!
//! // Unless the record has been mutated, the output is guaranteed to be byte-identical.
//! assert_eq!(in_buf, out_buf);
//! ```
//!
//! You can also use [`WarcRecord::write_with_checksum()`](warc::record::WarcRecord::write_with_checksum) to automatically
//! calculate and write SHA-1 checksums of the record contents and HTTP payload (if available).
//!
//! ## Write Compressed WARCs
//! Compressed WARC files consist of a series frames (or members in Gzip lingo) that can individually be decompressed,
//! one per record. To correctly write such a WARC file, use the compressing writers from FastWARC's [`stream_io`] module.
//!
//! ```no_run
//! use fastwarc::stream_io::gzip::GzipWriter;
//! use fastwarc::stream_io::traits::WarcWrite;
//! use fastwarc::warc::record::{WarcRecord, WarcRecordType};
//!
//! // Create a request record.
//! let mut request = WarcRecord::new();
//! request.init_headers(WarcRecordType::Request, None);
//! request.set_bytes_payload(b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec());
//! request.set_is_http(true);
//!
//! // Create a response record.
//! let mut response = WarcRecord::new();
//! response.init_headers(WarcRecordType::Response, None);
//! response.set_bytes_payload(b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
//!                            <!DOCTYPE html><html><body>Hello, world!</body></html>".to_vec());
//! response.set_is_http(true);
//!
//! if let Err(e) = || -> std::io::Result<()> {
//!     // Create output file.
//!     let mut gzip_writer = GzipWriter::from_path("out.warc.gz")?;
//!
//!     // Write records and end each compression member with finish().
//!     request.write(&mut gzip_writer)?;
//!     gzip_writer.finish()?;
//!     response.write(&mut gzip_writer)?;
//!     gzip_writer.finish()?;
//!
//!     Ok(())
//! }() { eprintln!("Write error: {}", e); }
//! ```
//!
//! ## Write Custom Zstandard Dictionaries
//! Zstandard-compressed WARCs support custom compression dictionaries to achieve better compression ratios. The
//! dictionary is stored in a special dictionary frame before the first record in the output file. Such a dictionary
//! can, e.g., be trained on the first few records of a WARC file. FastWARC supports reading and writing WARCs
//! with custom dictionaries. If FastWARC encounters a dictionary frame at the start of a Zstandard WARC, it will
//! automatically use it to decompress the remainder of the stream.
//!
//! Here's how you can create a `.warc.zst` file with a dictionary frame in it:
//!
//! ```no_run
//! use fastwarc::stream_io::zstd::{train_dictionary_from_samples, ZstdWriter};
//! use fastwarc::stream_io::traits::WarcWrite;
//! use fastwarc::warc::record::WarcRecord;
//! use std::fs::File;
//!
//! // let record_bytes = vec![b"WARC/1.1\r\n...", ...];
//! # let record_bytes = vec![b"..."];
//!
//! const MAX_DICT_SIZE: usize = 128 << 10;     // 128 KiB
//! if let Err(e) = || -> std::io::Result<()> {
//!     // Train dict on the first few records (I/O error if size is too small).
//!     let dict = train_dictionary_from_samples(&record_bytes[..10], MAX_DICT_SIZE)?;
//!
//!     // Write a Zstandard WARC with a dictionary frame.
//!     let out_file = File::create("out.warc.zst")?;
//!     let mut zstd_writer = ZstdWriter::with_dictionary(out_file, dict, None);
//!     for b in record_bytes {
//!         let mut r = WarcRecord::from_bytes(b.to_vec())?;
//!         r.write(&mut zstd_writer)?;
//!         zstd_writer.finish()?;
//!     }
//!     Ok(())
//! }() { eprintln!("Write error: {}", e); }
//! ```
//!
//!
//! # ClueWeb Notes
//!
//! FastWARC is a standards-compliant WARC parser. Unfortunately, the [ClueWeb](https://lemurproject.org/) authors were
//! somewhat creative with the standard. If you work with these datasets, you will inevitably notice certain defects in
//! the files that result in premature stream aborts. This applies to both the old ClueWeb09, as well as the new
//! ClueWeb22. Following is a list of known ClueWeb WARC defects and how to work around them:
//!
//! ## ClueWeb09
//! The [ClueWeb09](https://lemurproject.org/clueweb09.php/) uses a non-standard WARC version and the WARC headers are
//! separated by `LF` instead of `CRLF` line breaks. In addition, some header values contain line breaks without proper
//! continuation indents and non-ASCII characters. To prevent FastWARC from choking on this creative WARC flavour
//! use [`ArchiveIterator::with_quirks_mode(true)`](warc::iter::ArchiveIterator::with_quirks_mode).
//!
//! ## ClueWeb22
//! [ClueWeb22](https://lemurproject.org/clueweb22.php/) WARCs are a bit more predictable than ClueWeb09 WARCs, but
//! have non-trivial defects nonetheless.
//!
//! *First*, the initial `warcinfo` records are missing the required `Content-Length` header, so we have to rely on
//! heuristics to determine where the record ends. If quirks mode is on, FastWARC will read and buffer a `warcinfo`
//! record until a blank line is found (up to `max_header_len` bytes) and then scan forward to the next valid WARC
//! version line in the next iteration. Without quirks mode, FastWARC will emit a `warcinfo` record with an empty
//! payload and then exit with an `"Invalid WARC header"` error.
//!
//! *Second*, all records are of type `response` with `Content-Type: application/http; msgtype=response`, yet they
//! contain only the HTML body and not the full HTTP response (the correct record type would be `resource` with
//! `Content-Type: text/html`). This incorrect type description triggers FastWARC's automatic HTTP parsing, which
//! will result in empty or incomplete record bodies. To avoid this, explicitly set
//! [`ArchiveIterator::with_parse_http(false)`](warc::iter::ArchiveIterator::with_parse_http).
//!
//! *Third*, all `response` records have a number of `VDOM-*` headers, which can get extremely large (up to 1MiB).
//! When parsing ClueWeb22 WARCs, set the iterator's `max_header_len` option accordingly.
//!
//!
//! # Benchmarks
//!
//! Depending on your CPU, memory architecture, storage speed, and the WARC compression algorithm, FastWARC is typically
//! between 1.2x and 2.2x faster than other comparable (Rust-based or otherwise native) WARC readers. For readers
//! implemented in non-native languages, the speed-ups can even be much larger than this. E.g., the speed-up over
//! [warcio](https://github.com/webrecorder/warcio) is typically between 1.5x and 5x and in extreme cases up to 13x.
//! Especially for Gzip-compressed WARCs, FastWARC is often much faster than other parsers.
//!
//! Read directly from DDR4-DRAM, FastWARC can achieve throughput for uncompressed WARC files of more than 6.4 GiB/s
//! on a single core. In more realistic scenarios, the throughput is usually in the order of 1.5--2.5 GiB/s.
//! Gzip-compressed WARCs cap out at around 850-900 MiB/s. Zstd WARCS come at around 1 GiB/s (with compression level 3)
//! and LZ4 at around 1.6 GiB/s.
//!
//! The FastWARC GitHub repository contains a
//! [comprehensive suite of benchmarks](https://github.com/chatnoir-eu/chatnoir-resiliparse/tree/develop/benchmarks/warc#readme)
//! of FastWARC (both Rust native and Python), as well as several other popular open source WARC reading libraries.
//!
//! The following times were benchmarked on a 2020 M1 MacBook Pro:
//!
//! ```bash
//! # Uncompressed WARC:
//! FastWARC: 1.9s, 59283 records/s, 2748.7 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
//! WARCIO: 5.0s, 22642 records/s, 1049.8 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
//!
//! # Gzip WARC:
//! FastWARC: 6.5s, 17600 records/s, 816.0 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
//! WARCIO: 9.8s, 11706 records/s, 542.7 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
//! ```
//!
//! The following times were benchmarked on a Threadripper 2920X 12-Core CPU with a Samsung 980PRO NVMe SSD
//! (cold read without page cache):
//!
//! ```bash
//! # Uncompressed WARC:
//! FastWARC: 3.5s, 32395 records/s, 1502.0 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
//! rust_warc: 4.0s, 28473 records/s, 1320.2 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
//! warc-rs: 6.1s, 18849 records/s, 874.0 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
//! WARCIO: 15.7s, 7294 records/s, 338.2 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
//!
//! # Gzip WARC:
//! FastWARC: 6.9s, 16470 records/s, 763.7 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
//! rust_warc: 11.6s, 9865 records/s, 457.4 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
//! warc-rs: 13.7s, 8331 records/s, 386.3 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
//! WARCIO: 25.9s, 4419 records/s, 204.9 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
//! ```
//!
//! For more detailed information and benchmarking results, checkout the
//! [benchmarks folder](https://github.com/chatnoir-eu/chatnoir-resiliparse/tree/develop/benchmarks/warc)
//! in the GitHub repository.

/// Readers and writers for (compressed) WARC streams.
pub mod stream_io;

/// WARC record and archive iterators.
pub mod warc;
