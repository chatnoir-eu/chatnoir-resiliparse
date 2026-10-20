# Benchmark: FastWARC-gRPC

Benchmark for [FastWARC-gRPC](https://github.com/chatnoir-eu/chatnoir-resiliparse/) (Rust).

**NOTE:** This benchmark utilises multiple cores to make up for HTTP/2 serialisation and socket IO overhead. Thus, the
numbers are not comparable to other benchmarks in this suite and compressed WARCs can even outperform uncompressed ones.

Parses one archive uploaded over a Unix domain socket to an in-process `fastwarc-grpc` server. It uses one connection
and one request stream.

The parser options match the plain [`fastwarc` benchmark](../fastwarc):
HTTP parsing and digest verification are disabled, records are reused in place, and payloads are consumed without
returning their bytes. The gRPC server omits header blocks and batches up to 64 response events per message. Both
benchmarks build against this checkout's `fastwarc-rs` crate.

## Install Dependencies:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Build the Benchmark

Rust and `protoc` must be on the PATH. The gRPC benchmark requires Unix.

```bash
make
```

## Run the Benchmark

```bash
sync && echo 3 | sudo tee /proc/sys/vm/drop_caches
./profile WARCFILE.warc
```

The gRPC timer starts after server startup and connection establishment, before opening and uploading the file. It
includes upload, parsing, and response consumption. Its result measures service overhead for one stream; the client,
runtime, and parser can use multiple threads. It is not a single-core parser measurement. Record the machine, revision,
archive, storage, and cache state alongside any published results.

## Results (SSD)

These results were measured on an AMD Ryzen Threadripper 2920X 12-Core CPU with a Samsung 980PRO NVMe SSD (single-core
performance, read buffer size: 1 MiB).

### Uncompressed:

```console
$ sync && echo 3 | sudo tee /proc/sys/vm/drop_caches
$ ./profile CC-MAIN-20231005012006-20231005042006-00899.warc
Reading WARC file: CC-MAIN-20231005012006-20231005042006-00899.warc (one stream, upload over UDS, 1048576 byte chunks, parse-only)
34522 records/s, 933.9 MiB/s, 27.7 KiB/rec (17280 total, 467.5 MiB)
23350 records/s, 1000.6 MiB/s, 43.9 KiB/rec (28960 total, 968.0 MiB)
23577 records/s, 1033.9 MiB/s, 44.9 KiB/rec (40800 total, 1487.2 MiB)
21283 records/s, 961.1 MiB/s, 46.2 KiB/rec (51616 total, 1975.6 MiB)
22579 records/s, 992.4 MiB/s, 45.0 KiB/rec (62912 total, 2472.1 MiB)
21344 records/s, 967.7 MiB/s, 46.4 KiB/rec (73600 total, 2956.7 MiB)
17700 records/s, 1008.3 MiB/s, 58.3 KiB/rec (82464 total, 3461.7 MiB)
17645 records/s, 1030.7 MiB/s, 59.8 KiB/rec (91328 total, 3979.4 MiB)
16923 records/s, 989.2 MiB/s, 59.9 KiB/rec (99808 total, 4475.1 MiB)
17591 records/s, 1008.0 MiB/s, 58.7 KiB/rec (108640 total, 4981.2 MiB)
Summary: 5.4s, 21332 records/s, 989.1 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
```

### Gzip:

```console
$ sync && echo 3 | sudo tee /proc/sys/vm/drop_caches
$ ./profile CC-MAIN-20231005012006-20231005042006-00899.warc.gz
Reading WARC file: CC-MAIN-20231005012006-20231005042006-00899.warc.gz (one stream, upload over UDS, 1048576 byte chunks, parse-only)
28770 records/s, 687.3 MiB/s, 24.5 KiB/rec (14400 total, 344.0 MiB)
18740 records/s, 804.2 MiB/s, 43.9 KiB/rec (23776 total, 746.4 MiB)
19274 records/s, 848.0 MiB/s, 45.1 KiB/rec (33440 total, 1171.6 MiB)
19304 records/s, 838.8 MiB/s, 44.5 KiB/rec (43104 total, 1591.5 MiB)
18365 records/s, 834.4 MiB/s, 46.5 KiB/rec (52288 total, 2008.8 MiB)
18904 records/s, 807.0 MiB/s, 43.7 KiB/rec (61760 total, 2413.1 MiB)
18777 records/s, 847.2 MiB/s, 46.2 KiB/rec (71168 total, 2837.6 MiB)
16452 records/s, 895.5 MiB/s, 55.7 KiB/rec (79424 total, 3287.0 MiB)
15796 records/s, 911.3 MiB/s, 59.1 KiB/rec (87360 total, 3744.8 MiB)
14827 records/s, 898.4 MiB/s, 62.0 KiB/rec (94848 total, 4198.5 MiB)
15866 records/s, 881.9 MiB/s, 56.9 KiB/rec (102816 total, 4641.4 MiB)
14583 records/s, 863.8 MiB/s, 60.7 KiB/rec (110144 total, 5075.4 MiB)
Summary: 6.3s, 18242 records/s, 845.8 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
```

### Zstandard:

```console
$ sync && echo 3 | sudo tee /proc/sys/vm/drop_caches
$ ./profile CC-MAIN-20231005012006-20231005042006-00899.warc.zst
Reading WARC file: CC-MAIN-20231005012006-20231005042006-00899.warc.zst (one stream, upload over UDS, 1048576 byte chunks, parse-only)
36332 records/s, 999.2 MiB/s, 28.2 KiB/rec (18176 total, 499.9 MiB)
25024 records/s, 1088.4 MiB/s, 44.5 KiB/rec (30720 total, 1045.4 MiB)
25324 records/s, 1119.8 MiB/s, 45.3 KiB/rec (43392 total, 1605.8 MiB)
25174 records/s, 1117.5 MiB/s, 45.5 KiB/rec (56000 total, 2165.5 MiB)
26359 records/s, 1157.3 MiB/s, 45.0 KiB/rec (69184 total, 2744.3 MiB)
22700 records/s, 1215.2 MiB/s, 54.8 KiB/rec (80544 total, 3352.4 MiB)
20836 records/s, 1201.2 MiB/s, 59.0 KiB/rec (90976 total, 3953.9 MiB)
20181 records/s, 1190.8 MiB/s, 60.4 KiB/rec (101088 total, 4550.5 MiB)
20480 records/s, 1197.5 MiB/s, 59.9 KiB/rec (111328 total, 5149.3 MiB)
Summary: 4.6s, 24689 records/s, 1144.7 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
```

### LZ4:

```console
$ sync && echo 3 | sudo tee /proc/sys/vm/drop_caches
$ ./profile CC-MAIN-20231005012006-20231005042006-00899.warc.lz4
Reading WARC file: CC-MAIN-20231005012006-20231005042006-00899.warc.lz4 (one stream, upload over UDS, 1048576 byte chunks, parse-only)
53915 records/s, 1769.9 MiB/s, 33.6 KiB/rec (26976 total, 885.6 MiB)
45728 records/s, 2006.7 MiB/s, 44.9 KiB/rec (49856 total, 1889.6 MiB)
43453 records/s, 1931.1 MiB/s, 45.5 KiB/rec (71584 total, 2855.2 MiB)
35451 records/s, 2003.3 MiB/s, 57.9 KiB/rec (89312 total, 3857.0 MiB)
37613 records/s, 2180.6 MiB/s, 59.4 KiB/rec (108160 total, 4949.6 MiB)
Summary: 2.7s, 43069 records/s, 1996.9 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
```

## Results (RAM)

These results were measured on an AMD Ryzen Threadripper 2920X 12-Core CPU with the WARC read directly from RAM
(single-core performance, read buffer size: 1 MiB).

### Uncompressed:

```console
$ ./profile tmpfs/CC-MAIN-20231005012006-20231005042006-00899.warc
Reading WARC file: tmpfs/CC-MAIN-20231005012006-20231005042006-00899.warc (one stream, upload over UDS, 1048576 byte chunks, parse-only)
39069 records/s, 1127.1 MiB/s, 29.5 KiB/rec (19552 total, 564.1 MiB)
31898 records/s, 1385.9 MiB/s, 44.5 KiB/rec (35520 total, 1257.9 MiB)
29498 records/s, 1310.4 MiB/s, 45.5 KiB/rec (50272 total, 1913.2 MiB)
27848 records/s, 1221.4 MiB/s, 44.9 KiB/rec (64288 total, 2527.9 MiB)
24317 records/s, 1181.7 MiB/s, 49.8 KiB/rec (76480 total, 3120.4 MiB)
20732 records/s, 1183.0 MiB/s, 58.4 KiB/rec (86848 total, 3712.0 MiB)
18825 records/s, 1150.6 MiB/s, 62.6 KiB/rec (96320 total, 4290.9 MiB)
22188 records/s, 1217.4 MiB/s, 56.2 KiB/rec (107424 total, 4900.2 MiB)
Summary: 4.3s, 26615 records/s, 1234.0 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
```

### Gzip:

```console
$ ./profile tmpfs/CC-MAIN-20231005012006-20231005042006-00899.warc.gz
Reading WARC file: tmpfs/CC-MAIN-20231005012006-20231005042006-00899.warc.gz (one stream, upload over UDS, 1048576 byte chunks, parse-only)
28816 records/s, 692.1 MiB/s, 24.6 KiB/rec (14464 total, 347.4 MiB)
19347 records/s, 827.8 MiB/s, 43.8 KiB/rec (24160 total, 762.2 MiB)
19379 records/s, 849.2 MiB/s, 44.9 KiB/rec (33856 total, 1187.2 MiB)
19244 records/s, 843.0 MiB/s, 44.9 KiB/rec (43520 total, 1610.5 MiB)
19094 records/s, 860.7 MiB/s, 46.2 KiB/rec (53088 total, 2041.8 MiB)
18867 records/s, 821.2 MiB/s, 44.6 KiB/rec (62528 total, 2452.7 MiB)
18991 records/s, 841.0 MiB/s, 45.3 KiB/rec (72032 total, 2873.6 MiB)
15667 records/s, 877.6 MiB/s, 57.4 KiB/rec (79872 total, 3312.7 MiB)
15514 records/s, 888.4 MiB/s, 58.6 KiB/rec (87648 total, 3758.0 MiB)
14650 records/s, 898.3 MiB/s, 62.8 KiB/rec (94976 total, 4207.3 MiB)
15777 records/s, 879.3 MiB/s, 57.1 KiB/rec (102912 total, 4649.6 MiB)
14744 records/s, 865.0 MiB/s, 60.1 KiB/rec (110304 total, 5083.3 MiB)
Summary: 6.3s, 18254 records/s, 846.3 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
```

### Zstandard:

```console
$ ./profile tmpfs/CC-MAIN-20231005012006-20231005042006-00899.warc.zst
Reading WARC file: tmpfs/CC-MAIN-20231005012006-20231005042006-00899.warc.zst (one stream, upload over UDS, 1048576 byte chunks, parse-only)
35770 records/s, 977.7 MiB/s, 28.0 KiB/rec (17888 total, 489.0 MiB)
25505 records/s, 1105.4 MiB/s, 44.4 KiB/rec (30656 total, 1042.3 MiB)
24490 records/s, 1085.0 MiB/s, 45.4 KiB/rec (42912 total, 1585.3 MiB)
25023 records/s, 1112.1 MiB/s, 45.5 KiB/rec (55456 total, 2142.8 MiB)
25858 records/s, 1132.9 MiB/s, 44.9 KiB/rec (68416 total, 2710.6 MiB)
22539 records/s, 1190.8 MiB/s, 54.1 KiB/rec (79712 total, 3307.4 MiB)
20711 records/s, 1187.3 MiB/s, 58.7 KiB/rec (90080 total, 3901.8 MiB)
20285 records/s, 1191.9 MiB/s, 60.2 KiB/rec (100224 total, 4497.8 MiB)
19816 records/s, 1153.8 MiB/s, 59.6 KiB/rec (110144 total, 5075.4 MiB)
Summary: 4.7s, 24322 records/s, 1127.7 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
```

### LZ4:

```console
$ ./profile tmpfs/CC-MAIN-20231005012006-20231005042006-00899.warc.lz4
Reading WARC file: tmpfs/CC-MAIN-20231005012006-20231005042006-00899.warc.lz4 (one stream, upload over UDS, 1048576 byte chunks, parse-only)
55123 records/s, 1828.9 MiB/s, 34.0 KiB/rec (27872 total, 924.7 MiB)
46367 records/s, 2052.8 MiB/s, 45.3 KiB/rec (51072 total, 1951.9 MiB)
46963 records/s, 2114.6 MiB/s, 46.1 KiB/rec (74560 total, 3009.5 MiB)
37299 records/s, 2167.0 MiB/s, 59.5 KiB/rec (93216 total, 4093.3 MiB)
36680 records/s, 2132.3 MiB/s, 59.5 KiB/rec (111584 total, 5161.1 MiB)
Summary: 2.6s, 44459 records/s, 2061.4 MiB/s, 47.5 KiB/rec (114274 total, 5298.4 MiB)
```
