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

use prost::Message;

use super::ResponseSender;
use crate::proto::fastwarc::v1 as pb;

const MAX_BATCH_BYTES: usize = 2 << 20;
const ITEM_TAG: u32 = 1;

/// Groups protocol events into gRPC batch messages.
pub(super) struct BatchEmitter<'a> {
    tx: &'a ResponseSender,
    batch: Vec<pb::ParseWarcResponse>,
    batch_size: usize,
    batch_bytes: usize,
}

impl BatchEmitter<'_> {
    pub(super) fn new(tx: &ResponseSender, batch_size: usize) -> BatchEmitter<'_> {
        BatchEmitter {
            tx,
            batch: Vec::new(),
            batch_size,
            batch_bytes: 0,
        }
    }

    pub(super) fn emit(&mut self, response: pb::ParseWarcResponse) -> bool {
        if self.batch_size <= 1 {
            return send_response(self.tx, response);
        }

        let encoded_len = response.encoded_len();
        let response_bytes = encoded_len
            .saturating_add(prost::encoding::key_len(ITEM_TAG))
            .saturating_add(prost::encoding::encoded_len_varint(encoded_len as u64));
        if !self.batch.is_empty() && self.batch_bytes.saturating_add(response_bytes) > MAX_BATCH_BYTES && !self.flush()
        {
            return false;
        }

        self.batch_bytes = self.batch_bytes.saturating_add(response_bytes);
        self.batch.push(response);
        if self.batch.len() >= self.batch_size || self.batch_bytes >= MAX_BATCH_BYTES {
            self.flush()
        } else {
            true
        }
    }

    pub(super) fn flush(&mut self) -> bool {
        if self.batch.is_empty() {
            return true;
        }

        self.batch_bytes = 0;
        let items = std::mem::take(&mut self.batch);
        let response = if items.len() == 1 {
            items.into_iter().next().expect("checked non-empty")
        } else {
            pb::ParseWarcResponse {
                kind: Some(pb::parse_warc_response::Kind::Batch(pb::RecordBatch { items })),
            }
        };
        send_response(self.tx, response)
    }
}

fn send_response(tx: &ResponseSender, response: pb::ParseWarcResponse) -> bool {
    match tx.try_send(Ok(response)) {
        Ok(()) => true,
        Err(tokio::sync::mpsc::error::TrySendError::Full(message)) => tx.blocking_send(message).is_ok(),
        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => false,
    }
}
