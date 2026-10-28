ARG TARGET_ARCH=x86_64
FROM quay.io/pypa/manylinux_2_28_${TARGET_ARCH}:latest

RUN set -x \
    && yum install -y \
        clang-devel \
        curl \
        git \
        gcc-toolset-14-libasan-devel \
        ninja-build \
        protobuf-compiler \
        tar \
        unzip \
        zip \
   && yum clean all \
   && rm -rf /var/cache/yum

ENV VCPKG_DISABLE_METRICS="1"
ENV VCPKG_FORCE_SYSTEM_BINARIES="1"

RUN set -x \
  && git clone --depth=1 https://github.com/Microsoft/vcpkg /opt/vcpkg \
  && (cd /opt/vcpkg && ./bootstrap-vcpkg.sh) \
  && ln -nfs /opt/vcpkg/vcpkg /usr/local/bin

ENV RUSTUP_HOME=/opt/rust/rustup
ENV CARGO_HOME=/opt/rust/cargo
ENV PATH=/opt/rust/cargo/bin:$PATH
RUN set -x \
    && curl --proto '=https' --tlsv1.2 -Ssf https://sh.rustup.rs | sh -s -- -y \
    && cargo install cargo-llvm-cov

ENV FORCE_COLOR="1"
