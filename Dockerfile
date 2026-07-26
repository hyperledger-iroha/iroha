ARG IROHA_RUST_BUILDER_IMAGE
ARG IROHA_RUNTIME_IMAGE

# builder stage
FROM ${IROHA_RUST_BUILDER_IMAGE} AS builder

WORKDIR /app

ARG IROHA_RELEASE_PREPROVISIONED_BASES="0"
RUN set -eu; \
    if [ "${IROHA_RELEASE_PREPROVISIONED_BASES}" = "1" ]; then \
        command -v cc >/dev/null; \
        command -v mold >/dev/null; \
    else \
        apt-get update -y; \
        apt-get install -y build-essential mold; \
    fi

COPY . .
COPY dist/ /prebuilt-dist/
ARG PROFILE="deploy"
ARG CONFIG_PROFILE="single"
ARG RUSTFLAGS=""
ARG FEATURES=""
ARG CARGOFLAGS=""
ARG CARGO_BUILD_JOBS=""
ARG BINARIES="irohad iroha kagami"
ARG USE_PREBUILT="0"
ARG IROHA_GIT_COMMIT_HASH=""
ARG VALIDATOR_LOCK_SHA256=""
ARG VALIDATOR_SOURCE_TREE_SHA256=""
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    set -e; \
    mkdir -p /outbin /outprovenance; \
    locked_arg=""; \
    if [ "${CONFIG_PROFILE}" = "taira" ]; then \
        test -n "${VALIDATOR_LOCK_SHA256}" || { echo "VALIDATOR_LOCK_SHA256 is required for Taira builds" >&2; exit 1; }; \
        test "${#VALIDATOR_SOURCE_TREE_SHA256}" -eq 64 || { echo "VALIDATOR_SOURCE_TREE_SHA256 must be exactly 64 lowercase hex characters" >&2; exit 1; }; \
        case "${VALIDATOR_SOURCE_TREE_SHA256}" in *[!0-9a-f]*) echo "VALIDATOR_SOURCE_TREE_SHA256 must be exactly 64 lowercase hex characters" >&2; exit 1;; esac; \
        test -f /app/Cargo.lock || { echo "reviewed Cargo.lock is required for Taira builds" >&2; exit 1; }; \
        actual_lock_sha="$(sha256sum /app/Cargo.lock | awk '{print $1}')"; \
        test "${actual_lock_sha}" = "${VALIDATOR_LOCK_SHA256}" || { echo "Taira Cargo.lock checksum mismatch" >&2; exit 1; }; \
        cp /app/Cargo.lock /outprovenance/Cargo.lock; \
        printf '%s\n' "${VALIDATOR_SOURCE_TREE_SHA256}" > /outprovenance/source-tree.sha256; \
        test "${USE_PREBUILT}" != "1" || { echo "Taira images cannot use unproven prebuilt binaries" >&2; exit 1; }; \
        locked_arg="--locked"; \
    fi; \
    if [ "${USE_PREBUILT}" = "1" ]; then \
        for bin in ${BINARIES}; do \
            cp "/prebuilt-dist/docker-bin/${bin}" "/outbin/${bin}"; \
            chmod 755 "/outbin/${bin}"; \
        done; \
    else \
        effective_cargo_build_jobs="${CARGO_BUILD_JOBS}"; \
        if [ "${CONFIG_PROFILE}" = "taira" ] && [ "${effective_cargo_build_jobs}" = "1" ]; then \
            effective_cargo_build_jobs="2"; \
        fi; \
        regular_bins=""; \
        build_kagami=0; \
        for bin in ${BINARIES}; do \
            if [ "${bin}" = "kagami" ]; then \
                build_kagami=1; \
            else \
                regular_bins="${regular_bins} ${bin}"; \
            fi; \
        done; \
        cargo_target_profile_dir="${PROFILE}"; \
        if [ "${PROFILE}" = "dev" ] || [ "${PROFILE}" = "test" ]; then \
            cargo_target_profile_dir="debug"; \
        fi; \
        if [ -n "${regular_bins}" ]; then \
            set -- cargo ${CARGOFLAGS} build ${locked_arg} --profile "${PROFILE}" --features "${FEATURES}"; \
            for bin in ${regular_bins}; do \
                set -- "$@" --bin "$bin"; \
            done; \
            CARGO_BUILD_JOBS="${effective_cargo_build_jobs}" RUSTFLAGS="${RUSTFLAGS}" IROHA_GIT_COMMIT_HASH="${IROHA_GIT_COMMIT_HASH}" mold --run "$@"; \
            for bin in ${regular_bins}; do \
                cp "/app/target/${cargo_target_profile_dir}/${bin}" "/outbin/${bin}"; \
            done; \
        fi; \
        if [ "${build_kagami}" = "1" ]; then \
            CARGO_BUILD_JOBS="${effective_cargo_build_jobs}" RUSTFLAGS="${RUSTFLAGS}" IROHA_GIT_COMMIT_HASH="${IROHA_GIT_COMMIT_HASH}" mold --run cargo ${CARGOFLAGS} build ${locked_arg} --profile "${PROFILE}" --features "${FEATURES}" -p iroha_kagami --bin kagami; \
            cp "/app/target/${cargo_target_profile_dir}/kagami" "/outbin/kagami"; \
        fi; \
    fi

# final image
FROM ${IROHA_RUNTIME_IMAGE}

ARG PROFILE="deploy"
ARG CONFIG_PROFILE="single"
ARG IROHA_RELEASE_PREPROVISIONED_BASES="0"
ARG IROHA_GIT_COMMIT_HASH=""
ARG SOURCE_DATE_EPOCH=""
ARG VALIDATOR_LOCK_SHA256=""
ARG VALIDATOR_SOURCE_TREE_SHA256=""
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
LABEL org.soramitsu.iroha.validator-lock-sha256=$VALIDATOR_LOCK_SHA256
LABEL org.soramitsu.iroha.validator-source-tree-sha256=$VALIDATOR_SOURCE_TREE_SHA256
LABEL org.opencontainers.image.revision=$IROHA_GIT_COMMIT_HASH
LABEL org.soramitsu.iroha.source-date-epoch=$SOURCE_DATE_EPOCH

RUN <<EOT
  set -ex
  if [ "$IROHA_RELEASE_PREPROVISIONED_BASES" = "1" ]; then
    command -v curl >/dev/null
    command -v jq >/dev/null
    test -f /etc/ssl/certs/ca-certificates.crt
    if [ "$CONFIG_PROFILE" = "taira" ]; then
      command -v qemu-img >/dev/null
      command -v mkfs.ext4 >/dev/null
      command -v ip >/dev/null
      command -v iptables >/dev/null
    fi
  else
    apt-get update -y
    apt-get install -y curl ca-certificates jq
    if [ "$CONFIG_PROFILE" = "taira" ]; then
      apt-get install -y qemu-system-x86 qemu-system-arm qemu-utils e2fsprogs iproute2 iptables
    fi
  fi
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
COPY --from=builder /outprovenance/ $APP_DIR/provenance/
COPY scripts/docker_entrypoint.sh $BIN_PATH
COPY configs/soranexus/taira $APP_DIR/configs/soranexus/taira
COPY codec/rans/tables $APP_DIR/codec/rans/tables
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
