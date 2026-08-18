ARG TARGET_ARCH=x86_64
FROM quay.io/pypa/manylinux_2_28_${TARGET_ARCH}:latest

RUN set -x \
    && yum install -y \
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

COPY .vcpkg  /tmp/build/.vcpkg
COPY vcpkg.json vcpkg-configuration.json /tmp/build/

RUN set -x \
  && (cd /tmp/build \
      && TRIPLET=$(uname -m | sed s/x86_64/x64/)-linux \
      && git clone https://github.com/Microsoft/vcpkg \
      && ./vcpkg/bootstrap-vcpkg.sh \
      && ./vcpkg/vcpkg install --triplet=${TRIPLET} \
      && mkdir /opt/libs \
      && mv vcpkg_installed/${TRIPLET}/* /opt/libs/) \
  && rm -rf /tmp/build

ENV RUSTUP_HOME=/opt/rust/rustup
ENV CARGO_HOME=/opt/rust/cargo
ENV PATH=/opt/rust/cargo/bin:$PATH
RUN set -x \
    && curl --proto '=https' --tlsv1.2 -Ssf https://sh.rustup.rs | sh -s -- -y \
    && cargo install cargo-llvm-cov

ENV CPATH="/opt/libs/include"
ENV LIBRARY_PATH="/opt/libs/lib"
ENV LD_LIBRARY_PATH="/opt/libs/lib"
ENV FORCE_COLOR="1"
