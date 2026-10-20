# FastWARC-gRPC

Streaming gRPC server exposing the `fastwarc` WARC parser via binary gRPC messages (no JSON transcoding).

Usage documentation and client examples live in the crate docs: `cargo doc -p fastwarc-grpc --open`. The protobuf
contracts are in [`proto/`](proto).

## Service

- `fastwarc.v1.WarcService/ParseWarc` (bidirectional streaming): send one `config` message, then raw archive bytes
  (`chunk`): plain, gzip, zstd, or lz4 (auto-detected when `stream_detect` is enabled, the default). Set `archive_path`
  on the config to have the server open a local file instead of uploading chunks (rejected with `PERMISSION_DENIED`
  unless the server was started with `FASTWARC_GRPC_ALLOW_LOCAL_FILES=1`, since it grants clients read access to the
  server's filesystem; paths are confined to the directory named by `FASTWARC_GRPC_LOCAL_FILE_ROOT`, see  [Run](#run)).
  Receive per kept record: `record_start` (full metadata, lossless header blocks), `payload_chunk`* (offset-tagged),
  `record_end` (payload length). HTTP-header failures on a framed record yield a recoverable `record_error`; every other
  failure ends the stream (see the crate docs for the exact event sequence).
- `fastwarc.v1.WarcService/ParseArchive` (unary): the whole archive in one request, every kept record (metadata and
  whole payload) in one response, for single records and small archives within the gRPC message size limits (this server
  accepts 16 MiB; many clients default to 4 MiB). The decoded response is capped at 16 MiB as well; larger archives fail
  with `RESOURCE_EXHAUSTED` and belong on `ParseWarc`. Same parse pipeline, filters, and error model as the stream; a
  framing error returns the records parsed so far plus one non-recoverable error.
- `include_payload` / `include_headers` default to true. Set false to skip payload bytes and/or lossless header blocks.
  `response_batch_size` packs that many protocol events into one `batch` message (zero = one event per message; batches
  flush as they fill).
- Filters matching Python `ArchiveIterator`: `record_types`, `min_content_length`, `max_content_length`, and
  `BuiltinFilter` predicates.
- `grpc.health.v1.Health` for load-balancer probes.
- gRPC server reflection (v1), so tools like `grpcurl` can discover and call the service without local copies of the
  proto files:

```sh
grpcurl -plaintext localhost:50061 describe fastwarc.v1.WarcService
grpcurl -plaintext \
  -d "{\"config\":{}, \"archive\":\"$(base64 -w0 record.warc)\"}" \
  localhost:50061 fastwarc.v1.WarcService/ParseArchive
```

## Build

`fastwarc-grpc` is not a default workspace member due to the `protoc` dependency. After installing `protoc`, compile the
project with:

```sh
cargo build -p fastwarc-grpc
```

The server builds against the sibling `fastwarc-rs` crate via a path dependency.

## Run

```sh
FASTWARC_GRPC_ADDR="[::]:50061" cargo run -p fastwarc-grpc
```

`FASTWARC_GRPC_ADDR` also accepts `unix:///path.sock` or an absolute filesystem path. An existing socket path prevents
startup. After a crash, remove a leftover socket only after confirming that no server is using it. On SIGINT or SIGTERM,
the server shuts down gracefully and removes its socket if the path still refers to the socket it created. Keep the
socket directory under the server operator's control.

The service allows 32 concurrent parsers across uploads, local files, and unary calls. Excess requests fail immediately
with `RESOURCE_EXHAUSTED`. Set `FASTWARC_GRPC_MAX_CONCURRENT_PARSERS` to a positive integer to change this limit.
Embedders use `WarcParser::with_max_concurrent_parsers(limit)`; choose a limit below the runtime's blocking-thread
capacity to leave room for other blocking work. Idle uploads, and clients that stop reading responses, count toward the
limit until they finish or are cancelled. The server sends HTTP/2 keepalive pings so connections to unreachable peers
close and release their slots; a connected but silent client still holds its slot.

Local file access (`archive_path` on the request config) is disabled by default. To enable it, set both:

```sh
FASTWARC_GRPC_ALLOW_LOCAL_FILES=1 \
  FASTWARC_GRPC_LOCAL_FILE_ROOT=/data/warcs \
  cargo run -p fastwarc-grpc
```

`FASTWARC_GRPC_LOCAL_FILE_ROOT` selects the directory exposed to clients. Relative paths are opened beneath a retained
directory handle. Absolute paths must start with the canonical root; `..` and symlinks that escape it are rejected with
`PERMISSION_DENIED`. Symlink targets must be relative. The server requires a valid root when local access is enabled.
Embedders configure it with `WarcParser::with_local_files(root)`.

An example client streams a local archive and prints a per-record summary:

```sh
cargo run -p fastwarc-grpc --example parse -- fastwarc-rs/tests/fixtures/warcfile.warc.gz
```

A throughput benchmark following the shared profile format lives in [
`benchmarks/warc/fastwarc-grpc`](../benchmarks/warc/fastwarc-grpc).
