#!/usr/bin/env bash
# Shared top-level builder for private Sumeragi v2 release-binary bundles.
#
# Callers must define wait_for_external_cargo and run_cargo.  They pass the
# canonical repository root and the already verified workspace source digest to
# each public function below.

sumeragi_v2_localnet_binary_attestation_valid() {
  local prebuilt_repo_root="$1"
  local prebuilt_source_manifest_sha256="$2"
  if [[ -z "${IROHA_TEST_TARGET_DIR:-}" \
    || -z "${IROHA_RELEASE_PREBUILT_MANIFEST_SHA256:-}" ]]; then
    return 1
  fi
  python3 -I -S "${prebuilt_repo_root}/scripts/sumeragi_v2_prebuilt_bundle.py" validate \
    --repo-root "$prebuilt_repo_root" \
    --source-manifest "$prebuilt_source_manifest_sha256" \
    --bundle-dir "$IROHA_TEST_TARGET_DIR" \
    --manifest-sha256 "$IROHA_RELEASE_PREBUILT_MANIFEST_SHA256"
}

sumeragi_v2_ensure_source_bound_localnet_binaries() {
  local prebuilt_repo_root="$1"
  local prebuilt_source_manifest_sha256="$2"
  local prebuilt_helper="${prebuilt_repo_root}/scripts/sumeragi_v2_prebuilt_bundle.py"
  local prebuilt_source_root="${prebuilt_repo_root}/target/sumeragi-v2-release/${prebuilt_source_manifest_sha256}"
  local prebuilt_programs_root="${prebuilt_source_root}/programs"
  local prebuilt_build_root="${prebuilt_source_root}/program-build-cache"
  local prebuilt_default_cache="${prebuilt_build_root}/default"
  local prebuilt_message_control_cache="${prebuilt_build_root}/message-control"

  # An inherited manifest digest is the parent invocation's trust capability.
  # Never replace or rebuild a bundle when that capability is present.
  if [[ -n "${IROHA_RELEASE_PREBUILT_MANIFEST_SHA256:-}" ]]; then
    if sumeragi_v2_localnet_binary_attestation_valid \
      "$prebuilt_repo_root" "$prebuilt_source_manifest_sha256"; then
      return 0
    fi
    echo "inherited release prebuilt manifest or invocation bundle is invalid" >&2
    return 1
  fi

  # A standalone top-level runner always creates a new private bundle.  Ignore
  # an unanchored target supplied by the environment.
  unset IROHA_TEST_TARGET_DIR
  python3 -I -S "$prebuilt_helper" prepare-cache \
    --repo-root "$prebuilt_repo_root" \
    --source-manifest "$prebuilt_source_manifest_sha256" \
    --default-cache "$prebuilt_default_cache" \
    --message-control-cache "$prebuilt_message_control_cache"

  local prebuilt_build_lock="${prebuilt_build_root}/.sumeragi-v2-prebuild.lock"
  if ! mkdir -- "$prebuilt_build_lock"; then
    echo "another process owns the source-bound localnet build lock: ${prebuilt_build_lock}" >&2
    return 1
  fi
  local prebuilt_version_dir
  if ! prebuilt_version_dir="$(mktemp -d "${prebuilt_build_root}/tool-versions.XXXXXX")"; then
    rmdir -- "$prebuilt_build_lock"
    return 1
  fi
  local prebuilt_cargo_version_file="${prebuilt_version_dir}/cargo-version.txt"
  local prebuilt_rustc_version_file="${prebuilt_version_dir}/rustc-version.txt"
  local prebuilt_publication_output
  if ! prebuilt_publication_output="$(mktemp "${prebuilt_build_root}/publication.XXXXXX")"; then
    rmdir -- "$prebuilt_version_dir"
    rmdir -- "$prebuilt_build_lock"
    return 1
  fi

  if ! (
    cleanup_prebuilt_build() {
      local status=$?
      rm -f -- \
        "$prebuilt_cargo_version_file" \
        "$prebuilt_rustc_version_file"
      if ((status != 0)); then
        rm -f -- "$prebuilt_publication_output"
      fi
      rmdir -- "$prebuilt_version_dir" 2>/dev/null || status=1
      rmdir -- "$prebuilt_build_lock" 2>/dev/null || status=1
      trap - EXIT
      exit "$status"
    }
    trap cleanup_prebuilt_build EXIT

    # Cargo may otherwise accept a stale final executable whose dependency
    # metadata survived in the fixed cache.  Remove only the four exact
    # top-level outputs so this invocation must relink them.
    rm -f -- \
      "${prebuilt_default_cache}/release/iroha3d" \
      "${prebuilt_default_cache}/release/iroha" \
      "${prebuilt_default_cache}/release/kagami" \
      "${prebuilt_message_control_cache}/release/iroha3d"

    (
      export CARGO_TARGET_DIR="$prebuilt_default_cache"
      export ENABLE_RANS_BUNDLES=1
      export NORITO_SKIP_BINDINGS_SYNC=1
      run_cargo build --locked --offline --release -p irohad --bin iroha3d
      run_cargo build --locked --offline --release -p iroha_cli --bin iroha
      run_cargo build --locked --offline --release -p iroha_kagami --bin kagami
    )
    (
      export CARGO_TARGET_DIR="$prebuilt_message_control_cache"
      export ENABLE_RANS_BUNDLES=1
      export NORITO_SKIP_BINDINGS_SYNC=1
      run_cargo build --locked --offline --release -p irohad --bin iroha3d \
        --features test-network-message-control
    )

    # Keep the mandatory process snapshot outside redirected stdout so the
    # exact version transcript contains only the tool's bytes.
    wait_for_external_cargo
    command cargo --version >"$prebuilt_cargo_version_file"
    wait_for_external_cargo
    command rustc -vV >"$prebuilt_rustc_version_file"

    python3 -I -S "$prebuilt_helper" create \
      --repo-root "$prebuilt_repo_root" \
      --source-manifest "$prebuilt_source_manifest_sha256" \
      --default-cache "$prebuilt_default_cache" \
      --message-control-cache "$prebuilt_message_control_cache" \
      --programs-root "$prebuilt_programs_root" \
      --cargo-version-file "$prebuilt_cargo_version_file" \
      --rustc-version-file "$prebuilt_rustc_version_file" \
      >"$prebuilt_publication_output"
  ); then
    rm -f -- "$prebuilt_publication_output"
    return 1
  fi

  if [[ ! -f "$prebuilt_publication_output" || -L "$prebuilt_publication_output" \
    || "$(wc -l <"$prebuilt_publication_output" | tr -d '[:space:]')" != 2 ]]; then
    rm -f -- "$prebuilt_publication_output"
    echo "release bundle publisher returned malformed output" >&2
    return 1
  fi
  local prebuilt_bundle_line prebuilt_manifest_line
  prebuilt_bundle_line="$(sed -n '1p' "$prebuilt_publication_output")"
  prebuilt_manifest_line="$(sed -n '2p' "$prebuilt_publication_output")"
  rm -f -- "$prebuilt_publication_output"
  if [[ "$prebuilt_bundle_line" != $'bundle_dir\t'* \
    || "$prebuilt_manifest_line" != $'manifest_sha256\t'* ]]; then
    echo "release bundle publisher returned unexpected fields" >&2
    return 1
  fi
  export IROHA_TEST_TARGET_DIR="${prebuilt_bundle_line#*$'\t'}"
  export IROHA_RELEASE_PREBUILT_MANIFEST_SHA256="${prebuilt_manifest_line#*$'\t'}"
  if ! sumeragi_v2_localnet_binary_attestation_valid \
    "$prebuilt_repo_root" "$prebuilt_source_manifest_sha256"; then
    echo "fresh release binary bundle failed exact readback" >&2
    return 1
  fi
}

sumeragi_v2_export_source_bound_localnet_binaries() {
  local prebuilt_repo_root="$1"
  local prebuilt_source_manifest_sha256="$2"
  if ! sumeragi_v2_localnet_binary_attestation_valid \
    "$prebuilt_repo_root" "$prebuilt_source_manifest_sha256"; then
    echo "refusing to publish unattested source-bound localnet binaries" >&2
    return 1
  fi
  export TEST_NETWORK_BIN_IROHAD="${IROHA_TEST_TARGET_DIR}/release/iroha3d"
  export TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL="${IROHA_TEST_TARGET_DIR}/message-control/release/iroha3d"
  export TEST_NETWORK_BIN_IROHA="${IROHA_TEST_TARGET_DIR}/release/iroha"
  export KAGAMI_BIN="${IROHA_TEST_TARGET_DIR}/release/kagami"
}
