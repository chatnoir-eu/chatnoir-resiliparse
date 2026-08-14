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

//! HTTP/2 settings for bulk WARC transfer.
//!
//! Tonic/h2 default to a 64 KiB flow-control window, which starves a
//! multi-gigabyte archive stream. The server binary, the loopback
//! benchmark, and the integration tests share these helpers.

use tonic::transport::{Endpoint, Server};

use crate::proto::fastwarc::v1::warc_service_server::WarcServiceServer;

/// Connection-level HTTP/2 window.
///
/// Override with `FASTWARC_HTTP2_CONNECTION_WINDOW` (bytes, minimum 65535).
pub const HTTP2_CONNECTION_WINDOW: u32 = 32 * 1024 * 1024;
/// Per-stream HTTP/2 window. One `ParseWarc` RPC carries the whole archive.
///
/// Override with `FASTWARC_HTTP2_STREAM_WINDOW` (bytes, minimum 65535).
pub const HTTP2_STREAM_WINDOW: u32 = 16 * 1024 * 1024;
/// Largest HTTP/2 DATA frame. RFC 7540 allows up to 16 777 215 bytes; 1 MiB
/// keeps syscall count down without pinning large contiguous buffers.
pub const HTTP2_MAX_FRAME: u32 = 1024 * 1024;
/// gRPC message size cap. Covers a large `chunk` or a filled `batch`
/// (batches flush at 2 MiB). Tonic's default is 4 MiB.
pub const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

fn env_window(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|&value| value >= 65_535)
        .unwrap_or(default)
}

/// Effective connection-level HTTP/2 window after env override.
#[must_use]
pub fn connection_window() -> u32 {
    env_window("FASTWARC_HTTP2_CONNECTION_WINDOW", HTTP2_CONNECTION_WINDOW)
}

/// Effective per-stream HTTP/2 window after env override.
#[must_use]
pub fn stream_window() -> u32 {
    env_window("FASTWARC_HTTP2_STREAM_WINDOW", HTTP2_STREAM_WINDOW)
}

/// Apply bulk-transfer HTTP/2 settings to a tonic server builder.
///
/// Adaptive (BDP-probing) windows are deliberately off: they override the
/// fixed windows below and their ping-based estimator can stall a stream
/// that is saturated in both directions. Fixed large windows are the right
/// shape for bulk archive transfer.
#[must_use]
pub fn configure_server(builder: Server) -> Server {
    builder
        .tcp_nodelay(true)
        .initial_connection_window_size(Some(connection_window()))
        .initial_stream_window_size(Some(stream_window()))
        .max_frame_size(Some(HTTP2_MAX_FRAME))
}

/// Raise the generated service's encode/decode message caps for bulk WARC.
#[must_use]
pub fn configure_warc_server<S>(svc: WarcServiceServer<S>) -> WarcServiceServer<S> {
    svc.max_decoding_message_size(MAX_MESSAGE_SIZE)
        .max_encoding_message_size(MAX_MESSAGE_SIZE)
}

/// Apply the matching client-side HTTP/2 settings.
///
/// HTTP/2 flow control is the minimum of both peers; tuning only the server
/// leaves the client at 64 KiB and the stream stays starved.
#[must_use]
pub fn configure_endpoint(endpoint: Endpoint) -> Endpoint {
    endpoint
        .tcp_nodelay(true)
        .initial_connection_window_size(connection_window())
        .initial_stream_window_size(stream_window())
        .max_frame_size(HTTP2_MAX_FRAME)
}

/// Dial a FastWARC-gRPC endpoint with the same HTTP/2 settings as [`configure_endpoint`].
///
/// # Errors
///
/// Returns a transport error if the URI is invalid or the connection fails.
pub async fn connect(uri: impl Into<String>) -> Result<tonic::transport::Channel, tonic::transport::Error> {
    configure_endpoint(Endpoint::from_shared(uri.into())?).connect().await
}
