# builder stage
FROM rust:slim-bookworm AS builder

WORKDIR /app

# install required packages
RUN apt-get update -y && \
    apt-get install -y build-essential mold

COPY . .
COPY dist/ /prebuilt-dist/
ARG PROFILE="deploy"
ARG RUSTFLAGS=""
ARG FEATURES=""
ARG CARGOFLAGS=""
ARG CARGO_BUILD_JOBS=""
ARG BINARIES="irohad iroha kagami"
ARG USE_PREBUILT="0"
RUN set -e; \
    mkdir -p /outbin; \
    if [ "${USE_PREBUILT}" = "1" ]; then \
        for bin in ${BINARIES}; do \
            cp "/prebuilt-dist/docker-bin/${bin}" "/outbin/${bin}"; \
            chmod 755 "/outbin/${bin}"; \
        done; \
    else \
        set -- cargo ${CARGOFLAGS} build --profile "${PROFILE}" --features "${FEATURES}"; \
        for bin in ${BINARIES}; do \
            set -- "$@" --bin "$bin"; \
        done; \
        CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS}" RUSTFLAGS="${RUSTFLAGS}" mold --run "$@"; \
        for bin in ${BINARIES}; do \
            cp "/app/target/${PROFILE}/${bin}" "/outbin/${bin}"; \
        done; \
    fi

# final image
FROM debian:bookworm-slim

ARG PROFILE="deploy"
ARG CONFIG_PROFILE="single"
ARG APP_DIR=/opt/iroha
ARG  STORAGE=/storage
ARG  TARGET_DIR=/app/target/${PROFILE}
ENV  APP_DIR=$APP_DIR
ENV  BIN_PATH=/usr/local/bin/
ENV  CONFIG_DIR=/config
ENV  KURA_STORE_DIR=$STORAGE
ENV  SNAPSHOT_STORE_DIR=$STORAGE/snapshot
ENV  IROHA_IMAGE_CONFIG_PROFILE=$CONFIG_PROFILE
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
    --home "$APP_DIR" \
    --ingroup "$USER" \
    --no-create-home \
    --uid "$UID" \
    "$USER"
  mkdir -p "$APP_DIR"
  mkdir -p $CONFIG_DIR
  mkdir -p $STORAGE
  mkdir -p "$APP_DIR/configs/soranexus"
  chown $USER:$USER $STORAGE
  chown $USER:$USER $CONFIG_DIR
  chown -R $USER:$USER "$APP_DIR"
EOT

COPY --from=builder /outbin/ $BIN_PATH
COPY scripts/docker_entrypoint.sh $BIN_PATH
COPY configs/soranexus/taira $APP_DIR/configs/soranexus/taira
COPY defaults /tmp/defaults
RUN set -eu; \
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
    taira) \
      :; \
      ;; \
    *) \
      echo "Unsupported CONFIG_PROFILE ${CONFIG_PROFILE}" >&2; \
      exit 1; \
      ;; \
  esac; \
  chown -R "${UID}:${GID}" "${APP_DIR}"; \
  chmod 755 "${BIN_PATH}/docker_entrypoint.sh"; \
  rm -rf /tmp/defaults
WORKDIR $APP_DIR
USER ${UID}:${GID}
ENTRYPOINT ["docker_entrypoint.sh"]
