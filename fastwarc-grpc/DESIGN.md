# Design: fastwarc-grpc, a streaming gRPC server for FastWARC

## 1. Goals

- Expose the WARC parsing capabilities of the `fastwarc` crate as a binary
  gRPC service (no JSON transcoding).
- Contracts follow the buf Standard: `buf lint` passes with the `STANDARD`
  and `COMMENTS` categories, and `buf breaking` (FILE ruleset) guards future
  evolution.
- Streaming first: WARC archives of arbitrary size flow through a
  bidirectional stream with bounded memory on both sides.
- **Lossless headers**: every header byte the parser saw is representable in
  the response (ordered byte pairs + verbatim `raw_block`). Payload bytes are
  the view after any configured HTTP payload decoding (see §5).
- Written in Rust, the language of the underlying crate. Pure Rust end to
  end: no native libraries, no vcpkg, no Python/Cython anywhere in the
  serving path.
- Strict linting on both sides of the contract: buf `COMMENTS` for the
  schema, `clippy` pedantic + `#![deny(missing_docs)]` for the server.
- Zero footprint on the library: `fastwarc` itself carries no gRPC
  dependencies. The server is a separate crate that depends on it one-way.

## 2. Non-goals (v1)

- WARC writing, record mutation, digest computation for new records, and
  zstd dictionary training are not exposed.
- Arbitrary `func_filter` callables (Python) cannot be remoted; built-in
  predicates are exposed as `BuiltinFilter`.
- `inplace` is applied automatically when `include_payload` is false and
  `verify_digests` is false; it is not a client-facing option.
- `fsspec` path/URL opening is a Python binding concern. Clients stream
  `chunk` bytes, or set `archive_path` to a filesystem path the server can
  open.
- HTML/DOM parsing (the `resiliparse` crate) is deliberately out of scope to
  keep the project pure Rust with no native dependencies.
- Authentication/TLS termination is left to deployment (reverse proxy);
  tonic supports TLS natively if it is ever needed in-process.

## 3. Repository layout

`fastwarc-grpc` is a workspace member of the chatnoir-resiliparse
repository, but not a default member: building it requires `protoc`, and
that requirement must not leak into the default `cargo build`. The library
crates stay free of gRPC dependencies.

```
fastwarc-grpc/                  # workspace member (single crate)
  Cargo.toml                    # package: fastwarc-grpc
  build.rs                      # tonic-prost-build codegen from proto/
  proto/                        # buf module root (buf.yaml lives here)
    buf.yaml                    # v2 config: lint STANDARD+COMMENTS, breaking FILE
    buf.gen.yaml                # optional standalone stub generation
    fastwarc/v1/
      warc.proto                # lossless WARC data model
      warc_service.proto        # WarcService (ParseWarc + ParseArchive)
  src/
    lib.rs                      # generated-proto module + crate docs
    main.rs                     # server bootstrap
    warc_service.rs             # WarcService impl
    convert.rs                  # crate-type -> proto conversions
    transport.rs                # HTTP/2 settings for bulk WARC transfer
  tests/                        # in-process integration tests + fixtures
```

The service builds against the sibling `fastwarc-rs` crate through a path
dependency; it requires no patches in `fastwarc` itself.

## 4. Service API

### WarcService.ParseWarc (bidirectional streaming)

```
rpc ParseWarc(stream ParseWarcRequest) returns (stream ParseWarcResponse)
```

- First request message: `ParseWarcConfig`, the subset of Python
  `ArchiveIterator` / Rust `ArchiveIteratorOptions` knobs that make sense on
  a remote stream, plus `payload_chunk_size`:
  `parse_http`, `decode_http_payload`, `verify_digests`, `quirks_mode`,
  `max_header_len`, `record_types`, `min_content_length`, `max_content_length`,
  `stream_detect`, `input_buffer_size`, `filters` (`BuiltinFilter`),
  `include_payload`, `include_headers`, `response_batch_size`,
  `archive_path`.
- Subsequent messages: raw archive `chunk`s. Chunks are concatenated in
  order and fed through a channel into an `ArchiveIterator` running on a
  blocking thread. Empty `kind` is rejected. When `archive_path` is set,
  the server opens that filesystem path and ignores chunks.
- Responses per kept record, in order: `record_start` (full `RecordMetadata`),
  zero or more `payload_chunk`s, `record_end` (payload length + digest
  verification statuses).
- **Error model (honest):**
  - HTTP-header parse failure on an already-framed record → recoverable
    `record_error`; the iterator consumes the remainder and continues.
  - WARC framing failure (`Invalid WARC header`, lost reader) →
    non-recoverable `record_error` and the response stream ends. The
    underlying crate does not resume scanning for the next record boundary
    after a framing error; pretending otherwise would hang clients.
- Filtered-out records (type / length / builtin filters) are skipped silently,
  matching Python `ArchiveIterator` skip semantics.
- `verify_digests` **reports** `DigestStatus` on `record_end` and still emits
  the record. Python/Rust iterators **skip** records with missing/invalid
  block digests when this flag is set; an intentional service fork.
- `parse_http` defaults to **false** (proto3 zero). Python/Rust default to
  **true**. Set `parse_http=true` for Python-parity HTTP extraction; block
  digests are still verified before HTTP parsing when requested.
- Backpressure is natural: the blocking parser thread only reads as fast as
  the bounded response channel drains.

### WarcService.ParseArchive (unary)

```
rpc ParseArchive(ParseArchiveRequest) returns (ParseArchiveResponse)
```

The unary companion for single records and small archives that fit within
the gRPC message size limits (this server accepts 16 MiB; many clients
default to 4 MiB):
one request carrying `ParseWarcConfig` + the complete archive bytes, one
response carrying every kept record (`ParsedRecord`: metadata, whole
payload, digest statuses) and every record-level error, in stream order.
Both RPCs run the identical parse pipeline (the unary handler folds the
same `record_start` / `payload_chunk` / `record_end` / `record_error`
message sequence into a single response), so parsing semantics, filters,
and the error model cannot diverge. A non-recoverable framing error ends
the record list but keeps the records parsed up to that point.

### Health and reflection

The server registers the standard `grpc.health.v1.Health` service
(`tonic-health`) so load balancers can probe it, and server reflection v1
(`tonic-reflection`, fed by the descriptor set emitted in `build.rs`) so
generic clients such as `grpcurl` can discover the service without local
proto files.

## 5. Lossless data mapping

### fastwarc → `fastwarc.v1`

| Crate surface | Proto field | Notes |
|---|---|---|
| `WarcRecord::record_type()` | `RecordMetadata.record_type` | enum; original also in headers |
| `headers()` via `items_bytes()` | `HeaderBlock.fields` (repeated bytes pairs) | multimap: duplicates and order preserved; bytes avoid lossy UTF-8/Windows-1252 decoding |
| `status_line_bytes()` | `HeaderBlock.status_line` | HTTP blocks only |
| raw header block (`HeaderMap::write`) | `HeaderBlock.raw_block` | byte-exact copy of the source bytes |
| `encoding()` | `HeaderBlock.encoding` | Unicode vs Latin-1 |
| `content_length()` | `content_length` | post-HTTP-parse value |
| `stream_pos()` | `stream_pos` | offset in uncompressed stream |
| `is_http()`, `is_http_parsed()` | `is_http`, `http_parsed` | |
| `http_headers()` | `http_headers` (optional `HeaderBlock`) | same lossless treatment |
| `http_content_type()`, `http_charset()` | `http_content_type`, `http_charset` | optional strings |
| `record_id()` | `record_id` | convenience; raw stays in headers |
| `record_date()` | `record_date` (Timestamp) | raw string stays in headers |
| payload reader / `frozen_payload_bytes()` | `PayloadChunk` stream | chunked, offset-tagged, length-checked in `record_end` |
| `verify_block_digest()` / `verify_payload_digest()` | `DigestStatus` enums + `digest_detail` | computed on demand when `verify_digests` is set |

Key decisions:

1. **Headers are bytes, not strings.** The crate's string getters decode
   lossy; the `*_bytes()` accessors do not. Proto `bytes` fields carry the
   exact data.
2. **Headers are a repeated pair list, not a map.** `HeaderMap` is an
   ordered multimap; proto `map` would drop duplicates and order.
3. **The raw header block travels alongside the parsed view.** Even if a
   client disagrees with the parser's interpretation, it has the original
   bytes.
4. **Payload is streamed, never re-encoded by gRPC.** With
   `decode_http_payload` unset/`None`, payload bytes match the record block
   after optional HTTP header stripping. With transfer/content decoding on,
   the stream is the **decoded** view (same as local FastWARC), not the raw
   on-wire encoded bytes.
5. **Quirks mode is explicit**, so ClueWeb-style malformed archives parse
   identically to local runs.
6. **Digests are verified before HTTP parsing.** `WARC-Block-Digest` covers
   the raw record block, so the service verifies it against the unparsed
   block first, then parses HTTP manually per record. Verifying afterwards
   would report false mismatches.

## 6. Server implementation notes

- **Codegen**: `build.rs` runs `tonic-prost-build` over `proto/` (requires a
  `protoc` on the PATH; `buf generate` via `proto/buf.gen.yaml` is the
  standalone alternative). No hand-written stubs.
- **Async shape**: tonic + tokio. `ArchiveIterator` is synchronous and
  CPU/IO-bound, so WARC parsing runs in `tokio::task::spawn_blocking`; the
  request stream feeds bytes through an `mpsc` channel into a
  `std::io::BufRead` adapter (`ChannelReader`). Chunk fields are generated
  as `bytes::Bytes`, so a chunk decodes zero-copy out of the HTTP/2 receive
  buffer, and `ChannelReader::fill_buf` hands the parser windows into those
  chunks directly: header scans and payload skips run in place, with no
  intermediate `BufReader` copy. Parsed responses flow back through a second
  `mpsc` wrapped in `tokio_stream::wrappers::ReceiverStream`. A panic in the
  blocking task is mapped to gRPC `Internal` (not silent EOF).
- **Per-record pipeline**: filter → verify block digest (freezes the record)
  → parse HTTP manually when configured → verify payload digest →
  `record_start` → stream payload chunks → `record_end`.
- **Concurrency**: each RPC stream is independent; the service is stateless
  and holds no shared parser state.
- **HTTP/2 windows**: tonic/h2 default to a 64 KiB flow-control window,
  which starves a multi-gigabyte archive stream.
  `transport::{configure_server, configure_endpoint, connect}` raise the
  connection window to 32 MiB, the stream window to 16 MiB, and the max
  frame to 1 MiB, with TCP_NODELAY. Adaptive (BDP-probing) windows stay
  off: they override fixed windows and can stall a stream saturated in
  both directions. The gRPC message
  cap is 16 MiB. Both peers must be tuned: HTTP/2 flow control is the
  minimum of the two. `FASTWARC_HTTP2_CONNECTION_WINDOW` and
  `FASTWARC_HTTP2_STREAM_WINDOW` override the windows (bytes, minimum
  65535). The parser chunk queue holds about 32 MiB;
  `RESPONSE_CHANNEL_BOUND` is 1024.
- **`include_payload` / `include_headers`**: unset defaults to true (full
  lossless stream). Set false to skip payload copies and/or lossless header
  blocks; `record_end.payload_length` then comes from WARC `Content-Length`
  and the iterator consumes unread payload on the next step. When payload
  is omitted and digests are off, the parser enables `inplace`.
- **Response batching**: `response_batch_size` > 1 packs that many protocol
  events into one `batch` gRPC message. A batch flushes as soon as it fills
  or reaches 2 MiB. Combined with `try_send` (blocking only when the
  response channel is actually full), the parser is not parked on every
  per-record send. Zero sends one event per message.
- **Error mapping**: recoverable HTTP failures and non-recoverable framing /
  payload failures become `record_error` as above; empty request `kind`, a
  second `config`, and transport-level failures become gRPC statuses.
- **Lint gates** (CI-ready):
  - `buf lint` (STANDARD + COMMENTS, comment ignores disallowed)
  - `buf format --diff --exit-code`
  - `buf breaking --against '.git#branch=main,subdir=proto'` once merged
  - `cargo clippy --all-targets --no-deps -- -D warnings -D clippy::pedantic`
    (`--no-deps` scopes the pedantic gate to this crate's own code)
  - `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` (the crate additionally
    sets `#![deny(missing_docs)]`)

## 7. Testing plan

- **Golden integration tests** against `tests/data/`:
  `warcfile.warc` (plain), `.gz`, `.lz4`, `.zst`,
  `clipped.warc.gz`, `clueweb-quirk.warc.gz`, `block-sized-records.warc`.
  Stream each through a live in-process server and assert record counts,
  types, header bytes, and payload bytes against direct `ArchiveIterator`
  runs.
- **Losslessness assertions**: reassembled payloads are byte-identical to
  direct reads (matching decode options); `raw_block` and ordered header
  field lists match; duplicate headers are preserved.
- **Digest tests**: per-record `DigestStatus` parity with direct
  `verify_block_digest`/`verify_payload_digest` calls.
- **Protocol tests**: first message must be config; a second config and an
  empty `kind` are rejected (`InvalidArgument`).
- **Batching**: `response_batch_size=8` flattens to the same records as the
  unbatched stream.
- **Unary parity**: `ParseArchive` output must equal the folded `ParseWarc`
  stream for the same input and config (records, payload bytes, digest
  verdicts); missing config is rejected; a framing error returns partial
  results plus one non-recoverable error.
- **Edge cases**: empty archive; corrupt mid-stream (non-recoverable, no
  hang); HTTP header failure then continue; record_types / content-length
  filters; client cancel mid-payload.
- **Lint CI job** running the gates from §6.

## 8. Possible follow-ups

Deliberately not in v1: in-process TLS (tonic supports it; a reverse proxy
covers it today) and WARC writing with zstd dictionary support. Each should
be weighed against the goal of keeping this service small before being
added.
