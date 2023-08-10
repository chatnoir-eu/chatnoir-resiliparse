FROM quay.io/pypa/manylinux2014_x86_64:latest

RUN set -x \
    && yum install -y \
          devtoolset-10-libasan-devel \
          lz4-devel \
          uchardet-devel \
          zlib-devel

RUN set -x \
    && curl -Lf https://github.com/lexbor/lexbor/archive/refs/tags/v2.2.0.tar.gz > lexbor.tar.gz \
    && tar -xf lexbor.tar.gz \
    && (cd lexbor-* && mkdir build \
        && cmake \
            -DCMAKE_BUILD_TYPE=Release \
            -DCMAKE_INSTALL_LIBDIR=lib64 \
            -DCMAKE_INSTALL_PREFIX=/usr \
            -DBUILD_SHARED_LIBS=ON \
            -B build \
        && cmake --build build -j$(nproc) --target install) \
    && rm -rf lexbor*

RUN set -x \
    && curl -Lf https://github.com/abseil/abseil-cpp/archive/refs/tags/20230802.0.tar.gz > abseil.tar.gz \
    && tar -xf abseil.tar.gz \
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
    && curl -Lf https://github.com/google/re2/releases/download/2023-08-01/re2-2023-08-01.tar.gz > re2.tar.gz \
    && tar -xf re2.tar.gz \
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
