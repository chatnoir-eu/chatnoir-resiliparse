#!/usr/bin/env bash

# Helper script for downloading and compiling third-party libs to ./lib-third-party.
# Set CPATH to lib-third-party/include and (LD_)LIBRARY_PATH to lib-third-party/lib
# to use the downloading these libs instead of the version installed on your system.

set -xe

if [ -n "$1" ]; then
    cd "$1"
else
    mkdir -p lib-third-party && cd lib-third-party
fi

UCHARDET_VERSION=0.0.8
LEXBOR_VERSION=2.4.0
ABSEIL_VERSION=20240722.0
RE2_VERSION=2024-07-02

export BASE_DIR="$(pwd)"

curl -Lf https://gitlab.freedesktop.org/uchardet/uchardet/-/archive/v${UCHARDET_VERSION}/uchardet-v${UCHARDET_VERSION}.tar.gz | tar xz \
    && (cd uchardet-* && mkdir build \
        && cmake \
            -DCMAKE_BUILD_TYPE=Release \
            -DCMAKE_INSTALL_PREFIX="${BASE_DIR}" \
            -DCMAKE_MODULE_PATH="${BASE_DIR}/lib/cmake" \
            -DCMAKE_CXX_FLAGS="-I${BASE_DIR}/include -L${BASE_DIR}/lib" \
            -DCMAKE_CXX_STANDARD=17 \
            -DLEXBOR_BUILD_STATIC=ON \
            -B build \
        && cmake --build build -j$(nproc) --target install) \
    && rm -rf uchardet*

curl -Lf https://github.com/lexbor/lexbor/archive/refs/tags/v${LEXBOR_VERSION}.tar.gz | tar xz \
    && (cd lexbor-* && mkdir build \
        && cmake \
            -DCMAKE_BUILD_TYPE=Release \
            -DCMAKE_INSTALL_PREFIX="${BASE_DIR}" \
            -DCMAKE_MODULE_PATH="${BASE_DIR}/lib/cmake" \
            -DCMAKE_CXX_FLAGS="-I${BASE_DIR}/include -L${BASE_DIR}/lib" \
            -DLEXBOR_BUILD_SHARED=ON \
            -DLEXBOR_BUILD_STATIC=OFF \
            -DLEXBOR_OPTIMIZATION_LEVEL=-O3 \
            -B build \
        && cmake --build build -j$(nproc) --target install) \
    && rm -rf lexbor*

curl -Lf https://github.com/abseil/abseil-cpp/releases/download/${ABSEIL_VERSION}/abseil-cpp-${ABSEIL_VERSION}.tar.gz | tar xz \
    && (cd abseil-cpp-* && mkdir build \
        && cmake \
            -DCMAKE_BUILD_TYPE=Release \
            -DCMAKE_INSTALL_PREFIX="${BASE_DIR}" \
            -DCMAKE_MODULE_PATH="${BASE_DIR}/lib/cmake" \
            -DCMAKE_CXX_FLAGS="-I${BASE_DIR}/include -L${BASE_DIR}/lib" \
            -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
            -DCMAKE_CXX_STANDARD=17 \
            -DBUILD_SHARED_LIBS=OFF \
            -B build \
        && cmake --build build -j$(nproc) --target install) \
    && rm -rf abseil*

curl -Lf https://github.com/google/re2/releases/download/${RE2_VERSION}/re2-${RE2_VERSION}.tar.gz | tar xz \
    && (cd re2-* && mkdir build \
        && cmake \
            -DCMAKE_BUILD_TYPE=Release \
            -DCMAKE_INSTALL_PREFIX="${BASE_DIR}" \
            -DCMAKE_MODULE_PATH="${BASE_DIR}/lib/cmake" \
            -DCMAKE_CXX_FLAGS="-I${BASE_DIR}/include -L${BASE_DIR}/lib" \
            -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
            -DCMAKE_CXX_STANDARD=17 \
            -DBUILD_SHARED_LIBS=ON \
            -DRE2_BUILD_TESTING=OFF \
            -B build \
        && cmake --build build -j$(nproc) --target install) \
    && rm -rf re2*
