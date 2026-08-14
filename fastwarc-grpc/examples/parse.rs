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

//! Example client: stream a WARC file to a running server and print a
//! per-record summary.
//!
//! ```sh
//! cargo run &
//! cargo run --example parse -- tests/data/warcfile.warc.gz
//! ```
//!
//! The file is read in chunks on a separate thread and fed through a bounded
//! channel, so archives larger than memory stream fine, the same shape a
//! real client would use.

use std::io::Read;

use fastwarc_grpc::proto::fastwarc::v1 as pb;
use fastwarc_grpc::proto::fastwarc::v1::warc_service_client::WarcServiceClient;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

/// Bytes of archive data per request message.
const CHUNK_SIZE: usize = 64 << 10;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("usage: parse <warc-file> [server-url]")?;
    let url = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "http://localhost:50051".to_owned());

    let mut file = std::fs::File::open(&path)?;
    let (tx, rx) = mpsc::channel::<pb::ParseWarcRequest>(4);

    // Feed the file from a blocking thread; the channel bound provides
    // backpressure against the gRPC stream.
    std::thread::spawn(move || {
        let config = pb::ParseWarcConfig {
            parse_http: true,
            verify_digests: true,
            ..Default::default()
        };
        if tx
            .blocking_send(pb::ParseWarcRequest {
                kind: Some(pb::parse_warc_request::Kind::Config(config)),
            })
            .is_err()
        {
            return;
        }
        let mut buf = vec![0u8; CHUNK_SIZE];
        loop {
            match file.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let request = pb::ParseWarcRequest {
                        kind: Some(pb::parse_warc_request::Kind::Chunk(buf[..n].to_vec().into())),
                    };
                    if tx.blocking_send(request).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("read error: {e}");
                    break;
                }
            }
        }
    });

    let mut client = WarcServiceClient::new(fastwarc_grpc::transport::connect(url).await?)
        .max_decoding_message_size(fastwarc_grpc::transport::MAX_MESSAGE_SIZE)
        .max_encoding_message_size(fastwarc_grpc::transport::MAX_MESSAGE_SIZE);
    let mut stream = client.parse_warc(ReceiverStream::new(rx)).await?.into_inner();

    let mut records = 0u64;
    let mut payload_bytes = 0u64;
    while let Some(response) = stream.message().await? {
        handle_response(response, &mut records, &mut payload_bytes);
    }
    println!("{records} records, {payload_bytes} payload bytes total");
    Ok(())
}

fn handle_response(response: pb::ParseWarcResponse, records: &mut u64, payload_bytes: &mut u64) {
    match response.kind {
        Some(pb::parse_warc_response::Kind::RecordStart(start)) => {
            let metadata = start.metadata.unwrap_or_default();
            let record_type =
                pb::WarcRecordType::try_from(metadata.record_type).unwrap_or(pb::WarcRecordType::Unspecified);
            println!(
                "#{:<4} {:<12} pos={:<10} len={:<9} {}",
                metadata.record_index,
                record_type.as_str_name().trim_start_matches("WARC_RECORD_TYPE_"),
                metadata.stream_pos,
                metadata.content_length,
                metadata.record_id.as_deref().unwrap_or("-"),
            );
        }
        Some(pb::parse_warc_response::Kind::PayloadChunk(chunk)) => {
            *payload_bytes += chunk.data.len() as u64;
        }
        Some(pb::parse_warc_response::Kind::RecordEnd(end)) => {
            *records += 1;
            let block = pb::DigestStatus::try_from(end.block_digest_status).unwrap_or_default();
            let payload = pb::DigestStatus::try_from(end.payload_digest_status).unwrap_or_default();
            println!(
                "      -> {} payload bytes, block digest {}, payload digest {}",
                end.payload_length,
                block.as_str_name().trim_start_matches("DIGEST_STATUS_"),
                payload.as_str_name().trim_start_matches("DIGEST_STATUS_"),
            );
        }
        Some(pb::parse_warc_response::Kind::RecordError(e)) => {
            eprintln!("record error at pos {} (recoverable: {}): {}", e.stream_pos, e.recoverable, e.message);
        }
        Some(pb::parse_warc_response::Kind::Batch(batch)) => {
            for item in batch.items {
                handle_response(item, records, payload_bytes);
            }
        }
        None => {}
    }
}
