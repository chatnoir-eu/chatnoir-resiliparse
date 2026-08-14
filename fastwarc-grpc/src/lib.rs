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

//! FastWARC-gRPC exposes the [FastWARC](https://docs.rs/fastwarc) WARC parser as a streaming gRPC
//! service. Clients in any language with gRPC support can send WARC archives (uncompressed, Gzip,
//! Zstd, or LZ4) and receive parsed records with lossless headers and streamed payloads. The
//! service is binary protobuf only, with no JSON transcoding.
//!
//! FastWARC-gRPC belongs to the [ChatNoir Resiliparse toolkit](https://resiliparse.chatnoir.eu/en/stable/index.html)
//! for fast and robust web data processing. The `fastwarc` library itself carries no gRPC
//! dependencies; this crate is a separate server on top of it.
//!
//! # Running the Server
//!
//! Build and run from the repository root. The build requires `protoc` on the PATH.
//!
//! ```bash
//! FASTWARC_GRPC_ADDR="[::]:50051" cargo run --release -p fastwarc-grpc
//! ```
//!
//! `FASTWARC_GRPC_ADDR` defaults to `[::]:50051`. A Unix socket is accepted as `unix:///path.sock`
//! or as an absolute filesystem path. The server shuts down gracefully on SIGINT or SIGTERM.
//! Besides [`WarcService`](proto::fastwarc::v1::warc_service_server::WarcService), it serves the
//! standard `grpc.health.v1.Health` service for load-balancer probes and gRPC server reflection
//! (v1), so generic tools can discover the API without local proto files:
//!
//! ```bash
//! grpcurl -plaintext localhost:50051 describe fastwarc.v1.WarcService
//! ```
//!
//! # Parsing a Small Archive in One Call
//!
//! `ParseArchive` is the unary entry point for single records and archives that fit within the
//! gRPC message size limits. This server accepts 16 MiB ([`transport::MAX_MESSAGE_SIZE`]); many
//! clients default to 4 MiB and must raise their decode limit to match. One request carries the
//! configuration and the complete archive; the response carries every kept record with its
//! metadata, payload, and digest results.
//!
//! ```no_run
//! use fastwarc_grpc::proto::fastwarc::v1 as pb;
//! use fastwarc_grpc::proto::fastwarc::v1::warc_service_client::WarcServiceClient;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut client = WarcServiceClient::new(fastwarc_grpc::transport::connect("http://localhost:50051").await?)
//!     .max_decoding_message_size(fastwarc_grpc::transport::MAX_MESSAGE_SIZE)
//!     .max_encoding_message_size(fastwarc_grpc::transport::MAX_MESSAGE_SIZE);
//! let response = client
//!     .parse_archive(pb::ParseArchiveRequest {
//!         config: Some(pb::ParseWarcConfig { parse_http: true, ..Default::default() }),
//!         archive: std::fs::read("warcfile.warc.gz")?.into(),
//!     })
//!     .await?
//!     .into_inner();
//!
//! for record in &response.records {
//!     let metadata = record.metadata.as_ref().expect("record metadata is always set");
//!     println!(
//!         "{}: {} payload bytes",
//!         metadata.record_id.as_deref().unwrap_or("-"),
//!         record.payload.len()
//!     );
//! }
//! # Ok(())
//! # }
//! ```
//!
//! Compression is autodetected from magic bytes, so the same call works for `.warc`, `.warc.gz`,
//! `.warc.zst`, and `.warc.lz4` inputs.
//!
//! # Streaming Large Archives
//!
//! `ParseWarc` is the bidirectional streaming RPC for archives of arbitrary size. Memory stays
//! bounded on both sides. The client sends one `config` message first, then any number of `chunk`
//! messages with raw archive bytes. Chunk boundaries are arbitrary; the server concatenates them.
//! Set `archive_path` on the config to have the server open a local file instead of uploading
//! chunks (requires a server built with `WarcParser::with_local_files`, or
//! `FASTWARC_GRPC_ALLOW_LOCAL_FILES=1` for the bundled binary; otherwise `PermissionDenied`).
//!
//! For every kept record the server responds with an ordered sequence: one `record_start`
//! carrying all metadata, zero or more offset-tagged `payload_chunk` messages, and one
//! `record_end` with the total payload length and digest results.
//!
//! ```no_run
//! use fastwarc_grpc::proto::fastwarc::v1 as pb;
//! use fastwarc_grpc::proto::fastwarc::v1::warc_service_client::WarcServiceClient;
//! use tokio_stream::wrappers::ReceiverStream;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let (tx, rx) = tokio::sync::mpsc::channel(4);
//! tx.send(pb::ParseWarcRequest {
//!     kind: Some(pb::parse_warc_request::Kind::Config(pb::ParseWarcConfig {
//!         parse_http: true,
//!         verify_digests: true,
//!         ..Default::default()
//!     })),
//! })
//! .await?;
//! tokio::spawn(async move {
//!     for chunk in std::fs::read("warcfile.warc.gz").expect("readable file").chunks(64 << 10) {
//!         let request = pb::ParseWarcRequest {
//!             kind: Some(pb::parse_warc_request::Kind::Chunk(chunk.to_vec().into())),
//!         };
//!         if tx.send(request).await.is_err() {
//!             break; // Server closed the stream.
//!         }
//!     }
//! });
//!
//! let mut client = WarcServiceClient::new(fastwarc_grpc::transport::connect("http://localhost:50051").await?)
//!     .max_decoding_message_size(fastwarc_grpc::transport::MAX_MESSAGE_SIZE)
//!     .max_encoding_message_size(fastwarc_grpc::transport::MAX_MESSAGE_SIZE);
//! let mut stream = client.parse_warc(ReceiverStream::new(rx)).await?.into_inner();
//! while let Some(response) = stream.message().await? {
//!     handle_response(response);
//! }
//! # Ok(())
//! # }
//!
//! fn handle_response(response: pb::ParseWarcResponse) {
//!     match response.kind {
//!         Some(pb::parse_warc_response::Kind::RecordStart(start)) => {
//!             let metadata = start.metadata.unwrap_or_default();
//!             println!("record {} at offset {}", metadata.record_index, metadata.stream_pos);
//!         }
//!         Some(pb::parse_warc_response::Kind::PayloadChunk(chunk)) => {
//!             println!("  {} payload bytes at offset {}", chunk.data.len(), chunk.offset);
//!         }
//!         Some(pb::parse_warc_response::Kind::RecordEnd(end)) => {
//!             println!("  done, {} bytes total", end.payload_length);
//!         }
//!         Some(pb::parse_warc_response::Kind::RecordError(error)) => {
//!             eprintln!("record error (recoverable: {}): {}", error.recoverable, error.message);
//!         }
//!         Some(pb::parse_warc_response::Kind::Batch(batch)) => {
//!             for item in batch.items {
//!                 handle_response(item);
//!             }
//!         }
//!         None => {}
//!     }
//! }
//! ```
//!
//! This example reads the whole file up front for brevity. `examples/parse.rs` shows the
//! bounded-memory shape a real client should use: a reader thread feeding a bounded channel, so
//! archives larger than memory stream fine.
//!
//! # Configuration and Filters
//!
//! [`ParseWarcConfig`](proto::fastwarc::v1::ParseWarcConfig) mirrors the parse and filter options
//! of the local `ArchiveIterator` where they make sense on a remote stream: `parse_http`,
//! `decode_http_payload`, `verify_digests`, `quirks_mode`, `max_header_len`, `record_types`,
//! `min_content_length`, `max_content_length`, `stream_detect`, `input_buffer_size`,
//! `include_payload`, `include_headers`, `response_batch_size`, and `archive_path`.
//! Filtered-out records are skipped silently, matching local iteration.
//!
//! Two differences from the local APIs matter:
//!
//! * `parse_http` defaults to **false** (the proto3 zero value). Python and Rust default to true.
//!   Set it explicitly for parity.
//! * Arbitrary filter callables cannot travel over gRPC. The built-in predicates are available as
//!   [`BuiltinFilter`](proto::fastwarc::v1::BuiltinFilter) values in `filters`; anything custom
//!   should be filtered client-side from the streamed metadata.
//!
//! # Digest Verification
//!
//! With `verify_digests` set, the server verifies `WARC-Block-Digest` against the raw record
//! block before any HTTP parsing, then verifies `WARC-Payload-Digest`. Results are reported per
//! record as [`DigestStatus`](proto::fastwarc::v1::DigestStatus) values in `record_end`. Unlike
//! local iteration, which skips records with missing or failed block digests, the service always
//! emits the record and lets the client decide.
//!
//! # Error Handling
//!
//! Record-level problems arrive as `record_error` messages, not gRPC status codes:
//!
//! * An HTTP header parse failure on an already-framed record is **recoverable**. The record is
//!   reported without parsed HTTP headers and the stream continues.
//! * A WARC framing failure (invalid header, truncated stream) is **non-recoverable** and ends
//!   the response stream, because the parser cannot find the next record boundary.
//!
//! Protocol violations (missing or duplicate `config`, empty request `kind`) fail the RPC with
//! `InvalidArgument`.
//!
//! # Embedding the Server
//!
//! The service implementation is the [`warc_service::WarcParser`] struct. It is stateless, so it
//! can be mounted in an existing tonic server alongside other services. `WarcParser::new()`
//! rejects `archive_path` requests with `PermissionDenied`; construct it with
//! [`warc_service::WarcParser::with_local_files`] to let clients open files on the server
//! (only when every client is trusted with read access to the server's files):
//!
//! ```no_run
//! use fastwarc_grpc::proto::fastwarc::v1::warc_service_server::WarcServiceServer;
//! use fastwarc_grpc::warc_service::WarcParser;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! fastwarc_grpc::transport::configure_server(tonic::transport::Server::builder())
//!     .add_service(fastwarc_grpc::transport::configure_warc_server(WarcServiceServer::new(WarcParser::new())))
//!     .serve("[::1]:50051".parse()?)
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Clients in Other Languages
//!
//! The protobuf contracts live in `proto/` and follow the buf style guide, so stubs can be
//! generated for any gRPC language with `buf generate` (see `proto/buf.gen.yaml`) or plain
//! `protoc`. Ad-hoc calls work without any codegen through server reflection:
//!
//! ```bash
//! grpcurl -plaintext \
//!   -d "{\"config\":{}, \"archive\":\"$(base64 -w0 record.warc)\"}" \
//!   localhost:50051 fastwarc.v1.WarcService/ParseArchive
//! ```
//!
//! See `DESIGN.md` for the architecture and the lossless data mapping, and `README.md` for the
//! full Python-parity table.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]

pub mod convert;
pub mod transport;
pub mod warc_service;

/// Generated protobuf and gRPC stubs.
///
/// The stubs carry no doc comments or clippy annotations of their own; the
/// commented, linted source of truth is the schema in `proto`.
#[allow(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
pub mod proto {
    /// Encoded descriptor set of the `fastwarc.v1` package, for server
    /// reflection.
    pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("descriptor");

    /// Stubs for the `fastwarc.v1` package.
    pub mod fastwarc {
        /// Messages and `WarcService` server/client stubs.
        pub mod v1 {
            tonic::include_proto!("fastwarc.v1");
        }
    }
}
