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

//! Integration tests for `WarcService.ParseWarc`: fixtures from
//! `../tests/data` are streamed through an in-process server and compared
//! against direct `ArchiveIterator` runs with matching options.

mod common;

use std::io::Read;

use common::{RecordOutcome, collect_warc, data_path, direct_options, group_responses, records_only};
use fastwarc::warc::iter::{ArchiveIterator, filter};
use fastwarc_grpc::convert;
use fastwarc_grpc::proto::fastwarc::v1 as pb;

/// Most tests leave HTTP parsing off unless it is part of the behavior under
/// test. An unset `parse_http` is covered separately as the local-API default.
fn default_config() -> pb::ParseWarcConfig {
    pb::ParseWarcConfig {
        parse_http: Some(false),
        ..Default::default()
    }
}

/// Iterate a fixture directly and return the record types in stream order.
fn direct_record_types(file: &str, config: &pb::ParseWarcConfig) -> Vec<pb::WarcRecordType> {
    let reader = std::fs::File::open(data_path(file)).unwrap();
    ArchiveIterator::with_options(reader, direct_options(config))
        .map(|r| convert::warc_record_type(r.unwrap().borrow().record_type()))
        .collect()
}

/// The reference plain-WARC stream: chunk contiguity, per-record pairing, and
/// record count/type parity with a direct iterator run.
#[tokio::test]
async fn stream_warcfile_plain() {
    let config = default_config();
    let responses = collect_warc("warcfile.warc", 8 << 10, &config).await;
    let outcomes = group_responses(&responses);
    assert!(outcomes.iter().all(|o| matches!(o, RecordOutcome::Record { .. })));
    assert_eq!(outcomes.len(), 50);

    let direct_types = direct_record_types("warcfile.warc", &config);
    assert_eq!(outcomes.len(), direct_types.len());
    for (outcome, expected_type) in outcomes.iter().zip(&direct_types) {
        let RecordOutcome::Record { metadata, .. } = outcome else {
            unreachable!();
        };
        assert_eq!(metadata.record_type, *expected_type as i32);
    }
}

/// An unset `parse_http` follows the `FastWARC` default and parses embedded
/// HTTP messages.
#[tokio::test]
async fn parse_http_defaults_true() {
    let responses = collect_warc("warcfile.warc", 8 << 10, &pb::ParseWarcConfig::default()).await;
    let outcomes = group_responses(&responses);
    let records = records_only(&outcomes);
    assert!(records.iter().any(|(metadata, _, _)| metadata.is_http));
    assert!(
        records
            .iter()
            .filter(|(metadata, _, _)| metadata.is_http)
            .all(|(metadata, _, _)| metadata.http_parsed)
    );
}

/// Gzip-, lz4-, and zstd-compressed input is detected transparently from magic
/// bytes (odd chunk size stresses reassembly across channel boundaries).
#[tokio::test]
async fn stream_warcfile_compressed() {
    for file in ["warcfile.warc.gz", "warcfile.warc.lz4", "warcfile.warc.zst"] {
        let config = default_config();
        let responses = collect_warc(file, 997, &config).await;
        let outcomes = group_responses(&responses);
        assert!(
            outcomes.iter().all(|o| matches!(o, RecordOutcome::Record { .. })),
            "unexpected record_error for {file}"
        );

        let direct_types = direct_record_types(file, &config);
        assert_eq!(outcomes.len(), direct_types.len(), "record count mismatch for {file}");
        for (outcome, expected_type) in outcomes.iter().zip(&direct_types) {
            let RecordOutcome::Record { metadata, .. } = outcome else {
                unreachable!();
            };
            assert_eq!(metadata.record_type, *expected_type as i32);
        }
    }
}

/// clipped.warc.gz contains a single record whose payload is truncated: the
/// gzip container is intact, so iteration ends cleanly and the truncation is
/// observable only as a payload shorter than the declared Content-Length.
/// The service must reproduce exactly that (no `record_error`, short payload).
#[tokio::test]
async fn stream_clipped_gz() {
    let config = default_config();
    let responses = collect_warc("clipped.warc.gz", 4096, &config).await;
    let outcomes = group_responses(&responses);

    // Direct reference run.
    let reader = std::fs::File::open(data_path("clipped.warc.gz")).unwrap();
    let mut direct = Vec::new();
    let mut direct_errors = Vec::new();
    for item in ArchiveIterator::with_options(reader, direct_options(&config)) {
        match item {
            Ok(record) => {
                let mut record = record.borrow_mut();
                let mut payload = Vec::new();
                record.reader_mut().unwrap().read_to_end(&mut payload).unwrap();
                direct.push((record.content_length(), payload.len()));
            }
            Err(e) => direct_errors.push(e.to_string()),
        }
    }

    let errors: Vec<_> = outcomes
        .iter()
        .filter_map(|o| match o {
            RecordOutcome::Error(e) => Some(e),
            RecordOutcome::Record { .. } => None,
        })
        .collect();
    assert_eq!(errors.len(), direct_errors.len());
    for (streamed, direct) in errors.iter().zip(&direct_errors) {
        assert!(streamed.recoverable);
        assert_eq!(&streamed.message, direct);
    }

    let records = records_only(&outcomes);
    assert_eq!(records.len(), direct.len());
    for ((metadata, payload, end), (declared, direct_len)) in records.iter().zip(&direct) {
        assert_eq!(metadata.content_length, *declared);
        assert_eq!(u64::try_from(payload.len()).unwrap(), *direct_len as u64);
        assert_eq!(end.payload_length, *direct_len as u64);
    }
    assert_eq!(records.len(), 1);
    let (metadata, _, end) = records[0];
    // The record is clipped: the declared length exceeds the actual payload.
    assert!(end.payload_length < metadata.content_length, "clipped record should stream a short payload");
}

/// The clueweb-quirk fixture needs quirks mode to parse at all.
#[tokio::test]
async fn stream_clueweb_quirk() {
    let config = pb::ParseWarcConfig {
        quirks_mode: true,
        ..Default::default()
    };
    let responses = collect_warc("clueweb-quirk.warc.gz", 8 << 10, &config).await;
    let outcomes = group_responses(&responses);
    assert!(outcomes.iter().all(|o| matches!(o, RecordOutcome::Record { .. })));

    let direct_types = direct_record_types("clueweb-quirk.warc.gz", &config);
    assert_eq!(outcomes.len(), direct_types.len());
    assert_eq!(outcomes.len(), 30);
}

/// Digest verification has the same skip behavior as a direct iterator.
#[tokio::test]
async fn verify_digests() {
    let config = pb::ParseWarcConfig {
        parse_http: Some(true),
        verify_digests: true,
        ..Default::default()
    };
    let responses = collect_warc("warcfile.warc", 8 << 10, &config).await;
    let outcomes = group_responses(&responses);
    let records = records_only(&outcomes);
    let direct_types = direct_record_types("warcfile.warc", &config);
    assert_eq!(records.len(), direct_types.len());
    assert!(
        records
            .iter()
            .zip(direct_types)
            .all(|((metadata, _, _), record_type)| { metadata.record_type == record_type as i32 })
    );

    let unverified = collect_warc("warcfile.warc", 8 << 10, &default_config()).await;
    assert!(records.len() < records_only(&group_responses(&unverified)).len());
}

/// One directly-parsed record for the losslessness comparison.
struct DirectRecord {
    payload: Vec<u8>,
    warc_raw_block: Vec<u8>,
    warc_fields: Vec<(Vec<u8>, Vec<u8>)>,
    http_raw_block: Option<Vec<u8>>,
    http_fields: Vec<(Vec<u8>, Vec<u8>)>,
}

/// Losslessness: reassembled payloads, verbatim header blocks, and ordered
/// header field pairs (including duplicates) must match the direct parse.
#[tokio::test]
async fn lossless_payload_and_headers() {
    let config = pb::ParseWarcConfig {
        parse_http: Some(true),
        decode_http_payload: pb::AutoDecode::All as i32,
        ..Default::default()
    };
    let responses = collect_warc("warcfile.warc", 8 << 10, &config).await;
    let outcomes = group_responses(&responses);
    let records = records_only(&outcomes);

    let reader = std::fs::File::open(data_path("warcfile.warc")).unwrap();
    let mut direct_records = Vec::new();
    for item in ArchiveIterator::with_options(reader, direct_options(&config)) {
        let record = item.unwrap();
        let mut record = record.borrow_mut();

        let mut warc_raw_block = Vec::new();
        record.headers().write(&mut warc_raw_block).unwrap();
        let warc_fields = record
            .headers()
            .items_bytes()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        let (http_raw_block, http_fields) = if let Some(http) = record.http_headers() {
            let mut raw = Vec::new();
            http.write(&mut raw).unwrap();
            (
                Some(raw),
                http.items_bytes()
                    .map(|(k, v)| (k.into_owned(), v.into_owned()))
                    .collect(),
            )
        } else {
            (None, Vec::new())
        };
        let mut payload = Vec::new();
        record.reader_mut().unwrap().read_to_end(&mut payload).unwrap();
        direct_records.push(DirectRecord {
            payload,
            warc_raw_block,
            warc_fields,
            http_raw_block,
            http_fields,
        });
    }
    assert_eq!(records.len(), direct_records.len());

    let mut duplicate_header_records = 0;
    for ((metadata, payload, _), direct) in records.iter().zip(&direct_records) {
        assert_eq!(payload, &direct.payload, "payload mismatch at stream offset {}", metadata.stream_pos);

        let warc_headers = metadata.warc_headers.as_ref().unwrap();
        assert_eq!(warc_headers.raw_block, direct.warc_raw_block);
        let fields: Vec<(Vec<u8>, Vec<u8>)> = warc_headers
            .fields
            .iter()
            .map(|f| (f.name.clone(), f.value.clone()))
            .collect();
        assert_eq!(fields, direct.warc_fields, "WARC header fields mismatch at stream offset {}", metadata.stream_pos);

        match (&metadata.http_headers, &direct.http_raw_block) {
            (Some(streamed_http), Some(expected_raw)) => {
                assert_eq!(&streamed_http.raw_block, expected_raw);
                let http_fields: Vec<(Vec<u8>, Vec<u8>)> = streamed_http
                    .fields
                    .iter()
                    .map(|f| (f.name.clone(), f.value.clone()))
                    .collect();
                assert_eq!(http_fields, direct.http_fields, "HTTP header fields mismatch");

                // Duplicate header names (in order) must survive the round
                // trip; the full field-list equality above already proves
                // preservation, here we only verify the fixture exercises it.
                let mut names: Vec<&[u8]> = streamed_http.fields.iter().map(|f| f.name.as_slice()).collect();
                let total = names.len();
                names.sort_unstable();
                names.dedup();
                if names.len() != total {
                    duplicate_header_records += 1;
                }
            }
            (None, None) => {}
            _ => panic!("http_headers presence mismatch at stream offset {}", metadata.stream_pos),
        }
    }
    assert!(duplicate_header_records > 0, "fixture expected to contain duplicate HTTP headers");
}

/// The first request message must set `config`; anything else is rejected
/// with `InvalidArgument` before the stream starts.
#[tokio::test]
async fn first_message_must_be_config() {
    let mut client = common::warc_client().await;
    let requests = vec![pb::ParseWarcRequest {
        kind: Some(pb::parse_warc_request::Kind::Chunk(b"WARC/1.0\r\n".to_vec().into())),
    }];
    let status = client.parse_warc(tokio_stream::iter(requests)).await.unwrap_err();
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
}

/// A second `config` message mid-stream is a protocol violation and fails the
/// stream with `InvalidArgument`.
#[tokio::test]
async fn second_config_rejected() {
    let mut client = common::warc_client().await;
    let data = std::fs::read(data_path("warcfile.warc")).unwrap();
    let mut requests = vec![
        pb::ParseWarcRequest {
            kind: Some(pb::parse_warc_request::Kind::Config(default_config())),
        },
        pb::ParseWarcRequest {
            kind: Some(pb::parse_warc_request::Kind::Chunk(data[..8 << 10].to_vec().into())),
        },
        pb::ParseWarcRequest {
            kind: Some(pb::parse_warc_request::Kind::Config(default_config())),
        },
    ];
    requests.extend(data[8 << 10..].chunks(8 << 10).map(|chunk| pb::ParseWarcRequest {
        kind: Some(pb::parse_warc_request::Kind::Chunk(chunk.to_vec().into())),
    }));

    let mut stream = client
        .parse_warc(tokio_stream::iter(requests))
        .await
        .unwrap()
        .into_inner();
    let status = loop {
        match stream.message().await {
            Ok(Some(_)) => {}
            Ok(None) => panic!("stream ended without the expected InvalidArgument status"),
            Err(e) => break e,
        }
    };
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
}

/// An empty request `kind` after config is a protocol violation.
#[tokio::test]
async fn empty_kind_rejected() {
    let mut client = common::warc_client().await;
    let requests = vec![
        pb::ParseWarcRequest {
            kind: Some(pb::parse_warc_request::Kind::Config(default_config())),
        },
        pb::ParseWarcRequest { kind: None },
    ];

    let mut stream = client
        .parse_warc(tokio_stream::iter(requests))
        .await
        .unwrap()
        .into_inner();
    let status = loop {
        match stream.message().await {
            Ok(Some(_)) => {}
            Ok(None) => panic!("stream ended without the expected InvalidArgument status"),
            Err(e) => break e,
        }
    };
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
}

/// Empty archive: config only, no chunks: clean EOF, zero records.
#[tokio::test]
async fn empty_archive() {
    let mut client = common::warc_client().await;
    let requests = vec![pb::ParseWarcRequest {
        kind: Some(pb::parse_warc_request::Kind::Config(default_config())),
    }];
    let mut stream = client
        .parse_warc(tokio_stream::iter(requests))
        .await
        .unwrap()
        .into_inner();
    assert!(stream.message().await.unwrap().is_none());
}

/// Mid-stream WARC framing failure must emit one non-recoverable `record_error`
/// and end, never hang in a `No reader set` loop.
#[tokio::test]
async fn corrupt_midstream_ends_without_hang() {
    let data = common::corrupt_archive();
    let mut client = common::warc_client().await;
    let requests = common::warc_requests(&data, 64, &default_config());
    let mut stream = client
        .parse_warc(tokio_stream::iter(requests))
        .await
        .unwrap()
        .into_inner();

    let mut responses = Vec::new();
    while let Some(resp) = stream.message().await.unwrap() {
        responses.push(resp);
        assert!(responses.len() < 20, "parser appears to be looping on framing errors");
    }
    let outcomes = group_responses(&responses);
    assert!(matches!(
        outcomes.as_slice(),
        [RecordOutcome::Record { .. }, RecordOutcome::Error(error)] if !error.recoverable
    ));
}

/// Oversized HTTP headers fail recoverably; the following record still parses.
///
/// `max_header_len` applies to both WARC and HTTP headers, so the fixture keeps
/// WARC headers under the cap while the embedded HTTP block exceeds it.
#[tokio::test]
async fn http_parse_failure_is_recoverable() {
    use std::io::Write;

    let http_hdr = format!("HTTP/1.1 200 OK\r\nX: {}\r\n\r\nbody", "A".repeat(400));
    let payload = http_hdr.as_bytes();
    let mut data = Vec::new();
    write!(
        data,
        "WARC/1.0\r\nWARC-Type: response\r\nWARC-Record-ID: <urn:uuid:aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa>\r\nWARC-Date: 2020-01-01T00:00:00Z\r\nContent-Type: application/http; msgtype=response\r\nContent-Length: {}\r\n\r\n",
        payload.len()
    )
    .unwrap();
    data.extend_from_slice(payload);
    data.extend_from_slice(b"\r\n\r\n");
    write!(
        data,
        "WARC/1.0\r\nWARC-Type: resource\r\nWARC-Record-ID: <urn:uuid:bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb>\r\nWARC-Date: 2020-01-01T00:00:00Z\r\nContent-Length: 1\r\n\r\nZ\r\n\r\n"
    )
    .unwrap();

    let config = pb::ParseWarcConfig {
        parse_http: Some(true),
        max_header_len: 256,
        ..Default::default()
    };
    let mut client = common::warc_client().await;
    let requests = common::warc_requests(&data, 256, &config);
    let mut stream = client
        .parse_warc(tokio_stream::iter(requests))
        .await
        .unwrap()
        .into_inner();
    let mut responses = Vec::new();
    while let Some(resp) = stream.message().await.unwrap() {
        responses.push(resp);
    }
    let outcomes = group_responses(&responses);
    assert!(
        matches!(
            outcomes.as_slice(),
            [
                RecordOutcome::Error(error),
                RecordOutcome::Record { metadata, .. }
            ] if error.recoverable
                && error.message.contains("HTTP")
                && metadata.record_type == pb::WarcRecordType::Resource as i32
        ),
        "unexpected outcomes: {outcomes:?}"
    );
}

/// Large WARC and HTTP headers remain below the gRPC message cap when the
/// configured parser limit permits them.
#[tokio::test]
async fn megabyte_header_blocks_stream_losslessly() {
    use std::io::Write;

    let large_value = "A".repeat(1 << 20);
    let http = format!("HTTP/1.1 200 OK\r\nX-Large: {large_value}\r\n\r\nbody");
    let mut data = Vec::new();
    write!(
        data,
        "WARC/1.0\r\nWARC-Type: response\r\nWARC-Record-ID: <urn:uuid:aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa>\r\nWARC-Date: 2020-01-01T00:00:00Z\r\nContent-Type: application/http; msgtype=response\r\nX-Large: {large_value}\r\nContent-Length: {}\r\n\r\n{http}\r\n\r\n",
        http.len()
    )
    .unwrap();

    let config = pb::ParseWarcConfig {
        parse_http: Some(true),
        max_header_len: 2 << 20,
        ..Default::default()
    };
    let mut client = common::warc_client().await;
    let requests = common::warc_requests(&data, 64 << 10, &config);
    let mut stream = client
        .parse_warc(tokio_stream::iter(requests))
        .await
        .unwrap()
        .into_inner();
    let mut responses = Vec::new();
    while let Some(response) = stream.message().await.unwrap() {
        responses.push(response);
    }

    let outcomes = group_responses(&responses);
    let records = records_only(&outcomes);
    assert_eq!(records.len(), 1);
    let metadata = records[0].0;
    assert!(metadata.warc_headers.as_ref().unwrap().raw_block.len() > 1 << 20);
    assert!(metadata.http_headers.as_ref().unwrap().raw_block.len() > 1 << 20);
}

/// Header limits that could exceed one response message are rejected before
/// parsing starts.
#[tokio::test]
async fn excessive_header_limit_is_rejected() {
    let config = pb::ParseWarcConfig {
        max_header_len: (2 << 20) + 1,
        ..Default::default()
    };
    let mut client = common::warc_client().await;
    let error = client
        .parse_warc(tokio_stream::iter([pb::ParseWarcRequest {
            kind: Some(pb::parse_warc_request::Kind::Config(config)),
        }]))
        .await
        .unwrap_err();
    assert_eq!(error.code(), tonic::Code::InvalidArgument);
}

/// Client-controlled allocation and response sizes are rejected before the
/// parser task starts.
#[tokio::test]
async fn excessive_buffer_and_chunk_limits_are_rejected() {
    let configs = [
        pb::ParseWarcConfig {
            payload_chunk_size: (8 << 20) + 1,
            ..Default::default()
        },
        pb::ParseWarcConfig {
            input_buffer_size: (16 << 20) + 1,
            ..Default::default()
        },
        pb::ParseWarcConfig {
            min_content_length: Some(2),
            max_content_length: Some(1),
            ..Default::default()
        },
    ];

    for config in configs {
        let mut client = common::warc_client().await;
        let error = client
            .parse_warc(tokio_stream::iter([pb::ParseWarcRequest {
                kind: Some(pb::parse_warc_request::Kind::Config(config)),
            }]))
            .await
            .unwrap_err();
        assert_eq!(error.code(), tonic::Code::InvalidArgument);
    }
}

/// `record_types` filter keeps only the requested types (Python parity).
#[tokio::test]
async fn filter_record_types_response_only() {
    let config = pb::ParseWarcConfig {
        record_types: vec![pb::WarcRecordType::Response as i32],
        ..Default::default()
    };
    let responses = collect_warc("warcfile.warc", 8 << 10, &config).await;
    let outcomes = group_responses(&responses);
    let records = records_only(&outcomes);
    assert!(!records.is_empty());
    assert!(
        records
            .iter()
            .all(|(m, _, _)| m.record_type == pb::WarcRecordType::Response as i32)
    );

    // Direct filtered count for parity.
    let reader = std::fs::File::open(data_path("warcfile.warc")).unwrap();
    let mask = convert::record_types_mask(&config.record_types);
    let mut direct = 0usize;
    for item in
        ArchiveIterator::with_options(reader, direct_options(&config)).with_filter(filter::has_record_type(mask))
    {
        item.unwrap();
        direct += 1;
    }
    assert_eq!(records.len(), direct);
}

/// Content-length bounds skip records outside the window.
#[tokio::test]
async fn filter_content_length_bounds() {
    let config = pb::ParseWarcConfig {
        parse_http: Some(false),
        min_content_length: Some(100),
        max_content_length: Some(10_000),
        ..Default::default()
    };
    let responses = collect_warc("warcfile.warc", 8 << 10, &config).await;
    let outcomes = group_responses(&responses);
    let records = records_only(&outcomes);
    assert!(!records.is_empty());
    for (m, _, _) in &records {
        assert!(m.content_length >= 100);
        assert!(m.content_length <= 10_000);
    }
}

/// Content-length filters see the parsed HTTP payload length, matching
/// `ArchiveIterator::with_filter`.
#[tokio::test]
async fn filter_content_length_runs_after_http_parse() {
    use std::io::Write;

    let http = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nbody";
    let mut data = Vec::new();
    write!(
        data,
        "WARC/1.0\r\nWARC-Type: response\r\nWARC-Record-ID: <urn:uuid:aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa>\r\nWARC-Date: 2020-01-01T00:00:00Z\r\nContent-Type: application/http; msgtype=response\r\nContent-Length: {}\r\n\r\n",
        http.len()
    )
    .unwrap();
    data.extend_from_slice(http);
    data.extend_from_slice(b"\r\n\r\n");

    let config = pb::ParseWarcConfig {
        parse_http: Some(true),
        max_content_length: Some(4),
        ..Default::default()
    };
    let direct_count = ArchiveIterator::with_options(
        std::io::Cursor::new(data.clone()),
        fastwarc::warc::iter::ArchiveIteratorOptions::default(),
    )
    .with_filter(filter::has_content_length_lte(4))
    .count();
    assert_eq!(direct_count, 1);

    let mut client = common::warc_client().await;
    let requests = common::warc_requests(&data, 64, &config);
    let mut stream = client
        .parse_warc(tokio_stream::iter(requests))
        .await
        .unwrap()
        .into_inner();
    let mut responses = Vec::new();
    while let Some(response) = stream.message().await.unwrap() {
        responses.push(response);
    }
    let outcomes = group_responses(&responses);
    let records = records_only(&outcomes);
    assert_eq!(records.len(), direct_count);
    assert_eq!(records[0].0.content_length, 4);
}

/// Builtin `IS_HTTP` filter matches the crate predicate.
#[tokio::test]
async fn filter_builtin_is_http() {
    let config = pb::ParseWarcConfig {
        filters: vec![pb::BuiltinFilter::IsHttp as i32],
        ..Default::default()
    };
    let responses = collect_warc("warcfile.warc", 8 << 10, &config).await;
    let outcomes = group_responses(&responses);
    let records = records_only(&outcomes);
    assert!(!records.is_empty());
    assert!(records.iter().all(|(m, _, _)| m.is_http));
}

/// block-sized-records fixture: record count parity with a direct iterator.
#[tokio::test]
async fn stream_block_sized_records() {
    let config = default_config();
    let responses = collect_warc("block-sized-records.warc", 4096, &config).await;
    let outcomes = group_responses(&responses);
    assert!(outcomes.iter().all(|o| matches!(o, RecordOutcome::Record { .. })));
    let direct_types = direct_record_types("block-sized-records.warc", &config);
    assert_eq!(outcomes.len(), direct_types.len());
    assert!(!outcomes.is_empty());
}

/// The unary `ParseArchive` must return exactly what a folded `ParseWarc`
/// stream yields for the same input and config: same records in order, same
/// metadata and same payload bytes. Exercised on plain and gzip input.
#[tokio::test]
async fn unary_matches_stream() {
    for file in ["warcfile.warc", "warcfile.warc.gz"] {
        let config = pb::ParseWarcConfig {
            parse_http: Some(true),
            verify_digests: true,
            ..Default::default()
        };
        let responses = collect_warc(file, 8 << 10, &config).await;
        let outcomes = group_responses(&responses);
        let streamed = records_only(&outcomes);

        let mut client = common::warc_client().await;
        let unary = client
            .parse_archive(pb::ParseArchiveRequest {
                config: Some(config),
                archive: std::fs::read(data_path(file)).unwrap().into(),
            })
            .await
            .unwrap()
            .into_inner();

        assert!(unary.errors.is_empty(), "unexpected errors for {file}");
        assert_eq!(unary.records.len(), streamed.len(), "record count mismatch for {file}");
        for (record, (metadata, payload, end)) in unary.records.iter().zip(&streamed) {
            assert_eq!(record.metadata.as_ref().unwrap(), *metadata);
            assert_eq!(record.payload.as_slice(), *payload);
            assert_eq!(record.payload.len(), usize::try_from(end.payload_length).unwrap());
        }
    }
}

/// A `ParseArchive` request without `config` is rejected with
/// `InvalidArgument`, mirroring the stream's first-message rule.
#[tokio::test]
async fn unary_requires_config() {
    let mut client = common::warc_client().await;
    let status = client
        .parse_archive(pb::ParseArchiveRequest {
            config: None,
            archive: Vec::new().into(),
        })
        .await
        .unwrap_err();
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
}

/// `archive_path` belongs to the streaming RPC and must not be silently
/// ignored by the unary request.
#[tokio::test]
async fn unary_rejects_archive_path() {
    let mut client = common::warc_client().await;
    let status = client
        .parse_archive(pb::ParseArchiveRequest {
            config: Some(pb::ParseWarcConfig {
                archive_path: data_path("warcfile.warc").to_string_lossy().into_owned(),
                ..Default::default()
            }),
            archive: Vec::new().into(),
        })
        .await
        .unwrap_err();
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
}

/// An empty archive parses to an empty response, not an error.
#[tokio::test]
async fn unary_empty_archive() {
    let mut client = common::warc_client().await;
    let response = client
        .parse_archive(pb::ParseArchiveRequest {
            config: Some(default_config()),
            archive: Vec::new().into(),
        })
        .await
        .unwrap()
        .into_inner();
    assert!(response.records.is_empty());
    assert!(response.errors.is_empty());
}

/// A framing failure mid-archive returns the records parsed so far plus one
/// non-recoverable error; partial results are kept, not discarded.
#[tokio::test]
async fn unary_reports_framing_error() {
    let mut client = common::warc_client().await;
    let response = client
        .parse_archive(pb::ParseArchiveRequest {
            config: Some(default_config()),
            archive: common::corrupt_archive().into(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(response.records.len(), 1);
    assert_eq!(response.records[0].metadata.as_ref().unwrap().record_type, pb::WarcRecordType::Warcinfo as i32);
    assert_eq!(response.errors.len(), 1);
    assert!(!response.errors[0].recoverable);
}

/// Client cancel while payloads are flowing: dropping the response stream
/// closes the response channel, which stops the parser. The server must
/// survive the cancel and serve subsequent RPCs on the same connection.
#[tokio::test]
async fn client_cancel_mid_stream() {
    let mut client = common::warc_client().await;
    let data = std::fs::read(data_path("warcfile.warc")).unwrap();

    // Cancel several in-flight parses after a few messages each.
    for _ in 0..3 {
        let requests = common::warc_requests(&data, 8 << 10, &default_config());
        let mut stream = client
            .parse_warc(tokio_stream::iter(requests))
            .await
            .unwrap()
            .into_inner();
        for _ in 0..5 {
            assert!(stream.message().await.unwrap().is_some());
        }
        drop(stream);
    }

    // The same connection must still complete a full parse afterwards.
    let requests = common::warc_requests(&data, 8 << 10, &default_config());
    let mut stream = client
        .parse_warc(tokio_stream::iter(requests))
        .await
        .unwrap()
        .into_inner();
    let mut responses = Vec::new();
    while let Some(resp) = stream.message().await.unwrap() {
        responses.push(resp);
    }
    let outcomes = group_responses(&responses);
    assert_eq!(outcomes.len(), 50);
    assert!(outcomes.iter().all(|o| matches!(o, RecordOutcome::Record { .. })));
}

/// `include_payload=false` / `include_headers=false` still emits one start/end
/// pair per record, with Content-Length on `record_end` and no payload copies.
#[tokio::test]
async fn omit_payload_and_headers_counts_records() {
    let config = pb::ParseWarcConfig {
        include_payload: Some(false),
        include_headers: Some(false),
        ..Default::default()
    };
    let responses = collect_warc("warcfile.warc", 8 << 10, &config).await;
    let mut starts = 0u64;
    let mut chunks = 0u64;
    let mut ends = 0u64;
    let mut last_len = 0u64;
    common::for_each_event(&responses, |resp| match resp.kind.as_ref().unwrap() {
        pb::parse_warc_response::Kind::RecordStart(start) => {
            let metadata = start.metadata.as_ref().unwrap();
            assert!(metadata.warc_headers.is_none());
            assert!(metadata.http_headers.is_none());
            last_len = metadata.content_length;
            starts += 1;
        }
        pb::parse_warc_response::Kind::PayloadChunk(_) => chunks += 1,
        pb::parse_warc_response::Kind::RecordEnd(end) => {
            assert_eq!(end.payload_length, last_len);
            ends += 1;
        }
        pb::parse_warc_response::Kind::RecordError(e) => panic!("unexpected record_error: {}", e.message),
        pb::parse_warc_response::Kind::Batch(_) => panic!("nested batch after flatten"),
    });
    assert_eq!(starts, 50);
    assert_eq!(ends, 50);
    assert_eq!(chunks, 0);
}

/// `archive_path` parses a file on the server without uploading chunks.
#[tokio::test]
async fn archive_path_counts_records_without_chunks() {
    let config = pb::ParseWarcConfig {
        include_payload: Some(false),
        include_headers: Some(false),
        archive_path: data_path("warcfile.warc").to_string_lossy().into_owned(),
        response_batch_size: 8,
        ..Default::default()
    };
    let mut client = common::warc_client().await;
    let requests = vec![pb::ParseWarcRequest {
        kind: Some(pb::parse_warc_request::Kind::Config(config)),
    }];
    let mut stream = client
        .parse_warc(tokio_stream::iter(requests))
        .await
        .unwrap()
        .into_inner();
    let mut responses = Vec::new();
    while let Some(resp) = stream.message().await.unwrap() {
        responses.push(resp);
    }
    let mut ends = 0u64;
    common::for_each_event(&responses, |resp| {
        if matches!(resp.kind.as_ref(), Some(pb::parse_warc_response::Kind::RecordEnd(_))) {
            ends += 1;
        }
    });
    assert_eq!(ends, 50);
}

/// Batched responses flatten to the same records as the 1:1 wire.
#[tokio::test]
async fn batched_stream_matches_unbatched() {
    let unbatched = group_responses(&collect_warc("warcfile.warc", 8 << 10, &default_config()).await);
    let batched_config = pb::ParseWarcConfig {
        response_batch_size: 8,
        ..default_config()
    };
    let batched_raw = collect_warc("warcfile.warc", 8 << 10, &batched_config).await;
    assert!(
        batched_raw
            .iter()
            .any(|r| matches!(r.kind, Some(pb::parse_warc_response::Kind::Batch(_)))),
        "expected at least one batch message"
    );
    let batched = group_responses(&batched_raw);
    assert_eq!(unbatched.len(), batched.len());
    assert_eq!(unbatched.len(), 50);
}
