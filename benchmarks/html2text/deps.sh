#!/usr/bin/env bash
#
# Install the native dependencies (lexbor, re2, uchardet, lz4, zlib) for one Resiliparse source tree into
# <prefix>/<triplet>/{include,lib}, the layout setup.py expects in RESILIPARSE_VCPKG_PATH.
#
# Usage: deps.sh <source-tree> <prefix> [--from-source]
#
# By default, this runs vcpkg ($VCPKG_ROOT/vcpkg) with the tree's own manifest and overlay ports, as upstream CI does.
# --from-source is for environments where vcpkg cannot download sources: lexbor is cloned at the repository and ref
# named in the tree's lexbor port and built with CMake, uchardet is built with the tree's overlay patches (if any),
# and re2, lz4 and zlib come from the system (e.g. apt install libre2-dev liblz4-dev zlib1g-dev).

set -euo pipefail

SRC=$(realpath "$1")
PREFIX=$(realpath -m "$2")
MODE=${3:-vcpkg}

case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) TRIPLET=x64-linux ;;
    Linux-aarch64) TRIPLET=arm64-linux ;;
    Darwin-arm64) TRIPLET=arm64-osx ;;
    Darwin-x86_64) TRIPLET=x64-osx ;;
    *) echo "Unsupported platform: $(uname -s)-$(uname -m)" >&2; exit 1 ;;
esac
INSTALL_DIR="$PREFIX/$TRIPLET"

if [ "$MODE" != --from-source ]; then
    VCPKG="${VCPKG_ROOT:?Set VCPKG_ROOT to a vcpkg checkout or pass --from-source}/vcpkg"
    cd "$SRC"
    if [ -f vcpkg.json ]; then
        "$VCPKG" install --triplet="$TRIPLET" --x-install-root="$PREFIX"
    else
        # Older trees (e.g. Marin's resiliparse 0.17) have no manifest; their CI installs these packages explicitly
        "$VCPKG" install --triplet="$TRIPLET" --x-install-root="$PREFIX" \
            --overlay-ports=.vcpkg/ports --overlay-triplets=.vcpkg/triplets lexbor lz4 re2 uchardet zlib
    fi
    exit 0
fi

BUILD_DIR="$PREFIX/build"
mkdir -p "$BUILD_DIR" "$INSTALL_DIR"

# lexbor: same repository, ref and optimization level as the tree's vcpkg port
LEXBOR_PORT="$SRC/.vcpkg/ports/lexbor"
LEXBOR_REPO=$(sed -nE 's/^\s*REPO\s+(\S+).*/\1/p' "$LEXBOR_PORT/portfile.cmake")
LEXBOR_REF=$(sed -nE 's/^\s*REF\s+"?([^"[:space:]]+)"?.*/\1/p' "$LEXBOR_PORT/portfile.cmake")
LEXBOR_VERSION=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["version"])' "$LEXBOR_PORT/vcpkg.json")
LEXBOR_REF=${LEXBOR_REF//\$\{VERSION\}/$LEXBOR_VERSION}
echo "lexbor: $LEXBOR_REPO @ $LEXBOR_REF"
if [ ! -d "$BUILD_DIR/lexbor" ]; then
    git clone -q --filter=blob:none "https://github.com/$LEXBOR_REPO" "$BUILD_DIR/lexbor"
fi
git -C "$BUILD_DIR/lexbor" checkout -q "$LEXBOR_REF"
cmake -S "$BUILD_DIR/lexbor" -B "$BUILD_DIR/lexbor/build" -DCMAKE_BUILD_TYPE=Release \
    -DLEXBOR_BUILD_SHARED=ON -DLEXBOR_BUILD_STATIC=OFF -DLEXBOR_OPTIMIZATION_LEVEL=-O3 \
    -DCMAKE_INSTALL_PREFIX="$INSTALL_DIR" > /dev/null
cmake --build "$BUILD_DIR/lexbor/build" -j "$(nproc 2>/dev/null || sysctl -n hw.ncpu)" > /dev/null
cmake --install "$BUILD_DIR/lexbor/build" > /dev/null

# uchardet 0.0.8 plus the tree's overlay patches, if it has an overlay port
UCHARDET_URL=https://gitlab.freedesktop.org/uchardet/uchardet/-/archive/v0.0.8/uchardet-v0.0.8.tar.gz
rm -rf "$BUILD_DIR/uchardet"
mkdir -p "$BUILD_DIR/uchardet"
curl -fsSL "$UCHARDET_URL" | tar xz -C "$BUILD_DIR/uchardet" --strip-components=1
for patch in "$SRC"/.vcpkg/ports/uchardet/*.patch; do
    [ -e "$patch" ] || continue
    echo "uchardet: applying $(basename "$patch")"
    patch -d "$BUILD_DIR/uchardet" -p1 -s < "$patch"
done
cmake -S "$BUILD_DIR/uchardet" -B "$BUILD_DIR/uchardet/build" -DCMAKE_BUILD_TYPE=Release \
    -Wno-deprecated -DBUILD_STATIC=OFF -DBUILD_BINARY=OFF -DCMAKE_INSTALL_PREFIX="$INSTALL_DIR" -DCMAKE_INSTALL_LIBDIR=lib > /dev/null
cmake --build "$BUILD_DIR/uchardet/build" -j "$(nproc 2>/dev/null || sysctl -n hw.ncpu)" > /dev/null
cmake --install "$BUILD_DIR/uchardet/build" > /dev/null

echo "Installed into $INSTALL_DIR (re2, lz4 and zlib are taken from the system)"
