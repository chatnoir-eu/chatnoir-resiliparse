# Benchmark: FastWARC-gRPC

Benchmark for the [`fastwarc-grpc`](../../../fastwarc-grpc) crate in this repository.

**Note:** Unlike the other benchmarks, this one measures a full client/server round trip. The
profile binary starts the gRPC server in-process, streams the WARC file to it over a loopback TCP
socket, and receives every parsed record back. The numbers therefore include protobuf encoding,
HTTP/2 framing, and transport on top of the parser itself. Compare against the plain `fastwarc`
benchmark to see the service overhead.

The parse configuration matches the plain `fastwarc` benchmark: no HTTP parsing, no digest
verification.

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
