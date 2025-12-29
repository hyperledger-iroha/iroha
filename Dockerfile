# builder stage
FROM rust:slim-bookworm AS builder

WORKDIR /app

# install required packages
RUN apt-get update -y && \
    apt-get install -y build-essential mold

COPY . .
ARG PROFILE="deploy"
ARG RUSTFLAGS=""
ARG FEATURES=""
ARG CARGOFLAGS=""
RUN RUSTFLAGS="${RUSTFLAGS}" mold --run cargo ${CARGOFLAGS} build --profile "${PROFILE}" --features "${FEATURES}" --bin irohad --bin iroha --bin kagami

# final image
FROM debian:bookworm-slim

ARG PROFILE="deploy"
ARG CONFIG_PROFILE="single"
ARG  STORAGE=/storage
ARG  TARGET_DIR=/app/target/${PROFILE}
ENV  BIN_PATH=/usr/local/bin/
ENV  CONFIG_DIR=/config
ENV  KURA_STORE_DIR=$STORAGE
ENV  SNAPSHOT_STORE_DIR=$STORAGE/snapshot
ENV  USER=iroha
ENV  UID=1001
ENV  GID=1001

RUN <<EOT
  set -ex
  apt-get update -y && \
    apt-get install -y curl ca-certificates jq
  addgroup --gid $GID $USER &&
  adduser \
    --disabled-password \
    --gecos "" \
    --home /app \
    --ingroup "$USER" \
    --no-create-home \
    --uid "$UID" \
    "$USER"
  mkdir -p $CONFIG_DIR
  mkdir -p $STORAGE
  chown $USER:$USER $STORAGE
  chown $USER:$USER $CONFIG_DIR
EOT

COPY --from=builder $TARGET_DIR/irohad $BIN_PATH
COPY --from=builder $TARGET_DIR/iroha $BIN_PATH
COPY --from=builder $TARGET_DIR/kagami $BIN_PATH
COPY defaults /tmp/defaults
RUN set -euo pipefail; \
  case "${CONFIG_PROFILE}" in \
    single) \
      cp /tmp/defaults/genesis.json "${CONFIG_DIR}/genesis.json"; \
      cp /tmp/defaults/client.toml "${CONFIG_DIR}/client.toml"; \
      if [ -d /tmp/defaults/config.d ]; then \
        mkdir -p "${CONFIG_DIR}/config.d"; \
        cp -a /tmp/defaults/config.d/. "${CONFIG_DIR}/config.d/"; \
      fi \
      ;; \
    nexus) \
      cp /tmp/defaults/nexus/genesis.json "${CONFIG_DIR}/genesis.json"; \
      cp /tmp/defaults/nexus/client.toml "${CONFIG_DIR}/client.toml"; \
      cp /tmp/defaults/nexus/config.toml "${CONFIG_DIR}/config.toml"; \
      ;; \
    *) \
      echo "Unsupported CONFIG_PROFILE ${CONFIG_PROFILE}" >&2; \
      exit 1; \
      ;; \
  esac; \
  rm -rf /tmp/defaults
USER $USER
CMD ["irohad"]
