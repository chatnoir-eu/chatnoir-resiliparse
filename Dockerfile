ARG TARGET_ARCH
FROM quay.io/pypa/manylinux_2_28_${TARGET_ARCH}:latest

RUN set -x \
    && yum install -y \
          gcc-toolset-13-libasan-devel \
          lz4-devel \
          zlib-devel

COPY compile-third-party-libs.sh /usr/bin/compile-third-party-libs
RUN set -x \
  && /usr/bin/compile-third-party-libs /usr
