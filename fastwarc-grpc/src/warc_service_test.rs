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

use super::*;
use prost::Message;

#[test]
fn unary_budget_includes_protobuf_framing() {
    let metadata = pb::RecordMetadata::default();
    let start = pb::ParseWarcResponse {
        kind: Some(pb::parse_warc_response::Kind::RecordStart(pb::RecordStart {
            metadata: Some(metadata.clone()),
        })),
    };
    for length in [0, 1, 127, 128, 16_383, 16_384] {
        let chunk = pb::ParseWarcResponse {
            kind: Some(pb::parse_warc_response::Kind::PayloadChunk(pb::PayloadChunk {
                offset: 0,
                data: vec![0; length].into(),
            })),
        };
        let response = pb::ParseArchiveResponse {
            records: vec![pb::ParsedRecord {
                metadata: Some(metadata.clone()),
                payload: vec![0; length],
            }],
            errors: vec![],
        };
        assert!(event_size(&start) + event_size(&chunk) >= response.encoded_len());
    }
    let error = pb::RecordError {
        message: "bad record".into(),
        ..Default::default()
    };
    let response = pb::ParseArchiveResponse {
        records: vec![],
        errors: vec![error.clone()],
    };
    let event = pb::ParseWarcResponse {
        kind: Some(pb::parse_warc_response::Kind::RecordError(error)),
    };
    assert!(event_size(&event) >= response.encoded_len());
}

/// Cancelling a response must release the input channel even while the parser
/// is skipping a payload and the client keeps its upload open.
#[tokio::test]
async fn response_cancellation_stops_upload_and_parser() {
    let (request_tx, request_rx) = mpsc::channel(1);
    let (chunk_tx, chunk_rx) = mpsc::channel(1);
    let (response_tx, mut response_rx) = mpsc::channel(1);
    let config = pb::ParseWarcConfig {
        parse_http: Some(false),
        include_payload: Some(false),
        include_headers: Some(false),
        ..Default::default()
    };
    let mut parser = spawn_parser(ParserInput::Chunks(chunk_rx), response_tx.clone(), config);
    let mut forwarder = tokio::spawn(forward_chunks(ReceiverStream::new(request_rx), chunk_tx, response_tx));
    request_tx
        .send(Ok(pb::ParseWarcRequest {
            kind: Some(pb::parse_warc_request::Kind::Chunk(Bytes::from_static(
                b"WARC/1.0\r\nWARC-Type: resource\r\nContent-Length: 1000000\r\n\r\nbody",
            ))),
        }))
        .await
        .unwrap();

    // The record's start/end events precede the iterator's payload skip.
    for _ in 0..2 {
        tokio::time::timeout(std::time::Duration::from_secs(10), response_rx.recv())
            .await
            .expect("parser did not emit the record")
            .unwrap()
            .unwrap();
    }
    assert!(!parser.is_finished());
    drop(response_rx);
    let stopped = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        (&mut forwarder).await.unwrap();
        (&mut parser).await.unwrap();
    })
    .await;
    // Unblock both tasks on failure before the test runtime waits for blocking tasks.
    drop(request_tx);
    if stopped.is_err() {
        if !forwarder.is_finished() {
            forwarder.await.unwrap();
        }
        if !parser.is_finished() {
            parser.await.unwrap();
        }
    }
    assert!(stopped.is_ok(), "cancelled upload left the forwarder or parser running");
}

#[tokio::test]
async fn response_cancellation_stops_forwarder_with_full_input_channel() {
    let (request_tx, request_rx) = mpsc::channel(1);
    let (chunk_tx, chunk_rx) = mpsc::channel(1);
    let (response_tx, response_rx) = mpsc::channel(1);
    chunk_tx.send(Bytes::from_static(b"full")).await.unwrap();
    request_tx
        .send(Ok(pb::ParseWarcRequest {
            kind: Some(pb::parse_warc_request::Kind::Chunk(Bytes::from_static(b"next"))),
        }))
        .await
        .unwrap();
    let mut forwarder = tokio::spawn(forward_chunks(ReceiverStream::new(request_rx), chunk_tx, response_tx));
    // Wait until the request is consumed; its chunk cannot enter the full channel.
    drop(
        tokio::time::timeout(std::time::Duration::from_secs(10), request_tx.reserve())
            .await
            .expect("forwarder did not read the request")
            .unwrap(),
    );
    drop(response_rx);
    let stopped = tokio::time::timeout(std::time::Duration::from_secs(10), &mut forwarder).await;
    drop(chunk_rx);
    if stopped.is_err() {
        forwarder.await.unwrap();
    }
    stopped
        .expect("cancelled forwarder stayed blocked on the input channel")
        .unwrap();
}

/// Wait for the blocking parser to exit and release its directory handle.
#[tokio::test]
async fn local_file_parser_finishes_after_response_cancellation() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fastwarc-rs/tests/fixtures");
    let root = WarcParser::with_local_files(path).unwrap().local_file_root.unwrap();
    let root_ref = Arc::downgrade(&root);
    let (response_tx, mut response_rx) = mpsc::channel(1);
    let config = pb::ParseWarcConfig {
        parse_http: Some(false),
        payload_chunk_size: 1,
        response_batch_size: 8,
        ..Default::default()
    };
    let parser = spawn_parser(ParserInput::LocalFile(root, "warcfile.warc".into()), response_tx, config);
    tokio::time::timeout(std::time::Duration::from_secs(10), response_rx.recv())
        .await
        .expect("local-file parser did not emit a batch")
        .unwrap()
        .unwrap();
    assert!(!parser.is_finished());
    drop(response_rx);
    tokio::time::timeout(std::time::Duration::from_secs(10), parser)
        .await
        .expect("local-file parser did not stop after cancellation")
        .unwrap();
    assert!(root_ref.upgrade().is_none(), "parser retained the local directory handle");
}

#[test]
fn response_cancellation_stops_an_unflushed_batch() {
    let (tx, rx) = mpsc::channel(1);
    let mut emitter = BatchEmitter::new(&tx, 64);
    let event = pb::ParseWarcResponse {
        kind: Some(pb::parse_warc_response::Kind::RecordEnd(pb::RecordEnd { payload_length: 0 })),
    };
    assert!(emitter.emit(event.clone()));
    drop(rx);
    assert!(!emitter.emit(event), "batch kept accepting events after cancellation");
}

/// Keep parsing active until validation finishes so scheduling cannot hide errors.
#[tokio::test]
async fn local_file_requests_reject_invalid_messages_while_parsing() {
    for message in [
        Ok(pb::ParseWarcRequest {
            kind: Some(pb::parse_warc_request::Kind::Config(pb::ParseWarcConfig::default())),
        }),
        Ok(pb::ParseWarcRequest { kind: None }),
        Err(Status::data_loss("upload failed")),
    ] {
        let expected = message
            .as_ref()
            .err()
            .map_or(tonic::Code::InvalidArgument, Status::code);
        let (finish_tx, finish_rx) = tokio::sync::oneshot::channel::<()>();
        let parser = tokio::spawn(async move {
            let _ = finish_rx.await;
        });
        let (response_tx, mut response_rx) = mpsc::channel(1);
        let stream = tokio_stream::iter([
            Ok(pb::ParseWarcRequest {
                kind: Some(pb::parse_warc_request::Kind::Chunk(Bytes::from_static(b"ignored"))),
            }),
            message,
        ]);
        tokio::time::timeout(std::time::Duration::from_secs(10), validate_local_requests(stream, response_tx, parser))
            .await
            .expect("local-file validation did not finish");
        let status = response_rx.recv().await.unwrap().unwrap_err();
        assert_eq!(status.code(), expected);
        assert!(response_rx.recv().await.is_none());
        finish_tx.send(()).unwrap();
    }
}
