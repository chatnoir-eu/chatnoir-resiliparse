# Benchmark: FastWARC-gRPC

Benchmark for the [`fastwarc-grpc`](../../../fastwarc-grpc) crate in this repository.

The profile binary starts the gRPC server in-process and streams the WARC over a
Unix domain socket. HTTP/2 windows and message limits come from
`fastwarc_grpc::transport`, the same helpers the server binary uses. Compare
against the plain `fastwarc` benchmark for service overhead.

Default configuration matches `fastwarc`: no HTTP parsing, no digest
verification, no payload or header echo. The server parses in place.

```bash
./profile WARCFILE.warc                         # parse-only, 64 KiB chunks
FASTWARC_GRPC_FULL=1 ./profile WARCFILE.warc    # stream payload and headers back
FASTWARC_GRPC_LOCAL=1 ./profile WARCFILE.warc   # server opens WARCFILE; no upload
FASTWARC_GRPC_JOBS=8 ./profile WARCFILE.warc    # 8 concurrent streams; progress lines and summary are aggregate
```

`BUFFER_SIZE` is the gRPC input chunk size (default 64 KiB).
`response_batch_size` is 64; batches flush as they fill.

`rawuds` (built alongside `profile`) pushes the file through a bare Unix
socket with no framing, protobuf, or parsing. It provides a reference for
the machine's raw socket-copy cost.

```bash
./target/release/rawuds WARCFILE.warc          # 64 KiB writes
./target/release/rawuds WARCFILE.warc 262144   # 256 KiB writes
```

For a remote TCP run, start the server on one host and the profile binary as a
client on another:

```bash
FASTWARC_GRPC_ADDR=0.0.0.0:50061 cargo run -p fastwarc-grpc --release
# add FASTWARC_GRPC_ALLOW_LOCAL_FILES=1 to the server env for FASTWARC_GRPC_LOCAL runs
FASTWARC_GRPC_URL=http://server:50061 ./profile WARCFILE.warc
```

## Install Dependencies:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Building also requires a `protoc` binary on the PATH (Debian/Ubuntu: `apt install protobuf-compiler`).

## Build the Benchmark

```bash
make
```

## Run the Benchmark

```bash
sync && echo 3 | sudo tee /proc/sys/vm/drop_caches
./profile WARCFILE.warc
```

## Results

Reviewer reproduction on Ubuntu with a Threadripper 2920X and the 5298 MiB
uncompressed Common Crawl file
`CC-MAIN-20231005012006-20231005042006-00899.warc`:

| Path | Storage | MiB/s |
|---|---|---:|
| `fastwarc` bench, in-process | page cache | 5283.3 |
| `fastwarc-grpc`, upload over Unix socket, parse-only | tmpfs | 927.5 |
| `fastwarc-grpc`, upload over Unix socket, parse-only | SSD | 753.4 |

These are single-stream measurements. With `FASTWARC_GRPC_JOBS=N`, every
stream parses a complete copy of the archive; the reported rate is aggregate
throughput across all copies, not the latency of one file.
