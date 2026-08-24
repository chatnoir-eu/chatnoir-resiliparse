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

//! Shared HTTP/2 settings for WARC transfer.

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
/// HTTP/2 DATA frame size: 1 MiB.
pub const HTTP2_MAX_FRAME: u32 = 1024 * 1024;
/// gRPC message size limit: 16 MiB.
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

/// Apply the shared HTTP/2 settings to a tonic server builder.
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

/// Apply the shared HTTP/2 settings to a client endpoint.
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
