ARG BASE_IMAGE=ubuntu:20.04
FROM $BASE_IMAGE AS rust-base

ENV CARGO_HOME=/cargo_home \
    RUSTUP_HOME=/rustup_home \
    DEBIAN_FRONTEND=noninteractive
ENV PATH="$CARGO_HOME/bin:$PATH"

RUN set -ex; \
    apt-get update  -yq; \
    apt-get install -y --no-install-recommends curl apt-utils; \
    apt-get install -y --no-install-recommends \
       build-essential \
       ca-certificates \
       clang \
       llvm-dev; \
    rm -rf /var/lib/apt/lists/*

ARG TOOLCHAIN=stable
RUN set -ex; \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs >/tmp/rustup.sh; \
    sh /tmp/rustup.sh -y --no-modify-path --default-toolchain "$TOOLCHAIN"; \
    rm /tmp/*.sh

FROM rust-base as cargo-chef
RUN cargo install cargo-chef

FROM cargo-chef as planner
WORKDIR /iroha
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM cargo-chef as builder
ARG PROFILE
WORKDIR /iroha
COPY --from=planner /iroha/recipe.json recipe.json
RUN cargo chef cook $PROFILE --recipe-path recipe.json
COPY . .
RUN cargo build $PROFILE --all

FROM $BASE_IMAGE
COPY core/config.json .
COPY core/trusted_peers.json .
COPY core/genesis.json .
ARG BIN=iroha
ARG TARGET_DIR=debug
COPY --from=builder /iroha/target/$TARGET_DIR/$BIN .
ENV IROHA_TARGET_BIN=$BIN
CMD ./$IROHA_TARGET_BIN
