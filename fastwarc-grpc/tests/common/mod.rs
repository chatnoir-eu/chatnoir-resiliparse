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

//! Shared helpers for the `fastwarc-grpc` integration tests.
//!
//! This module is compiled into every integration-test target, but each
//! target uses only a subset of the helpers.
#![allow(dead_code)]

use std::net::SocketAddr;
use std::path::PathBuf;

use fastwarc::warc::iter::ArchiveIteratorOptions;
use fastwarc_grpc::proto::fastwarc::v1 as pb;
use fastwarc_grpc::proto::fastwarc::v1::warc_service_client::WarcServiceClient;
use fastwarc_grpc::proto::fastwarc::v1::warc_service_server::WarcServiceServer;
use fastwarc_grpc::warc_service::WarcParser;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Channel;

/// Path to an existing repository test fixture.
pub fn data_path(name: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repository_fixture = manifest_dir.join("../tests/data").join(name);
    if repository_fixture.exists() {
        repository_fixture
    } else {
        manifest_dir.join("../fastwarc-rs/tests/fixtures").join(name)
    }
}

/// Iterator options matching what the service applies for a given config.
pub fn direct_options(config: &pb::ParseWarcConfig) -> ArchiveIteratorOptions {
    ArchiveIteratorOptions {
        stream_detect: config.stream_detect.unwrap_or(true),
        // The service parses HTTP manually per record; direct comparison
        // runs must do the same.
        parse_http: false,
        decode_http_payload: fastwarc_grpc::convert::auto_decode(config.decode_http_payload),
        verify_digests: config.verify_digests,
        quirks_mode: config.quirks_mode,
        max_header_len: if config.max_header_len == 0 {
            32 << 10
        } else {
            config.max_header_len as usize
        },
        inplace: false,
    }
}

/// Start a `WarcService` server on an ephemeral localhost port.
pub async fn start_warc_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        fastwarc_grpc::transport::configure_server(tonic::transport::Server::builder())
            .add_service(fastwarc_grpc::transport::configure_warc_server(WarcServiceServer::new(
                WarcParser::with_local_files(),
            )))
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    addr
}

/// Build a request sequence: one `config` message followed by the archive
/// bytes in `chunk_size` slices.
pub fn warc_requests(data: &[u8], chunk_size: usize, config: &pb::ParseWarcConfig) -> Vec<pb::ParseWarcRequest> {
    let mut requests = vec![pb::ParseWarcRequest {
        kind: Some(pb::parse_warc_request::Kind::Config(config.clone())),
    }];
    requests.extend(data.chunks(chunk_size).map(|chunk| pb::ParseWarcRequest {
        kind: Some(pb::parse_warc_request::Kind::Chunk(chunk.to_vec().into())),
    }));
    requests
}

/// Stream a fixture file through an in-process server and collect all
/// response messages. Panics on transport-level errors.
pub async fn collect_warc(file: &str, chunk_size: usize, config: &pb::ParseWarcConfig) -> Vec<pb::ParseWarcResponse> {
    let addr = start_warc_server().await;
    let mut client = WarcServiceClient::new(
        fastwarc_grpc::transport::connect(format!("http://{addr}"))
            .await
            .unwrap(),
    )
    .max_decoding_message_size(fastwarc_grpc::transport::MAX_MESSAGE_SIZE)
    .max_encoding_message_size(fastwarc_grpc::transport::MAX_MESSAGE_SIZE);
    let data = std::fs::read(data_path(file)).unwrap();
    let requests = warc_requests(&data, chunk_size, config);
    let mut stream = client
        .parse_warc(tokio_stream::iter(requests))
        .await
        .unwrap()
        .into_inner();
    let mut responses = Vec::new();
    while let Some(resp) = stream.message().await.unwrap() {
        responses.push(resp);
    }
    responses
}

/// A three-record archive whose second record has corrupt WARC framing.
///
/// Parsers see one good record, then a framing failure; the third record is
/// unreachable because the crate does not resume after framing errors.
pub fn corrupt_archive() -> Vec<u8> {
    use std::io::Write;

    let mut data = Vec::new();
    write!(
        data,
        "WARC/1.0\r\nWARC-Type: warcinfo\r\nWARC-Record-ID: <urn:uuid:11111111-1111-1111-1111-111111111111>\r\nWARC-Date: 2020-01-01T00:00:00Z\r\nContent-Length: 3\r\n\r\nABC\r\n\r\n"
    )
    .unwrap();
    data.extend_from_slice(b"NOT-A-WARC-HEADER\r\n\r\n");
    write!(
        data,
        "WARC/1.0\r\nWARC-Type: resource\r\nWARC-Record-ID: <urn:uuid:22222222-2222-2222-2222-222222222222>\r\nWARC-Date: 2020-01-01T00:00:00Z\r\nContent-Length: 1\r\n\r\nX\r\n\r\n"
    )
    .unwrap();
    data
}

/// One record's complete response sequence, or a record-level parse error.
#[derive(Debug)]
pub enum RecordOutcome {
    /// A complete `record_start` / `payload_chunk`* / `record_end` sequence.
    Record {
        /// Metadata from the `record_start` message.
        metadata: Box<pb::RecordMetadata>,
        /// Reassembled payload bytes from all `payload_chunk` messages.
        payload: Vec<u8>,
        /// The closing `record_end` message.
        end: pb::RecordEnd,
    },
    /// A `record_error` message (always outside any record sequence).
    Error(pb::RecordError),
}

/// Group a flat response stream into per-record outcomes, asserting the
/// protocol invariants: record sequences nest properly, chunk offsets are
/// contiguous from zero, and
/// `record_end.payload_length` matches the streamed chunk bytes.
///
/// `batch` messages are flattened first so batched and unbatched streams
/// share the same assertions.
pub fn group_responses(responses: &[pb::ParseWarcResponse]) -> Vec<RecordOutcome> {
    let mut outcomes = Vec::new();
    // (metadata, payload bytes) of the currently open record.
    let mut open: Option<(pb::RecordMetadata, Vec<u8>)> = None;
    for_each_event(responses, |resp| match resp.kind.as_ref().unwrap() {
        pb::parse_warc_response::Kind::RecordStart(start) => {
            let metadata = start.metadata.clone().unwrap();
            assert!(open.is_none(), "record_start while record still open");
            open = Some((metadata, Vec::new()));
        }
        pb::parse_warc_response::Kind::PayloadChunk(chunk) => {
            let (_, payload) = open.as_mut().expect("payload_chunk outside of record");
            assert_eq!(chunk.offset, u64::try_from(payload.len()).unwrap(), "non-contiguous payload chunk offset");
            payload.extend_from_slice(&chunk.data);
        }
        pb::parse_warc_response::Kind::RecordEnd(end) => {
            let (metadata, payload) = open.take().expect("record_end outside of record");
            assert_eq!(
                end.payload_length,
                u64::try_from(payload.len()).unwrap(),
                "payload_length does not match streamed bytes"
            );
            outcomes.push(RecordOutcome::Record {
                metadata: Box::new(metadata),
                payload,
                end: *end,
            });
        }
        pb::parse_warc_response::Kind::RecordError(e) => {
            assert!(open.take().is_none(), "record_error in the middle of a record: {}", e.message);
            outcomes.push(RecordOutcome::Error(e.clone()));
        }
        pb::parse_warc_response::Kind::Batch(_) => {
            panic!("flattening missed a nested batch");
        }
    });
    assert!(open.is_none(), "stream ended with an open record");
    outcomes
}

/// Walk `record_start` / `payload_chunk` / `record_end` / `record_error`
/// events, flattening `batch` messages in stream order.
pub fn for_each_event(responses: &[pb::ParseWarcResponse], mut visit: impl FnMut(&pb::ParseWarcResponse)) {
    fn walk(resp: &pb::ParseWarcResponse, visit: &mut impl FnMut(&pb::ParseWarcResponse)) {
        if let Some(pb::parse_warc_response::Kind::Batch(batch)) = resp.kind.as_ref() {
            for item in &batch.items {
                walk(item, visit);
            }
        } else {
            visit(resp);
        }
    }
    for resp in responses {
        walk(resp, &mut visit);
    }
}

/// Convenience: extract only the successful record outcomes.
pub fn records_only(outcomes: &[RecordOutcome]) -> Vec<(&pb::RecordMetadata, &[u8], &pb::RecordEnd)> {
    outcomes
        .iter()
        .filter_map(|o| match o {
            RecordOutcome::Record { metadata, payload, end } => Some((&**metadata, payload.as_slice(), end)),
            RecordOutcome::Error(_) => None,
        })
        .collect()
}

/// Connect a `WarcServiceClient` to a fresh in-process server.
pub async fn warc_client() -> WarcServiceClient<Channel> {
    let addr = start_warc_server().await;
    WarcServiceClient::new(
        fastwarc_grpc::transport::connect(format!("http://{addr}"))
            .await
            .unwrap(),
    )
    .max_decoding_message_size(fastwarc_grpc::transport::MAX_MESSAGE_SIZE)
    .max_encoding_message_size(fastwarc_grpc::transport::MAX_MESSAGE_SIZE)
}
