# fastwarc-grpc

Streaming gRPC server exposing the `fastwarc` WARC parser. Binary gRPC only
(no JSON transcoding); the protobuf contracts live in [`proto/`](proto) and
follow the buf Standard (`STANDARD` + `COMMENTS` lint categories).

This crate is a separate workspace member; the `fastwarc` library itself is
untouched and carries no gRPC dependencies. The server sits directly on the
`fastwarc` Rust crate; there is no Python or Cython in the serving path.

Usage documentation with worked client examples lives in the crate docs:
`cargo doc -p fastwarc-grpc --open`.

## Service

- `fastwarc.v1.WarcService/ParseWarc` (bidirectional streaming): send one
  `config` message, then raw archive bytes (`chunk`): plain, gzip, zstd, or
  lz4 (auto-detected when `stream_detect` is enabled, the default). Set
  `archive_path` on the config to have the server open a local file instead
  of uploading chunks (rejected with `PERMISSION_DENIED` unless the server
  was started with `FASTWARC_GRPC_ALLOW_LOCAL_FILES=1`, since it grants
  clients read access to the server's filesystem). Receive per kept record: `record_start` (full
  metadata, lossless header blocks), `payload_chunk`* (offset-tagged),
  `record_end` (payload length). HTTP-header
  failures on a framed record yield a recoverable `record_error`; WARC
  framing failures end the stream (non-recoverable).
- `fastwarc.v1.WarcService/ParseArchive` (unary): the whole archive in one
  request, every kept record (metadata and whole payload) in
  one response, for single records and small archives within the gRPC
  message size limits (this server accepts 16 MiB; many clients default to
  4 MiB). Same parse pipeline, filters, and error model as the stream; a
  framing error returns the records parsed so far plus one non-recoverable
  error.
- `include_payload` / `include_headers` default to true. Set false to skip
  payload bytes and/or lossless header blocks. `response_batch_size` packs
  that many protocol events into one `batch` message (zero = one event per
  message; batches flush as they fill).
- Filters matching Python `ArchiveIterator`: `record_types`,
  `min_content_length`, `max_content_length`, and `BuiltinFilter` predicates.
- `grpc.health.v1.Health` for load-balancer probes.
- gRPC server reflection (v1), so tools like `grpcurl` can discover and
  call the service without local copies of the proto files:

```sh
grpcurl -plaintext localhost:50061 describe fastwarc.v1.WarcService
grpcurl -plaintext \
  -d "{\"config\":{}, \"archive\":\"$(base64 -w0 record.warc)\"}" \
  localhost:50061 fastwarc.v1.WarcService/ParseArchive
```

### Python parity notes

| Local Python default | gRPC |
|---|---|
| `parse_http=True` | same; set `false` to leave HTTP headers in the payload |
| `verify_digests` skips bad records | same |
| `func_filter=callable` | use `BuiltinFilter` or filter client-side |
| writing / fsspec / pickle | out of scope (local-only) |

## Build

Pure Rust plus `protoc`: `build.rs` generates the gRPC stubs with
`tonic-prost-build`, which needs a `protoc` binary on the PATH. No other
native libraries, no vcpkg, no libclang. Because of the `protoc` requirement
the crate is not a workspace default member; build it explicitly from the
repository root:

```sh
cargo build -p fastwarc-grpc
```

The server builds against the sibling `fastwarc-rs` crate via a path
dependency.

## Run

```sh
FASTWARC_GRPC_ADDR="[::]:50061" cargo run -p fastwarc-grpc
```

`FASTWARC_GRPC_ADDR` also accepts `unix:///path.sock` or an absolute
filesystem path. The server shuts down gracefully on SIGINT or SIGTERM.

An example client streams a local archive and prints a per-record summary:

```sh
cargo run -p fastwarc-grpc --example parse -- fastwarc-grpc/tests/data/warcfile.warc.gz
```

A throughput benchmark following the shared profile format lives in
[`benchmarks/warc/fastwarc-grpc`](../benchmarks/warc/fastwarc-grpc).

## Lint and test gates

The crate is developed against these gates:

```sh
cd fastwarc-grpc/proto && buf lint && buf format --diff --exit-code
cargo fmt -p fastwarc-grpc --check
cargo clippy -p fastwarc-grpc --all-targets --no-deps -- -D warnings -D clippy::pedantic
cargo test -p fastwarc-grpc
RUSTDOCFLAGS="-D warnings" cargo doc -p fastwarc-grpc --no-deps
```

The tests use the existing WARC fixtures from this repository, including the
zstd archive in `fastwarc-rs/tests/fixtures`.
