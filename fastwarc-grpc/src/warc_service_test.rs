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
