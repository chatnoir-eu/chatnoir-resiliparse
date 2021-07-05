# FastWARC

FastWARC is a high-performance WARC parsing library for Python written in C++/Cython. The API is inspired in large parts by [WARCIO](https://github.com/webrecorder/warcio), but does not aim at being a drop-in replacement.  FastWARC supports compressed and uncompressed WARC/1.0 and WARC/1.1 streams. Supported compression algorithms are GZip and LZ4.

FastWARC belongs to the [ChatNoir Resiliparse toolkit](https://github.com/chatnoir-eu/chatnoir-resiliparse/) for fast and robust web data processing.

## Why FastWARC and not WARCIO?
WARCIO is a fantastic tool for reading and writing WARCs, but it is implemented entirely in Python and thus becomes rather inefficient for large web crawls at the tera- or petabyte scale where a few seconds of additional processing time add up quickly. FastWARC solves these performance issues by being written in efficient, low-level C++. We also took the opportunity to add support for LZ4, a much, much (!) faster compression algorithm than GZip, which unfortunately is the only compression algorithm mentioned in the [WARC specification](https://iipc.github.io/warc-specifications/) (and thus also the only one supported by WARCIO, although it wouldn't be a big deal to add that).

FastWARC's design goals are high speed, a low and fixed memory footprint, and simplicity. For the latter reason, we decided against adding support for the legacy ARC format. If you need that kind of backwards compatibility, use WARCIO instead.

## Installing FastWARC

Pre-built FastWARC binaries for most Linux platforms can be installed from PyPi:
```bash
pip install fastwarc
```
**However:** these binaries are provided *purely for your convenience*. Since they are built on the very old `manylinux` base system for better compatibility, their performance isn't optimal (though still better than WARCIO). For best performance, see the next section on how to build FastWARC yourself.

## Building FastWARC

You can compile FastWARC either from the PyPi source package or directly from this repository, though in any case, you need to install all build-time dependencies first. For Debian / Ubuntu, this is done with:
```bash
sudo apt install build-essential python3-dev zlib1g-dev liblz4-dev
```
Then to build FastWARC from PyPi, run
```bash
pip install --no-binary fastwarc fastwarc
```
That's it. If you prefer to build directly from this repository instead, run:
```bash
# Create venv (recommended, but not required)
python3 -m venv venv && source venv/bin/activate

# Install additional build dependencies
pip install cython setuptools

# Build and install:
BUILD_PACKAGES=fastwarc python setup.py install
```

## Iterating WARC Files:

The central class for stream-processing WARC files is `fastwarc.warc.ArchiveIterator`:
```python
from fastwarc.warc import ArchiveIterator

for record in ArchiveIterator(open('warcfile.warc.gz', 'rb')):
    print(record.record_id)
```
This will iterate over all records in the file and print out their IDs. You can pass any file-like Python object to `ArchiveIterator`, for either an uncompressed or a GZip- or LZ4-compressed WARC. FastWARC will try to auto-detect the stream format, but if you know the compression algorithm beforehand, you can speed up the process a little by explicitly passing a `GZipStream` or `LZ4Stream` object instead:
```python
from fastwarc.stream_io import *

# GZip:
stream = GZipStream(open('warcfile.warc.gz', 'rb'))

# LZ4:
stream = LZ4Stream(open('warcfile.warc.lz4', 'rb'))
```
As a further optimization for local files, it is recommended that you use a `FileStream` instead of a Python file object. `FileStream` is a native file reader that circumvents the entire Python I/O stack for better performance:
```python
from fastwarc.stream_io import *
stream = GZipStream(FileStream('warcfile.warc.gz', 'rb'))
```

### Filtering Records
FastWARC provides several ways in which you can filter and efficiently skip records you are not interested in. These filters are checked very early in the parsing process, right after the WARC header block has been read. Multiple types of filters can be combined.

#### Record Type Filter
If you want only records of a certain type, you can skip all other records efficiently by specifying a bitmask of the desired record types:
```python
from fastwarc.warc import ArchiveIterator, WarcRecordType

for record in ArchiveIterator(stream, record_types=WarcRecordType.request | WarcRecordType.response):
    pass
```
This will skip all records with a `WARC-Type` other than `request` or `response`.

#### Content-Length Filter
You can automatically skip any records whose `Content-Length` exceeds or is lower than a certain value:
```python
from fastwarc.warc import ArchiveIterator

# Skip all records that are larger than 500 KiB
for record in ArchiveIterator(stream, max_content_length=512000):
    pass

# Skip all records that are smaller than 128 bytes
for record in ArchiveIterator(stream, min_content_length=128):
    pass
```

#### Function Filter
If the above-mentioned filter mechanisms are not sufficient, you can pass a function object that accepts as its only parameter a `WarcRecord` and returns a `bool` value. This filter type is much slower than the previous filters, but probably still more efficient than checking the same thing later on in the loop. Be aware that since the record body hasn't been seen yet, you cannot access any information beyond what is in the record headers.

FastWARC comes with a handful of existing filters that you can use:
```python
from fastwarc.warc import *

# Skip any non-HTTP records
for record in ArchiveIterator(stream, func_filter=is_http):
    pass

# Skip records without a block digest
for record in ArchiveIterator(stream, func_filter=has_block_digest):
    pass

# Skip records that are not WARC/1.1
for record in ArchiveIterator(stream, func_filter=is_warc_11):
    pass
```
The full list of pre-defined function filters is: `is_warc_10`, `is_warc_11`, `has_block_digest`, `has_payload_digest`, `is_http`, `is_concurrent`. Besides these, you can pass any Python callable that accepts a `WarcRecord` and returns a `bool`:
```python
# Skip records which haven't been identified as HTML pages
for record in ArchiveIterator(stream, func_filter=lambda r: r.headers.get('WARC-Identified-Payload-Type') == 'text/html'):
    pass

# Skip records without any sort of digest header
for record in ArchiveIterator(stream, func_filter=lambda r: has_block_digest(r) and has_payload_digest(r)):
    pass
```

#### Digest Filter
This is the only filter that is executed after the content is available and will skip any records without or with an invalid block digest:
```python
for record in ArchiveIterator(stream, verify_digests=True):
    pass
```
This is the most expensive filter of all and it will create an in-memory copy of the whole record. See [Verifying record digests](#Verifying-record-digests) for more information on how digest verification works.

### Record Properties
The `ArchiveIterator` returns objects of type `WarcRecord`, which have various properties:
```python
for record in ArchiveIterator(stream):
    record.headers          # Dict-like object containing the WARC headers
    record.record_id        # Shorthand for record.headers['WARC-Record-ID']
    record.record_type      # Shorthand for record.headers['WARC-Type']
    record.content_length   # Effective record payload length
    record.stream_pos       # Record start offset in the (uncompressed) stream
    record.is_http          # Boolean indicating whether record is an HTTP record
    record.http_headers     # Dict-like object containing the parsed HTTP headers
    record.http_charset     # HTTP charset/encoding as reported by the server (if any)
    record.reader           # A BufferedReader for the record content

    # Read and return up to 1024 bytes from the record stream
    body = record.reader.read(1024)
    
    # Consume and return the remaining record bytes
    body += record.reader.read()

    # Or: Consume rest of stream without allocating a buffer for it (i.e., skip over)
    record.reader.consume()
```
As you can see, HTTP request and response records are parsed automatically for convenience. If not needed, you can disable this behaviour by passing `parse_http=False` to the `ArchiveIterator` constructor to avoid unnecessary processing. `record.reader` will then start at the beginning of the HTTP header block instead of the HTTP body. You can parse HTTP headers later on a per-record basis by calling `record.parse_http()` as long as the `BufferedReader` hasn't been consumed at that point.

#### Verifying Record Digests
If a record has digest headers, you can verify the consistency of the record contents and/or its HTTP payload:
```python
for record in ArchiveIterator(stream, parse_http=False):
    if 'WARC-Block-Digest' in record.headers:
        print('Block digest OK:', record.verify_block_digest())

    if 'WARC-Payload-Digest' in record.headers:
        record.parse_http()    # It's safe to call this even if the record has no HTTP payload
        print('Payload digest OK:', record.verify_payload_digest())
```
Note that the `verify_*` methods will simply return `False` if the headers do not exist, so check that first. Also keep in mind that the block verification will fail if the reader has been (partially) consumed, so automatic HTTP parsing has to be turned off for this to work.

A word of warning: Calling either of these two methods will create an in-memory copy of the remaining record stream to preserve its contents for further processing (that's why verifying the HTTP payload digest after verifying the block digest worked in the first place). If your records are very large, you need to ensure that they fit into memory entirely (e.g. by checking `record.content_length`). If you do not want to preserve the stream contents, you can set `consume=True` as a parameter. This will avoid the creation of a stream copy altogether and fully consume the rest of the record instead.

## Command Line Interface (CLI)
Besides the Python API, FastWARC also provides a command line interface with the `fastwarc` command:
```bash
$ fastwarc --help
Usage: fastwarc [OPTIONS] COMMAND [ARGS]...

  FastWARC Command Line Interface.

Options:
  -h, --help  Show this message and exit.

Commands:
  benchmark   Benchmark FastWARC performance.
  check       Verify WARC consistency by checking all digests.
  recompress  Recompress a WARC file with different settings.
```

### Check Digests
You can verify all block and payload digests in the given WARC file and print a summary of all corrupted and (optionally) all intact records with
```bash
fastwarc check INFILE
```
The command will exit with a non-zero exit code if at least one record fails verification.

Run `fastwarc check --help` for a full help listing.

### Recompress WARC
If your WARC is uncompressed or not compressed properly at the record-level or you want to recompress a GZip WARC as LZ4 or vice versa, you can do that with
```bash
fastwarc recompress INFILE OUTFILE
```

Run `fastwarc recompress --help` for a full help listing.

### Benchmark FastWARC vs. WARCIO
The FastWARC CLI comes with a benchmarking tool that allows you to test record decompression and parsing speeds and compare them with WARCIO. Depending on your CPU, your storage speed, and the used compression algorithm, you can typically expect speedups between 1.3x and 6x over WARCIO.

Here are a few example runs:
```bash
# Uncompressed WARC
$ fastwarc benchmark read foo.warc --bench-warcio
Benchmarking read performance from 1 input path(s)...
FastWARC: 126049 records read in 1.92 seconds (65694.38 records/s).
WARCIO:   126049 records read in 9.18 seconds (13734.69 records/s).
Time difference: -7.26 seconds, speedup: 4.78

# GZip WARC
$ fastwarc benchmark read foo.warc.gz --bench-warcio
Benchmarking read performance from 1 input path(s)...
FastWARC: 126049 records read in 13.73 seconds (9179.65 records/s).
WARCIO:   126049 records read in 22.79 seconds (5529.95 records/s).
Time difference: -9.06 seconds, speedup: 1.66

# LZ4 WARC (direct comparison not possible, since WARCIO does not support LZ4)
$ fastwarc benchmark read foo.warc.lz4
Benchmarking read performance from 1 input path(s)...
FastWARC: 126049 records read in 2.70 seconds (46668.25 records/s).
```
The benchmarking tool has additional options, such as reading WARCs directly from a remote S3 data source using `Boto3`. Run `fastwarc benchmark --help` for more information.
