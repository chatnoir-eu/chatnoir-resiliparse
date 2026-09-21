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

//! Batching of `ParseWarc` response events into `RecordBatch` messages.

use prost::Message;

use super::ResponseSender;
use crate::proto::fastwarc::v1 as pb;

/// Flush threshold for encoded batch items. A single larger event is sent unbatched.
const MAX_BATCH_BYTES: usize = 2 << 20;
/// Protobuf field number of `RecordBatch.items`.
const ITEM_TAG: u32 = 1;

/// Groups events by count and encoded size without reordering them.
/// Call [`Self::flush`] after the last event to send any remaining items.
pub(super) struct BatchEmitter<'a> {
    /// Response stream that receives the finished batches.
    tx: &'a ResponseSender,
    /// Events accumulated for the current batch, in emission order.
    batch: Vec<pb::ParseWarcResponse>,
    /// Number of events that triggers a flush; `<= 1` disables batching.
    batch_size: usize,
    /// Encoded size of `batch` as `RecordBatch.items` entries, including each
    /// item's key and length prefix.
    batch_bytes: usize,
}

impl BatchEmitter<'_> {
    /// An emitter that groups `batch_size` events per response on `tx`.
    pub(super) fn new(tx: &ResponseSender, batch_size: usize) -> BatchEmitter<'_> {
        BatchEmitter {
            tx,
            batch: Vec::new(),
            batch_size,
            batch_bytes: 0,
        }
    }

    /// Queues an event, flushing when either limit is reached.
    /// Returns `false` when the response channel is closed.
    pub(super) fn emit(&mut self, response: pb::ParseWarcResponse) -> bool {
        if self.tx.is_closed() {
            return false;
        }
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

    /// Sends pending events, leaving a single event unwrapped.
    /// Returns `false` when the response channel is closed.
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

/// Sends from the blocking parser task, waiting only when the channel is full.
fn send_response(tx: &ResponseSender, response: pb::ParseWarcResponse) -> bool {
    match tx.try_send(Ok(response)) {
        Ok(()) => true,
        Err(tokio::sync::mpsc::error::TrySendError::Full(message)) => tx.blocking_send(message).is_ok(),
        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => false,
    }
}
