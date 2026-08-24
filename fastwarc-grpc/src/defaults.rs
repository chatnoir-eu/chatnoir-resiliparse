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

//! Defaults and limits for
//! [`ParseWarcConfig`](crate::proto::fastwarc::v1::ParseWarcConfig).

/// Default cap on WARC and HTTP header block length: 32 KiB, matching the
/// `fastwarc` crate's `ArchiveIteratorOptions` default.
pub const DEFAULT_MAX_HEADER_LEN: usize = 32 << 10;

/// Hard limit for `max_header_len`: 2 MiB.
pub const MAX_HEADER_LEN: usize = 2 << 20;

/// Default payload bytes per `payload_chunk` message: 64 KiB.
pub const DEFAULT_PAYLOAD_CHUNK_SIZE: usize = 64 << 10;

/// Hard limit for `payload_chunk_size`: 8 MiB.
pub const MAX_PAYLOAD_CHUNK_SIZE: usize = crate::transport::MAX_MESSAGE_SIZE / 2;

/// Default read buffer for `archive_path` file input: 64 KiB. This setting
/// does not affect streamed chunks.
pub const DEFAULT_INPUT_BUFFER_SIZE: usize = 64 << 10;

/// Hard limit for `input_buffer_size`: 16 MiB.
pub const MAX_INPUT_BUFFER_SIZE: usize = crate::transport::MAX_MESSAGE_SIZE;
