ARG BASE_IMAGE=ubuntu:20.04
FROM $BASE_IMAGE AS builder

ENV CARGO_HOME=/cargo_home \
    RUSTUP_HOME=/rustup_home \
    DEBIAN_FRONTEND=noninteractive
ENV PATH="$CARGO_HOME/bin:$PATH"

RUN set -ex; \
    apt-get update  -yq; \
    apt-get install -y --no-install-recommends curl pkg-config apt-utils; \
    apt-get install -y --no-install-recommends \
       build-essential \
       ca-certificates \
       clang \
       llvm-dev \
       libssl-dev; \
    rm -rf /var/lib/apt/lists/*

ARG TOOLCHAIN=stable
RUN set -ex; \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs >/tmp/rustup.sh; \
    sh /tmp/rustup.sh -y --no-modify-path --default-toolchain "$TOOLCHAIN"; \
    rm /tmp/*.sh

COPY . /iroha/
WORKDIR /iroha
RUN cargo build --bin iroha --release

FROM $BASE_IMAGE
RUN set -ex; \
    apt-get update  -yq; \
    apt-get install -y --no-install-recommends pkg-config apt-utils; \
    apt-get install -y --no-install-recommends ca-certificates libssl1.1; \
    rm -rf /var/lib/apt/lists/*
COPY iroha/config.json .
COPY iroha/trusted_peers.json .
COPY iroha/genesis.json .
COPY --from=builder /iroha/target/release/iroha .
CMD ["./iroha"]
