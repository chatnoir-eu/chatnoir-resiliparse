#!/usr/bin/env bash

# Helper script for downloading and compiling third-party libs to ./lib-third-party.
# Set CPATH to lib-third-party/include and (LD_)LIBRARY_PATH to lib-third-party/lib
# to use the downloading these libs instead of the version installed on your system.

set -xe

mkdir -p lib-third-party && cd lib-third-party

curl -Lf https://gitlab.freedesktop.org/uchardet/uchardet/-/archive/v0.0.8/uchardet-v0.0.8.tar.gz | tar xz \
    && (cd uchardet-* && mkdir build \
        && cmake \
            -DCMAKE_BUILD_TYPE=Release \
            -DCMAKE_INSTALL_PREFIX=$(pwd)/.. \
            -DCMAKE_CXX_STANDARD=17 \
            -DLEXBOR_BUILD_STATIC=ON \
            -B build \
        && cmake --build build -j$(nproc) --target install) \
    && rm -rf uchardet*

curl -Lf https://github.com/lexbor/lexbor/archive/refs/tags/v2.3.0.tar.gz | tar xz \
    && (cd lexbor-* && mkdir build \
        && cmake \
            -DCMAKE_BUILD_TYPE=Release \
            -DCMAKE_INSTALL_PREFIX=$(pwd)/.. \
            -DLEXBOR_BUILD_SHARED=ON \
            -DLEXBOR_BUILD_STATIC=OFF \
            -DLEXBOR_OPTIMIZATION_LEVEL=-O3 \
            -B build \
        && cmake --build build -j$(nproc) --target install) \
    && rm -rf lexbor*

curl -Lf https://github.com/abseil/abseil-cpp/releases/download/20240116.1/abseil-cpp-20240116.1.tar.gz | tar xz \
    && (cd abseil-cpp-* && mkdir build \
        && cmake \
            -DCMAKE_BUILD_TYPE=Release \
            -DCMAKE_INSTALL_PREFIX=$(pwd)/.. \
            -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
            -DCMAKE_CXX_STANDARD=17 \
            -DBUILD_SHARED_LIBS=OFF \
            -B build \
        && cmake --build build -j$(nproc) --target install) \
    && rm -rf abseil*

curl -Lf https://github.com/google/re2/releases/download/2024-04-01/re2-2024-04-01.tar.gz | tar xz \
    && (cd re2-* && mkdir build \
        && cmake \
            -DCMAKE_BUILD_TYPE=Release \
            -DCMAKE_INSTALL_PREFIX=$(pwd)/.. \
            -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
            -DCMAKE_CXX_STANDARD=17 \
            -DBUILD_SHARED_LIBS=ON \
            -DRE2_BUILD_TESTING=OFF \
            -B build \
        && cmake --build build -j$(nproc) --target install) \
    && rm -rf re2*

cd ..
