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

`BUFFER_SIZE` is the gRPC input chunk size (default 64 KiB). Throughput is
flat for chunk sizes between 32 KiB and 1 MiB; multi-MiB chunks lose ~30%
to whole-message buffering. `response_batch_size` is 64; batches flush as
they fill.

`rawuds` (built alongside `profile`) is a reference floor: it pushes the
file through a bare Unix socket with no framing, protobuf, or parsing.
Whatever it reports is the ceiling for any socket-based transport on the
machine; compare the gRPC rows against it rather than against in-process
parsing alone.

```bash
./target/release/rawuds WARCFILE.warc          # 64 KiB writes
./target/release/rawuds WARCFILE.warc 262144   # 256 KiB writes
```

For a remote TCP run, start the server on one host and the profile binary as a
client on another:

```bash
FASTWARC_GRPC_ADDR=0.0.0.0:50051 cargo run -p fastwarc-grpc --release
# add FASTWARC_GRPC_ALLOW_LOCAL_FILES=1 to the server env for FASTWARC_GRPC_LOCAL runs
FASTWARC_GRPC_URL=http://server:50051 ./profile WARCFILE.warc
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

Environment: Ryzen 9 9950X3D (16C/32T), 128 GB DDR5, NVMe, Linux. File:
`CC-MAIN-20231005012006-20231005042006-00899.warc(.gz)`, 5298 MiB
uncompressed. Page cache warm (all rows share the same input path, so rows
stay comparable; cold-cache runs shift every row toward disk speed).
Each figure is the median of 5-7 runs; run-to-run spread on this machine is
roughly +/-10%. Rates are MiB/s of uncompressed archive bytes, the same
metric as the other WARC benchmarks.

Reference points:

| Path | MiB/s (median) |
|---|---:|
| `fastwarc` bench, in-process | 13937 |
| raw Unix socket byte pipe, no framing/serde/parse (`rawuds`) | ~8500 |
| `fastwarc` bench, in-process, gzip | 1444 |

`fastwarc-grpc` profile binary, single stream:

| Mode | MiB/s (median) |
|---|---:|
| `archive_path` (server reads the file; no upload) | 14380 |
| upload, Unix socket, parse-only (default) | 4552 |
| upload, loopback TCP, parse-only | 3979 |
| upload, Unix socket, full payload echo | 2869 |
| upload, loopback TCP, full payload echo | 2861 |
| upload, Unix socket, gzip, parse-only | 1463 |

Concurrent streams (`FASTWARC_GRPC_JOBS=N`, aggregate):

| Streams | gzip upload | gzip `archive_path` | uncompressed upload |
|---:|---:|---:|---:|
| 1 | 1463 | - | 4552 |
| 4 | 5530 | - | 6591 |
| 8 | 10607 | 11003 | - |
| 16 | 16090 | 21460 | - |

Uncompressed upload plateaus near 6.5 GiB/s aggregate: the upload path
crosses RAM several times per byte (client read, protobuf encode, kernel
in/out, HTTP/2 receive buffer), so concurrency saturates memory bandwidth
before it saturates the parser. Gzip streams are decompression-bound and
scale close to linearly up to the physical core count. With `archive_path`
the upload leg does not exist and throughput matches in-process parsing.
