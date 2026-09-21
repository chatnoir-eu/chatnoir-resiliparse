# Benchmark: FastWARC-gRPC

Parses one archive uploaded over a Unix domain socket to an in-process
`fastwarc-grpc` server. It uses one connection and one request stream.

The parser options match the plain [`fastwarc` benchmark](../fastwarc):
HTTP parsing and digest verification are disabled, records are reused in
place, and payloads are consumed without returning their bytes. The gRPC
server omits header blocks and batches up to 64 response events per message.
Both benchmarks build against this checkout's `fastwarc-rs` crate.

## Build

Rust and `protoc` must be on the PATH. The gRPC benchmark requires Unix.

```sh
make -C benchmarks/warc/fastwarc
make -C benchmarks/warc/fastwarc-grpc
```

## Compare

Run from the repository root with the same archive and `BUFFER_SIZE`:

```sh
BUFFER_SIZE=1048576 benchmarks/warc/fastwarc/profile /data/archive.warc
BUFFER_SIZE=1048576 benchmarks/warc/fastwarc-grpc/profile /data/archive.warc
```

`BUFFER_SIZE` defaults to 1 MiB in both programs. It sets the plain parser's
read buffer and the gRPC client's upload chunk size. The gRPC benchmark
accepts up to 8 MiB per chunk to leave room for protobuf framing.

Compare runs on the same machine with the same compression and cache state.
Check that both summaries report the same record count and payload byte
total. Each program counts parsed payload bytes, not archive bytes read
from disk. The gRPC benchmark skips recoverable record errors and fails on fatal
parser errors instead of reporting throughput for a partial archive.

The gRPC timer starts after server startup and connection establishment,
before opening and uploading the file. It includes upload, parsing, and
response consumption. Its result measures service overhead for one stream;
the client, runtime, and parser can use multiple threads. It is not a
single-core parser measurement. Record the machine, revision, archive,
storage, and cache state alongside any published results.
