ARG IROHA_RUST_BUILDER_IMAGE
ARG IROHA_RUNTIME_IMAGE

# builder stage
FROM ${IROHA_RUST_BUILDER_IMAGE} AS builder

WORKDIR /build-context

ARG IROHA_RELEASE_PREPROVISIONED_BASES="0"
RUN set -eu; \
    if [ "${IROHA_RELEASE_PREPROVISIONED_BASES}" = "1" ]; then \
        command -v cc >/dev/null; \
        command -v mold >/dev/null; \
    else \
        apt-get update -y; \
        apt-get install -y build-essential mold; \
    fi

COPY . /build-context/
ARG PROFILE="deploy"
ARG CONFIG_PROFILE="single"
ARG RUSTFLAGS=""
ARG FEATURES=""
ARG CARGOFLAGS=""
ARG CARGO_BUILD_JOBS=""
ARG BINARIES="irohad sorafs_governance_dag iroha kagami attachment_sanitizer"
ARG USE_PREBUILT="0"
ARG IROHA_GIT_COMMIT_HASH=""
ARG VALIDATOR_LOCK_SHA256=""
ARG VALIDATOR_SOURCE_TREE_SHA256=""
ARG WORKSPACE_SOURCE_MANIFEST_SHA256=""
ARG SEALED_SOURCE_ARCHIVE_SHA256=""
ARG SEALED_SOURCE_PATH_LIST_SHA256=""
ARG SEALED_CONTEXT_CONTROL_SHA256=""
RUN set -eu; \
    if [ "${CONFIG_PROFILE}" = "taira" ]; then \
        test ! -e /app && test ! -L /app || { echo "Taira source destination must not already exist" >&2; exit 1; }; \
        mkdir /app; \
        for digest_name in \
            WORKSPACE_SOURCE_MANIFEST_SHA256 \
            SEALED_SOURCE_ARCHIVE_SHA256 \
            SEALED_SOURCE_PATH_LIST_SHA256 \
            SEALED_CONTEXT_CONTROL_SHA256; do \
            eval "digest_value=\${${digest_name}}"; \
            test "${#digest_value}" -eq 64 || { echo "${digest_name} must be exactly 64 lowercase hex characters" >&2; exit 1; }; \
            case "${digest_value}" in *[!0-9a-f]*) echo "${digest_name} must be exactly 64 lowercase hex characters" >&2; exit 1;; esac; \
        done; \
        python3 -I -S /build-context/scripts/compute_workspace_source_manifest.py \
            --validate-sealed-context /build-context; \
        actual_context_control_sha="$(sha256sum /build-context/context-control.sha256 | awk '{print $1}')"; \
        test "${actual_context_control_sha}" = "${SEALED_CONTEXT_CONTROL_SHA256}" || { echo "sealed context control checksum mismatch" >&2; exit 1; }; \
        (cd /build-context && sha256sum -c context-control.sha256); \
        actual_source_archive_sha="$(sha256sum /build-context/taira-workspace-source-v1.seal | awk '{print $1}')"; \
        test "${actual_source_archive_sha}" = "${SEALED_SOURCE_ARCHIVE_SHA256}" || { echo "sealed source archive checksum mismatch" >&2; exit 1; }; \
        actual_source_path_list_sha="$(sha256sum /build-context/taira-workspace-source-paths-v1.bin | awk '{print $1}')"; \
        test "${actual_source_path_list_sha}" = "${SEALED_SOURCE_PATH_LIST_SHA256}" || { echo "sealed source path-list checksum mismatch" >&2; exit 1; }; \
        test "$(tr -d '\n' < /build-context/workspace-source-manifest.sha256)" = "${WORKSPACE_SOURCE_MANIFEST_SHA256}" || { echo "sealed workspace manifest control mismatch" >&2; exit 1; }; \
        test "$(awk '{print $1}' /build-context/source-archive.sha256)" = "${SEALED_SOURCE_ARCHIVE_SHA256}" || { echo "sealed source archive control mismatch" >&2; exit 1; }; \
        test "$(awk '{print $1}' /build-context/source-path-list.sha256)" = "${SEALED_SOURCE_PATH_LIST_SHA256}" || { echo "sealed source path-list control mismatch" >&2; exit 1; }; \
        python3 -I -S /build-context/scripts/compute_workspace_source_manifest.py \
            --root /app \
            --path-list /build-context/taira-workspace-source-paths-v1.bin \
            --extract-sealed-archive /build-context/taira-workspace-source-v1.seal \
            --destination /app \
            --expected-manifest "${WORKSPACE_SOURCE_MANIFEST_SHA256}" \
            --expected-archive-sha256 "${SEALED_SOURCE_ARCHIVE_SHA256}" \
            --expected-path-list-sha256 "${SEALED_SOURCE_PATH_LIST_SHA256}"; \
        cmp /build-context/Dockerfile /app/Dockerfile; \
        cmp /build-context/scripts/compute_workspace_source_manifest.py /app/scripts/compute_workspace_source_manifest.py; \
        cmp /build-context/scripts/taira_image_smoke.sh /app/scripts/taira_image_smoke.sh; \
        detached_digest="$(python3 -I -S /build-context/scripts/compute_workspace_source_manifest.py \
            --root /app \
            --path-list /build-context/taira-workspace-source-paths-v1.bin \
            --require-exact-closure)"; \
        test "${detached_digest}" = "${WORKSPACE_SOURCE_MANIFEST_SHA256}" || { echo "detached Taira source digest mismatch" >&2; exit 1; }; \
    else \
        mkdir -p /app; \
        test ! -L /app || { echo "generic source destination must not be a symlink" >&2; exit 1; }; \
        cp -a /build-context/. /app/; \
    fi

WORKDIR /app
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/cargo-target \
    set -e; \
    export CARGO_TARGET_DIR=/cargo-target; \
    selected_binaries="${BINARIES}"; \
    if [ "${CONFIG_PROFILE}" = "taira" ]; then \
        case " ${selected_binaries} " in \
            *" taira_bootle_lantern_broker "*) : ;; \
            *) selected_binaries="${selected_binaries} taira_bootle_lantern_broker" ;; \
        esac; \
    fi; \
    mkdir -p /outbin /outprovenance; \
    locked_arg=""; \
    workspace_source_manifest_before=""; \
    if [ "${CONFIG_PROFILE}" = "taira" ]; then \
        test -n "${VALIDATOR_LOCK_SHA256}" || { echo "VALIDATOR_LOCK_SHA256 is required for Taira builds" >&2; exit 1; }; \
        test "${#VALIDATOR_SOURCE_TREE_SHA256}" -eq 64 || { echo "VALIDATOR_SOURCE_TREE_SHA256 must be exactly 64 lowercase hex characters" >&2; exit 1; }; \
        case "${VALIDATOR_SOURCE_TREE_SHA256}" in *[!0-9a-f]*) echo "VALIDATOR_SOURCE_TREE_SHA256 must be exactly 64 lowercase hex characters" >&2; exit 1;; esac; \
        test "${#WORKSPACE_SOURCE_MANIFEST_SHA256}" -eq 64 || { echo "WORKSPACE_SOURCE_MANIFEST_SHA256 must be exactly 64 lowercase hex characters" >&2; exit 1; }; \
        case "${WORKSPACE_SOURCE_MANIFEST_SHA256}" in *[!0-9a-f]*) echo "WORKSPACE_SOURCE_MANIFEST_SHA256 must be exactly 64 lowercase hex characters" >&2; exit 1;; esac; \
        test -f /app/Cargo.lock || { echo "reviewed Cargo.lock is required for Taira builds" >&2; exit 1; }; \
        actual_lock_sha="$(sha256sum /app/Cargo.lock | awk '{print $1}')"; \
        test "${actual_lock_sha}" = "${VALIDATOR_LOCK_SHA256}" || { echo "Taira Cargo.lock checksum mismatch" >&2; exit 1; }; \
        workspace_source_manifest_before="$(python3 -I -S /build-context/scripts/compute_workspace_source_manifest.py \
            --root /app \
            --path-list /build-context/taira-workspace-source-paths-v1.bin \
            --require-exact-closure)"; \
        test "${workspace_source_manifest_before}" = "${WORKSPACE_SOURCE_MANIFEST_SHA256}" || { echo "Taira source changed before Cargo" >&2; exit 1; }; \
        cp /app/Cargo.lock /outprovenance/Cargo.lock; \
        printf '%s\n' "${VALIDATOR_SOURCE_TREE_SHA256}" > /outprovenance/source-tree.sha256; \
        printf '%s\n' "${workspace_source_manifest_before}" > /outprovenance/workspace-source-manifest.sha256; \
        test "${USE_PREBUILT}" != "1" || { echo "Taira images cannot use unproven prebuilt binaries" >&2; exit 1; }; \
        locked_arg="--locked"; \
    fi; \
    if [ "${USE_PREBUILT}" = "1" ]; then \
        for bin in ${selected_binaries}; do \
            cp "/app/dist/docker-bin/${bin}" "/outbin/${bin}"; \
            chmod 755 "/outbin/${bin}"; \
        done; \
    else \
        regular_bins=""; \
        build_kagami=0; \
        for bin in ${selected_binaries}; do \
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
            CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS}" RUSTFLAGS="${RUSTFLAGS}" IROHA_GIT_COMMIT_HASH="${IROHA_GIT_COMMIT_HASH}" mold --run "$@"; \
            for bin in ${regular_bins}; do \
                cp "/cargo-target/${cargo_target_profile_dir}/${bin}" "/outbin/${bin}"; \
            done; \
        fi; \
        if [ "${build_kagami}" = "1" ]; then \
            CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS}" RUSTFLAGS="${RUSTFLAGS}" IROHA_GIT_COMMIT_HASH="${IROHA_GIT_COMMIT_HASH}" mold --run cargo ${CARGOFLAGS} build ${locked_arg} --profile "${PROFILE}" --features "${FEATURES}" -p iroha_kagami --bin kagami; \
            cp "/cargo-target/${cargo_target_profile_dir}/kagami" "/outbin/kagami"; \
        fi; \
        if [ "${CONFIG_PROFILE}" = "taira" ]; then \
            test "${PROFILE}" = "release" || { echo "Taira native privacy evidence requires PROFILE=release" >&2; exit 1; }; \
            test "${FEATURES}" = "embedded-soracloud-runtime,zk-stark" || { echo "Taira images require the exact embedded-soracloud-runtime,zk-stark feature set" >&2; exit 1; }; \
            validator_privacy_features="$(cargo ${CARGOFLAGS} tree ${locked_arg} -e features,no-dev -p irohad --features "${FEATURES}" -i iroha_core)"; \
            case "${validator_privacy_features}" in *privacy-release-evidence*) echo "Taira irohad must not contain privacy-release-evidence" >&2; exit 1;; esac; \
            validator_fixture_features="$(cargo ${CARGOFLAGS} tree ${locked_arg} -e features,no-dev -p irohad --features "${FEATURES}" -i iroha_data_model)"; \
            case "${validator_fixture_features}" in *'iroha_data_model feature "test-fixtures"'*) echo "Taira irohad must not contain deterministic privacy test fixtures" >&2; exit 1;; esac; \
            runner_privacy_features="$(cargo ${CARGOFLAGS} tree ${locked_arg} -e features,no-dev -p iroha_test_network --features privacy-release-evidence -i iroha_core)"; \
            case "${runner_privacy_features}" in *'iroha_test_network feature "privacy-release-evidence"'*) :;; *) echo "Taira privacy runner feature graph omits privacy-release-evidence" >&2; exit 1;; esac; \
            case "${runner_privacy_features}" in *'iroha_core feature "privacy-release-evidence"'*) :;; *) echo "Taira privacy runner does not forward privacy-release-evidence to iroha_core" >&2; exit 1;; esac; \
            runner_fixture_features="$(cargo ${CARGOFLAGS} tree ${locked_arg} -e features,no-dev -p iroha_test_network --features privacy-release-evidence -i iroha_data_model)"; \
            case "${runner_fixture_features}" in *'iroha_data_model feature "test-fixtures"'*) :;; *) echo "Taira privacy runner omits compiled exact12 semantics" >&2; exit 1;; esac; \
            CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS}" RUSTFLAGS="${RUSTFLAGS}" IROHA_GIT_COMMIT_HASH="${IROHA_GIT_COMMIT_HASH}" mold --run cargo ${CARGOFLAGS} rustc ${locked_arg} --profile "${PROFILE}" -p iroha_test_network --bin taira_privacy_release_runner --features privacy-release-evidence -- -C target-feature=+crt-static; \
            cp "/cargo-target/${cargo_target_profile_dir}/taira_privacy_release_runner" /outbin/taira_privacy_release_runner; \
            command -v readelf >/dev/null || { echo "readelf is required to authenticate the static Taira privacy runner" >&2; exit 1; }; \
            ! readelf -lW /outbin/taira_privacy_release_runner | grep -Fq ' INTERP ' || { echo "Taira privacy runner must not contain PT_INTERP" >&2; exit 1; }; \
            ! readelf -dW /outbin/taira_privacy_release_runner | grep -Fq '(NEEDED)' || { echo "Taira privacy runner must not contain DT_NEEDED" >&2; exit 1; }; \
            test -s /app/fixtures/privacy/exact12_v1.tsv && test ! -L /app/fixtures/privacy/exact12_v1.tsv && test "$(stat -c '%h' /app/fixtures/privacy/exact12_v1.tsv)" = 1 || { echo "Taira exact12 matrix is missing or not one singly linked regular file" >&2; exit 1; }; \
            test -s /app/fixtures/privacy/native_release_expectations_v1.norito && test ! -L /app/fixtures/privacy/native_release_expectations_v1.norito && test "$(stat -c '%h' /app/fixtures/privacy/native_release_expectations_v1.norito)" = 1 || { echo "Taira native privacy Norito expectations are missing or not one singly linked regular file" >&2; exit 1; }; \
            test -s /app/fixtures/privacy/native_release_expectations_v1.json && test ! -L /app/fixtures/privacy/native_release_expectations_v1.json && test "$(stat -c '%h' /app/fixtures/privacy/native_release_expectations_v1.json)" = 1 || { echo "Taira native privacy JSON expectations are missing or not one singly linked regular file" >&2; exit 1; }; \
            test -s /app/fixtures/privacy/zk_x509_native_resource_v1.norito && test ! -L /app/fixtures/privacy/zk_x509_native_resource_v1.norito && test "$(stat -c '%h' /app/fixtures/privacy/zk_x509_native_resource_v1.norito)" = 1 || { echo "Taira X.509 native-resource Norito certificate is missing or not one singly linked regular file" >&2; exit 1; }; \
            test -s /app/fixtures/privacy/zk_x509_native_resource_v1.json && test ! -L /app/fixtures/privacy/zk_x509_native_resource_v1.json && test "$(stat -c '%h' /app/fixtures/privacy/zk_x509_native_resource_v1.json)" = 1 || { echo "Taira X.509 native-resource JSON certificate is missing or not one singly linked regular file" >&2; exit 1; }; \
            mkdir -p /outprovenance/privacy-native; \
            /outbin/taira_privacy_release_runner generate \
                --build-profile release \
                --source-sha256 "${workspace_source_manifest_before}" \
                --exact12-matrix /app/fixtures/privacy/exact12_v1.tsv \
                --expectations-norito /app/fixtures/privacy/native_release_expectations_v1.norito \
                --expectations-json /app/fixtures/privacy/native_release_expectations_v1.json \
                --x509-resource-norito /app/fixtures/privacy/zk_x509_native_resource_v1.norito \
                --x509-resource-json /app/fixtures/privacy/zk_x509_native_resource_v1.json \
                --cargo-lock /outprovenance/Cargo.lock \
                --validator-binary /outbin/irohad \
                --command-manifest-norito-out /outprovenance/privacy-native/command-manifest-v1.norito \
                --command-manifest-json-out /outprovenance/privacy-native/command-manifest-v1.json \
                --stage-artifacts-norito-out /outprovenance/privacy-native/stage-artifacts-v1.norito \
                --stage-artifacts-json-out /outprovenance/privacy-native/stage-artifacts-v1.json \
                --receipt-norito-out /outprovenance/privacy-native/receipt-v1.norito \
                --receipt-json-out /outprovenance/privacy-native/receipt-v1.json; \
            cp /app/fixtures/privacy/exact12_v1.tsv /outprovenance/privacy-native/exact12-v1.tsv; \
            cp /app/fixtures/privacy/native_release_expectations_v1.norito /outprovenance/privacy-native/expectations-v1.norito; \
            cp /app/fixtures/privacy/native_release_expectations_v1.json /outprovenance/privacy-native/expectations-v1.json; \
            cp /app/fixtures/privacy/zk_x509_native_resource_v1.norito /outprovenance/privacy-native/zk-x509-resource-v1.norito; \
            cp /app/fixtures/privacy/zk_x509_native_resource_v1.json /outprovenance/privacy-native/zk-x509-resource-v1.json; \
            printf '%s\n' "${workspace_source_manifest_before}" > /outprovenance/privacy-native/workspace-source-manifest.sha256; \
            for evidence_path in \
                /outprovenance/privacy-native/exact12-v1.tsv \
                /outprovenance/privacy-native/expectations-v1.norito \
                /outprovenance/privacy-native/expectations-v1.json \
                /outprovenance/privacy-native/zk-x509-resource-v1.norito \
                /outprovenance/privacy-native/zk-x509-resource-v1.json \
                /outprovenance/privacy-native/command-manifest-v1.norito \
                /outprovenance/privacy-native/command-manifest-v1.json \
                /outprovenance/privacy-native/stage-artifacts-v1.norito \
                /outprovenance/privacy-native/stage-artifacts-v1.json \
                /outprovenance/privacy-native/receipt-v1.norito \
                /outprovenance/privacy-native/receipt-v1.json; do \
                test -s "${evidence_path}" && test ! -L "${evidence_path}" || { echo "Taira native privacy evidence is missing or not regular: ${evidence_path}" >&2; exit 1; }; \
            done; \
            /outbin/taira_privacy_release_runner verify \
                --build-profile release \
                --source-sha256 "${workspace_source_manifest_before}" \
                --exact12-matrix /outprovenance/privacy-native/exact12-v1.tsv \
                --expectations-norito /outprovenance/privacy-native/expectations-v1.norito \
                --expectations-json /outprovenance/privacy-native/expectations-v1.json \
                --x509-resource-norito /outprovenance/privacy-native/zk-x509-resource-v1.norito \
                --x509-resource-json /outprovenance/privacy-native/zk-x509-resource-v1.json \
                --cargo-lock /outprovenance/Cargo.lock \
                --validator-binary /outbin/irohad \
                --command-manifest-norito /outprovenance/privacy-native/command-manifest-v1.norito \
                --command-manifest-json /outprovenance/privacy-native/command-manifest-v1.json \
                --stage-artifacts-norito /outprovenance/privacy-native/stage-artifacts-v1.norito \
                --stage-artifacts-json /outprovenance/privacy-native/stage-artifacts-v1.json \
                --receipt-norito /outprovenance/privacy-native/receipt-v1.norito \
                --receipt-json /outprovenance/privacy-native/receipt-v1.json; \
            (cd /outprovenance/privacy-native && find . -type f ! -name sha256sums.txt -print | LC_ALL=C sort | xargs sha256sum > sha256sums.txt); \
            workspace_source_manifest_after="$(python3 -I -S /build-context/scripts/compute_workspace_source_manifest.py \
                --root /app \
                --path-list /build-context/taira-workspace-source-paths-v1.bin \
                --require-exact-closure)"; \
            test "${workspace_source_manifest_after}" = "${workspace_source_manifest_before}" || { echo "Taira source changed after native privacy evidence" >&2; exit 1; }; \
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
ARG WORKSPACE_SOURCE_MANIFEST_SHA256=""
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
LABEL org.soramitsu.iroha.workspace-source-manifest-sha256=$WORKSPACE_SOURCE_MANIFEST_SHA256
LABEL org.opencontainers.image.revision=$IROHA_GIT_COMMIT_HASH
LABEL org.soramitsu.iroha.source-date-epoch=$SOURCE_DATE_EPOCH

RUN <<EOT
  set -ex
  if [ "$IROHA_RELEASE_PREPROVISIONED_BASES" = "1" ]; then
    command -v curl >/dev/null
    command -v jq >/dev/null
    command -v bwrap >/dev/null
    test -f /etc/ssl/certs/ca-certificates.crt
    if [ "$CONFIG_PROFILE" = "taira" ]; then
      command -v qemu-img >/dev/null
      command -v mkfs.ext4 >/dev/null
      command -v ip >/dev/null
      command -v iptables >/dev/null
    fi
  else
    apt-get update -y
    apt-get install -y curl ca-certificates jq bubblewrap
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
COPY --from=builder /app/scripts/docker_entrypoint.sh $BIN_PATH
COPY --from=builder /app/configs/soranexus/taira $APP_DIR/configs/soranexus/taira
COPY --from=builder /app/codec/rans/tables $APP_DIR/codec/rans/tables
COPY --from=builder /app/defaults /tmp/defaults
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
  if [ "${CONFIG_PROFILE}" = "taira" ]; then \
    /usr/local/bin/taira_privacy_release_runner verify \
      --build-profile release \
      --source-sha256 "${WORKSPACE_SOURCE_MANIFEST_SHA256}" \
      --exact12-matrix /opt/iroha/provenance/privacy-native/exact12-v1.tsv \
      --expectations-norito /opt/iroha/provenance/privacy-native/expectations-v1.norito \
      --expectations-json /opt/iroha/provenance/privacy-native/expectations-v1.json \
      --x509-resource-norito /opt/iroha/provenance/privacy-native/zk-x509-resource-v1.norito \
      --x509-resource-json /opt/iroha/provenance/privacy-native/zk-x509-resource-v1.json \
      --cargo-lock /opt/iroha/provenance/Cargo.lock \
      --validator-binary /usr/local/bin/irohad \
      --command-manifest-norito /opt/iroha/provenance/privacy-native/command-manifest-v1.norito \
      --command-manifest-json /opt/iroha/provenance/privacy-native/command-manifest-v1.json \
      --stage-artifacts-norito /opt/iroha/provenance/privacy-native/stage-artifacts-v1.norito \
      --stage-artifacts-json /opt/iroha/provenance/privacy-native/stage-artifacts-v1.json \
      --receipt-norito /opt/iroha/provenance/privacy-native/receipt-v1.norito \
      --receipt-json /opt/iroha/provenance/privacy-native/receipt-v1.json; \
  fi; \
  chown -R "${UID}:${GID}" "${APP_DIR}"; \
  chmod 755 "${BIN_PATH}/docker_entrypoint.sh"; \
  rm -rf /tmp/defaults
WORKDIR $APP_DIR
USER ${UID}:${GID}
ENTRYPOINT ["docker_entrypoint.sh"]
