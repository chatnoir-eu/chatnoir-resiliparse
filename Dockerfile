ARG TARGET_ARCH=x86_64
FROM quay.io/pypa/manylinux_2_28_${TARGET_ARCH}:latest

RUN set -x \
    && yum install -y \
        clang-devel \
        cmake \
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

COPY resiliparse-rs/vcpkg /tmp/build/vcpkg
COPY resiliparse-rs/vcpkg.json resiliparse-rs/vcpkg-configuration.json /tmp/build/

RUN set -x \
    && git clone --depth=1 https://github.com/Microsoft/vcpkg /opt/vcpkg \
    && /opt/vcpkg/bootstrap-vcpkg.sh \
    && (cd /tmp/build \
        && case "$(uname -m)" in \
                x86_64) triplet=x64-linux ;; \
                aarch64) triplet=arm64-linux ;; \
                *) echo "Unsupported architecture: $(uname -m)" >&2; exit 1 ;; \
            esac \
        && /opt/vcpkg/vcpkg install --triplet=${triplet} --x-install-root=/tmp/build/vcpkg_installed \
        && mkdir /opt/libs \
        && mv "/tmp/build/vcpkg_installed/$triplet/"* /opt/libs/) \
    && rm -rf /tmp/build

ENV RESILIPARSE_USE_SYSTEM_LIBS="1"
ENV RESILIPARSE_STATIC_LIBS="1"
ENV CPATH="/opt/libs/include"
ENV RUSTFLAGS="-L native=/opt/libs/lib"

ENV RUSTUP_HOME=/opt/rust/rustup
ENV CARGO_HOME=/opt/rust/cargo
ENV PATH=/opt/vcpkg:/opt/rust/cargo/bin:$PATH
RUN set -x \
    && curl --proto '=https' --tlsv1.2 -Ssf https://sh.rustup.rs | sh -s -- -y \
    && cargo install cargo-llvm-cov

ENV FORCE_COLOR="1"
