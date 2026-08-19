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

//! Server-side defaults and hard limits for the tunable
//! [`ParseWarcConfig`](crate::proto::fastwarc::v1::ParseWarcConfig) values.
//!
//! A zero (or unset) config value selects the default below; a value above
//! the corresponding hard limit is rejected with `InvalidArgument` before
//! parsing starts. The limits exist so that a remote client cannot make the
//! server allocate unbounded buffers or emit messages larger than
//! [`MAX_MESSAGE_SIZE`](crate::transport::MAX_MESSAGE_SIZE).

/// Default cap on WARC and HTTP header block length: 32 KiB, matching the
/// `fastwarc` crate's `ArchiveIteratorOptions` default.
pub const DEFAULT_MAX_HEADER_LEN: usize = 32 << 10;

/// Hard limit for `max_header_len`: 2 MiB. Large enough for the roughly
/// 1 MiB header blocks seen in large web-crawl captures, small enough that
/// a lossless `HeaderBlock` always fits within one response message.
pub const MAX_HEADER_LEN: usize = 2 << 20;

/// Default payload bytes per `payload_chunk` message: 64 KiB.
pub const DEFAULT_PAYLOAD_CHUNK_SIZE: usize = 64 << 10;

/// Hard limit for `payload_chunk_size`: half the message cap, so one chunk
/// message including its framing always fits in a single gRPC message.
pub const MAX_PAYLOAD_CHUNK_SIZE: usize = crate::transport::MAX_MESSAGE_SIZE / 2;

/// Default read buffer for `archive_path` file input: 64 KiB. Streamed
/// chunks are parsed in place and do not use this buffer.
pub const DEFAULT_INPUT_BUFFER_SIZE: usize = 64 << 10;

/// Hard limit for `input_buffer_size`: the gRPC message cap.
pub const MAX_INPUT_BUFFER_SIZE: usize = crate::transport::MAX_MESSAGE_SIZE;
