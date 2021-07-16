# ChatNoir Resiliparse

A collection of robust and fast processing tools for parsing and analyzing (not only) web archive data.

Resiliparse is a part of the [ChatNoir web analytics toolkit](https://github.com/chatnoir-eu/).

## Installing Resiliparse
Pre-built Resiliparse binaries can be installed from PyPi:
```bash
pip install resiliparse
```

## Building Resiliparse
To build Resiliparse from sources, you can either compile it from the PyPi source package or directly from this repository. To build Resiliparse from PyPi, run:
```bash
pip install --no-binary resiliparse resiliparse
```
If you prefer to build directly from this repository instead, run:
```bash
# Create venv (recommended, but not required)
python3 -m venv venv && source venv/bin/activate

# Install build dependencies
sudo apt install libuchardet-dev
pip install cython setuptools

# Build and install
BUILD_PACKAGES=resiliparse python setup.py install
```

## Usage Instructions
For detailed usage instructions, please consult the [Resiliparse User Manual](https://resiliparse.chatnoir.eu/en/latest/index.html).
