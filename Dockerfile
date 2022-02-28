FROM quay.io/pypa/manylinux2014_x86_64:latest

#RUN set -x && cat <<'EOF' > /etc/yum.repos.d/lexbor.repo \
#[lexbor] \
#name=lexbor repo \
#baseurl=https://packages.lexbor.com/centos/$releasever/$basearch/ \
#gpgcheck=0 \
#enabled=1 \
#EOF

RUN set -x \
    && git clone -b fix/use-after-free-tokenizer https://github.com/phoerious/lexbor.git \
    && mkdir lexbor/build \
    && (cd lexbor/build \
        && cmake -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_LIBDIR=lib64 -DCMAKE_INSTALL_PREFIX=/usr .. \
        && make -j$(nproc) \
        && make install) \
    && rm -rf lexbor

RUN set -x \
    && yum install -y zlib-devel lz4-devel uchardet-devel re2-devel
