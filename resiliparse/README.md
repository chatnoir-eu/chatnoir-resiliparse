# ChatNoir Resiliparse

A collection of robust and fast processing tools for parsing and analyzing (not only) web archive data.

Resiliparse is a part of the [ChatNoir web analytics toolkit](https://github.com/chatnoir-eu/).

## Installing Resiliparse
Pre-built Resiliparse binaries can be installed from PyPi:
```bash
pip install resiliparse
```

## Building Resiliparse From Source
You can compile Resiliparse either from the PyPi source package or directly from this repository, though in any case, you need to install all required build-time dependencies first. On Ubuntu, this is done as follows:
```bash
# Add Lexbor repository
curl -L https://lexbor.com/keys/lexbor_signing.key | sudo apt-key add -
echo "deb https://packages.lexbor.com/ubuntu/ $(lsb_release -sc) liblexbor" | \
    sudo tee /etc/apt/sources.list.d/lexbor.list

# Install build dependencies
sudo apt update
sudo apt install build-essential python3-dev libuchardet-dev liblexbor-dev
```
To build and install Resiliparse from PyPi, run
```bash
pip install --no-binary resiliparse resiliparse
```
That's it. If you prefer to build and install directly from this repository instead, run:
```bash
pip install -e resiliparse
```
To build the wheels without installing them, run:
```bash
pip wheel -e resiliparse

# Or:
pip install build && python -m build --wheel resiliparse
```

## Usage Instructions
For detailed usage instructions, please consult the [Resiliparse User Manual](https://resiliparse.chatnoir.eu/en/latest/).
