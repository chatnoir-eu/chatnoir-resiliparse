# FastWARC

FastWARC is a high-performance WARC parsing library for Python written in C++/Cython. The API is inspired in large parts by [WARCIO](https://github.com/webrecorder/warcio), but does not aim at being a drop-in replacement.  FastWARC supports compressed and uncompressed WARC/1.0 and WARC/1.1 streams. Supported compression algorithms are GZip and LZ4.

FastWARC belongs to the [ChatNoir Resiliparse toolkit](https://github.com/chatnoir-eu/chatnoir-resiliparse/) for fast and robust web data processing.

## Installing FastWARC

Pre-built FastWARC binaries for most Linux platforms can be installed from PyPi:
```bash
pip install fastwarc
```
**However:** these binaries are provided *solely for your convenience*. Since they are built on the very old `manylinux` base system for better compatibility, their performance isn't optimal (though still better than WARCIO). For best performance, see the next section on how to build FastWARC yourself.

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

## Usage Instructions
For detailed usage instructions, please consult the [FastWARC User Manual](https://resiliparse.chatnoir.eu/en/latest/man/fastwarc.html).
