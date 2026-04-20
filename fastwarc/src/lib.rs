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

pub mod record;
pub mod stream_io;

// TODO: Unchecked conversion AI slop from here on:
//
// /// Archive iterator configuration.
// #[derive(Debug, Clone)]
// pub struct ArchiveIteratorConfig {
//     /// Whether to parse HTTP records automatically
//     pub parse_http: bool,
//     /// Skip records which have no or an invalid block digest
//     pub verify_digests: bool,
//     /// Skip records with Content-Length less than this
//     pub min_content_length: Option<usize>,
//     /// Skip records with Content-Length larger than this
//     pub max_content_length: Option<usize>,
//     /// Bitmask of record types to return (others will be skipped)
//     pub record_type_filter: u16,
//     /// Enforce strict spec compliance
//     pub strict_mode: bool,
// }
//
// impl Default for ArchiveIteratorConfig {
//     fn default() -> Self {
//         ArchiveIteratorConfig {
//             parse_http: true,
//             verify_digests: false,
//             min_content_length: None,
//             max_content_length: None,
//             record_type_filter: WarcRecordType::AnyType as u16,
//             strict_mode: true,
//         }
//     }
// }
//
// /// WARC record stream iterator.
// pub struct ArchiveIterator<R: BufRead> {
//     reader: R,
//     config: ArchiveIteratorConfig,
//     current_pos: usize,
// }
//
// impl<R: BufRead> ArchiveIterator<R> {
//     /// Create a new archive iterator.
//     ///
//     /// # Arguments
//     ///
//     /// * `reader` - Input stream
//     /// * `config` - Iterator configuration
//     pub fn new(reader: R, config: ArchiveIteratorConfig) -> Self {
//         ArchiveIterator {
//             reader,
//             config,
//             current_pos: 0,
//         }
//     }
//
//     /// Read the next WARC record from the stream.
//     ///
//     /// # Returns
//     ///
//     /// `Ok(Some(record))` if a record was read, `Ok(None)` if EOF, or an error
//     pub fn read_next(&mut self) -> Result<Option<WarcRecord>, Box<dyn std::error::Error>> {
//         loop {
//             let mut record = WarcRecord::new();
//             record.stream_pos = self.current_pos;
//
//             // Read version line
//             let mut version_line = String::new();
//             loop {
//                 version_line.clear();
//                 let n = self.reader.read_line(&mut version_line)?;
//                 if n == 0 {
//                     return Ok(None); // EOF
//                 }
//                 self.current_pos += n;
//
//                 let trimmed = version_line.trim();
//                 if trimmed.is_empty() {
//                     continue;
//                 }
//
//                 if trimmed.starts_with("WARC/1.") || trimmed.starts_with("WARC/0.") {
//                     record.headers.set_status_line(trimmed.as_bytes());
//                     break;
//                 }
//
//                 if self.config.strict_mode {
//                     return Ok(None);
//                 }
//             }
//
//             // Parse WARC headers
//             let bytes_consumed = parse_header_block(&mut self.reader, &mut record.headers, false, self.config.strict_mode)?;
//             self.current_pos += bytes_consumed;
//
//             // Extract record metadata
//             if let Some(warc_type) = record.headers.get("WARC-Type") {
//                 record.record_type = WarcRecordType::from_str(&warc_type);
//             }
//
//             if let Some(content_length_str) = record.headers.get("Content-Length") {
//                 record.content_length = content_length_str.parse().unwrap_or(0);
//             }
//
//             if let Some(content_type) = record.headers.get("Content-Type") {
//                 record.is_http = content_type.contains("application/http");
//             }
//
//             // Check filters
//             let should_skip = !record.record_type.matches_bitmask(self.config.record_type_filter)
//                 || self.config.min_content_length.map_or(false, |min| record.content_length < min)
//                 || self.config.max_content_length.map_or(false, |max| record.content_length > max);
//
//             // Read content
//             let mut content = vec![0u8; record.content_length];
//             self.reader.read_exact(&mut content)?;
//             self.current_pos += record.content_length;
//             record.content = content;
//
//             // Skip trailing CRLF
//             let mut separator = vec![0u8; 4];
//             let _ = self.reader.read(&mut separator);
//             self.current_pos += 4;
//
//             if should_skip {
//                 continue;
//             }
//
//             // Parse HTTP if requested
//             if self.config.parse_http && record.is_http {
//                 record.parse_http(self.config.strict_mode)?;
//             }
//
//             return Ok(Some(record));
//         }
//     }
// }
//
// impl<R: BufRead> Iterator for ArchiveIterator<R> {
//     type Item = Result<WarcRecord, Box<dyn std::error::Error>>;
//
//     fn next(&mut self) -> Option<Self::Item> {
//         self.read_next().transpose()
//     }
// }
//
// /// Filter predicate for checking if record is a WARC/1.0 record.
// ///
// /// # Arguments
// ///
// /// * `record` - WARC record
// pub fn is_warc_10(record: &WarcRecord) -> bool {
//     record.headers().status_line_bytes() == b"WARC/1.0"
// }
//
// /// Filter predicate for checking if record is a WARC/1.1 record.
// ///
// /// # Arguments
// ///
// /// * `record` - WARC record
// pub fn is_warc_11(record: &WarcRecord) -> bool {
//     record.headers().status_line_bytes() == b"WARC/1.1"
// }
//
// /// Filter predicate for checking if record has a block digest.
// ///
// /// # Arguments
// ///
// /// * `record` - WARC record
// pub fn has_block_digest(record: &WarcRecord) -> bool {
//     record.headers().contains_key("WARC-Block-Digest")
// }
//
// /// Filter predicate for checking if record has a payload digest.
// ///
// /// # Arguments
// ///
// /// * `record` - WARC record
// pub fn has_payload_digest(record: &WarcRecord) -> bool {
//     record.headers().contains_key("WARC-Payload-Digest")
// }
//
// /// Filter predicate for checking if record is an HTTP record.
// ///
// /// # Arguments
// ///
// /// * `record` - WARC record
// pub fn is_http(record: &WarcRecord) -> bool {
//     record.is_http()
// }
//
// /// Filter predicate for checking if record is concurrent to another record.
// ///
// /// # Arguments
// ///
// /// * `record` - WARC record
// pub fn is_concurrent(record: &WarcRecord) -> bool {
//     record.headers().contains_key("WARC-Concurrent-To")
// }
