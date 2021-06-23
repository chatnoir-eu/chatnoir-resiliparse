# ChatNoir Resiliparse

A collection of robust and fast processing tools for parsing and analyzing web archive data.

Resiliparse is part of the [ChatNoir](https://chatnoir.eu/) toolkit. If you use ChatNoir or any of its tools for a publication, you can make us happy by citing our ECIR demo paper:
```
@InProceedings{bevendorff:2018,
  address =             {Berlin Heidelberg New York},
  author =              {Janek Bevendorff and Benno Stein and Matthias Hagen and Martin Potthast},
  booktitle =           {Advances in Information Retrieval. 40th European Conference on IR Research (ECIR 2018)},
  editor =              {Leif Azzopardi and Allan Hanbury and Gabriella Pasi and Benjamin Piwowarski},
  ids =                 {potthast:2018c,stein:2018c},
  month =               mar,
  publisher =           {Springer},
  series =              {Lecture Notes in Computer Science},
  site =                {Grenoble, France},
  title =               {{Elastic ChatNoir: Search Engine for the ClueWeb and the Common Crawl}},
  year =                2018
}
```

At the moment, the only component of Resiliparse is *FastWARC*.


# FastWARC

FastWARC is a high-performance WARC parsing library for Python written in C++/Cython. The API is inspired in large parts by [WARCIO](https://github.com/webrecorder/warcio), but does not aim at being a drop-in replacement.  FastWARC supports compressed and uncompressed WARC/1.0 and WARC/1.1 streams. Supported compression algorithms are GZip and LZ4. The legacy ARC format is not supported.

## Building FastWARC

Since FastWARC is written in C++, it needs to be compiled for each target platform individually.

Before you can build the binaries, you need to install all build-time dependencies. For Debian / Ubuntu, this is done with:
```
apt install build-essential python3-dev zlib1g-dev liblz4-dev
```

You can then build and install the binaries like so:
```
# Create venv (recommended, but not required):
python3 -m venv venv && source venv/bin/activate

# Build and install:
pip install cython setuptools
python3 setup.py build_ext
python3 setup.py install
```

## Iterating WARC files:

The central class for iterating through WARC files is `fastwarc.warc.ArchiveIterator`. You can create one like so:
```python
from fastwarc.warc import ArchiveIterator

for record in ArchiveIterator(open('warcfile.warc.gz', 'rb')):
    print(record.record_id)
```
This will load the WARC file and print out all record IDs. You can pass any file-like Python object, either uncompressed or compress with GZip or LZ4. FastWARC will try to auto-detect the stream format, but if you know the used compression algorithm beforehand, you can speed up the process a little by explicitly passing a `GZipStream` or `LZ4Stream` object:
```python
from fastwarc.stream_io import *

# GZip:
stream = GZipStream(open('warcfile.warc.gz', 'rb'))

# LZ4:
stream = LZ4Stream(open('warcfile.warc.lz4', 'rb'))
```
If you are reading local files, it is recommended that you use a `FileStream` instead of a Python file object. `FileStream` is a native file reader that circumvents the entire Python I/O stack for better performance:
```python
from fastwarc.stream_io import *
stream = GZipStream(FileStream('warcfile.warc.gz', 'rb'))
```

### Filtering records
If you are interested only in records of a certain type, you can skip all other records efficiently by passing a bitmask of all desired record types to the `ArchiveIterator` constructor:
```python
from fastwarc.warc import ArchiveIterator, WarcRecordType

for record in ArchiveIterator(stream, WarcRecordType.request | WarcRecordType.response):
    print(record.record_id)
```
This will skip all records with a `WARC-Type` other than `request` or `response`.

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
    record.reader           # A BufferedReader for the record content

    # Consume up to 1024 bytes from the record stream
    b = record.reader.read(1024)
    
    # Consume and return the remaining record bytes
    b += record.reader.read()
```
As you can see, HTTP request and response records are parsed automatically. If not needed, you can disable this behaviour by passing `parse_http=False` to the `ArchiveIterator` constructor to avoid unnecessary processing. `record.reader` will then start at the beginning of the HTTP header block instead of the HTTP body. You can parse HTTP headers later on a per-record basis by calling `record.parse_http()` as long as the `BufferedReader` hasn't been consumed.

#### Verifying Record Digests
If a record has digest headers, you can verify the record contents and/or its HTTP payload:
```python
for record in ArchiveIterator(stream, parse_http=False):
    if 'WARC-Block-Digest' in record.headers:
        print('Block digest OK:', record.verify_block_digest())

    if 'WARC-Payload-Digest' in record.headers:
        record.parse_http()    # It's safe to call this even if the record has no HTTP payload
        print('Payload digest OK:', record.verify_payload_digest())
```
Note that the `verify_*` methods will simply return `False` if the headers do not exist, so check that first. Also keep in mind that the block verification will fail if the reader has been (partially) consumed, so automatic HTTP parsing has to be turned off for this to work.

Another small warning: Calling either of these two methods will create an in-memory copy of the remaining record stream to preserve its contents for further processing (that's why verifying the HTTP payload digest after verifying the block digest worked in the first place). If your records are very large, you need to ensure that they fit entirely into memory (e.g. by checking `record.content_length`).

## Command Line Interface (CLI)
Besides the Python API, FastWARC also provides a command line interface via the `fastwarc` command:
```bash
$ fastwarc --help
Usage: fastwarc [OPTIONS] COMMAND [ARGS]...

Options:
  -h, --help  Show this message and exit.

Commands:
  check       Verify WARC consistency by checking all digests.
  recompress  Recompress a WARC file with different settings
```

### Check Digests
You can verify all block and payload digests in the given WARC file and print a summary of all corrupted and (optionally) intact records with
```bash
$ fastwarc check INFILE
```
The command will exit with a non-zero exit code if at least one record fails verification.

Run `fastwarc check --help` for a full help listing.

### Recompress WARC
If your WARC is uncompressed or not compressed properly at the record-level or you want to recompress a GZip WARC as LZ4 or vice versa, you can do that with
```bash
$ fastwarc recompress INFILE OUTFILE
```

Run `fastwarc recompress --help` for a full help listing.
