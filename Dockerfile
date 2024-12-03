ARG TARGET_ARCH
FROM quay.io/pypa/manylinux_2_28_${TARGET_ARCH}:latest

RUN set -x \
    && yum install -y \
          gcc-toolset-13-libasan-devel \
          lz4-devel \
          zlib-devel

RUN set -x \
    && curl -Lf https://gitlab.freedesktop.org/uchardet/uchardet/-/archive/v0.0.8/uchardet-v0.0.8.tar.gz | tar xz \
    && (cd uchardet-* && mkdir build \
        && cmake \
            -DCMAKE_BUILD_TYPE=Release \
            -DCMAKE_INSTALL_PREFIX=/usr \
            -DCMAKE_CXX_STANDARD=17 \
            -DLEXBOR_BUILD_STATIC=ON \
            -B build \
        && cmake --build build -j$(nproc) --target install) \
    && rm -rf uchardet*

RUN set -x \
    && curl -Lf https://github.com/lexbor/lexbor/archive/refs/tags/v2.3.0.tar.gz | tar xz \
    && (cd lexbor-* && mkdir build \
        && cmake \
            -DCMAKE_BUILD_TYPE=Release \
            -DCMAKE_INSTALL_PREFIX=/usr \
            -DLEXBOR_BUILD_SHARED=ON \
            -DLEXBOR_BUILD_STATIC=OFF \
            -DLEXBOR_OPTIMIZATION_LEVEL=-O3 \
            -B build \
        && cmake --build build -j$(nproc) --target install) \
    && rm -rf lexbor*

RUN set -x \
    && curl -Lf https://github.com/abseil/abseil-cpp/releases/download/20240116.1/abseil-cpp-20240116.1.tar.gz | tar xz \
    && (cd abseil-cpp-* && mkdir build \
        && cmake \
            -DCMAKE_BUILD_TYPE=Release \
            -DCMAKE_INSTALL_PREFIX=/usr \
            -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
            -DCMAKE_CXX_STANDARD=17 \
            -DBUILD_SHARED_LIBS=OFF \
            -B build \
        && cmake --build build -j$(nproc) --target install) \
    && rm -rf abseil*

RUN set -x \
    && curl -Lf https://github.com/google/re2/releases/download/2024-04-01/re2-2024-04-01.tar.gz | tar xz \
    && (cd re2-* && mkdir build \
        && cmake \
            -DCMAKE_BUILD_TYPE=Release \
            -DCMAKE_INSTALL_PREFIX=/usr \
            -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
            -DCMAKE_CXX_STANDARD=17 \
            -DBUILD_SHARED_LIBS=ON \
            -DRE2_BUILD_TESTING=OFF \
            -B build \
        && cmake --build build -j$(nproc) --target install) \
    && rm -rf re2*
