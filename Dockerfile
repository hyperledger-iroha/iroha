ARG IROHA_RUST_BUILDER_IMAGE
ARG IROHA_RUNTIME_IMAGE

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

COPY . /app/

ARG PROFILE="deploy"
ARG RUSTFLAGS=""
ARG FEATURES="external-software-signer-bin"
ARG CARGOFLAGS=""
ARG CARGO_BUILD_JOBS=""
ARG BINARIES="iroha3d iroha3d_taira sorafs_governance_dag iroha kagami attachment_sanitizer sorafs_external_software_signer"
ARG USE_PREBUILT="0"
ARG IROHA_GIT_COMMIT_HASH=""
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/cargo-target \
    set -eu; \
    export CARGO_TARGET_DIR=/cargo-target; \
    mkdir -p /outbin; \
    if [ "${USE_PREBUILT}" = "1" ]; then \
        for bin in ${BINARIES}; do \
            cp "/app/dist/docker-bin/${bin}" "/outbin/${bin}"; \
        done; \
    else \
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
            set -- cargo ${CARGOFLAGS} build --locked --profile "${PROFILE}" --features "${FEATURES}"; \
            for bin in ${regular_bins}; do \
                set -- "$@" --bin "${bin}"; \
            done; \
            CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS}" RUSTFLAGS="${RUSTFLAGS}" IROHA_GIT_COMMIT_HASH="${IROHA_GIT_COMMIT_HASH}" mold --run "$@"; \
            for bin in ${regular_bins}; do \
                cp "/cargo-target/${cargo_target_profile_dir}/${bin}" "/outbin/${bin}"; \
            done; \
        fi; \
        if [ "${build_kagami}" = "1" ]; then \
            CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS}" RUSTFLAGS="${RUSTFLAGS}" IROHA_GIT_COMMIT_HASH="${IROHA_GIT_COMMIT_HASH}" mold --run cargo ${CARGOFLAGS} build --locked --profile "${PROFILE}" -p iroha_kagami --bin kagami; \
            cp "/cargo-target/${cargo_target_profile_dir}/kagami" /outbin/kagami; \
        fi; \
    fi; \
    chmod 755 /outbin/*

FROM ${IROHA_RUNTIME_IMAGE}

ARG CONFIG_PROFILE="single"
ARG IROHA_RELEASE_PREPROVISIONED_BASES="0"
ARG IROHA_GIT_COMMIT_HASH=""
ARG SOURCE_DATE_EPOCH=""
ARG APP_DIR=/opt/iroha
ARG STORAGE=/storage
ENV APP_DIR=$APP_DIR
ENV BIN_PATH=/usr/local/bin/
ENV CONFIG_DIR=/config
ENV KURA_STORE_DIR=$STORAGE
ENV SNAPSHOT_STORE_DIR=$STORAGE/snapshot
ENV IROHA_IMAGE_CONFIG_PROFILE=$CONFIG_PROFILE
ENV USER=iroha
ENV UID=1001
ENV GID=1001
LABEL org.opencontainers.image.revision=$IROHA_GIT_COMMIT_HASH
LABEL org.soramitsu.iroha.source-date-epoch=$SOURCE_DATE_EPOCH

RUN <<EOT
  set -eu
  if [ "$IROHA_RELEASE_PREPROVISIONED_BASES" = "1" ]; then
    command -v curl >/dev/null
    command -v jq >/dev/null
    command -v bwrap >/dev/null
    test -f /etc/ssl/certs/ca-certificates.crt
    if [ "$CONFIG_PROFILE" = "taira" ]; then
      command -v python3 >/dev/null
      command -v qemu-img >/dev/null
      test -x /usr/sbin/mke2fs || test -x /sbin/mke2fs
      command -v ip >/dev/null
      command -v iptables >/dev/null
      test -x /usr/bin/bwrap
      test -x /usr/bin/nsenter
      test -x /usr/bin/socat
      test -x /usr/bin/setpriv
      test -x /usr/bin/ldd
    fi
  else
    apt-get update -y
    apt-get install -y curl ca-certificates jq bubblewrap
    if [ "$CONFIG_PROFILE" = "taira" ]; then
      apt-get install -y \
        python3-minimal util-linux socat \
        qemu-system-x86 qemu-system-arm qemu-utils \
        e2fsprogs iproute2 iptables
    fi
  fi
  addgroup --gid "$GID" "$USER"
  adduser --disabled-password --gecos "" --home "$APP_DIR" --ingroup "$USER" --no-create-home --uid "$UID" "$USER"
  mkdir -p "$APP_DIR" "$CONFIG_DIR" "$STORAGE" "$APP_DIR/configs/soranexus"
  chown "$USER:$USER" "$STORAGE" "$CONFIG_DIR"
  chown -R "$USER:$USER" "$APP_DIR"
  chown root:root "$APP_DIR"
  chmod 0755 "$APP_DIR"
EOT

COPY --from=builder /outbin/ $BIN_PATH
COPY --from=builder /app/scripts/docker_entrypoint.sh $BIN_PATH
COPY --from=builder /app/scripts/ci/package_inrou_runtime_v1.py /usr/local/libexec/package_inrou_runtime_v1.py
COPY --from=builder /app/configs/soranexus/taira $APP_DIR/configs/soranexus/taira
COPY --from=builder /app/configs/sorafs/external_software_signer $APP_DIR/install/sorafs/external_software_signer
COPY --from=builder /app/configs/sorafs/runtime_provider_broker $APP_DIR/install/sorafs/runtime_provider_broker
COPY --from=builder /app/codec/rans/tables $APP_DIR/codec/rans/tables
COPY --from=builder /app/defaults /tmp/defaults
RUN set -eu; \
    test -x "${BIN_PATH}/sorafs_external_software_signer"; \
    mkdir -p /usr/local/libexec; \
    cp "${BIN_PATH}/sorafs_external_software_signer" /usr/local/libexec/iroha-runtime-provider-broker-v1; \
    chmod 0555 /usr/local/libexec/iroha-runtime-provider-broker-v1; \
    cmp "${BIN_PATH}/sorafs_external_software_signer" /usr/local/libexec/iroha-runtime-provider-broker-v1; \
    "${BIN_PATH}/sorafs_external_software_signer" --help >/dev/null; \
    /usr/local/libexec/iroha-runtime-provider-broker-v1 --help >/dev/null; \
    case "${CONFIG_PROFILE}" in \
        single) \
            test -x "${BIN_PATH}/iroha3d"; \
            cp /tmp/defaults/genesis.json "${CONFIG_DIR}/genesis.json"; \
            cp /tmp/defaults/client.toml "${CONFIG_DIR}/client.toml"; \
            if [ -d /tmp/defaults/config.d ]; then \
                mkdir -p "${CONFIG_DIR}/config.d"; \
                cp -a /tmp/defaults/config.d/. "${CONFIG_DIR}/config.d/"; \
            fi \
            ;; \
        nexus) \
            test -x "${BIN_PATH}/iroha3d"; \
            cp /tmp/defaults/nexus/genesis.json "${CONFIG_DIR}/genesis.json"; \
            cp /tmp/defaults/nexus/client.toml "${CONFIG_DIR}/client.toml"; \
            cp /tmp/defaults/nexus/config.toml "${CONFIG_DIR}/config.toml" \
            ;; \
        taira) \
            test -x "${BIN_PATH}/iroha3d_taira" \
            ;; \
        *) \
            echo "Unsupported CONFIG_PROFILE ${CONFIG_PROFILE}" >&2; \
            exit 1 \
            ;; \
    esac; \
    chown -R "${UID}:${GID}" "${APP_DIR}"; \
    chown root:root "${APP_DIR}"; \
    chmod 0755 "${APP_DIR}"; \
    chmod 0555 /usr/local/libexec/package_inrou_runtime_v1.py; \
    if [ "${CONFIG_PROFILE}" = "taira" ]; then \
        python3 /usr/local/libexec/package_inrou_runtime_v1.py; \
    fi; \
    chmod 755 "${BIN_PATH}/docker_entrypoint.sh"; \
    rm -rf /tmp/defaults

WORKDIR $APP_DIR
USER ${UID}:${GID}
ENTRYPOINT ["docker_entrypoint.sh"]
