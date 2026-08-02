#!/usr/bin/env bash
# Execute the source-bound Sumeragi v2 PR or production-release corridor.

set -euo pipefail

profile="${1:---pr}"
if [[ $# -gt 1 ]] || [[ "$profile" != "--pr" && "$profile" != "--release" ]]; then
  echo "usage: $0 [--pr|--release]" >&2
  exit 2
fi

readonly repo_root="$(cd -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
# A production release is launched only by the externally authenticated
# bootstrap.  Validate that capability before changing the bootstrap's closed
# environment or executing any other candidate helper.  The sealed child is
# validated through the same immutable bootstrap evidence propagated by this
# outer process and through its own source-seal checkpoints below.
if [[ "$profile" == "--release" ]]; then
  for bootstrap_suffix in \
    COMPLETION \
    IDENTITY_ATTESTATION \
    IDENTITY_TRANSCRIPT \
    IDENTITY \
    EVIDENCE_DIR; do
    sumeragi_name="SUMERAGI_V2_RELEASE_BOOTSTRAP_${bootstrap_suffix}"
    iroha_name="IROHA_RELEASE_BOOTSTRAP_${bootstrap_suffix}"
    if [[ -z "${!sumeragi_name:-}" || "${!sumeragi_name}" != "${!iroha_name:-}" ]]; then
      echo "production release requires matching bootstrap path aliases for ${bootstrap_suffix}" >&2
      exit 1
    fi
  done
  if [[ -z "${SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256:-}" \
    || "${SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256}" \
      != "${IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256:-}" ]]; then
    echo "production release requires matching out-of-band bootstrap marker digests" >&2
    exit 1
  fi
  if [[ ! "$SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256" \
    =~ ^[0-9a-f]{64}$ ]]; then
    echo "bootstrap completion digest must be one lowercase SHA-256 value" >&2
    exit 1
  fi
  bootstrap_python="${SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR}/python3"
  if [[ ! -f "$bootstrap_python" || ! -x "$bootstrap_python" ]]; then
    echo "production release requires the bootstrap-archived Python" >&2
    exit 1
  fi
  if [[ "${IROHA_RELEASE_SEALED_WORKTREE:-0}" == 1 ]]; then
    if [[ -z "${IROHA_RELEASE_BOOTSTRAP_CANDIDATE_ROOT:-}" \
      || -z "${IROHA_RELEASE_BOOTSTRAP_RUNNER:-}" ]]; then
      echo "sealed release requires the authenticated outer candidate binding" >&2
      exit 1
    fi
    "$bootstrap_python" -I -S \
      "$repo_root/scripts/validate_sumeragi_v2_release_bootstrap.py" \
      --candidate-root "$IROHA_RELEASE_BOOTSTRAP_CANDIDATE_ROOT" \
      --runner "$IROHA_RELEASE_BOOTSTRAP_RUNNER" \
      --profile --release \
      --checkpoint sealed
  else
    "$bootstrap_python" -I -S \
      "$repo_root/scripts/validate_sumeragi_v2_release_bootstrap.py" \
      --candidate-root "$repo_root" \
      --runner "$repo_root/scripts/run_sumeragi_v2_release_gates.sh" \
      --profile --release \
      --checkpoint entry
    export IROHA_RELEASE_BOOTSTRAP_CANDIDATE_ROOT="$repo_root"
    export IROHA_RELEASE_BOOTSTRAP_RUNNER="$repo_root/scripts/run_sumeragi_v2_release_gates.sh"
  fi
  for scaling_digest_name in \
    IROHA_RELEASE_SCALING_CONFIGURATION_SHA256 \
    IROHA_RELEASE_SCALING_IROHAD_SHA256 \
    IROHA_RELEASE_SCALING_IROHA_CLI_SHA256 \
    IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256; do
    if [[ ! "${!scaling_digest_name:-}" =~ ^[0-9a-f]{64}$ ]]; then
      echo "production release requires authenticated lowercase SHA-256 scaling trust anchor ${scaling_digest_name}" >&2
      exit 1
    fi
  done
  if [[ -z "${IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST:-}" ]]; then
    echo "production release requires IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST from the authenticated bootstrap environment" >&2
    exit 1
  fi
fi
readonly inherited_cargo_cache_home="${CARGO_HOME:-${HOME:-}/.cargo}"
cd "$repo_root"
bash ci/check_sumeragi_v2_multilane_release_inventory.sh
# Every real-network leg in this parent shell must fail rather than translate a
# socket/sandbox denial into a successful developer skip.
export IROHA_TEST_REQUIRE_NETWORK=1
unset TEST_NETWORK_BIN_IROHAD KAGAMI_BIN CARGO_BIN_EXE_irohad CARGO_BIN_EXE_kagami
unset TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL TEST_NETWORK_BIN_IROHA CARGO_BIN_EXE_iroha
unset TEST_NETWORK_IROHAD_FEATURES TEST_NETWORK_CARGO
unset IROHA_TEST_SKIP_BUILD IROHA_TEST_ALLOW_REENTRANT_BUILD
unset IROHA_TEST_TARGET_DIR IROHA_RELEASE_PREBUILT_MANIFEST_SHA256
unset IROHA_TEST_BUILD_PROFILE IROHA_TEST_BUILD_TIMEOUT_MS PROFILE
unset TLAPM_BIN TLAPM_STDLIB TLA2TOOLS_JAR
unset JAVA_BIN SUMERAGI_V2_TLC_PROFILE SUMERAGI_TLAPS_THREADS
unset SUMERAGI_V2_FORMAL_EVIDENCE_DIR SUMERAGI_V2_CHAOS_EVIDENCE_DIR
unset SUMERAGI_V2_SEED_MATRIX_EVIDENCE_DIR
unset PYTEST_ADDOPTS PYTHONPATH NODE_OPTIONS
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY
unset GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_COMMON_DIR GIT_NAMESPACE
unset GIT_REPLACE_REF_BASE GIT_CONFIG GIT_CONFIG_GLOBAL GIT_CONFIG_SYSTEM
unset GIT_CONFIG_COUNT GIT_CONFIG_PARAMETERS GIT_CONFIG_NOSYSTEM
unset GIT_GRAFT_FILE GIT_SHALLOW_FILE GIT_EXEC_PATH GIT_TEMPLATE_DIR
unset GIT_EXTERNAL_DIFF GIT_DIFF_OPTS GIT_PAGER GIT_ASKPASS SSH_ASKPASS
unset GIT_SSH GIT_SSH_COMMAND GIT_PROTOCOL_FROM_USER GIT_ATTR_SOURCE
export GIT_NO_REPLACE_OBJECTS=1
export GIT_CONFIG_NOSYSTEM=1
export GIT_CONFIG_GLOBAL=/dev/null
unset RUSTC RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER RUSTFLAGS RUSTDOCFLAGS
unset CARGO_ENCODED_RUSTFLAGS CARGO_BUILD_TARGET CARGO_BUILD_RUSTC
unset CARGO_BUILD_RUSTC_WRAPPER CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER
unset CARGO_BUILD_RUSTFLAGS CARGO_BUILD_RUSTDOCFLAGS
unset CARGO CARGO_HOME CARGO_NET_OFFLINE
unset RUSTUP_TOOLCHAIN RUSTC_BOOTSTRAP RUST_TEST_NOCAPTURE CARGO_INCREMENTAL
unset CLICOLOR CLICOLOR_FORCE
export CARGO_INCREMENTAL=0
export CARGO_TERM_COLOR=never
export NO_COLOR=1
while IFS='=' read -r inherited_name _; do
  case "$inherited_name" in
    CARGO_BUILD_*|CARGO_TARGET_*|CARGO_PROFILE_*|RUST_TEST_*|GIT_CONFIG_KEY_*|GIT_CONFIG_VALUE_*)
      unset "$inherited_name"
      ;;
  esac
done < <(env)
export GIT_CONFIG_COUNT=2
export GIT_CONFIG_KEY_0=core.hooksPath
export GIT_CONFIG_VALUE_0=/dev/null
export GIT_CONFIG_KEY_1=core.fsmonitor
export GIT_CONFIG_VALUE_1=false

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

wait_for_external_cargo() {
  local active_compilers
  while true; do
    # This exact snapshot is a release requirement and is intentionally kept
    # separate from the machine-readable filter below.
    ps -axo pid,etime,command
    active_compilers="$(
      ps -axo pid=,command= | awk '
        {
          executable = $2
          sub(/^.*\//, "", executable)
          if (executable == "cargo" || executable == "rustc") {
            print
          }
        }
      '
    )"
    if [[ -z "$active_compilers" ]]; then
      return
    fi
    printf '%s\n' \
      "waiting for active Cargo/rustc processes before release command:" \
      "$active_compilers" >&2
    sleep 10
  done
}

run_cargo() {
  wait_for_external_cargo
  command cargo "$@"
}

source "${repo_root}/scripts/sumeragi_v2_prebuilt_bundle.sh"

localnet_binary_attestation_valid() {
  sumeragi_v2_localnet_binary_attestation_valid \
    "$repo_root" "$IROHA_RELEASE_SOURCE_MANIFEST_SHA256"
}

ensure_source_bound_localnet_binaries() {
  sumeragi_v2_ensure_source_bound_localnet_binaries \
    "$repo_root" "$IROHA_RELEASE_SOURCE_MANIFEST_SHA256"
}

export_source_bound_localnet_binaries() {
  sumeragi_v2_export_source_bound_localnet_binaries \
    "$repo_root" "$IROHA_RELEASE_SOURCE_MANIFEST_SHA256"
}

canonical_executable() {
  local executable
  executable="$(command -v "$1")" || return 1
  python3 -I -S -c \
    'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve(strict=True))' \
    "$executable"
}

canonical_git_executable() {
  local executable
  executable="$(canonical_executable git)" || return 1
  # `/usr/bin/git` on macOS is an xcode-select launcher. Archiving that shim
  # would leave the selected developer-tool Git binary outside the protected
  # digest and evidence directory. Resolve the implementation that the shim
  # would execute; the caller-supplied SHA-256 policy below then authenticates
  # those exact bytes before they are used.
  if [[ "$(uname -s)" == Darwin && "$executable" == /usr/bin/git ]]; then
    executable="$(/usr/bin/xcrun --find git)" || return 1
    executable="$(canonical_path "$executable")" || return 1
  fi
  printf '%s\n' "$executable"
}

canonical_path() {
  python3 -I -S -c \
    'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve(strict=True))' \
    "$1"
}

release_identity_json() {
  python3 -I -S scripts/compute_workspace_source_manifest.py \
    --root "$repo_root" \
    --release-identity-json
}

identity_field() {
  local identity_path="$1"
  local field="$2"
  python3 -I -S -c \
    'import json, sys; print(json.load(open(sys.argv[1], encoding="utf-8"))[sys.argv[2]])' \
    "$identity_path" "$field"
}

# Production evidence never executes in the caller's mutable checkout. Require
# one committed source tree, reproduce it in a unique detached worktree, copy
# the separately-bound ignored Cargo.lock, and remove source write permission.
# The complete corridor then re-enters this script from that sealed source.
if [[ "$profile" == "--release" && "${IROHA_RELEASE_SEALED_WORKTREE:-0}" != 1 ]]; then
  release_bootstrap_evidence_dir="$(
    canonical_path "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR"
  )" || {
    echo "the authenticated bootstrap evidence directory is unavailable" >&2
    exit 1
  }
  if [[ "$release_bootstrap_evidence_dir" \
    != "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR" ]]; then
    echo "the authenticated bootstrap evidence directory is not canonical" >&2
    exit 1
  fi
  readonly release_bootstrap_evidence_dir
  release_scaling_evidence_manifest="$(
    canonical_path "$IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST"
  )" || {
    echo "the configured G-SCALE evidence manifest is unavailable" >&2
    exit 1
  }
  if [[ "$release_scaling_evidence_manifest" \
      != "$IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST" \
    || "${release_scaling_evidence_manifest##*/}" != scaling_evidence.json \
    || ! -f "$release_scaling_evidence_manifest" \
    || -L "$release_scaling_evidence_manifest" ]]; then
    echo "G-SCALE evidence must be an absolute canonical regular non-symlink scaling_evidence.json" >&2
    exit 1
  fi
  readonly release_scaling_evidence_manifest
  release_git_bin="$(canonical_git_executable)" || {
    echo "a resolved Git executable is required for the release corridor" >&2
    exit 1
  }
  if [[ -z "${SUMERAGI_V2_RELEASE_SSH_KEYGEN_BIN:-}" ]]; then
    echo "production release requires a relocatable tool path in SUMERAGI_V2_RELEASE_SSH_KEYGEN_BIN" >&2
    exit 1
  fi
  release_ssh_keygen_bin="$(canonical_path "$SUMERAGI_V2_RELEASE_SSH_KEYGEN_BIN")" || {
    echo "the configured release ssh-keygen executable could not be resolved" >&2
    exit 1
  }
  if [[ ! -f "$release_ssh_keygen_bin" || ! -x "$release_ssh_keygen_bin" ]]; then
    echo "the configured release ssh-keygen path is not an executable regular file" >&2
    exit 1
  fi
  release_python_bin="$(canonical_executable python3)" || {
    echo "a resolved Python executable is required for the release corridor" >&2
    exit 1
  }
  release_bash_bin="$(canonical_executable bash)" || {
    echo "a resolved Bash executable is required for the release corridor" >&2
    exit 1
  }
  readonly release_git_bin release_ssh_keygen_bin release_python_bin release_bash_bin
  if [[ "$release_git_bin" != "$release_bootstrap_evidence_dir/git" \
    || "$release_ssh_keygen_bin" != "$release_bootstrap_evidence_dir/ssh-keygen" \
    || "$release_python_bin" != "$release_bootstrap_evidence_dir/python3" \
    || "$release_bash_bin" != "$release_bootstrap_evidence_dir/bash" ]]; then
    echo "release executables escaped the authenticated bootstrap archive" >&2
    exit 1
  fi
  for release_policy_name in \
    SUMERAGI_V2_RELEASE_EXPECTED_GIT_SHA256 \
    SUMERAGI_V2_RELEASE_EXPECTED_SSH_KEYGEN_SHA256 \
    SUMERAGI_V2_RELEASE_EXPECTED_SIGNER_FINGERPRINT \
    SUMERAGI_V2_RELEASE_SSH_ALLOWED_SIGNERS \
    SUMERAGI_V2_RELEASE_EXPECTED_SSH_ALLOWED_SIGNERS_SHA256 \
    SUMERAGI_V2_RELEASE_SSH_REVOCATION_FILE \
    SUMERAGI_V2_RELEASE_EXPECTED_SSH_REVOCATION_SHA256; do
    if [[ -z "${!release_policy_name:-}" ]]; then
      echo "production release requires protected policy input ${release_policy_name}" >&2
      exit 1
    fi
  done
  for release_digest_name in \
    SUMERAGI_V2_RELEASE_EXPECTED_GIT_SHA256 \
    SUMERAGI_V2_RELEASE_EXPECTED_SSH_KEYGEN_SHA256 \
    SUMERAGI_V2_RELEASE_EXPECTED_SSH_ALLOWED_SIGNERS_SHA256 \
    SUMERAGI_V2_RELEASE_EXPECTED_SSH_REVOCATION_SHA256; do
    if [[ ! "${!release_digest_name}" =~ ^[0-9a-f]{64}$ ]]; then
      echo "protected release digest ${release_digest_name} must be lowercase SHA-256" >&2
      exit 1
    fi
  done
  release_ssh_allowed_signers="$(
    canonical_path "$SUMERAGI_V2_RELEASE_SSH_ALLOWED_SIGNERS"
  )" || {
    echo "the configured release allowed-signers policy could not be resolved" >&2
    exit 1
  }
  release_ssh_revocation_file="$(
    canonical_path "$SUMERAGI_V2_RELEASE_SSH_REVOCATION_FILE"
  )" || {
    echo "the configured release SSH revocation policy could not be resolved" >&2
    exit 1
  }
  if [[ ! -f "$release_ssh_allowed_signers" || ! -f "$release_ssh_revocation_file" ]]; then
    echo "release SSH policies must resolve to regular files" >&2
    exit 1
  fi
  if [[ "$release_ssh_allowed_signers" \
      != "$release_bootstrap_evidence_dir/bootstrap-allowed-signers" \
    || "$release_ssh_revocation_file" \
      != "$release_bootstrap_evidence_dir/bootstrap-revocation" ]]; then
    echo "release SSH policies escaped the authenticated bootstrap archive" >&2
    exit 1
  fi
  readonly release_ssh_allowed_signers release_ssh_revocation_file
  export PATH="${release_bootstrap_evidence_dir}:${PATH}"
  candidate_identity_path="$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY"
  if [[ "$candidate_identity_path" != "$release_bootstrap_evidence_dir/candidate-identity.json" ]]; then
    echo "bootstrap candidate identity path changed after authentication" >&2
    exit 1
  fi
  candidate_identity_json="$(<"$candidate_identity_path")"
  candidate_manifest_sha256="$(
    python3 -I -S -c 'import json, sys; print(json.loads(sys.argv[1])["workspace_source_manifest_sha256"])' \
      "$candidate_identity_json"
  )"
  candidate_head="$(
    python3 -I -S -c 'import json, sys; print(json.loads(sys.argv[1])["head_commit"])' \
      "$candidate_identity_json"
  )"
  candidate_lock_mode="$(
    python3 -I -S -c \
      'from pathlib import Path; import stat, sys; print(format(stat.S_IMODE(Path(sys.argv[1]).lstat().st_mode), "04o"))' \
      "$repo_root/Cargo.lock"
  )"
  if [[ ! "$candidate_lock_mode" =~ ^0[0-7]{3}$ ]]; then
    echo "candidate Cargo.lock mode is not canonical octal" >&2
    exit 1
  fi
  if [[ ! "$candidate_manifest_sha256" =~ ^[0-9a-f]{64}$ \
    || ! "$candidate_head" =~ ^([0-9a-f]{40}|[0-9a-f]{64})$ ]]; then
    echo "bootstrap candidate identity contains a malformed source digest or commit OID" >&2
    exit 1
  fi
  readonly candidate_identity_path candidate_identity_json candidate_manifest_sha256
  readonly candidate_head candidate_lock_mode

  release_rustup_bin="$(canonical_executable rustup)" || {
    echo "rustup is required to resolve the repository-pinned release toolchain" >&2
    exit 1
  }
  release_cargo_bin="$(canonical_path "$("$release_rustup_bin" which cargo)")"
  release_rustc_bin="$(canonical_path "$("$release_rustup_bin" which rustc)")"
  release_node_bin="$(canonical_executable node)" || {
    echo "a resolved Node executable is required for the release corridor" >&2
    exit 1
  }
  readonly release_rustup_bin release_cargo_bin release_rustc_bin release_node_bin
  wait_for_external_cargo
  if [[ ! -x "$release_cargo_bin" \
    || "$("$release_cargo_bin" --version)" != "cargo 1.93.1 (083ac5135 2025-12-15)" \
    || ! -x "$release_rustc_bin" \
    || "$("$release_rustc_bin" --version)" != "rustc 1.93.1 (01f6ddf75 2026-02-11)" ]]; then
    echo "rustup did not resolve the exact rust-toolchain.toml Cargo/rustc release pair" >&2
    exit 1
  fi

  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64)
      release_tlapm_platform="arm64-darwin"
      release_verus_platform="arm64-macos"
      release_verus_sha256="f11f8a863103a3c8fcaf27e6189edfdba31081516591365b5e29b0a66f570451"
      release_cargo_verus_sha256="f918c6229c8d714640c9c9ec3d60b9c1d2e0aafc09bba8ff037332b04f85d078"
      ;;
    Linux-x86_64)
      release_tlapm_platform="x86_64-linux-gnu"
      release_verus_platform="x86-linux"
      release_verus_sha256="c5911ee43c7a92c49a48d2c8646c604d252a38c71c87bda88ad4d33eb9e7e0fc"
      release_cargo_verus_sha256="42a79c9afd700f8312a9ac7ab212070723e71beeb07f5ab855453010455bdc6d"
      ;;
    *)
      echo "unsupported Sumeragi v2 validator release host: $(uname -s)-$(uname -m); admitted hosts are Linux-x86_64 and Darwin-arm64" >&2
      exit 1
      ;;
  esac
  readonly release_tlapm_platform release_verus_platform
  readonly release_verus_sha256 release_cargo_verus_sha256
  readonly release_tlapm_root="${repo_root}/target/tlapm/toolchains/3ab43c7ff31db4ced850619d4746fa4c841a7681/${release_tlapm_platform}"
  readonly release_tlapm_bin="${release_tlapm_root}/tlapm/bin/tlapm"
  readonly release_tlapm_stdlib="${release_tlapm_root}/tlapm/lib/tlapm/stdlib"
  readonly release_tla2tools_jar="${repo_root}/target/tla2tools/1.7.4/tla2tools.jar"
  readonly release_verus_dir="${repo_root}/target/verus/toolchains/0.2026.05.31.5dd6d83/${release_verus_platform}"
  readonly release_verus_bin="${release_verus_dir}/verus"
  readonly release_cargo_verus_bin="${release_verus_dir}/cargo-verus"
  release_java_bin="$("$repo_root/scripts/formal/resolve_java.sh")" || {
    echo "a resolved Java runtime is required for the release corridor" >&2
    exit 1
  }
  readonly release_java_bin
  if [[ ! -x "$release_tlapm_bin" \
    || "$($release_tlapm_bin --version 2>&1)" != "3ab43c7" \
    || ! -f "$release_tlapm_stdlib/Functions.tla" \
    || "$(sha256_file "$release_tlapm_stdlib/Functions.tla")" != "b54ff63b7c76c327525c17c188d5f9f5e53d92f3fd701f5e2ba54f0f54391063" \
    || ! -f "$release_tlapm_stdlib/Folds.tla" \
    || "$(sha256_file "$release_tlapm_stdlib/Folds.tla")" != "aa59063fd600bb640b2ae24dc85ef770277ef5bf7955092b76b8b471790086da" ]]; then
    echo "the caller target lacks the checksum-validated pinned TLAPM toolchain" >&2
    exit 1
  fi
  if [[ ! -f "$release_tla2tools_jar" \
    || "$(sha256_file "$release_tla2tools_jar")" != "936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88" ]]; then
    echo "the caller target lacks pinned TLA2Tools v1.7.4" >&2
    exit 1
  fi
  if [[ ! -x "$release_verus_bin" || ! -x "$release_cargo_verus_bin" \
    || "$(sha256_file "$release_verus_bin")" != "$release_verus_sha256" \
    || "$(sha256_file "$release_cargo_verus_bin")" != "$release_cargo_verus_sha256" ]] \
    || ! "$release_verus_bin" --version 2>&1 | grep -Fq '0.2026.05.31.5dd6d83'; then
    echo "the caller target lacks the checksum-validated pinned Verus toolchain" >&2
    exit 1
  fi
  "$release_java_bin" -version >/dev/null 2>&1 || {
    echo "the resolved Java runtime is not executable" >&2
    exit 1
  }

  # The bootstrap evidence parent is already a private, owner-bound 0700
  # directory.  Keep all mutable release state below it and refuse reuse; this
  # removes the predictable shared-/tmp ancestor from the authentication
  # boundary.
  release_invocation_root="${release_bootstrap_evidence_dir}/release-runner"
  if [[ -e "$release_invocation_root" || -L "$release_invocation_root" ]]; then
    echo "authenticated release invocation root already exists" >&2
    exit 1
  fi
  mkdir -m 0700 -- "$release_invocation_root"
  release_invocation_root="$(canonical_path "$release_invocation_root")" || {
    echo "the authenticated release invocation root could not be resolved" >&2
    exit 1
  }
  readonly release_invocation_root
  readonly sealed_repo_root="${release_invocation_root}/source"
  readonly release_host_root="${release_invocation_root}/output"
  readonly sealed_identity_path="${release_invocation_root}/sealed-identity.json"
  readonly aggregate_receipt_path="${release_host_root}/release/RELEASE_COMPLETED.json"
  readonly release_signature_evidence_dir="$release_bootstrap_evidence_dir"
  readonly release_signature_attestation="${release_signature_evidence_dir}/identity-attestation.json"
  readonly release_signature_transcript="${release_signature_evidence_dir}/identity-transcript.json"
  readonly release_signature_raw_commit="${release_signature_evidence_dir}/identity-raw-commit"
  readonly release_signature_cargo_lock="${release_signature_evidence_dir}/identity-Cargo.lock"
  readonly release_signature_allowed_signers="${release_signature_evidence_dir}/identity-allowed-signers"
  readonly release_signature_revocation="${release_signature_evidence_dir}/identity-revocation"
  readonly release_verified_git_bin="${release_signature_evidence_dir}/identity-git"
  readonly release_verified_ssh_keygen_bin="${release_signature_evidence_dir}/identity-ssh-keygen"
  mkdir -m 0700 -- "$release_host_root"
  mkdir -m 0700 -- \
    "$release_host_root/workspace-target" \
    "$release_host_root/tmp" \
    "$release_host_root/cache" \
    "$release_host_root/cargo-home" \
    "$release_host_root/release"
  for cargo_cache in registry git; do
    if [[ -d "${inherited_cargo_cache_home}/${cargo_cache}" ]]; then
      ln -s "$(canonical_path "${inherited_cargo_cache_home}/${cargo_cache}")" \
        "$release_host_root/cargo-home/${cargo_cache}"
    fi
  done
  export PATH="${release_signature_evidence_dir}:${PATH}"

  sealed_worktree_added=0
  sealed_worktree_retained=0
  cleanup_sealed_worktree() {
    local status=$?
    if ((sealed_worktree_added && ! sealed_worktree_retained)); then
      if [[ -d "$sealed_repo_root" ]]; then
        python3 -I -S "$repo_root/scripts/seal_workspace_source.py" \
          --unseal --root "$sealed_repo_root" >/dev/null 2>&1 || status=1
      fi
      "$release_verified_git_bin" -c core.hooksPath=/dev/null \
        -C "$repo_root" worktree remove --force "$sealed_repo_root" \
        >/dev/null 2>&1 || status=1
    fi
    trap - EXIT
    exit "$status"
  }
  trap cleanup_sealed_worktree EXIT

  "$release_verified_git_bin" -c core.hooksPath=/dev/null \
    worktree add --detach "$sealed_repo_root" "$candidate_head"
  sealed_worktree_added=1
  cp -- "$release_signature_cargo_lock" "$sealed_repo_root/Cargo.lock"
  chmod "$candidate_lock_mode" "$sealed_repo_root/Cargo.lock"
  reproduced_identity_json="$(
    python3 -I -S "$sealed_repo_root/scripts/compute_workspace_source_manifest.py" \
      --root "$sealed_repo_root" \
      --release-identity-json
  )"
  if [[ "$reproduced_identity_json" != "$candidate_identity_json" ]]; then
    echo "detached release worktree does not reproduce the candidate identity" >&2
    exit 1
  fi

  "$release_python_bin" - "$sealed_repo_root" <<'PY'
from pathlib import Path
import sys

root = Path(sys.argv[1]).resolve(strict=True)
for parent in root.parents:
    for name in ("config", "config.toml"):
        candidate = parent / ".cargo" / name
        if candidate.exists() or candidate.is_symlink():
            raise SystemExit(
                f"external Cargo config is forbidden above sealed source: {candidate}"
            )
PY

  ln -s "$release_host_root/workspace-target" "$sealed_repo_root/target"
  python3 -I -S "$sealed_repo_root/scripts/seal_workspace_source.py" \
    --seal --root "$sealed_repo_root" --writable target
  python3 -I -S "$sealed_repo_root/scripts/seal_workspace_source.py" \
    --verify --root "$sealed_repo_root" --writable target
  # Keep both digests. The candidate manifest records the original checkout;
  # the sealed manifest may change because the manifest intentionally binds all
  # permission bits. Every child build/evidence item uses the sealed digest,
  # while the final receipt also binds candidate HEAD/tree/Cargo.lock.
  sealed_identity_json="$(
    python3 -I -S "$sealed_repo_root/scripts/compute_workspace_source_manifest.py" \
      --root "$sealed_repo_root" \
      --release-identity-json
  )"
  printf '%s\n' "$sealed_identity_json" >"$sealed_identity_path"
  chmod 0400 "$sealed_identity_path"

  set +e
  IROHA_RELEASE_SEALED_WORKTREE=1 \
    IROHA_RELEASE_SEALED_ROOT="$sealed_repo_root" \
    IROHA_RELEASE_HOST_ROOT="$release_host_root" \
    IROHA_RELEASE_WORKSPACE_TARGET="$release_host_root/workspace-target" \
    IROHA_RELEASE_INVOCATION_ROOT="$release_invocation_root" \
    IROHA_RELEASE_CANDIDATE_IDENTITY_PATH="$candidate_identity_path" \
    IROHA_RELEASE_EXPECTED_IDENTITY_PATH="$sealed_identity_path" \
    IROHA_RELEASE_AGGREGATE_RECEIPT_PATH="$aggregate_receipt_path" \
    IROHA_RELEASE_SCALING_CONFIGURATION_SHA256="$IROHA_RELEASE_SCALING_CONFIGURATION_SHA256" \
    IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST="$release_scaling_evidence_manifest" \
    IROHA_RELEASE_SCALING_IROHAD_SHA256="$IROHA_RELEASE_SCALING_IROHAD_SHA256" \
    IROHA_RELEASE_SCALING_IROHA_CLI_SHA256="$IROHA_RELEASE_SCALING_IROHA_CLI_SHA256" \
    IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256="$IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256" \
    IROHA_RELEASE_BOOTSTRAP_CANDIDATE_ROOT="$repo_root" \
    IROHA_RELEASE_BOOTSTRAP_RUNNER="$repo_root/scripts/run_sumeragi_v2_release_gates.sh" \
    IROHA_RELEASE_TLAPM_BIN="$release_tlapm_bin" \
    IROHA_RELEASE_TLAPM_STDLIB="$release_tlapm_stdlib" \
    IROHA_RELEASE_TLA2TOOLS_JAR="$release_tla2tools_jar" \
    IROHA_RELEASE_VERUS_DIR="$release_verus_dir" \
    IROHA_RELEASE_JAVA_BIN="$release_java_bin" \
    IROHA_RELEASE_GIT_BIN="$release_verified_git_bin" \
    IROHA_RELEASE_SSH_KEYGEN_BIN="$release_verified_ssh_keygen_bin" \
    IROHA_RELEASE_SIGNATURE_ATTESTATION="$release_signature_attestation" \
    IROHA_RELEASE_SIGNATURE_TRANSCRIPT="$release_signature_transcript" \
    IROHA_RELEASE_SIGNATURE_RAW_COMMIT="$release_signature_raw_commit" \
    IROHA_RELEASE_SIGNATURE_CARGO_LOCK="$release_signature_cargo_lock" \
    IROHA_RELEASE_SIGNATURE_ALLOWED_SIGNERS="$release_signature_allowed_signers" \
    IROHA_RELEASE_SIGNATURE_REVOCATION="$release_signature_revocation" \
    IROHA_RELEASE_EXPECTED_GIT_SHA256="$SUMERAGI_V2_RELEASE_EXPECTED_GIT_SHA256" \
    IROHA_RELEASE_EXPECTED_SSH_KEYGEN_SHA256="$SUMERAGI_V2_RELEASE_EXPECTED_SSH_KEYGEN_SHA256" \
    IROHA_RELEASE_EXPECTED_SIGNER_FINGERPRINT="$SUMERAGI_V2_RELEASE_EXPECTED_SIGNER_FINGERPRINT" \
    IROHA_RELEASE_EXPECTED_ALLOWED_SIGNERS_SHA256="$SUMERAGI_V2_RELEASE_EXPECTED_SSH_ALLOWED_SIGNERS_SHA256" \
    IROHA_RELEASE_EXPECTED_REVOCATION_SHA256="$SUMERAGI_V2_RELEASE_EXPECTED_SSH_REVOCATION_SHA256" \
    IROHA_RELEASE_CARGO_BIN="$release_cargo_bin" \
    IROHA_RELEASE_RUSTC_BIN="$release_rustc_bin" \
    IROHA_RELEASE_PYTHON_BIN="$release_python_bin" \
    IROHA_RELEASE_NODE_BIN="$release_node_bin" \
    IROHA_RELEASE_BASH_BIN="$release_bash_bin" \
    IROHA_RELEASE_CARGO_HOME="$release_host_root/cargo-home" \
    CARGO_INCREMENTAL=0 \
    TMPDIR="$release_host_root/tmp" \
    XDG_CACHE_HOME="$release_host_root/cache" \
    "$release_bash_bin" "$sealed_repo_root/scripts/run_sumeragi_v2_release_gates.sh" --release
  sealed_status=$?
  set -e
  if ((sealed_status == 0)); then
    if [[ ! -s "$aggregate_receipt_path" || -L "$aggregate_receipt_path" ]]; then
      echo "sealed release corridor passed without an aggregate receipt" >&2
      sealed_status=1
    else
      # The authenticated bootstrap must revalidate this exact sealed tree
      # after the child exits. Retain the read-only worktree and its identity
      # as durable release evidence; failed runs are still cleaned up.
      sealed_worktree_retained=1
      echo "aggregate release receipt: ${aggregate_receipt_path}" >&2
    fi
  fi
  exit "$sealed_status"
fi

verify_release_identity() {
  local checkpoint="$1"
  [[ "$profile" == "--release" ]] || return 0
  if [[ "${IROHA_RELEASE_SEALED_WORKTREE:-0}" != 1 \
    || "${IROHA_RELEASE_SEALED_ROOT:-}" != "$repo_root" \
    || -z "${IROHA_RELEASE_HOST_ROOT:-}" \
    || -z "${IROHA_RELEASE_WORKSPACE_TARGET:-}" \
    || -z "${IROHA_RELEASE_EXPECTED_IDENTITY_PATH:-}" \
    || -z "${IROHA_RELEASE_CANDIDATE_IDENTITY_PATH:-}" \
    || -z "${IROHA_RELEASE_AGGREGATE_RECEIPT_PATH:-}" \
    || -z "${IROHA_RELEASE_SCALING_CONFIGURATION_SHA256:-}" \
    || -z "${IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST:-}" \
    || -z "${IROHA_RELEASE_SCALING_IROHAD_SHA256:-}" \
    || -z "${IROHA_RELEASE_SCALING_IROHA_CLI_SHA256:-}" \
    || -z "${IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256:-}" \
    || -z "${IROHA_RELEASE_BOOTSTRAP_CANDIDATE_ROOT:-}" \
    || -z "${IROHA_RELEASE_BOOTSTRAP_RUNNER:-}" \
    || -z "${IROHA_RELEASE_PYTHON_BIN:-}" \
    || -z "${SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION:-}" \
    || -z "${SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION:-}" \
    || -z "${SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_TRANSCRIPT:-}" \
    || -z "${SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY:-}" \
    || -z "${SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR:-}" \
    || -z "${SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256:-}" \
    || "$IROHA_RELEASE_AGGREGATE_RECEIPT_PATH" \
      != "$IROHA_RELEASE_HOST_ROOT/release/RELEASE_COMPLETED.json" \
    || -z "${IROHA_RELEASE_SIGNATURE_ATTESTATION:-}" \
    || -z "${IROHA_RELEASE_SIGNATURE_TRANSCRIPT:-}" \
    || -z "${IROHA_RELEASE_SIGNATURE_RAW_COMMIT:-}" \
    || -z "${IROHA_RELEASE_SIGNATURE_CARGO_LOCK:-}" \
    || -z "${IROHA_RELEASE_SIGNATURE_ALLOWED_SIGNERS:-}" \
    || -z "${IROHA_RELEASE_SIGNATURE_REVOCATION:-}" \
    || -z "${IROHA_RELEASE_EXPECTED_GIT_SHA256:-}" \
    || -z "${IROHA_RELEASE_EXPECTED_SSH_KEYGEN_SHA256:-}" \
    || -z "${IROHA_RELEASE_EXPECTED_SIGNER_FINGERPRINT:-}" \
    || -z "${IROHA_RELEASE_EXPECTED_ALLOWED_SIGNERS_SHA256:-}" \
    || -z "${IROHA_RELEASE_EXPECTED_REVOCATION_SHA256:-}" \
    || ! -f "$IROHA_RELEASE_EXPECTED_IDENTITY_PATH" \
    || ! -f "$IROHA_RELEASE_CANDIDATE_IDENTITY_PATH" \
    || ! -f "$IROHA_RELEASE_SIGNATURE_ATTESTATION" \
    || ! -f "$IROHA_RELEASE_SIGNATURE_TRANSCRIPT" \
    || ! -f "$IROHA_RELEASE_SIGNATURE_RAW_COMMIT" \
    || ! -f "$IROHA_RELEASE_SIGNATURE_CARGO_LOCK" \
    || ! -f "$IROHA_RELEASE_SIGNATURE_ALLOWED_SIGNERS" \
    || ! -f "$IROHA_RELEASE_SIGNATURE_REVOCATION" \
    || ! -f "$IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST" \
    || -L "$IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST" ]]; then
    echo "production release corridor must run from its signed, sealed detached worktree" >&2
    return 1
  fi
  local signature_digest_name
  for signature_digest_name in \
    IROHA_RELEASE_EXPECTED_GIT_SHA256 \
    IROHA_RELEASE_EXPECTED_SSH_KEYGEN_SHA256 \
    IROHA_RELEASE_EXPECTED_ALLOWED_SIGNERS_SHA256 \
    IROHA_RELEASE_EXPECTED_REVOCATION_SHA256; do
    if [[ ! "${!signature_digest_name}" =~ ^[0-9a-f]{64}$ ]]; then
      echo "signed release trust anchor ${signature_digest_name} changed at ${checkpoint}" >&2
      return 1
    fi
  done
  local scaling_digest_name
  for scaling_digest_name in \
    IROHA_RELEASE_SCALING_CONFIGURATION_SHA256 \
    IROHA_RELEASE_SCALING_IROHAD_SHA256 \
    IROHA_RELEASE_SCALING_IROHA_CLI_SHA256 \
    IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256; do
    if [[ ! "${!scaling_digest_name}" =~ ^[0-9a-f]{64}$ ]]; then
      echo "scaling trust anchor ${scaling_digest_name} changed at ${checkpoint}" >&2
      return 1
    fi
  done
  if [[ ! "$IROHA_RELEASE_EXPECTED_SIGNER_FINGERPRINT" =~ ^SHA256:[A-Za-z0-9+/]{43}$ ]]; then
    echo "signed release fingerprint trust anchor changed at ${checkpoint}" >&2
    return 1
  fi
  local observed_scaling_manifest
  if ! observed_scaling_manifest="$(
    canonical_path "$IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST"
  )" \
    || [[ "$observed_scaling_manifest" \
        != "$IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST" \
      || "${observed_scaling_manifest##*/}" != scaling_evidence.json ]]; then
    echo "G-SCALE evidence path changed or became noncanonical at ${checkpoint}" >&2
    return 1
  fi
  local observed_target expected_target
  if [[ ! -L "$repo_root/target" ]]; then
    echo "sealed release target is not the external output symlink at ${checkpoint}" >&2
    return 1
  fi
  observed_target="$(
    python3 -I -S -c 'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve(strict=True))' \
      "$repo_root/target"
  )"
  expected_target="$(
    python3 -I -S -c 'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve(strict=True))' \
      "$IROHA_RELEASE_WORKSPACE_TARGET"
  )"
  if [[ "$observed_target" != "$expected_target" ]]; then
    echo "sealed release target changed at ${checkpoint}" >&2
    return 1
  fi
  local expected observed
  expected="$(<"$IROHA_RELEASE_EXPECTED_IDENTITY_PATH")"
  if ! observed="$(release_identity_json)"; then
    echo "failed to compute release identity at ${checkpoint}" >&2
    return 1
  fi
  if [[ "$observed" != "$expected" ]]; then
    echo "release source identity changed at ${checkpoint}" >&2
    return 1
  fi
  if ! python3 -I -S scripts/seal_workspace_source.py \
    --verify --root "$repo_root" --writable target; then
    echo "release source seal changed at ${checkpoint}" >&2
    return 1
  fi
  if ! "$IROHA_RELEASE_PYTHON_BIN" -I -S \
    "$repo_root/scripts/validate_sumeragi_v2_release_bootstrap.py" \
    --candidate-root "$IROHA_RELEASE_BOOTSTRAP_CANDIDATE_ROOT" \
    --runner "$IROHA_RELEASE_BOOTSTRAP_RUNNER" \
    --profile --release \
    --checkpoint sealed; then
    echo "authenticated bootstrap evidence changed at ${checkpoint}" >&2
    return 1
  fi
}

verify_release_identity "release corridor entry"
if [[ "$profile" == "--release" ]]; then
  readonly IROHA_RELEASE_SCALING_CONFIGURATION_SHA256
  readonly IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST
  readonly IROHA_RELEASE_SCALING_IROHAD_SHA256
  readonly IROHA_RELEASE_SCALING_IROHA_CLI_SHA256
  readonly IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256
fi

# Both profiles are release evidence. Bind the complete corridor, including the
# PR matrix, to one checkout manifest and one content-addressed build root.
release_source_manifest_sha256="$(
  python3 -I -S scripts/compute_workspace_source_manifest.py --root "$repo_root"
)"
if [[ ! "$release_source_manifest_sha256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "workspace source manifest helper returned an invalid digest" >&2
  exit 1
fi
readonly release_source_manifest_sha256
release_head_commit=""
release_head_tree=""
release_cargo_lock_sha256=""
candidate_source_manifest_sha256="$release_source_manifest_sha256"
if [[ "$profile" == "--release" ]]; then
  release_head_commit="$(identity_field "$IROHA_RELEASE_EXPECTED_IDENTITY_PATH" head_commit)"
  release_head_tree="$(identity_field "$IROHA_RELEASE_EXPECTED_IDENTITY_PATH" head_tree)"
  release_cargo_lock_sha256="$(
    identity_field "$IROHA_RELEASE_EXPECTED_IDENTITY_PATH" cargo_lock_sha256
  )"
  expected_manifest_sha256="$(
    identity_field "$IROHA_RELEASE_EXPECTED_IDENTITY_PATH" workspace_source_manifest_sha256
  )"
  candidate_source_manifest_sha256="$(
    identity_field "$IROHA_RELEASE_CANDIDATE_IDENTITY_PATH" workspace_source_manifest_sha256
  )"
  if [[ "$expected_manifest_sha256" != "$release_source_manifest_sha256" ]]; then
    echo "sealed release identity disagrees with the workspace source manifest" >&2
    exit 1
  fi
fi
readonly release_head_commit release_head_tree release_cargo_lock_sha256
readonly candidate_source_manifest_sha256
export IROHA_RELEASE_HEAD_COMMIT="$release_head_commit"
export IROHA_RELEASE_HEAD_TREE="$release_head_tree"
export IROHA_RELEASE_CARGO_LOCK_SHA256="$release_cargo_lock_sha256"
export IROHA_RELEASE_CANDIDATE_SOURCE_MANIFEST_SHA256="$candidate_source_manifest_sha256"
readonly release_source_bound_root="${repo_root}/target/sumeragi-v2-release/${release_source_manifest_sha256}"
export IROHA_RELEASE_SOURCE_MANIFEST_SHA256="$release_source_manifest_sha256"
export CARGO_TARGET_DIR="${release_source_bound_root}/test-suite"
export IROHA_TEST_SKIP_BUILD=1
export IROHA_TEST_ALLOW_REENTRANT_BUILD=0
export IROHA_TEST_BUILD_TIMEOUT_MS=3600
export IROHA_TEST_BUILD_PROFILE=release
export PROFILE=release
export CARGO_NET_OFFLINE=true
if [[ "$profile" == "--release" ]]; then
  for required_tool_path in \
    IROHA_RELEASE_TLAPM_BIN \
    IROHA_RELEASE_TLAPM_STDLIB \
    IROHA_RELEASE_TLA2TOOLS_JAR \
    IROHA_RELEASE_VERUS_DIR \
    IROHA_RELEASE_JAVA_BIN \
    IROHA_RELEASE_GIT_BIN \
    IROHA_RELEASE_SSH_KEYGEN_BIN \
    IROHA_RELEASE_CARGO_BIN \
    IROHA_RELEASE_RUSTC_BIN \
    IROHA_RELEASE_PYTHON_BIN \
    IROHA_RELEASE_NODE_BIN \
    IROHA_RELEASE_BASH_BIN \
    IROHA_RELEASE_CARGO_HOME; do
    if [[ -z "${!required_tool_path:-}" ]]; then
      echo "sealed release is missing pinned tool path ${required_tool_path}" >&2
      exit 1
    fi
  done
  export TLAPM_BIN="$IROHA_RELEASE_TLAPM_BIN"
  export TLAPM_STDLIB="$IROHA_RELEASE_TLAPM_STDLIB"
  export TLA2TOOLS_JAR="$IROHA_RELEASE_TLA2TOOLS_JAR"
  export JAVA_BIN="$IROHA_RELEASE_JAVA_BIN"
  export SUMERAGI_V2_TLC_PROFILE=ci
  export SUMERAGI_TLAPS_THREADS=1
  export CARGO_NET_OFFLINE=true
  export CARGO_HOME="$IROHA_RELEASE_CARGO_HOME"
  export CARGO="$IROHA_RELEASE_CARGO_BIN"
  export RUSTC="$IROHA_RELEASE_RUSTC_BIN"
  export PATH="${IROHA_RELEASE_VERUS_DIR}:$(dirname "$IROHA_RELEASE_GIT_BIN"):$(dirname "$IROHA_RELEASE_SSH_KEYGEN_BIN"):$(dirname "$IROHA_RELEASE_CARGO_BIN"):$(dirname "$IROHA_RELEASE_RUSTC_BIN"):$(dirname "$IROHA_RELEASE_PYTHON_BIN"):$(dirname "$IROHA_RELEASE_NODE_BIN"):$(dirname "$IROHA_RELEASE_BASH_BIN"):${PATH}"
  for pinned_tool in git ssh-keygen cargo rustc python3 node bash; do
    pinned_variable="IROHA_RELEASE_${pinned_tool^^}_BIN"
    [[ "$pinned_tool" == python3 ]] && pinned_variable=IROHA_RELEASE_PYTHON_BIN
    [[ "$pinned_tool" == ssh-keygen ]] && pinned_variable=IROHA_RELEASE_SSH_KEYGEN_BIN
    if [[ "$(canonical_executable "$pinned_tool")" != "${!pinned_variable}" ]]; then
      echo "sealed release PATH does not resolve pinned ${pinned_tool}" >&2
      exit 1
    fi
  done
fi

# Build every real-localnet executable before any Cargo test process starts.
# Test processes are then permanently skip-build/reentrant-disabled, avoiding a
# Cargo-under-Cargo lock cycle while retaining a source/lock/binary attestation.
ensure_source_bound_localnet_binaries
export_source_bound_localnet_binaries

corridor_enabled=0
corridor_completion_path=""
corridor_leg_index=0
corridor_last_log=""
if [[ "$profile" == "--release" ]]; then
  corridor_enabled=1
  corridor_evidence_root="${release_source_bound_root}/evidence/corridor"
  mkdir -p -- "$corridor_evidence_root"
  corridor_evidence_dir="$(mktemp -d "${corridor_evidence_root}/invocation.XXXXXX")"
  corridor_logs_dir="${corridor_evidence_dir}/logs"
  corridor_summary="${corridor_evidence_dir}/summary.tsv"
  corridor_required_tests="${corridor_evidence_dir}/production-required-tests.tsv"
  corridor_g_unit_inventory="${corridor_evidence_dir}/g-unit-required-tests.tsv"
  corridor_completion_path="${corridor_evidence_dir}/COMPLETED.tsv"
  mkdir -p -- "$corridor_logs_dir"
  printf '%s\n' $'leg_index\tleg_id\tkind\trequired_test_count\tobserved_test_count\tcommand_status\ttee_status\tlog_sha256\tlog\tcommand' \
    >"$corridor_summary"
fi
readonly corridor_enabled

record_corridor_log() {
  local leg_id="$1"
  local kind="$2"
  local required_test_count="$3"
  local command_text="$4"
  local log_path="$5"
  local command_status="$6"
  local tee_status="$7"
  corridor_last_log="$log_path"
  ((corridor_enabled)) || return 0
  if ((command_status != 0 || tee_status != 0)); then
    echo "release corridor leg ${leg_id} failed (command=${command_status}, tee=${tee_status})" >&2
    return 1
  fi
  local observed_test_count
  observed_test_count="$(python3 -I -S - "$kind" "$log_path" <<'PY'
from pathlib import Path
import re
import sys

kind, raw_path = sys.argv[1:]
lines = Path(raw_path).read_text(encoding="utf-8").splitlines()
if kind == "cargo-focus":
    running = [line for line in lines if line == "running 1 test"]
    results = [
        line
        for line in lines
        if re.fullmatch(
            r"test result: ok\. 1 passed; 0 failed; 0 ignored; "
            r"0 measured; [0-9]+ filtered out; finished in .+",
            line,
        )
    ]
    if not running or len(running) != len(results):
        raise SystemExit("ambiguous focused Cargo test transcript")
    print(len(results))
elif kind.startswith("cargo-"):
    running = [re.fullmatch(r"running ([0-9]+) tests?", line) for line in lines]
    running = [match for match in running if match]
    results = [
        re.fullmatch(
            r"test result: ok\. ([0-9]+) passed; 0 failed; 0 ignored; "
            r"0 measured; [0-9]+ filtered out; finished in .+",
            line,
        )
        for line in lines
    ]
    results = [match for match in results if match]
    if len(running) != 1 or len(results) != 1 or running[0].group(1) != results[0].group(1):
        raise SystemExit("ambiguous Cargo test transcript")
    print(results[0].group(1))
elif kind == "pytest":
    matches = [
        re.fullmatch(
            r"([0-9]+) passed in [0-9]+(?:\.[0-9]+)?s"
            r"(?: \([0-9]+:[0-5][0-9]:[0-5][0-9]\))?",
            line,
        )
        for line in lines
    ]
    matches = [match for match in matches if match]
    if len(matches) != 1:
        raise SystemExit("ambiguous pytest transcript")
    print(matches[0].group(1))
elif kind == "node":
    passed = [re.fullmatch(r"# pass ([0-9]+)", line) for line in lines]
    passed = [match for match in passed if match]
    if len(passed) != 1 or lines.count("# fail 0") != 1:
        raise SystemExit("ambiguous Node test transcript")
    print(passed[0].group(1))
elif kind == "native-amx-sdk":
    matches = [
        re.fullmatch(
            r"native-amx-v2-grouped-parity surface=[a-z]+ tests=([0-9]+) "
            r"fixture_sha256=[0-9a-f]{64} "
            r"suite_source_manifest_sha256=[0-9a-f]{64}",
            line,
        )
        for line in lines
    ]
    matches = [match for match in matches if match]
    if len(matches) != 1:
        raise SystemExit("ambiguous grouped Native AMX V2 SDK transcript")
    print(matches[0].group(1))
elif kind == "command":
    print(0)
else:
    raise SystemExit(f"unknown corridor leg kind: {kind}")
PY
)" || {
    echo "release corridor leg ${leg_id} has invalid test output" >&2
    return 1
  }
  if [[ "$kind" == "native-amx-sdk" ]]; then
    local expected_surface="${leg_id#native-amx-grouped-}"
    local expected_marker
    expected_marker="native-amx-v2-grouped-parity surface=${expected_surface} tests=${observed_test_count} fixture_sha256=${native_amx_grouped_fixture_sha256:-} suite_source_manifest_sha256=${native_amx_grouped_suite_source_manifest_sha256:-}"
    if [[ "$(grep -Fxc -- "$expected_marker" "$log_path" || true)" != 1 ]]; then
      echo "release corridor leg ${leg_id} is not bound to the exact grouped Native AMX V2 corpus and suite sources" >&2
      return 1
    fi
  fi
  if [[ "$kind" == "cargo-module" ]]; then
    if ((observed_test_count == 0 || observed_test_count < required_test_count)); then
      echo "release corridor module ${leg_id} ran fewer tests than its required inventory" >&2
      return 1
    fi
  elif [[ "$observed_test_count" != "$required_test_count" ]]; then
    echo "release corridor leg ${leg_id} ran ${observed_test_count} tests, expected ${required_test_count}" >&2
    return 1
  fi
  local relative_log log_sha256
  relative_log="logs/$(basename "$log_path")"
  log_sha256="$(sha256_file "$log_path")"
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$corridor_leg_index" "$leg_id" "$kind" "$required_test_count" \
    "$observed_test_count" "$command_status" "$tee_status" "$log_sha256" \
    "$relative_log" "$command_text" >>"$corridor_summary"
  ((corridor_leg_index += 1))
}

run_corridor_leg() {
  local leg_id="$1"
  local kind="$2"
  local required_test_count="$3"
  local command_text="$4"
  shift 4
  if ((!corridor_enabled)); then
    "$@"
    return
  fi
  local log_path
  printf -v log_path '%s/%02d-%s.log' \
    "$corridor_logs_dir" "$corridor_leg_index" "$leg_id"
  set +e
  "$@" 2>&1 | tee "$log_path"
  local pipeline_status=("${PIPESTATUS[@]}")
  set -e
  record_corridor_log \
    "$leg_id" "$kind" "$required_test_count" "$command_text" "$log_path" \
    "${pipeline_status[0]}" "${pipeline_status[1]}"
}

corridor_contract_log_path() {
  local leg_id="$1"
  if ((corridor_enabled)); then
    printf '%s/%02d-%s.log\n' \
      "$corridor_logs_dir" "$corridor_leg_index" "$leg_id"
  else
    mktemp "${TMPDIR:-/tmp}/${leg_id}.XXXXXX"
  fi
}

# Inventory and execute the production adapter/runtime ownership boundary before
# the slower network and formal corridors. The pure reducer harness cannot
# exercise worker cancellation, queued completion rebinding, or watchdog
# classification, so these exact tests are source-bound release inputs.
required_production_liveness_tests=(
  kura::tests::progress_witness_durability::absent_progress_namespace_requires_every_directory_barrier
  kura::tests::progress_witness_durability::bound_progress_recovery_handles_crash_phases_without_path_escape
  kura::tests::progress_witness_durability::certified_lane_block_strict_retry_reissues_every_barrier
  kura::tests::progress_witness_durability::direct_receipt_snapshot_preserves_sparse_and_mixed_format_entries
  kura::tests::progress_witness_durability::initial_preindex_data_sync_failure_rolls_back_payload_before_retry
  kura::tests::progress_witness_durability::lane_block_application_receipt_strict_retry_reissues_every_barrier
  kura::tests::progress_witness_durability::predecessor_application_receipt_fails_closed_while_durability_barrier_fails
  kura::tests::progress_witness_durability::progress_sidecar_mutation_rejects_symlinks_without_external_writes
  kura::tests::progress_witness_durability::progress_prepend_directory_failure_retries_without_corruption
  kura::tests::progress_witness_durability::strict_sidecar_retry_reissues_barriers_for_exact_existing_payload
  kura::tests::progress_witness_durability::unindexed_crash_suffix_is_repaired_before_retry_or_append
  kura::tests::certified_lane_block_encoding_enforces_source_envelope
  kura::tests::replace_top_block_replay_metadata_preflight_fails_closed_without_mutation
  kura::lane_geometry::tests::first_release_retirement_classifies_recovery_sync_failure_as_retryable
  kura::lane_geometry::tests::first_release_retirement_discards_unpublished_temp_for_every_fixed_pair
  kura::lane_geometry::tests::first_release_retirement_rejects_obsolete_autonomous_rewrite_without_promotion
  kura::lane_geometry::tests::first_release_retirement_recovers_complete_certified_rewrite_before_snapshot
  kura::lane_geometry::tests::first_release_retirement_recovery_rejects_temp_symlink_without_external_writes
  kura::lane_geometry::tests::first_release_retirement_rejects_directory_substitution_at_pair_refresh
  kura::lane_geometry::tests::first_release_retirement_requires_bound_progress_sidecar_durability
  kura::lane_geometry::tests::geometry_gc_requires_bound_merge_receipt_durability_before_deletion
  nexus::lane_relay::tests::actor_backpressure_retains_exact_relay_and_fifo_ticket
  nexus::lane_relay::tests::blocked_relay_does_not_starve_a_responsive_relay
  nexus::lane_relay::tests::terminal_actor_failures_return_exact_relay_ownership
  nexus::lane_relay::tests::saturated_relay_owner_returns_sixty_fifth_without_actor_ticket
  sumeragi::v2_core::tests::prior_view_commit_votes_rebuild_the_exact_locked_round_quorum
  sumeragi::v2_core::tests::vote_statement_identity_excludes_only_the_authenticated_signer
  sumeragi::v2_core::tests::certificate_height_subject_identity_ignores_round_and_phase_only
  sumeragi::v2_core::tests::view_zero_binds_semantic_parent_decision_across_reproposal_rounds
  sumeragi::v2_core::tests::replay_resigns_proposal_with_equivalent_parent_reproposal_round
  sumeragi::v2_core::tests::higher_tc_lock_prunes_superseded_commit_retransmission
  sumeragi::v2_core::tests::same_lock_tc_resigns_local_commit_and_rebuilds_quorum_without_self_delivery
  sumeragi::v2_core::tests::tc_highest_prepare_missed_locally_requires_same_subject_reproposal_before_commit
  sumeragi::v2_core::tests::replay_rejects_later_view_commit_even_after_exact_tc_lock_installation
  sumeragi::v2_core::tests::higher_conflicting_prepare_intent_fences_historical_commit_reconstruction
  sumeragi::v2_core::tests::higher_same_subject_prepare_fences_historical_commit_reconstruction
  sumeragi::v2_core::tests::replay_does_not_resign_proposal_superseded_by_same_round_lock
  sumeragi::v2_core::tests::replay_does_not_resign_commit_superseded_by_higher_tc_lock
  sumeragi::v2_core::tests::replay_resigns_current_proposal_prepare_then_commit_fifo
  sumeragi::v2_core::tests::replay_resigns_current_timeout_then_durable_old_round_commit_fifo
  sumeragi::v2_core::tests::current_view_commit_waits_for_the_exact_durable_lock
  sumeragi::v2_core::tests::decision_retains_in_flight_body_pipeline_without_duplicate_fetch
  sumeragi::v2_core::tests::timeout_elapsed_cannot_start_durable_timeout_after_decision
  sumeragi::v2_core::tests::quorum_completing_timeout_vote_cannot_form_tc_after_decision
  sumeragi::v2_core::tests::commit_qc_cannot_overtake_timeout_frontier
  sumeragi::v2_core::tests::future_view_commit_qc_uses_current_owner_through_application
  sumeragi::v2_core::tests::later_reproposal_commit_qc_replays_and_applies_its_exact_certified_round
  sumeragi::v2_core::tests::valid_commit_qc_supersedes_different_subject_prepare_lock_live_and_replay
  sumeragi::v2_core::tests::earlier_same_body_commit_qc_supersedes_a_later_reproposal_lock
  sumeragi::v2_core::tests::height_context_requires_one_same_round_parent_commit_geometry
  sumeragi::v2_core::tests::stale_generation_completion_is_rejected_after_view_change
  sumeragi::v2_core::tests::stale_persistence_completions_stutter_while_current_append_is_pending
  sumeragi::v2_core::tests::strictly_ahead_install_timeout_advances_owner_and_protects_highest_prepare
  sumeragi::v2_core::tests::tc_omitting_the_local_high_keeps_its_exact_prepare_qc_retransmittable
  sumeragi::v2_core::tests::same_round_timeout_upgrade_rebinds_lock_and_retains_current_timeout_vote
  sumeragi::v2_core::tests::same_round_timeout_upgrade_is_exact_local_proposal_justification
  sumeragi::v2_core::tests::recovery_uses_same_round_timeout_upgrade_as_exact_local_proposal_justification
  sumeragi::v2_core::tests::recovery_excludes_proposal_intent_superseded_by_same_round_timeout_upgrade
  sumeragi::v2_core::tests::later_reproposal_commit_ack_retires_durable_old_round_commit_pool
  sumeragi::v2_core::tests::tc_lock_survives_closed_view_and_commits_after_later_same_subject_reproposal
  sumeragi::v2_core::tests::replay_resigns_same_subject_reproposal_fifo_without_relabelling_old_commit
  sumeragi::v2_core::tests::replay_accepts_strictly_higher_matching_prepare_qc_proposal
  sumeragi::v2_core::tests::future_prepare_qc_is_transactionally_ignored_without_retransmit_ownership
  sumeragi::v2_core::refinement::tests::durable_intent_refinement_accepts_exact_stutters_and_rejects_mutations
  sumeragi::v2_core::refinement::tests::locked_commit_progress_witness_accepts_exact_owners_and_rejects_mutations
  sumeragi::v2_core::refinement::tests::lock_and_commit_requires_one_current_vote_and_proposal_round
  sumeragi::v2_core::refinement::tests::decision_ack_retires_competing_owners_and_keeps_one_body_pipeline
  sumeragi::v2_core::refinement::tests::strict_same_round_refinement_kernels_reject_split_round_mutations
  sumeragi::v2_core::refinement::tests::wal_retirement_authorization_rejects_split_round_decision_and_receipt
  sumeragi::v2_core::refinement::tests::semantic_commit_decision_identity_ignores_only_qc_rounds
  sumeragi::v2_core::reducer::source_link_tests::certified_fetch_capability_requires_the_exact_proposal_origin
  sumeragi::v2_core::reducer::source_link_tests::closed_proposal_round_cannot_create_a_new_commit_intent
  sumeragi::v2_core::wal::byte_lifecycle_tests::same_round_timeout_replay_accepts_only_a_strict_prepare_origin_upgrade
  sumeragi::v2_core::refinement::tests::retransmit_may_reconstruct_one_final_decision_body_stage
  sumeragi::v2_core::refinement::tests::source_linked_effective_lock_body_kernels_reject_adversarial_inputs
  sumeragi::v2_core::refinement::tests::applied_successor_kernel_rejects_foreign_same_height_authority_and_status_mutations
  sumeragi::v2_core::refinement::tests::recovered_successor_kernel_keeps_complete_tip_and_snapshot_authority_disjoint
  sumeragi::v2_core::refinement::tests::successor_startup_lifecycle_preserves_running_on_failure_and_separates_restart_sources
  sumeragi::v2_core::refinement::tests::durable_intent_accepts_only_empty_persistence_completion_stutters
  sumeragi::v2_core::refinement::tests::durable_timeout_boundary_preserves_record_and_successor_owner_rounds
  sumeragi::v2_core::refinement::tests::two_stage_relay_retry_kernel_rejects_source_rotation_eligibility_and_fifo_mutations
  sumeragi::v2_core::refinement::tests::historical_body_pipeline_kernel_rejects_request_subject_and_owner_substitution
  sumeragi::v2_core::refinement::tests::historical_certificate_kernel_rejects_foreign_admission_and_unretired_request
  sumeragi::v2_core::reducer::source_link_tests::retransmit_body_stage_requires_an_exact_durable_decision_capability
  sumeragi::v2_core::reducer::source_link_tests::replay_refinement_binds_the_complete_durable_fifo
  sumeragi::v2_core::reducer::source_link_tests::replay_refinement_rejects_malformed_post_states_even_with_the_right_first_effect
  sumeragi::v2_core::reducer::source_link_tests::enter_view_projection_selects_and_fetches_the_exact_post_install_lock
  sumeragi::v2_core::reducer::source_link_tests::enter_view_without_a_lock_carries_and_fetches_nothing
  sumeragi::v2_core::reducer::source_link_tests::enter_view_effect_cannot_substitute_an_equal_reference_certificate
  sumeragi::authoritative_runtime_gate_tests::anonymous_and_authenticated_non_validator_sources_use_distinct_bounded_lanes
  sumeragi::evidence::tests::sumeragi_v2_equivocation_authenticates_vote_origin_and_execution
  sumeragi::serviced_candidate_store::tests::leader_wire_gate_retains_independent_cross_origin_phase_and_chunk_slots
  sumeragi::authoritative_runtime_gate_tests::byzantine_v2_source_cannot_consume_honest_ingress_reservations_or_service_turns
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_completion_bound_overflow_fails_closed
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_completion_corridor_survives_ordinary_progress_and_timeout_saturation
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_completion_owner_is_source_isolated_and_queue_scoped
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_coalesces_semantic_request_and_attaches_independent_routes
  sumeragi::authoritative_runtime_gate_tests::alternate_reply_route_attaches_before_authenticated_source_lane_cap
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_required_serve_gate_precedes_open
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_certified_request_cutoff_blocks_later_same_source_serve
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_certified_request_cutoff_blocks_later_churn
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_occurrence_ordinal_coalesces_and_overflow_closes
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_checked_dequeue_freezes_one_physical_cut_per_occurrence
  sumeragi::authoritative_runtime_gate_tests::restored_older_logical_owner_cannot_cross_an_earlier_physical_leader_wire
  sumeragi::authoritative_runtime_gate_tests::restored_productive_retry_freezes_the_current_physical_source_prefix
  sumeragi::authoritative_runtime_gate_tests::restored_productive_retry_stays_behind_an_earlier_certified_request_carrier
  sumeragi::authoritative_runtime_gate_tests::restored_productive_retry_ordinal_exhaustion_keeps_the_owner_dormant
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_exact_ownership_carrier_tracks_route_actions_and_cursors
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_projection_distinguishes_identical_bytes_from_distinct_origins
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_ownership_projection_ignores_route_liveness_until_maintenance
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_exact_max_chunk_bound_matches_canonical_wire
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_exact_response_bound_accepts_required_and_rejects_required_minus_one
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_minimum_capacity_admits_timeout_votes
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_prepare_vote_cannot_consume_commit_progress_reservation
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_recommended_context_fits_default_disjoint_byte_partitions
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_reservation_potential_does_not_increase_on_service
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_reserves_timeout_vote_bytes_behind_auxiliary_pressure
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_rejects_timeout_vote_larger_than_its_byte_reserve
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_saturated_peer_cannot_block_an_empty_validator_timeout
  sumeragi::authoritative_runtime_gate_tests::direct_and_synthetic_envelopes_keep_identity_roles_consistent
  sumeragi::authoritative_runtime_gate_tests::atomic_lane_certificate_uses_the_shared_progress_owner
  sumeragi::authoritative_runtime_gate_tests::oversized_atomic_lane_certificate_is_returned_exactly
  sumeragi::authoritative_runtime_gate_tests::relayed_origin_churn_uses_one_via_lane_and_preserves_protocol_origin
  sumeragi::authoritative_runtime_gate_tests::authenticated_non_validator_source_cap_retries_third_source_until_one_lane_drains
  sumeragi::authoritative_runtime_gate_tests::roster_origin_relay_completion_has_authenticated_source_count_and_byte_owner
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_wire_index_keeps_authenticated_origins_distinct
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_reserves_same_source_transport_completion_behind_auxiliary_pressure
  sumeragi::authoritative_runtime_gate_tests::v2_ingress_rejects_capacity_without_per_validator_progress_reservations
  sumeragi::authoritative_runtime_gate_tests::transport_reply_route_construction_is_fallible_and_target_bound
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_maximum_merge_sidecar_chunk_frame_matches_canonical_wire
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_minimal_layout_enforces_exact_block_sync_frame_boundary
  sumeragi::authoritative_runtime_gate_tests::sidecar_allocations_require_roster_requester_before_lane_queue_admission
  merge_sidecar::tests::pending_pruning_keeps_only_authoritative_live_carrier_identities
  merge_sidecar::tests::holder_derivation_rejects_noncanonical_qc_rosters_and_bitmaps
  merge_sidecar::tests::unsolicited_and_wrong_sender_chunks_are_rejected
  merge_sidecar::tests::request_id_hash_length_and_epoch_mismatches_are_rejected
  merge_sidecar::tests::oversized_counts_payloads_and_duplicate_chunks_are_rejected
  merge_sidecar::tests::out_of_order_chunks_complete_and_release_exact_deferred_block
  merge_sidecar::tests::progressive_max_size_response_does_not_expire_while_chunks_advance
  merge_sidecar::tests::standalone_close_retries_until_exact_ack_then_terminates
  merge_sidecar::tests::outbound_chunk_drain_is_fair_across_bounded_sessions
  merge_sidecar::tests::session_and_deferred_caps_fail_closed_without_unbounded_growth
  merge_sidecar::tests::restart_drops_partial_assemblies
  merge_sidecar::tests::corrupt_canonical_bytes_are_rejected
  merge_sidecar::tests::exact_active_delivery_retry_preserves_decreasing_chunk_rank
  merge_sidecar::tests::alternate_source_progress_and_reconnect_preserve_independent_cursors
  merge_sidecar::tests::reused_actor_ordinals_under_different_tenures_are_rejected_atomically
  merge_sidecar::tests::later_delivery_preserves_the_current_source_cursor
  merge_sidecar::tests::later_delivery_while_chunk_is_in_flight_waits_for_flush_before_next_emit
  merge_sidecar::tests::late_old_exact_item_receipt_completes_reconnected_attempt_once
  merge_sidecar::tests::equal_sequence_with_different_semantic_identity_is_rejected_before_materialization
  merge_sidecar::tests::response_materialization_requires_and_consumes_its_exact_admission_gate
  merge_sidecar::tests::inactive_reply_route_is_rejected_before_server_gate_admission
  merge_sidecar::tests::completed_source_later_and_reconnect_stay_terminal_while_sibling_progresses
  merge_sidecar::tests::exact_delivery_retry_stays_terminal_beyond_retired_ttl_horizon
  merge_sidecar::tests::request_stream_close_floor_advances_only_over_a_contiguous_terminal_prefix
  merge_sidecar::tests::authenticated_close_floor_retires_covered_output_and_rejects_replay_or_regression
  merge_sidecar::tests::rejected_request_does_not_consume_server_stream_state
  merge_sidecar::tests::completed_source_does_not_block_a_new_alternate_source
  merge_sidecar::tests::configured_route_source_capacity_bounds_semantic_attempts
  merge_sidecar::tests::configured_source_geometry_reserves_more_than_eight_independent_attempts
  merge_sidecar::tests::third_session_from_one_hub_is_rejected_while_another_hub_progresses
  merge_sidecar::tests::source_byte_overflow_is_rejected_while_another_hub_progresses
  merge_sidecar::tests::completed_short_session_replacement_cannot_starve_an_older_long_session
  merge_sidecar::tests::unsent_request_restores_holder_and_backoff_state
  merge_sidecar::tests::idle_request_retry_starts_strictly_after_the_fairness_cursor
  merge_sidecar::tests::route_retirement_between_admission_and_enqueue_releases_all_response_reservations
  merge_sidecar::tests::saturated_materializer_does_not_erase_same_request_alternate_session
  merge_sidecar::tests::saturated_materializer_does_not_erase_same_request_alternate_bytes
  merge_sidecar::tests::sidecar_flush_refinement_advances_only_exact_source_chunk
  merge_sidecar::tests::sidecar_flush_admission_retains_timeout_attempt_identity
  merge_sidecar::tests::authenticated_source_limits_are_fixed_at_four_gates_two_sessions_and_sixteen_mibibytes
  merge_sidecar::tests::cached_sidecar_payload_objects_scale_with_chunks_not_sources
  merge_sidecar::tests::sidecar_admission_matches_the_cached_arc_without_changing_ownership
  merge_sidecar::tests::height_rollover_retries_only_each_sources_current_in_flight_chunk
  merge_sidecar::tests::durable_requester_restart_advances_sequence_and_carries_close_floor
  merge_sidecar::tests::durable_requester_crash_before_send_closes_unobserved_sequence
  merge_sidecar::tests::durable_lifecycle_rejects_canonical_payload_with_stale_digest
  merge_sidecar::tests::durable_responder_restart_preserves_same_hub_gate_budget
  merge_sidecar::tests::durable_responder_restart_allows_new_source_while_recovered_source_is_offline
  merge_sidecar::tests::durable_responder_restart_preserves_terminal_source_cursor_and_rebinds_capability
  merge_sidecar::tests::durable_response_drain_persists_pending_identity_before_handoff
  merge_sidecar::tests::authenticated_generation_hint_retires_old_attempt_before_reissue
  merge_sidecar::tests::close_covers_allocated_but_unsent_sequence_after_requester_recovery
  merge_sidecar::tests::failed_response_rotation_persists_close_before_sequence_exhaustion
  merge_sidecar::tests::delayed_same_payload_flush_cannot_advance_the_successor_occurrence
  merge_sidecar::tests::same_occurrence_advances_piggybacked_floor_without_rematerializing_current_output
  merge_sidecar::tests::semantic_sequence_norito_decode_rejects_zero
  merge_sidecar::tests::lifecycle_v3_roundtrip_and_restore_enforce_gate_and_attempt_bounds_separately
  merge_sidecar::tests::legacy_lifecycle_v1_snapshot_is_rejected_without_migration
  merge_sidecar::tests::legacy_lifecycle_v2_snapshot_is_rejected_without_layout_guessing
  merge_sidecar::tests::durable_lifecycle_v3_root_high_water_is_exact_monotonic_and_noop_stable
  merge_sidecar::tests::durable_lifecycle_v3_bootstrap_recovers_first_commit_and_rejects_missing_roots
  merge_sidecar::tests::durable_lifecycle_v3_rejects_crossed_bootstrap_and_committed_root_shapes
  merge_sidecar::tests::durable_lifecycle_v3_recovers_regular_temps_and_rejects_unsafe_artifacts
  merge_sidecar::tests::durable_lifecycle_v3_validates_semantics_before_retiring_crash_artifacts
  merge_sidecar::tests::durable_lifecycle_v3_rejects_split_generations_and_rehashed_state
  merge_sidecar::tests::durable_lifecycle_v3_generation_exhaustion_precedes_close_mutation
  merge_sidecar::tests::durable_lifecycle_v3_generation_exhaustion_precedes_writer_flush_cas
  merge_sidecar::tests::durable_lifecycle_v3_recovers_predecessor_before_state_directory_sync
  merge_sidecar::tests::durable_lifecycle_v3_recovers_predecessor_between_state_and_root_publication
  merge_sidecar::tests::durable_lifecycle_v3_resyncs_replaced_root_before_predecessor_cleanup
  merge_sidecar::tests::durable_lifecycle_v3_recovers_successor_after_root_publication
  merge_sidecar::tests::durable_lifecycle_v3_rejects_missing_state_with_surviving_root_high_water
  merge_sidecar::tests::durable_lifecycle_rejects_legacy_stream_state_without_guessing_a_layout
  merge_sidecar::tests::durable_lifecycle_rejects_regressed_duplicate_and_cross_epoch_state
  merge_sidecar::tests::durable_lifecycle_rejects_source_geometry_and_pending_marker_corruption
  merge_sidecar::tests::durable_roster_change_rejects_non_roster_geometry_drift
  merge_sidecar::tests::durable_roster_replacement_restores_prior_geometry_then_fences_once
  merge_sidecar::tests::durable_roster_transition_failure_preserves_predecessor_then_commits_successor
  merge_sidecar::tests::durable_stream_epochs_and_service_generations_bound_peer_churn
  merge_sidecar::tests::durable_terminal_retirement_is_not_restored_and_exact_replay_is_fresh
  merge_sidecar::tests::failed_terminal_retirement_persist_leaves_memory_unchanged
  merge_sidecar::tests::future_service_generation_is_rejected_without_hint_or_server_state
  merge_sidecar::tests::generation_hint_fence_survives_lifecycle_restart
  merge_sidecar::tests::generation_rollover_consumes_a_late_writer_flush_without_mutating_the_successor
  merge_sidecar::tests::higher_stream_epoch_atomically_retires_old_output_and_rejects_stale_control
  merge_sidecar::tests::higher_stream_epoch_capacity_rejection_is_fail_atomic
  merge_sidecar::tests::inactive_outbound_reclamation_releases_bytes_and_preserves_exact_cursor
  merge_sidecar::tests::inactive_source_reclamation_releases_budget_and_reconnect_rematerializes
  merge_sidecar::tests::later_delivery_during_materialization_keeps_exact_authorized_route
  merge_sidecar::tests::materialization_scheduler_chooses_lowest_occurrence_within_requester
  merge_sidecar::tests::materialization_scheduler_round_robins_requesters_without_starvation
  merge_sidecar::tests::quiescent_multi_source_pressure_never_rolls_or_bypasses_source_caps
  merge_sidecar::tests::reclaimed_source_releases_capacity_and_resumes_at_durable_cursor
  merge_sidecar::tests::reply_unwritable_reclamation_applies_a_late_exact_flush_once
  merge_sidecar::tests::reply_unwritable_reclamation_persists_pending_cursor_across_restart
  merge_sidecar::tests::reply_unwritable_route_parks_inflight_materialization_without_bytes
  merge_sidecar::tests::reply_unwritable_routes_do_not_block_roster_transition
  merge_sidecar::tests::request_id_binds_occurrence_but_excludes_monotonic_close_floor
  merge_sidecar::tests::authenticated_source_quota_rejects_origin_churn_and_preserves_other_source
  merge_sidecar::tests::responder_roster_digest_is_order_and_duplicate_independent
  merge_sidecar::tests::roster_transition_rejects_authorized_or_active_output
  merge_sidecar::tests::same_cardinality_roster_replacement_reclaims_inactive_output_and_preserves_requester_state
  merge_sidecar::tests::same_roster_identity_preserves_server_state_and_changed_size_rolls_once
  merge_sidecar::tests::server_capacity_geometry_separates_streams_gates_and_attempts
  merge_sidecar::tests::service_generation_overflow_rejects_without_compacting_server_state
  merge_sidecar::tests::full_server_table_never_advances_generation_without_a_changed_roster
  merge_sidecar::tests::durable_exact_output_fence_clears_unconfirmed_debt_without_forging_close
  merge_sidecar::tests::durable_exact_output_fence_rejects_service_generation_exhaustion_before_mutation
  merge_sidecar::tests::service_generation_rollover_journal_failure_is_fail_atomic
  merge_sidecar::tests::fifth_gate_from_one_hub_is_rejected_while_another_hub_progresses
  merge_sidecar::tests::stale_close_ack_cannot_terminate_a_reallocated_stream_epoch
  merge_sidecar::tests::stale_close_yields_an_exact_generation_hint_without_allocating_server_state
  merge_sidecar::tests::stale_request_replay_is_stateless_under_an_obstructed_lifecycle_journal
  merge_sidecar::tests::stream_epoch_overflow_rejects_without_allocating_or_reusing_an_epoch
  merge_sidecar::tests::terminal_retirement_releases_multi_source_quota_for_honest_admission
  merge_sidecar::tests::transient_materialization_release_keeps_exact_retry
  merge_sidecar::tests::transient_response_capacity_defers_materialization_on_the_same_delivery
  merge_sidecar::tests::writable_reconnect_during_materialization_keeps_exact_authorized_tenure
  sumeragi::v2::tests::deferred_locked_commit_delivery_tracks_generation_after_tc
  sumeragi::v2::tests::prelock_current_commit_is_readmitted_with_priority_neutral_service_identity
  sumeragi::v2::tests::tc_reset_readmits_exact_locked_commit_once_per_generation
  sumeragi::v2::tests::tc_promoted_lock_requires_same_subject_reproposal_before_commit
  sumeragi::v2::tests::timeout_vote_installs_embedded_qc_before_forming_tc
  sumeragi::v2::tests::exact_local_completion_after_decision_reports_body_validated_progress
  sumeragi::v2::tests::busy_local_completion_during_decision_wal_reaches_apply_once
  sumeragi::v2::tests::enter_view_conversion_uses_effect_carried_lock_not_reducer_lock
  sumeragi::v2::tests::saturated_normal_lane_retains_exact_local_proposal_completion
  sumeragi::v2::tests::unsafe_proposal_admission_preserves_duplicate_and_equivocation_semantics
  sumeragi::v2::tests::admission_keeps_only_the_exact_locked_commit_vote_beyond_one_rotation
  sumeragi::v2::tests::deferred_service_cursor_cycles_nonempty_classes
  sumeragi::v2::tests::unowned_busy_certificates_roll_back_staged_registry_and_active_subject
  sumeragi::v2::tests::unowned_busy_exact_locked_vote_rolls_back_and_remains_retryable
  sumeragi::v2::tests::capacity_bypass_records_follow_current_lock_and_timeout_view
  sumeragi::v2::tests::deferred_progress_capacity_matches_partition_geometry
  sumeragi::v2::tests::deferred_progress_partition_owns_every_vote_and_certificate_class
  sumeragi::v2::tests::full_normal_deferred_lane_cannot_drop_absolute_timeout
  sumeragi::v2::tests::protected_locked_vote_uses_reserved_capacity_without_evicting_certificate_ownership
  sumeragi::v2::tests::leader_without_owned_candidate_work_reports_missing_proposal_state
  sumeragi::v2::tests::successor_context_requires_the_durable_cryptographic_parent
  sumeragi::v2::tests::authentication_rejects_valid_commitment_conflicts_without_mutating_adapter
  sumeragi::v2::tests::deferred_adapter_activation_marker_survives_a_no_progress_publication
  sumeragi::v2::tests::deferred_adapter_replay_with_startup_effects_publishes_no_status
  sumeragi::v2::tests::persistence_macro_step_budgets_have_exact_five_effect_maximum
  sumeragi::v2::tests::drive_effects_rejects_oversized_non_persisting_batch
  sumeragi::v2::tests::drive_effects_rejects_record_specific_overbudget_before_wal_append
  sumeragi::v2::tests::drive_effects_rejects_multiple_persist_owners_before_wal_append
  sumeragi::v2::tests::post_wal_oversized_continuation_fails_closed_and_replays_exact_record
  sumeragi::v2::tests::deferred_dispatch_decreases_rank_by_exactly_one_macro_step_per_turn
  sumeragi::v2::tests::deferred_service_contract_violation_is_terminal
  sumeragi::v2::tests::busy_deferred_input_blocks_terminal_readiness_until_serviced
  sumeragi::v2::tests::deferred_actor_source_never_aliases_across_adapter_instances
  sumeragi::v2::tests::deferred_occurrence_capability_binds_direct_authenticated_provenance
  sumeragi::v2::tests::deferred_adapter_rejects_foreign_and_replayed_capabilities_before_reducer_step
  sumeragi::v2::tests::deferred_authenticated_retry_retains_exact_original_and_effective_tags
  sumeragi::v2::tests::deferred_ordinal_exhaustion_fails_adapter_closed_before_wrap
  sumeragi::v2::tests::deferred_service_debt_overflow_is_typed_and_fail_closed
  sumeragi::v2::tests::deferred_service_evidence_rejects_every_owner_and_rank_mutation
  sumeragi::v2::tests::deferred_zero_ordinal_is_exact_single_use_and_never_reminted
  sumeragi::v2::tests::authenticated_deferred_service_rejects_same_kind_envelope_swap_before_reducer
  sumeragi::v2::tests::strict_same_round_tc_preserves_and_retags_timeout_vote_owners
  sumeragi::v2::tests::deferred_projection_distinguishes_authenticated_proposal_origins
  sumeragi::v2::tests::vote_body_ownership_uses_the_authenticated_proposal_origin
  sumeragi::v2::tests::locked_subject_reproposal_and_strict_higher_prepare_are_safe
  sumeragi::v2::tests::registry_rejects_split_round_vote_and_qc_reference
  sumeragi::v2_body_store::tests::rotating_leader_locked_body_reproposal_is_stored_and_revalidated_per_round
  sumeragi::v2_body_store::tests::rotating_leader_reproposal_authenticates_the_immutable_header_leader
  sumeragi::v2_block_sync::tests::discovery_outputs_only_normal_commit_qc_ingress_and_waits_for_enqueue
  sumeragi::v2_block_sync::tests::catch_up_is_strictly_sequential_across_contexts
  sumeragi::v2_block_sync::tests::historical_body_comes_from_kura_and_a_non_signer_archive_can_serve
  sumeragi::v2_apply::tests::committed_merge_reservation_rejects_bare_norito
  sumeragi::v2_effects::tests::retained_locked_body_survives_same_lock_view_churn_before_fetch_adopts_it
  sumeragi::v2_effects::tests::authenticated_genesis_satisfies_manifestless_certified_decision_fetch_locally
  sumeragi::v2_effects::tests::reproposal_commit_qc_applies_the_exact_unchanged_body
  sumeragi::v2_effects::tests::reproposal_commit_signing_uses_its_same_round_validation_marker
  sumeragi::v2_effects::tests::split_round_commit_signing_is_rejected_before_service_dispatch
  sumeragi::v2_effects::tests::deferred_merge_sidecar_accepts_earlier_carrier_and_rejects_future_or_foreign
  sumeragi::v2_effects::tests::higher_different_lock_releases_retained_cache_before_replacement_staging
  sumeragi::v2_effects::tests::synthetic_higher_round_same_subject_retires_origin_bound_stages_before_raw_cache_reuse
  sumeragi::v2_effects::tests::queued_protected_store_keeps_one_work_id_across_repeated_tcs
  sumeragi::v2_effects::tests::tc_body_rebind_preserves_the_exact_fetch_until_reconstruction_completes
  sumeragi::v2_effects::tests::tc_body_rebind_preserves_certified_request_ownership_through_signed_response
  sumeragi::v2_effects::tests::tc_body_rebind_uses_the_effective_local_lock_when_the_tc_omits_or_lowers_it
  sumeragi::v2_effects::tests::enter_view_rejects_a_tc_high_without_an_effective_protected_body
  sumeragi::v2_effects::tests::tc_body_rebind_retags_a_queued_body_available_completion
  sumeragi::v2_effects::tests::tc_body_rebind_retires_a_superseded_completion_and_releases_capacity
  sumeragi::v2_effects::tests::serialized_runtime_rebinds_busy_deferred_body_completion_before_service
  sumeragi::v2_effects::tests::tc_body_rebind_cancels_fetch_superseded_by_a_higher_different_qc
  sumeragi::v2_effects::tests::same_tag_higher_lock_retires_exact_origin_ownership_before_staging
  sumeragi::v2_effects::tests::same_tag_higher_lock_retires_fetch_store_and_validation_owners
  sumeragi::v2_effects::tests::first_lock_retires_unlocked_fetch_store_and_validation_owners
  sumeragi::v2_effects::tests::higher_lock_retires_queued_store_validation_and_local_proposal_completions
  sumeragi::v2_effects::tests::lock_reconciliation_rejects_same_round_conflict_and_late_lower_lock
  sumeragi::v2_effects::tests::failed_lock_cleanup_keeps_exact_owner_and_requires_restart
  sumeragi::v2_effects::tests::lock_cleanup_rejects_inconsistent_certified_request_before_mutation
  sumeragi::v2_effects::tests::lock_cleanup_status_failure_preserves_committed_replacement
  sumeragi::v2_effects::tests::higher_round_same_subject_retires_old_origin_pipeline_with_same_tag
  sumeragi::v2_effects::tests::decision_installed_by_same_runtime_step_retires_stale_terminal_effects
  sumeragi::v2_effects::tests::decision_installed_by_same_runtime_step_keeps_exact_commit_and_body_work
  sumeragi::v2_effects::tests::different_subject_decision_supersedes_protected_lock_and_frees_losing_capacity
  sumeragi::v2_effects::tests::failed_decision_cleanup_keeps_losing_owner_and_requires_restart
  sumeragi::v2_effects::tests::decision_cleanup_fetch_failure_preserves_exact_local_pipeline_consumer
  sumeragi::v2_effects::tests::decision_cleanup_rejects_inconsistent_certified_request_before_mutation
  sumeragi::v2_effects::tests::decision_rebinds_exact_local_validation_to_reducer_progress
  sumeragi::v2_effects::tests::decision_preserves_current_tag_local_proposal_for_direct_apply
  sumeragi::v2_effects::tests::decision_commitment_mismatch_fails_closed_before_apply
  sumeragi::v2_effects::tests::stale_generation_local_completion_uses_durable_recovery
  sumeragi::v2_effects::tests::missing_merge_sidecar_retains_exact_validation_until_retry
  sumeragi::v2_effects::tests::decided_apply_retries_after_exact_merge_sidecar_recovery
  sumeragi::v2_effects::tests::apply_rejects_matching_commit_qc_from_foreign_context_without_scheduling_work
  sumeragi::v2_effects::tests::production_certified_body_request_rejects_locally_conflicting_qc_without_fail_close
  sumeragi::v2_effects::tests::production_commit_certificate_response_conflict_keeps_discovery_outstanding_and_runtime_open
  sumeragi::v2_effects::tests::proposal_a_distinct_prepare_qc_b_and_timeout_sign_progress_at_capacity_two
  sumeragi::v2_effects::tests::passive_fetch_does_not_block_prepare_qc_or_timeout_in_serialized_runtime
  sumeragi::v2_effects::tests::fetch_retransmissions_reuse_one_work_slot_and_one_signed_request
  sumeragi::v2_effects::tests::apply_retransmissions_reuse_one_work_slot
  sumeragi::v2_effects::tests::full_capacity_certified_fetch_retains_its_exact_owner_until_capacity_releases
  sumeragi::v2_effects::tests::certified_request_pressure_retains_higher_authority_upgrade_under_one_owner
  sumeragi::v2_effects::tests::reconstructible_new_certified_fetch_acquires_ownership_from_retained_admission
  sumeragi::v2_effects::tests::production_capacity_saturation_admits_response_and_reconstructible_fetch
  sumeragi::v2_effects::tests::durable_sign_preemption_orders_speculative_certified_and_locked_fetches
  sumeragi::v2_effects::tests::retained_producer_suffix_allows_exact_payload_chunk_to_release_fetch_capacity
  sumeragi::v2_effects::tests::retained_producer_suffix_allows_exact_certified_response_to_release_fetch_capacity
  sumeragi::v2_effects::tests::retained_effect_batch_rejects_overtaking_and_oversize_before_partial_dispatch
  sumeragi::v2_effects::tests::exact_candidate_retry_coalesces_and_owner_replacement_fails_closed
  sumeragi::v2_effects::tests::fetch_owner_replacement_is_rejected_before_upgrade_refinement_or_request_work
  sumeragi::v2_effects::tests::adapter_effect_retry_policy_is_closed_over_all_eleven_effect_classes
  sumeragi::v2_effects::tests::retained_effect_tail_is_fifo_and_refilters_after_durable_decision
  sumeragi::v2_effects::tests::pending_work_producer_inventory_is_exhaustive_and_source_linked
  sumeragi::v2_effects::tests::reconciled_decision_rejects_same_round_subject_commitment_drift
  sumeragi::v2_effects::tests::runtime_step_dispatches_entire_effect_batch_before_returning
  sumeragi::v2_effects::tests::effect_dispatch_consumes_leader_wire_terminal_created_while_batch_drains
  sumeragi::v2_effects::tests::retained_live_retry_consumes_decision_retirement_terminal_same_cycle
  sumeragi::v2_effects::tests::retained_recovery_retry_consumes_decision_retirement_terminal_same_cycle
  sumeragi::v2_effects::tests::live_runtime_step_rejects_missing_scheduler_ownership_before_callbacks
  sumeragi::v2_effects::tests::recovery_runtime_step_rejects_invalid_scheduler_ownership_before_callbacks
  sumeragi::v2_effects::tests::owned_payload_chunk_rejects_source_swap_before_service_and_keeps_unknown_work_nonfatal
  sumeragi::v2_effects::tests::certified_body_response_carrier_swap_fails_closed_before_fetch_mutation
  sumeragi::v2_effects::tests::failed_view_cleanup_keeps_stale_fetch_and_requires_restart
  sumeragi::v2_effects::tests::view_cleanup_rejects_inconsistent_protected_request_before_lock_mutation
  sumeragi::v2_effects::tests::view_cleanup_second_cancellation_failure_commits_no_fetch_retirement
  sumeragi::v2_effects::tests::discovered_commit_certificate_mints_exact_reducer_admission_only_after_enqueue
  sumeragi::v2_lane_work::tests::direct_decision_quiesces_losing_lane_and_retransmission_work
  sumeragi::v2_lane_work::tests::applied_lane_certificate_retires_alternative_qc_replays_without_weakening_conflicts
  sumeragi::v2_lane_work::tests::native_amx_signing_guard_capacity_preserves_explicit_runtime_bound
  sumeragi::v2_lane_work::tests::native_amx_signing_guard_capacity_preserves_exact_hard_boundary
  sumeragi::v2_lane_work::tests::native_amx_signing_guard_capacity_rejects_above_hard_bound_without_clipping
  sumeragi::v2_lane_work::tests::native_amx_signing_guard_limits_reject_oversized_record_and_anchor_bytes
  sumeragi::v2_lane_work::tests::native_amx_adapter_opens_with_bounded_production_like_limits
  sumeragi::v2_lane_work::tests::native_amx_request_rejects_inactive_reply_route_before_signing
  sumeragi::v2_lane_work::tests::persisted_lane_session_uses_only_selected_qc_signer_pops
  sumeragi::v2_lane_work::tests::same_proposal_shortcut_rejects_unvalidated_certificate_variants
  sumeragi::v2_lane_work::tests::planner_view_one_binds_rotated_global_leader_to_fresh_lane_view
  sumeragi::v2_lane_work::tests::enabled_nexus_binds_independent_lane_author_distinct_from_global_leader
  sumeragi::v2_lane_work::tests::lane_work_stays_quiescent_until_the_exact_global_prepare_lock
  sumeragi::v2_lane_work::tests::global_body_lock_replacement_requires_higher_prepare_round_and_exact_subject
  sumeragi::v2_lane_work::tests::superseded_commit_protected_lane_session_cannot_retransmit
  sumeragi::v2_lane_work::tests::higher_same_subject_lock_retains_unchanged_body_binding
  sumeragi::v2_lane_work::tests::validator_storage_platform_gate_rejects_voters_and_allows_observers
  sumeragi::v2_lane_work::tests::durable_lane_certificate_is_one_atomic_kura_backed_response
  sumeragi::v2_lane_work::tests::durable_lane_certificate_serves_rotated_validator_after_pressure
  sumeragi::v2_lane_work::tests::historical_certificate_survives_successor_lock_decision_persistence_and_restart
  sumeragi::v2_lane_work::tests::prior_height_hydration_stays_local_under_successor_backpressure
  sumeragi::v2_lane_work::tests::carrier_replacement_filters_persistence_and_output_sources_together
  sumeragi::v2_lane_work::tests::duplicate_reply_effect_preserves_exact_source_delivery
  sumeragi::v2_lane_work::tests::reply_effect_rejects_missing_or_retargeted_route_set
  sumeragi::v2_lane_work::tests::duplicate_reply_effect_updates_only_later_delivery_from_same_source
  sumeragi::v2_lane_work::tests::duplicate_reply_effect_retains_alternate_sources_across_source_update
  sumeragi::v2_lane_work::tests::temporarily_unserviceable_effect_requeues_behind_later_reserved_work
  sumeragi::v2_lane_work::tests::retired_sidecar_route_between_drain_and_lane_queue_preserves_live_sibling
  sumeragi::v2_lane_work::tests::late_old_sidecar_flush_removes_only_reconnected_source_retry
  sumeragi::v2_lane_work::tests::durable_lane_certificate_coalescing_preserves_alternate_ingress_owners
  sumeragi::v2_lane_work::tests::sidecar_lifecycle_journal_failure_latches_restart_before_request_dispatch
  sumeragi::v2_lane_work::tests::sidecar_close_journal_failure_latches_restart_and_blocks_queued_chunk
  sumeragi::v2_lane_work::tests::sidecar_close_ack_journal_failure_latches_restart_before_completion
  sumeragi::v2_lane_work::tests::sidecar_timeout_journal_failure_latches_restart_before_retry_dispatch
  sumeragi::v2_lane_work::tests::advanced_responder_serves_exact_finalized_historical_merge_sidecar
  sumeragi::v2_lane_work::tests::close_generation_hint_journal_failure_preserves_last_close_occurrence
  sumeragi::v2_lane_work::tests::current_height_sidecar_service_rejects_a_different_carrier_parent
  sumeragi::v2_lane_work::tests::exact_sidecar_request_replays_after_missing_kura_entry_materializes
  sumeragi::v2_lane_work::tests::historical_merge_sidecar_rejects_noncanonical_future_and_non_holder_requests
  sumeragi::v2_lane_work::tests::historical_merge_sidecar_requires_verified_finality_and_exact_context
  sumeragi::v2_lane_work::tests::request_generation_hint_journal_failure_preserves_queued_occurrence
  sumeragi::v2_lane_work::tests::retained_sidecar_handoff_rejects_foreign_owner_and_wrong_successor
  sumeragi::v2_lane_work::tests::retryable_control_flood_cannot_fail_stop_close_or_block_request_progress
  sumeragi::v2_lane_work::tests::stranded_responder_control_queue_slot_replaces_with_a_writable_trigger
  sumeragi::v2_lane_work::tests::sidecar_close_cancels_and_coalesces_dominant_stream_epoch
  sumeragi::v2_lane_work::tests::sidecar_ingress_materializes_the_fair_scheduler_job_not_the_newest_request
  sumeragi::v2_lane_work::tests::sidecar_server_allocations_require_roster_requester_but_not_roster_relay
  sumeragi::v2_lane_work::tests::stale_generation_hint_bypasses_obstructed_journal_without_latching_restart
  sumeragi::v2_lane_work::tests::terminal_retirement_journal_failure_latches_lane_restart_with_gate_unchanged
  sumeragi::v2_lane_work::tests::typed_changed_roster_v3_lifecycle_failure_preserves_predecessor_pair
  sumeragi::v2_lane_work::tests::duplicate_generation_hint_coalesces_alternate_reply_sources
  sumeragi::v2_lane_work::tests::typed_finality_handoff_preserves_same_roster_current_chunk_for_retry
  sumeragi::v2_lane_work::tests::typed_finality_handoff_fences_changed_roster_after_sealing_active_writer
  sumeragi::v2_runtime::tests::adapter_effect_binding_is_exact_route_neutral_and_three_bounded
  sumeragi::v2_runtime::tests::certified_body_pipeline_retains_statement_and_owner_across_stage_kinds
  sumeragi::v2_runtime::tests::body_pipeline_acquires_commit_authority_monotonically_under_one_owner
  sumeragi::v2_runtime::tests::retiring_exact_body_completion_releases_a_capacity_one_ingress_slot
  sumeragi::v2_runtime::tests::exact_authenticated_qc_from_distinct_sources_coalesces_in_one_runtime_slot
  sumeragi::v2_runtime::tests::exact_authenticated_timeout_certificate_from_distinct_sources_coalesces_in_one_runtime_slot
  sumeragi::v2_runtime::tests::same_semantic_qc_with_conflicting_route_authority_fails_closed_atomically
  sumeragi::v2_runtime::tests::runtime_ingress_carrier_capacity_returns_backpressure_atomically
  sumeragi::v2_runtime::tests::exact_authenticated_progress_retransmission_is_queue_coalesced
  sumeragi::v2_runtime::tests::commit_certificate_response_coalesces_with_exact_busy_deferred_qc
  sumeragi::v2_runtime::tests::completion_retries_coalesce_across_ingress_and_busy_deferred_ownership
  sumeragi::v2_runtime::tests::body_available_rebind_accepts_same_view_higher_generation
  sumeragi::v2_runtime::tests::body_available_rebind_rejects_uninstalled_destination_without_mutation
  sumeragi::v2_runtime::tests::body_available_rebind_coalesces_exact_busy_deferred_destination_owner
  sumeragi::v2_runtime::tests::body_available_rebind_destination_conflicts_and_duplicates_fail_closed_before_mutation
  sumeragi::v2_runtime::tests::duplicate_body_available_rebind_and_retirement_fail_closed_before_mutation
  sumeragi::v2_runtime::tests::unbound_direct_prepare_and_commit_votes_are_recoverable_after_validation
  sumeragi::v2_runtime::tests::conflicting_body_pipeline_evidence_fails_closed_before_body_available_pruning
  sumeragi::v2_runtime::tests::conflicting_local_and_validated_receipts_do_not_coalesce
  sumeragi::v2_runtime::tests::applied_body_pipeline_phases_suppress_retries_before_ordinal_allocation
  sumeragi::v2_runtime::tests::applied_validation_failure_suppresses_retry_and_rejects_opposite_outcome
  sumeragi::v2_runtime::tests::applied_local_proposal_handoff_suppresses_retry_before_ordinal_allocation
  sumeragi::v2_runtime::tests::drained_internal_ignore_uses_exact_durable_tombstone_before_readmission
  sumeragi::v2_runtime::tests::queued_body_completion_coalesces_only_its_incumbent_owner
  sumeragi::v2_runtime::tests::stale_internal_callback_is_marker_free_and_malformed_callback_spends_no_ordinal
  sumeragi::v2_runtime::tests::body_pipeline_retirement_spans_ingress_and_busy_deferred_owners_and_rejects_duplicates
  sumeragi::v2_runtime::tests::decision_retires_proposal_owners_but_preserves_body_and_application_completions
  sumeragi::v2_runtime::tests::decision_retires_stale_local_completion_for_durable_recovery
  sumeragi::v2_runtime::tests::progress_cursor_decision_preserves_outer_ingress_completion_until_apply
  sumeragi::v2_runtime::tests::decision_cleanup_preserves_unique_busy_deferred_completion
  sumeragi::v2_runtime::tests::decision_commitment_mismatch_fails_closed_before_retirement
  sumeragi::v2_runtime::tests::decision_retirement_releases_queued_leader_wire_runtime_owner
  sumeragi::v2_runtime::tests::lock_retirement_releases_busy_deferred_leader_wire_runtime_owner
  sumeragi::v2_runtime::tests::production_authenticated_preflight_is_never_semantic_only_coalesce
  sumeragi::v2_runtime::tests::semantic_only_authenticated_coalesce_fails_before_receipt_registration
  sumeragi::v2_runtime::tests::successor_activation_snapshot_requires_armed_live_clocks
  sumeragi::v2_runtime::tests::production_ingress_pop_uses_shared_selector_for_every_ready_mask
  sumeragi::v2_runtime::tests::network_admission_uses_exact_normal_and_progress_reservations
  sumeragi::v2_runtime::tests::serviceable_adapter_debt_drains_one_macro_step_before_new_work
  sumeragi::v2_runtime::tests::serviceable_adapter_debt_runs_without_runtime_ingress
  sumeragi::v2_runtime::tests::runtime_rejects_driver_selection_outside_eligible_deferred_owner_set
  sumeragi::v2_runtime::tests::runtime_physical_cut_is_monotone_and_regression_fails_closed
  sumeragi::v2_runtime::tests::deferred_physical_cut_blocks_only_pre_cut_leader_wire_occurrences
  sumeragi::v2_runtime::tests::distinct_pre_runtime_leader_wire_qc_waits_behind_busy_deferred_owner
  sumeragi::v2_runtime::tests::post_cut_old_logical_replay_cannot_overtake_fenced_busy_deferred_target
  sumeragi::v2_runtime::tests::pre_dequeue_probe_validates_unfrozen_leader_wire_identity
  sumeragi::v2_runtime::tests::real_adapter_signature_completion_precedes_deferred_timeout_and_newer_ingress
  sumeragi::v2_runtime::tests::adapter_command_identity_is_derived_from_exact_immutable_payload
  sumeragi::v2_runtime::tests::admission_ordinal_exhaustion_fails_runtime_closed
  sumeragi::v2_runtime::tests::runtime_rejects_replayed_foreign_and_mutated_deferred_tokens
  sumeragi::v2_runtime::tests::scheduler_owner_carrier_covers_live_recovery_and_typed_deferred_branches
  sumeragi::v2_runtime::tests::scheduler_owner_carrier_pins_exact_fifo_identity_and_rank_fields
  sumeragi::v2_runtime::tests::scheduler_owner_must_be_taken_before_a_later_step_can_enter
  sumeragi::v2_runtime::tests::selected_owner_without_a_runtime_minted_ordinal_fails_closed
  sumeragi::v2_runtime::tests::runtime_merges_alternate_sources_for_one_semantic_request
  sumeragi::v2_runtime::tests::runtime_keeps_identical_wire_requests_from_distinct_semantic_origins_independent
  sumeragi::v2_runtime::tests::busy_deferred_request_merges_alternate_source_and_services_exact_carrier
  sumeragi::v2_runtime::tests::busy_deferred_older_aggregate_rebases_owner_and_rejects_identity_mutation
  sumeragi::v2_runtime::tests::dormant_local_fifo_metadata_rejects_wrong_stage_ordinal_and_capacity
  sumeragi::v2_runtime::tests::later_same_semantic_fair_retry_retains_runtime_lifecycle_root
  sumeragi::v2_runtime::tests::network_runtime_rejects_unminted_and_unrelated_colliding_fair_ordinals
  sumeragi::v2_runtime::tests::older_frozen_aggregate_carrier_rebases_queued_runtime_minimum
  sumeragi::v2_runtime::tests::ordinary_fair_predecessor_remains_before_serve_until_runtime_consumes_it
  sumeragi::v2_runtime::tests::restored_serve_high_watermark_precedes_startup_runtime_owner
  sumeragi::v2_runtime::tests::full_runtime_churn_cannot_cross_an_exact_serve_ordinal
  sumeragi::v2_runtime::tests::preassigned_batch_lifecycles_require_shared_mint_and_exact_root
  sumeragi::v2_runtime::tests::restart_dormant_completion_batch_atomically_replaces_latent_slots
  sumeragi::v2_runtime::tests::restart_dormant_local_fifo_reservation_survives_full_class_churn
  sumeragi::v2_transport::tests::reproposal_commit_qc_authenticates_its_exact_same_round_body
  sumeragi::v2_recovery::tests::all_hash_only_snapshot_recovers_exact_authenticated_successor
  sumeragi::v2_recovery::tests::finalized_tip_derives_one_idempotent_successor_context
  sumeragi::v2_recovery::tests::successor_rejects_foreign_same_height_predecessor_and_mismatched_receipt
  sumeragi::v2_runner::tests::same_tag_higher_lock_retires_all_local_proposal_owners
  sumeragi::v2_runner::tests::fail_closed_authenticated_coalesce_releases_gate_and_suppresses_retry
  sumeragi::v2_runner::tests::reserved_lane_output_bypasses_unserviceable_head_without_losing_owner
  sumeragi::v2_runner::tests::runner_dispatch_preserves_durable_lane_certificate_reply_routes
  sumeragi::v2_runner::tests::runner_dispatch_preserves_certified_sidecar_chunk_reply_routes
  sumeragi::v2_runner::tests::bounded_sidecar_admission_turn_applies_only_its_budget
  sumeragi::v2_runner::tests::runner_dispatch_prunes_retired_sidecar_source_without_losing_live_sibling
  sumeragi::v2_runner::tests::runner_dispatch_advances_certified_sidecar_only_after_writer_flush
  sumeragi::v2_runner::tests::runner_dispatch_retired_admission_race_emits_no_sidecar_receipt
  sumeragi::v2_runner::tests::runner_closed_sidecar_flush_reconnect_retries_same_chunk_then_advances_once
  sumeragi::v2_runner::tests::runner_preflight_enqueue_race_retains_sidecar_source_until_capacity_reopens
  sumeragi::v2_runner::tests::runner_old_flushed_sidecar_receipt_cancels_queued_reconnect_retry
  sumeragi::v2_runner::tests::closed_sidecar_prefix_handoff_requeues_only_failed_suffix
  sumeragi::v2_runner::tests::runner_dispatch_rejects_certified_sidecar_chunk_without_reply_route
  sumeragi::v2_runner::tests::runner_dispatch_rejects_durable_response_without_reply_routes
  sumeragi::v2_runner::tests::exact_locked_body_is_reencoded_at_the_reproposal_round_without_byte_drift
  sumeragi::v2_runner::tests::replayed_proposal_sign_reserves_only_the_exact_current_lock_owner
  sumeragi::v2_runner::tests::first_same_subject_lock_preserves_pending_local_proposal_events
  sumeragi::v2_runner::tests::higher_same_subject_lock_retires_prior_origin_work
  sumeragi::v2_runner::tests::first_same_subject_lock_from_prior_view_retires_unlocked_work
  sumeragi::v2_runner::tests::late_old_rejection_cannot_arm_heartbeat_for_replacement_lock
  sumeragi::v2_runner::tests::decision_retires_local_work_before_prepared_delivery
  sumeragi::v2_runner::tests::finalized_rollover_closes_ingress_before_successor_replay
  sumeragi::v2_runner::tests::synthesized_durable_rollover_contract_allows_successor_after_dead_target_handoff
  sumeragi::v2_runner::tests::successor_activation_is_published_only_after_ingress_is_open
  sumeragi::v2_runner::tests::complete_tip_recovery_uses_the_same_live_successor_boundary
  sumeragi::v2_runner::tests::successor_startup_failure_stays_running_and_fails_closed_without_activation
  sumeragi::v2_runner::tests::status_guard_retains_failure_snapshot_and_clears_clean_shutdown
  sumeragi::v2_runner::tests::unsupported_storage_platform_rejects_runner_voter_and_admits_observer
  sumeragi::v2_runner::tests::successor_construction_rejects_foreign_same_height_predecessor_authority
  sumeragi::v2_runner::tests::direct_close_ack_retains_reply_route_from_lane_through_worker
  sumeragi::v2_runner::tests::empty_drain_after_peek_is_restart_required_without_panicking
  sumeragi::v2_runner::tests::relayed_generation_hint_preserves_reply_route_from_lane_through_worker
  sumeragi::v2_runner::tests::deferred_startup_producer_turn_is_retained_until_one_exclusive_claim
  sumeragi::v2_worker::tests::fetch_consumer_rebind_preserves_live_or_queued_reconstruction_owner
  sumeragi::v2_worker::tests::entered_view_accepts_same_view_higher_generation_supersession
  sumeragi::v2_worker::tests::invalid_fetch_consumer_rebind_fails_closed_without_consuming_owner
  sumeragi::v2_worker::tests::locked_candidate_requests_coalesce_by_immutable_subject
  sumeragi::v2_worker::tests::locked_candidate_completion_uses_latest_consumer_without_reloading
  sumeragi::v2_worker::tests::locked_candidate_consumer_rebind_rejects_stale_or_regressive_tags
  sumeragi::v2_worker::tests::locked_candidate_duplicate_or_wrong_completion_is_rejected
  sumeragi::v2_worker::tests::locked_candidate_future_completion_is_rejected_without_replacing_owner
  sumeragi::v2_worker::tests::higher_different_lock_replaces_load_and_retires_stale_completion
  sumeragi::v2_worker::tests::superseded_locked_candidate_failure_starts_latest_acquisition
  sumeragi::v2_worker::tests::unavailable_locked_candidate_waits_for_matching_durable_store
  sumeragi::v2_worker::tests::unavailable_locked_candidate_rebinds_latest_consumer_before_retry
  sumeragi::v2_worker::tests::outbound_payload_retention_is_constant_across_many_view_changes
  sumeragi::v2_worker::tests::late_stale_proposal_signature_cannot_restore_pruned_outbound_payload
  sumeragi::v2_worker::tests::nonzero_view_proposal_intent_replays_through_production_services
  sumeragi::v2_worker::tests::decision_retires_candidate_and_outbound_work_but_keeps_exact_sidecar_deferral
  sumeragi::v2_worker::tests::worker_completion_is_retained_behind_a_full_runtime_fifo
  sumeragi::v2_worker::tests::production_drain_publishes_worker_completion_behind_full_runtime_fifo
  sumeragi::v2_worker::tests::successful_auxiliary_drain_republishes_cleared_completion_ownership
  sumeragi::v2_worker::tests::auxiliary_completion_drain_is_batch_bounded
  sumeragi::v2_worker::tests::actor_backpressure_retains_exact_final_lane_commit_qc_post
  sumeragi::v2_worker::tests::actor_backpressure_retains_complete_merge_share_fanout
  sumeragi::v2_worker::tests::exact_serve_predecessor_episode_services_older_local_without_admitting_later_io
  sumeragi::v2_worker::tests::repeated_exact_serve_claims_close_all_older_sources_before_later_io
  sumeragi::v2_worker::tests::exact_serve_claim_waits_out_full_control_prefix_before_older_causal_admission
  sumeragi::v2_worker::tests::fair_ingress_exact_ticket_coalesces_and_commits_before_later_io_producers
  sumeragi::v2_worker::tests::drained_exact_retransmission_gets_fresh_scheduler_ordinal
  sumeragi::v2_worker::tests::fair_ingress_gate_overflow_closes_without_partial_admission
  sumeragi::v2_worker::tests::fair_ingress_classifies_current_historical_future_and_unauthenticated_requests
  sumeragi::v2_worker::tests::fair_ingress_rollover_retires_ticket_before_old_service_teardown
  sumeragi::v2_worker::tests::selected_serve_physical_carrier_precedes_reactivated_older_leader_lifecycle
  sumeragi::v2_worker::tests::checked_serve_dequeue_rejects_mutated_fair_lifecycle_ordinal
  sumeragi::v2_worker::tests::dormant_exact_head_fail_stops_after_saturated_fair_prefix_without_repair
  sumeragi::v2_worker::tests::dormant_serve_waiters_fail_stop_without_requester_ordinal_repair
  sumeragi::v2_worker::tests::durable_raw_admission_restart_reuses_lifecycle_and_excludes_family_replacement
  sumeragi::v2_worker::tests::durable_raw_higher_view_drop_restarts_into_local_successor_completion
  sumeragi::v2_worker::tests::durable_raw_waiter_rejects_mutated_logical_lineage
  sumeragi::v2_worker::tests::durable_serve_state_v4_rejects_v3_header_and_payload_layouts
  sumeragi::v2_worker::tests::invalid_requester_signed_qc_quarantines_one_family_without_consuming_honest_capacity
  sumeragi::v2_worker::tests::raw_admission_persistence_failure_rolls_back_logical_lineage
  sumeragi::v2_worker::tests::fair_ingress_producer_episode_wins_or_yields_without_partial_exact_admission
  sumeragi::v2_worker::tests::fair_ingress_full_prefix_materializes_exact_serve_before_later_churn
  sumeragi::v2_worker::tests::fair_ingress_serve_only_prefix_materializes_after_frozen_completion_ack
  sumeragi::v2_worker::tests::fair_ingress_terminal_retry_replays_without_lifecycle_resurrection
  sumeragi::v2_worker::tests::fair_ingress_higher_view_waits_out_active_family_before_admission
  sumeragi::v2_worker::tests::durable_serve_restart_before_terminal_seal_resumes_same_lifecycle
  sumeragi::v2_worker::tests::restored_serve_waiter_advances_shared_runtime_source
  sumeragi::v2_worker::tests::durable_serve_abort_before_commit_restarts_into_local_completion
  sumeragi::v2_worker::tests::durable_serve_seal_before_completion_post_restores_terminal_replay
  sumeragi::v2_worker::tests::durable_serve_seal_survives_post_before_physical_ack
  sumeragi::v2_worker::tests::durable_serve_corruption_fails_closed_without_highwater_reset
  sumeragi::v2_worker::tests::durable_serve_frame_bound_covers_max_layout_manifest_hashes
  sumeragi::v2_worker::tests::durable_higher_view_abort_republishes_displaced_terminal_before_restart
  sumeragi::v2_worker::tests::durable_higher_view_admission_crash_locally_completes_successor_union
  sumeragi::v2_worker::tests::durable_serve_restore_rejects_capacity_owner_swap_across_replacement
  sumeragi::v2_worker::tests::durable_serve_state_is_pruned_only_with_successor_rollover_root
  sumeragi::v2_worker::tests::certified_serve_future_slot_blocks_control_and_consensus_replenishment
  sumeragi::v2_worker::tests::certified_serve_cross_relay_retry_replays_one_terminal_tombstone
  sumeragi::v2_worker::tests::certified_serve_terminal_rejects_mismatched_response_hash_without_releasing_owner
  sumeragi::v2_worker::tests::certified_serve_observer_owner_contains_prepare_and_commit_subfamilies
  sumeragi::v2_worker::tests::certified_serve_higher_view_abort_restores_terminal_high_watermark
  sumeragi::v2_worker::tests::certified_serve_receiver_close_aborts_reserved_replacement_without_orphan
  sumeragi::v2_worker::tests::certified_serve_delayed_lower_view_cross_relay_cannot_resurrect
  sumeragi::v2_worker::tests::certified_serve_receiver_close_rolls_back_pending_capacity_replacement
  sumeragi::v2_worker::tests::certified_serve_receiver_close_rolls_back_materialized_unclaimed_replacement
  sumeragi::v2_worker::tests::certified_serve_shutdown_rolls_back_materialized_unclaimed_replacement
  sumeragi::v2_worker::tests::certified_serve_terminal_replay_waits_for_barrier_then_bypasses_full_serve_fifo
  sumeragi::v2_worker::tests::certified_serve_terminal_replay_source_retains_retired_route_and_reconnects
  sumeragi::v2_worker::tests::exact_output_coalescing_preserves_distinct_fair_ingress_admissions
  sumeragi::v2_worker::tests::same_tenure_updates_and_reconnect_preserve_current_item
  sumeragi::v2_worker::tests::delayed_old_tenure_delivery_cannot_replace_newer_worker_reply_route
  sumeragi::v2_worker::tests::closed_flush_on_delivery_active_unwritable_route_parks_without_cursor_advance
  sumeragi::v2_worker::tests::closed_flush_racing_final_receiver_retirement_is_nonfatal
  sumeragi::v2_worker::tests::unavailable_admission_racing_retirement_is_nonfatal
  sumeragi::v2_worker::tests::ordinary_reply_late_old_flush_after_reconnect_advances_exactly_once
  sumeragi::v2_worker::tests::closed_sidecar_source_reconnect_retries_current_item_while_sibling_backpressures
  sumeragi::v2_worker::tests::completed_sidecar_reconnect_preserves_terminal_cursor_without_capacity_charge
  sumeragi::v2_worker::tests::later_delivery_cannot_requeue_pending_or_unapplied_sidecar_flush_but_other_attempts_progress
  sumeragi::v2_worker::tests::mixed_source_retry_retains_pending_flush_target_without_resetting_live_siblings
  sumeragi::v2_worker::tests::inactive_reply_target_tombstone_rejects_cross_source_equal_ordinal_collision
  sumeragi::v2_worker::tests::owned_reply_history_merge_retries_candidate_retirement_after_prune
  sumeragi::v2_worker::tests::newly_observed_alternate_hub_starts_at_zero_without_resetting_parked_source
  sumeragi::v2_worker::tests::a_b_a_hub_reconnect_preserves_each_source_cursor
  sumeragi::v2_worker::tests::owned_reply_transfer_retirement_after_validation_is_atomic
  sumeragi::v2_worker::tests::bulk_backpressure_does_not_block_reserved_lane_or_safety_output
  sumeragi::v2_worker::tests::non_roster_targets_cannot_consume_frozen_validator_reservations
  sumeragi::v2_worker::tests::partial_fanout_progress_releases_only_the_completed_target_unit
  sumeragi::v2_worker::tests::ownership_units_reject_reservation_spill_and_release_exact_target
  sumeragi::v2_worker::tests::backpressured_source_does_not_block_other_sources_or_consume_their_reserve
  sumeragi::v2_worker::tests::production_output_path_serves_later_fanout_while_target_stays_backpressured
  sumeragi::v2_worker::tests::response_outputs_without_exact_routes_fail_stop
  sumeragi::v2_worker::tests::orphan_chunk_coalescing_preserves_alternate_fair_ingress_routes
  sumeragi::v2_worker::tests::owned_orphan_chunk_replay_preserves_alternate_source_routes_and_cursors
  sumeragi::v2_worker::tests::durable_reconstructed_body_terminalizes_late_chunk_across_arrival_order
  sumeragi::v2_worker::tests::productive_retry_after_proofless_reconstruction_does_not_become_orphan
  sumeragi::v2_worker::tests::sidecar_flush_ack_identity_mismatch_fails_closed
  sumeragi::v2_worker::tests::reply_flush_attempt_identity_mismatch_fails_without_cursor_or_attempt_advance
  sumeragi::v2_worker::tests::sidecar_receipts_use_a_separate_bounded_control_queue
  sumeragi::v2_worker::tests::actor_backpressure_cannot_change_returned_payload_identity
  sumeragi::v2_worker::tests::exact_output_retry_rejects_a_different_message_identity
  sumeragi::v2_worker::tests::full_exact_output_corridor_does_not_disguise_non_progress_routes_as_backpressure
  sumeragi::v2_worker::tests::applied_height_handoff_retires_all_sidecar_flush_states_without_blocking_successor
  sumeragi::v2_worker::tests::applied_height_handoff_counts_and_clears_parked_reply_cursor_atomically
  sumeragi::v2_worker::tests::applied_height_handoff_rejects_output_without_reconstruction
  sumeragi::v2_worker::tests::applied_height_handoff_rejects_unbound_lane_output_atomically
  sumeragi::v2_worker::tests::applied_height_handoff_rejects_wrong_height_global_output
  sumeragi::v2_worker::tests::applied_height_handoff_accepts_historical_kura_global_responses_atomically
  sumeragi::v2_worker::tests::applied_height_handoff_accepts_only_exact_historical_kura_lane_certificate
  sumeragi::v2_worker::tests::adaptive_reply_timeout_grows_closed_preserves_and_flushed_resets_attempt
  sumeragi::v2_worker::tests::certified_sidecar_close_cancels_only_the_exact_stream_epoch
  sumeragi::v2_worker::tests::certified_sidecar_transfer_identity_binds_stream_epoch
  sumeragi::v2_worker::tests::generation_hint_uses_ordinary_reply_flush_ownership
  sumeragi::v2_worker::tests::generation_hint_uses_exact_reply_ownership_without_topology_fallback
  sumeragi::v2_worker::tests::generation_hint_requires_exact_reply_route_ownership
  sumeragi::v2_worker::tests::responder_control_retry_coalesces_alternate_return_route
  sumeragi::v2_worker::tests::stranded_responder_control_is_replaced_by_a_writable_authenticated_trigger
  sumeragi::v2_worker::tests::multi_route_stranded_responder_control_uses_one_dedicated_reservation
  sumeragi::v2_worker::tests::stranded_responder_control_fifo_collision_is_fail_atomic
  sumeragi::v2_worker::tests::responder_control_replacement_cancels_ticket_after_index_commit
  sumeragi::v2_worker::tests::final_exact_output_seal_is_one_shot_and_blocks_late_enqueue
  sumeragi::v2_worker::tests::finalized_cleanup_without_exact_output_seal_latches_restart
  sumeragi::v2_worker::tests::generation_close_dominance_cancels_queued_and_admitted_sidecar_occurrences
  sumeragi::v2_worker::tests::non_frozen_responder_controls_remain_strictly_shared_bounded
  sumeragi::v2_worker::tests::ordinary_exact_output_does_not_suppress_retryable_sidecar_control_ownership
  sumeragi::v2_worker::tests::ordinary_reply_timeout_grows_only_its_source_attempt_while_sibling_progresses
  sumeragi::v2_worker::tests::parked_same_target_reply_and_full_shared_pool_do_not_block_request_or_close
  sumeragi::v2_worker::tests::reliable_flush_projection_preserves_exact_stream_epoch
  sumeragi::v2_worker::tests::responder_control_uses_ordinary_reply_flush_ownership
  sumeragi::v2_worker::tests::retryable_sidecar_controls_are_bounded_and_fair_per_semantic_target
  sumeragi::v2_worker::tests::writable_responder_control_source_retains_one_distinct_pending_control
  sumeragi::v2_worker::tests::unrelated_parked_reply_does_not_suppress_responsive_target_control
  sumeragi::status::v2_liveness_watchdog_tests::blocker_classifier_has_stable_specific_precedence
  sumeragi::status::v2_liveness_watchdog_tests::current_view_timeout_path_yields_only_to_an_exact_locked_commit_owner
  sumeragi::status::v2_liveness_watchdog_tests::locked_candidate_load_overlay_precedes_commit_quorum_diagnosis
  sumeragi::status::v2_liveness_watchdog_tests::aged_queue_without_service_debt_does_not_claim_scheduler_starvation
  sumeragi::status::v2_liveness_watchdog_tests::network_ingress_service_clock_distinguishes_stopped_and_active_scans
  sumeragi::status::v2_liveness_watchdog_tests::repeated_tc_reconstruction_of_same_locked_commit_pool_does_not_reset_height_clock
  sumeragi::status::v2_liveness_watchdog_tests::apply_waiting_on_merge_sidecar_is_application_pending_not_body_unavailable
  sumeragi::status::v2_liveness_watchdog_tests::effect_completion_overlay_preserves_capacity_age_and_service_debt
  sumeragi::status::v2_liveness_watchdog_tests::effect_dispatch_overlay_exposes_age_without_claiming_scheduler_starvation
  sumeragi::status::v2_liveness_watchdog_tests::live_effect_completion_observer_survives_stopped_runner_and_clears_stale_depth
  sumeragi::status::v2_liveness_watchdog_tests::successor_handoff_is_visible_until_the_exact_successor_becomes_active_once
  sumeragi::status::v2_liveness_watchdog_tests::completed_predecessor_work_alone_never_claims_successor_activation
  sumeragi::status::v2_liveness_watchdog_tests::complete_tip_recovery_publishes_one_exact_live_successor_boundary
  sumeragi::status::v2_liveness_watchdog_tests::rejected_successor_activation_never_mutates_or_replaces_the_predecessor
  sumeragi::status::v2_liveness_watchdog_tests::successor_handoff_rejects_every_incomplete_predecessor_witness
  sumeragi::status::v2_liveness_watchdog_tests::successor_startup_overlays_never_cross_the_height_context_boundary
  sumeragi::status::v2_liveness_watchdog_tests::active_watchdog_is_deadline_driven_edge_triggered_and_recovers_on_progress
  sumeragi::status::v2_liveness_watchdog_tests::active_watchdog_resets_on_successor_owner_and_status_clear
  sumeragi::status::v2_liveness_watchdog_tests::rejected_running_successor_failure_projection_preserves_status
  zk::kagemusha_finality::tests::aggregate_signature_authenticates_proposal_origin
  block::consensus_v2::finality::tests::header_binding_allows_unchanged_reproposal_but_rejects_earlier_decision_round
  offline::kagemusha_v4_topup_provenance_tests::compact_qc_rejects_foreign_or_future_proposal_origin
  block::consensus_v2::tests::height_context_identity_ignores_reproposal_round_and_rejects_split_rounds
  block::consensus_v2::tests::timeout_proposal_accepts_only_the_selected_prepare_subject
  sumeragi_v2_runner::prepare_qc_split_tests::locked_commit_progress_witness_rejects_inexact_or_empty_ownership
  sumeragi_v2_runner::prepare_qc_split_tests::locked_commit_progress_witness_accepts_each_exact_owner
  sumeragi_v2_runner::prepare_qc_split_tests::distinct_prepare_qc_view_zero_wait_covers_deadline_without_masking_view_one
  sumeragi_v2_runner::prepare_qc_split_tests::exact_prepare_qc_requires_both_count_and_power_quorum
  peer::run::tests::authenticated_source_credit_precedes_network_and_subscriber_backlogs
  peer::run::tests::recoverable_post_acknowledges_only_after_full_write_and_flush
  peer::run::tests::partial_write_error_closes_ack_without_false_completion
  peer::run::tests::coalesced_batch_acknowledges_every_item_only_after_flush
  peer::run::tests::maximum_frame_uses_a_bounded_number_of_source_reservations
  peer::shared_byte_budget_tests::frame_retention_coalesces_each_distinct_source_owner_without_reaccounting
  peer::shared_byte_budget_tests::authenticated_source_count_registry_bounds_identity_churn_and_capacity_drift
  peer::shared_byte_budget_tests::pending_protected_sources_reserve_released_owner_slots_from_identity_churn
  peer::shared_byte_budget_tests::impossible_protected_projection_preserves_last_valid_authority
  peer::shared_byte_budget_tests::authenticated_source_byte_reserves_fail_closed_until_authority_is_installed
  peer::shared_byte_budget_tests::inbound_only_obsolete_lease_defers_protected_source_until_drain
  peer::shared_byte_budget_tests::outbound_only_obsolete_lease_defers_protected_source_until_drain
  peer::shared_byte_budget_tests::shared_source_geometry_counts_all_owner_kinds_by_unique_peer_id
  peer::run::tests::dispatch_worker_shutdown_drains_reliable_replaced_connection_to_actor
  peer::run::tests::peer_task_abort_drains_queued_worker_then_notifies_exact_connection_once
  peer::run::tests::peer_task_panic_closes_delivery_producer_and_notifies_exact_connection_once
  peer::run::tests::dispatch_worker_join_error_is_returned_after_fail_closed_teardown
  peer::run::tests::full_write_without_flush_ack_closes_actor_witness_and_retries_on_replacement
  peer::run::tests::consensus_lane_and_v2_topics_share_authenticated_high_source_credit
  network::inbound_source_memory_bound_tests::authenticated_source_count_share_is_checked_and_never_zero
  network::tests::actor_progress_bypasses_full_deferred_owner_and_waits_for_writer_flush
  network::tests::actor_progress_lease_survives_topology_transition
  network::tests::actor_progress_retries_exactly_once_on_peer_writer_replacement
  network::tests::actor_progress_retry_round_robin_bypasses_partitioned_target
  network::tests::cap_one_blocked_source_cannot_prevent_live_source_service
  network::tests::actor_progress_lease_survives_debug_packet_loss_until_delivery_retries
  network::tests::actor_broadcast_retry_targets_only_failed_peers
  network::tests::reliable_subscriber_is_single_consumer_under_clone_budget_pressure
  network::tests::reconnecting_peer_cannot_multiply_retained_source_credits
  network::handle_update_tests::progress_budget_preserves_fifo_for_three_registered_producers
  network::tests::reliable_progress_class_matches_actor_reservations_exactly
  network::tests::reply_flush_identity_binds_ticket_tenure_source_payload_and_delivery_occurrence
  network::tests::reply_flush_identity_requires_and_exposes_timeout_attempt
  network::tests::reply_timeout_attempt_is_retained_by_actor_admission_ticket
  network::tests::reply_flush_test_fixture_binds_exact_canonical_post_and_opaque_actor
  network::tests::reply_flush_test_fixture_distinguishes_success_timeout_and_close
  network::tests::adaptive_reply_timeout_scaling_handles_extreme_duration_without_panicking
  network::tests::adaptive_reply_attempt_flushes_between_base_and_doubled_deadline
  network::tests::cancelled_pending_exact_reply_observes_ready_flush_before_release
  network::tests::expired_exact_deadline_beats_closed_peer_writer_receiver
  network::tests::full_exact_writer_queue_times_out_closes_route_and_releases_actor_budget
  network::tests::nonready_exact_reply_ack_cannot_keep_stale_route_alive
  network::tests::pending_queue_drop_observes_ready_exact_flush_before_shutdown_close
  network::tests::ready_exact_reply_flush_wins_connection_replacement
  network::tests::ready_exact_reply_flush_wins_route_retirement
  network::tests::reply_writer_deadline_retirement_is_idempotent
  network::tests::stale_reply_writer_deadline_does_not_terminate_replacement
  network::tests::terminal_fence_observes_deadline_flush_published_after_initial_poll
  network::tests::terminal_fence_observes_inactive_route_flush_published_after_initial_poll
  network::tests::terminal_fence_observes_replacement_flush_published_after_initial_poll
  network::tests::terminal_fence_observes_send_before_close_and_rejects_send_after_close
  network::tests::topology_writer_full_retry_does_not_acquire_exact_reply_deadline
  network::tests::peer_message_mints_actor_global_delivery_ordinals_across_connection_tenures
  network::tests::reply_route_survives_peer_message_clone_mapping_and_split
  network::tests::peer_message_rehydration_rejects_second_reply_route_without_retargeting
  network::tests::reply_source_key_groups_relay_origins_and_orders_actor_instances
  network::tests::reply_route_source_updates_are_ordinal_monotonic_and_target_scoped
  network::tests::dependent_test_fixture_mints_opaque_tenures_and_delivery_ordinals
  network::tests::cancelled_newer_hub_cannot_erase_older_independent_route_attempt
  network::tests::dependent_fixture_models_bounded_actor_global_multi_hub_ownership
  network::tests::reply_route_pruning_retains_equal_ordinal_tenure_tombstone
  network::tests::delayed_superseded_tenure_cannot_replace_or_tombstone_newer_same_source_writer
  network::tests::reply_route_binding_rejects_evicted_tombstone_collision
  network::tests::reply_route_set_isolates_sources_preserves_cursors_and_prunes_retired_capacity
  network::tests::route_cancelled_between_preflight_and_admission_retires_without_queue_ownership
  network::tests::reply_admission_rejects_retargeting_foreign_handles_and_wrong_tickets
  network::tests::reply_actor_admission_does_not_complete_writer_flush_ack
  network::tests::reply_flush_ack_cancellation_between_precheck_and_budget_lock_returns_none
  network::tests::reply_wrapper_exposes_delivery_active_unwritable_no_ownership
  network::tests::retired_reply_tenure_closes_flush_ack_without_false_completion
  network::tests::reply_flush_ack_completes_only_after_peer_writer_flush
  network::handle_update_tests::progress_ticket_rejects_a_different_same_length_payload
  network::tests::configured_assist_hub_connection_cannot_overflow_reliable_geometry
  network::tests::topology_larger_than_reliable_target_geometry_is_rejected_atomically
  network::tests::assist_hub_refresh_above_reliable_geometry_is_rejected_atomically
  network::tests::public_observer_is_rejected_before_source_authority_and_admitted_after_explicit_empty
  network::tests::trusted_observers_survive_topology_updates
  network::tests::blocked_a_to_b_drains_old_route_and_suppresses_obsolete_reconnect
  network::tests::a_to_b_to_a_source_authority_commits_only_newest_snapshot
  network::tests::impossible_source_authority_snapshot_preserves_last_valid_projection
  network::tests::configured_hub_handoff_waits_for_retained_old_source_and_commits_on_reconnect
  network::tests::spoke_startup_dials_only_trusted_hub_and_selects_after_authenticated_hub_role
  network::tests::topology_removal_cancels_every_deferred_owner_for_removed_peer
  network::tests::deferred_progress_survives_ttl_but_explicit_peer_removal_cancels_it
  network::tests::outside_topology_retransmit_is_not_misreported_as_delivered
  network::tests::accepted_draining_connection_delivers_reliable_progress_after_replacement
  network::tests::rejected_authenticated_connection_is_cancelled_and_remains_cap_accounted
  network::tests::deferred_queue_preserves_order_and_connection_bindings
  network::tests::missing_session_retains_unbound_consensus_frame_and_schedules_reconnect
  network::tests::flush_deferred_frames_sends_unbound_entries_to_current_session
  network::tests::flush_deferred_frames_drops_stale_connection_binding_without_posting
  network::tests::flush_deferred_frames_rebinds_reliable_stale_connection
  network::tests::flush_deferred_frames_closed_session_restores_remaining_unbound
  network::tests::live_session_backpressure_defers_retry_with_current_connection
  network::tests::live_session_closed_defers_retry_unbound_and_removes_peer
  network::tests::live_session_post_overflow_disconnect_policy_defers_unbound
  network::handle_update_tests::targetized_broadcast_coalesces_only_the_same_digest_and_membership
  network::tests::distinct_broadcast_residual_is_target_isolated_and_its_rank_decreases
  network::tests::exact_broadcast_retry_coalesces_but_distinct_and_direct_requests_do_not
  network::tests::removed_membership_cancels_only_old_broadcast_debt_across_readd
  network::tests::cancelled_target_child_with_pending_flush_ack_releases_exactly_once
  network::tests::requested_topology_is_not_authority_and_closed_fanout_returns_all_targets
  network::tests::reliable_delivery_waits_for_its_route_subscriber
  network::tests::closed_reliable_subscriber_transfers_actor_pending_backlog_to_replacement
  network::tests::network_actor_drop_retires_routes_and_only_its_waiters
  network::tests::reply_route_tenure_retires_only_after_final_receiver_guard_drops
  network::tests::duplicate_configured_termination_does_not_advance_backoff_or_metrics
  network::inbound_source_memory_bound_tests::reliable_actor_waiter_geometry_rejects_zero_and_combined_overflow
  network::handle_update_tests::configured_producer_geometry_gives_six_same_source_waiters_decreasing_ranks
  consensus_message_control::tests::controlled_v2_admission_preserves_distinct_relay_identity
  consensus_message_control::tests::stale_duplicate_reordered_and_unknown_releases_are_atomic
  consensus_message_control::tests::hold_capacity_is_bounded_by_count_bytes_and_checked_arithmetic
  consensus_message_control::tests::drain_fence_holds_racing_chunks_fifo_until_atomic_cutover
  consensus_message_control::tests::failed_release_clears_in_flight_ownership_and_latches_fatal
  consensus_message_control::tests::fatal_controller_rejects_an_unchanged_command_poll
  consensus_message_control::tests::private_reader_treats_safe_atomic_replacement_as_retryable_identity_churn
  consensus_message_control::tests::retired_release_finishes_drain_without_claiming_delivery
  network_relay_tests::obsolete_sumeragi_relay_message_completes_as_delivered
  network_relay_tests::test_control_hold_release_preserves_live_route_and_retires_canceled_reentry
  network_relay_tests::certified_merge_sidecar_close_is_limited_but_responder_controls_are_critical
  network_relay_tests::certified_merge_sidecar_messages_preserve_ingress_reply_route
  tests::relay_fairness::daemon_source_credit_layers_over_upstream_and_preserves_the_ninth_exact_owner
  tests::relay_fairness::saturated_sumeragi_dispatch_does_not_hold_normal_worker_permits
  tests::relay_fairness::real_inner_ingress_retry_preserves_a_copies_and_bounds_b_service_rank
  tests::relay_fairness::same_source_safety_and_shared_high_credits_cross_daemon_without_head_of_line_wait
  tests::relay_fairness::base_one_four_sources_reserve_both_upstream_lanes_without_head_of_line_wait
  tests::relay_fairness::hold_release_same_source_reconnect_retires_old_delivery_without_rebinding_new_route
  tests::relay_fairness::hold_release_preserves_exact_layered_ownership_until_recorded_terminal
  parameters::actual::tests::sumeragi_v2_exact_output_geometry_checks_every_arithmetic_boundary
  parameters::actual::tests::sumeragi_v2_config_format_changes_the_handshake_fingerprint
  parameters::user::duration_clamp_tests::sumeragi_v2_exact_output_geometry_accepts_network_source_boundary
  parameters::user::duration_clamp_tests::sumeragi_v2_exact_output_geometry_accepts_equal_capacity_boundary
  parameters::user::duration_clamp_tests::sumeragi_v2_exact_output_geometry_rejects_unreservable_network_sources
  parameters::user::duration_clamp_tests::sumeragi_authenticated_non_validator_sources_must_fit_network_geometry
  parameters::user::duration_clamp_tests::sumeragi_authenticated_non_validator_sources_use_effective_lane_profile_geometry
)
readonly expected_production_liveness_test_count=826
if (( ${#required_production_liveness_tests[@]} != expected_production_liveness_test_count )); then
  echo "expected exactly ${expected_production_liveness_test_count} production Sumeragi v2 liveness tests, found ${#required_production_liveness_tests[@]}" >&2
  exit 1
fi
production_unit_list="$(run_cargo test --locked --offline -p iroha_core --lib -- --list)"
production_ignored_unit_list="$(
  run_cargo test --locked --offline -p iroha_core --lib -- --list --ignored
)"
production_integration_unit_list="$(
  run_cargo test --locked --offline -p integration_tests --test sumeragi_v2_runner_isolated -- --list
)"
production_integration_ignored_unit_list="$(
  run_cargo test --locked --offline -p integration_tests --test sumeragi_v2_runner_isolated -- --list --ignored
)"
production_data_model_unit_list="$(run_cargo test --locked --offline -p iroha_data_model --lib -- --list)"
production_data_model_ignored_unit_list="$(
  run_cargo test --locked --offline -p iroha_data_model --lib -- --list --ignored
)"
# This source-bound corridor intentionally exercises `iroha_p2p`'s production
# default feature set (`default = []`). Feature-gated QUIC first-packet geometry
# tests remain useful transport regressions, but are not claimed by this
# thirty-eight-module pre-network inventory.
production_p2p_unit_list="$(run_cargo test --locked --offline -p iroha_p2p --lib -- --list)"
production_p2p_ignored_unit_list="$(
  run_cargo test --locked --offline -p iroha_p2p --lib -- --list --ignored
)"
production_irohad_unit_list="$(
  run_cargo test --locked --offline -p irohad --bin irohad --features test-network-message-control -- --list
)"
production_irohad_ignored_unit_list="$(
  run_cargo test --locked --offline -p irohad --bin irohad --features test-network-message-control -- --list --ignored
)"
production_config_unit_list="$(run_cargo test --locked --offline -p iroha_config --lib -- --list)"
production_config_ignored_unit_list="$(
  run_cargo test --locked --offline -p iroha_config --lib -- --list --ignored
)"
multilane_config_runtime_unit_list="$(
  run_cargo test --locked --offline -p iroha_config \
    --test sumeragi_v2_merge_runtime_config -- --list
)"
multilane_config_runtime_ignored_unit_list="$(
  run_cargo test --locked --offline -p iroha_config \
    --test sumeragi_v2_merge_runtime_config -- --list --ignored
)"
multilane_config_fixtures_unit_list="$(
  run_cargo test --locked --offline -p iroha_config --test fixtures -- --list
)"
multilane_config_fixtures_ignored_unit_list="$(
  run_cargo test --locked --offline -p iroha_config --test fixtures -- --list --ignored
)"
production_data_model_modules=(
  block::consensus_v2::finality::tests
  offline::kagemusha_v4_topup_provenance_tests
  block::consensus_v2::tests
)
is_production_data_model_module() {
  local candidate="$1"
  local data_model_module
  for data_model_module in "${production_data_model_modules[@]}"; do
    [[ "$candidate" == "$data_model_module" ]] && return 0
  done
  return 1
}
for required_test in "${required_production_liveness_tests[@]}"; do
  required_unit_list="$production_unit_list"
  required_ignored_unit_list="$production_ignored_unit_list"
  if [[ "$required_test" == sumeragi_v2_runner::* ]]; then
    required_unit_list="$production_integration_unit_list"
    required_ignored_unit_list="$production_integration_ignored_unit_list"
  elif [[ "$required_test" == peer::* || "$required_test" == network::* ]]; then
    required_unit_list="$production_p2p_unit_list"
    required_ignored_unit_list="$production_p2p_ignored_unit_list"
  elif [[ "$required_test" == consensus_message_control::tests::* \
    || "$required_test" == network_relay_tests::* \
    || "$required_test" == tests::relay_fairness::* ]]; then
    required_unit_list="$production_irohad_unit_list"
    required_ignored_unit_list="$production_irohad_ignored_unit_list"
  elif [[ "$required_test" == parameters::* ]]; then
    required_unit_list="$production_config_unit_list"
    required_ignored_unit_list="$production_config_ignored_unit_list"
  else
    required_test_module="${required_test%::*}"
    if is_production_data_model_module "$required_test_module"; then
      required_unit_list="$production_data_model_unit_list"
      required_ignored_unit_list="$production_data_model_ignored_unit_list"
    fi
  fi
  if ! grep -Fqx -- "${required_test}: test" <<<"$required_unit_list"; then
    echo "missing required production Sumeragi v2 liveness test: ${required_test}" >&2
    exit 1
  fi
  if grep -Fqx -- "${required_test}: test" <<<"$required_ignored_unit_list"; then
    echo "required production Sumeragi v2 liveness test is ignored: ${required_test}" >&2
    exit 1
  fi
done

# Keep the multilane closure-critical focused tests explicit even when they do
# not belong to the canonical 826-test liveness inventory above. The later
# source-sealed workspace leg executes these non-ignored tests; this preflight
# prevents a rename, deletion, or accidental `#[ignore]` from hiding behind
# Cargo's successful zero-test filtering.
required_multilane_core_focus_tests=(
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_checked_dequeue_freezes_one_physical_cut_per_occurrence
  sumeragi::v2::tests::deferred_occurrence_capability_binds_direct_authenticated_provenance
  sumeragi::v2_runtime::tests::runtime_rejects_driver_selection_outside_eligible_deferred_owner_set
  sumeragi::v2_runtime::tests::runtime_physical_cut_is_monotone_and_regression_fails_closed
  sumeragi::v2_runtime::tests::deferred_physical_cut_blocks_only_pre_cut_leader_wire_occurrences
  sumeragi::v2_runtime::tests::post_cut_old_logical_replay_cannot_overtake_fenced_busy_deferred_target
  sumeragi::v2_runtime::tests::pre_dequeue_probe_validates_unfrozen_leader_wire_identity
  native_amx::tests::signing_guard_durably_binds_full_source_session_and_participant_incarnation
  native_amx::tests::signing_guard_is_restart_safe_idempotent_and_rejects_body_equivocation
  kura::tests::native_amx_manifest_artifact_rejects_leaf_or_proof_tampering
  kura::tests::native_amx_latest_index_rebuilds_idempotently_after_receipt_append_crash
  kura::tests::native_amx_retention_window_advances_base_and_bounds_index
  kura::tests::native_amx_startup_retention_waits_for_complete_post_wsv_evidence
  kura::tests::native_amx_prepublication_retains_previous_pair_until_post_wsv_cleanup
  kura::tests::native_amx_latest_index_startup_rejects_oversized_append_indexes_before_scanning
  kura::tests::native_amx_latest_index_startup_rejects_oversized_aggregate_data_before_scanning
  kura::tests::native_amx_latest_index_startup_truncates_unindexed_append_tail
  kura::tests::native_amx_latest_index_startup_leaves_missing_evidence_repair_pending
  kura::tests::native_amx_latest_index_startup_rejects_present_invalid_manifest_proof
  kura::tests::native_amx_latest_index_startup_rejects_legacy_v1_filename
  kura::tests::native_amx_latest_index_startup_rejects_fully_unbacked_pointer
  kura::tests::native_amx_latest_index_startup_rejects_manifest_binding_drift_without_receipt
  kura::tests::native_amx_drain_evidence_requires_exact_manifest_receipt_finality_and_latest_index
  kura::tests::native_amx_retirement_scan_rejects_old_incarnation_evidence_after_aba_recreation
  kura::tests::autonomous_claim_inventory_rejects_unexpected_artifacts_before_any_cleanup_or_stage
  kura::tests::autonomous_claim_runtime_inventory_enforces_boundary_without_partial_staging
  kura::tests::autonomous_claim_startup_inventory_bound_fails_before_temp_reconciliation
  kura::tests::autonomous_claim_startup_rejects_symlinks_and_multiple_links_without_following_them
  kura::tests::autonomous_first_attempt_uses_only_versioned_files_and_repairs_missing_pointers
  kura::tests::autonomous_startup_rejects_a_view_removed_after_pointer_publication
  kura::tests::autonomous_startup_rejects_an_aggregate_oversized_attempt_namespace
  kura::tests::autonomous_startup_rejects_an_unretired_same_height_orphan_successor
  kura::tests::lane_block_artifact_recreation_repairs_canonical_slot_and_bounds_retired_history_scan
  kura::tests::latest_lane_block_artifact_scan_counts_sparse_and_malformed_slots_exactly
  kura::tests::pending_queue_plan_admission_bytes_participate_in_exact_disk_accounting
  kura::tests::pending_queue_plan_admission_startup_completes_hard_link_publication
  kura::tests::pending_queue_plan_admission_startup_recovers_unpublished_temporary
  kura::tests::pending_queue_plan_admission_startup_rejects_conflicting_publication_without_deletion
  kura::tests::pending_queue_plan_admission_startup_rejects_oversized_artifact
  kura::tests::pending_queue_plan_admission_store_is_exact_bounded_and_deterministic
  kura::tests::pending_queue_plan_admission_store_rejects_empty_and_oversized_bytes
  kura::tests::pending_queue_plan_admission_store_rejects_symlink_and_unexpected_artifacts
  kura::tests::pending_queue_plan_admission_survives_retired_purge_and_process_reopen
  kura::lane_geometry::tests::first_release_retirement_rejects_obsolete_autonomous_rewrite_without_promotion
  sumeragi::v2_lane_work::tests::native_amx_request_rejects_same_next_height_wrong_coordinator_predecessor_hash
  sumeragi::v2_lane_work::tests::native_participant_recovery_marker_rejects_malformed_height_source_and_aba_shapes
  sumeragi::v2_lane_work::tests::native_participant_recovery_authority_rejects_missing_leaf_execution_and_finality_drift
  sumeragi::v2_lane_work::tests::native_participant_recovery_wire_request_is_certificate_free_and_frame_bounded
  sumeragi::v2_lane_work::tests::native_participant_recovery_response_accepts_valid_carrier_above_autonomous_bundle_bound
  sumeragi::v2_lane_work::tests::historical_recovery_request_rejects_missing_extra_and_tampered_signer_pops
  sumeragi::v2_lane_work::tests::historical_recovery_request_survives_current_state_key_pruning
  sumeragi::v2_lane_work::tests::historical_recovery_request_rejects_stale_incarnation_and_unanchored_view
  sumeragi::v2_core::refinement::tests::in_flight_reservation_kernel_accepts_only_identity_bound_local_owner_steps
  queue::reservation_journal::tests::crash_at_every_operation_frame_write_boundary_is_prefix_atomic
  queue::tests::concurrent_lane_reserve_attempts_cannot_duplicate_one_transaction
  queue::tests::lane_reservation_group_diagnostics_follow_durable_commit_forget_boundary
  kura::tests::committed_merge_reservation_lookup_reconstructs_from_canonical_indexes_after_restart
  kura::tests::merge_frontier_startup_requires_geometry_only_after_committed_execution
  kura::tests::committed_merge_reservation_lookup_fails_closed_on_log_mutation
  kura::tests::committed_merge_reservation_lookup_requires_complete_unique_transaction_index
  kura::lane_geometry::tests::native_amx_archive_is_admissible_accounted_and_purged_without_touching_sibling
  kura::lane_geometry::tests::native_amx_archive_gc_rejects_malformed_truncated_and_oversized_evidence
  kura::lane_geometry::tests::native_amx_archive_gc_rejects_symlinked_evidence_without_following_it
  kura::lane_geometry::tests::zero_file_create_intent_rolls_back_to_a_sealed_image_and_replays
  state::tests::merge_execution_predecessor_rejects_wrong_frontier_hash
  state::tests::merge_execution_canonical_order_is_route_first
  state::tests::autonomous_lane_diagnostic_queue_finalization_is_terminal
  sumeragi::v2_lane_work::tests::autonomous_local_author_reserves_fifo_before_durable_hint_free_publication
  sumeragi::v2_lane_work::tests::autonomous_restart_hydrates_durable_hint_free_payload_and_queue_owner
  sumeragi::v2_apply::tests::native_amx_prepublication_failure_leaves_wsv_unchanged
  sumeragi::v2_apply::tests::live_merge_publication_persists_application_receipt_before_retry
  sumeragi::v2_apply::tests::committed_merge_reservation_is_finalized_exactly_once
  sumeragi::v2_apply::tests::startup_reconciliation_consumes_replayed_committed_merge_reservation
  sumeragi::v2_apply::tests::autonomous_reservation_cross_store_crash_matrix_preserves_fifo_exactly_once
  kura::tests::terminal_frontier_compaction_retains_every_later_pending_slot
  kura::tests::terminal_frontier_compaction_fails_before_replacing_malformed_pending_slot
  kura::tests::terminal_auxiliary_cleanup_resumes_after_each_mutation_budget
  kura::tests::autonomous_lane_slot_retirement_rejects_conflict_and_incarnation_aba
  kura::tests::merge_application_receipt_is_first_release_retirement_admissible_and_fails_closed_after_lane_recreation
  state::tests::certified_autoscale_scale_in_rechecks_late_authenticated_unmerged_relay
  state::tests::certified_autoscale_scale_in_rechecks_late_unapplied_certified_lane_block
  state::tests::certified_autoscale_scale_in_rechecks_late_unrepaired_direct_application_marker
  state::tests::autonomous_lane_diagnostic_same_identity_drift_is_conflict
  state::tests::autonomous_lane_diagnostic_certified_payload_without_bundle_reports_exact_stall
  state::tests::pending_queue_plan_evidence_blocks_every_bound_route_and_classifies_losers
  state::tests::queue_plan_only_carriers_require_exact_committed_active_lane_bindings
  state::tests::queue_plan_registry_presence_is_bounded_and_malformed_markers_fail_closed
  state::tests::queue_plan_registry_staging_is_an_exact_idempotent_compare_and_set
  state::tests::same_carrier_queue_plan_certificate_cannot_authorize_autonomous_execution
  smartcontracts::ivm::host::tests::state_syscalls_cannot_forge_delete_or_disclose_queue_plan_admission_marker
  sumeragi::v2_lane_work::tests::repeated_heartbeat_retries_never_make_autonomous_routes_ordinary_eligible
  sumeragi::v2_runner::tests::deferred_autonomous_work_timeout_arms_only_an_empty_heartbeat
  torii_proxy::tests::torii_transaction_admission_wire_indexes_are_stable
  torii_proxy::tests::queue_plan_synced_request_identity_is_semantic_and_source_bound
  torii_proxy::tests::queue_plan_certificate_rejects_noncanonical_semantic_request_identity
  torii_proxy::tests::obsolete_transaction_admission_tags_fail_closed
  torii_proxy::tests::native_admission_binds_participants_but_uses_only_coordinator_quorum
  torii_proxy::tests::torii_proxy_v5_envelope_roundtrips_exact_request
  torii_proxy::tests::historical_v2_submit_and_network_carrier_cannot_be_accepted_as_v5
  torii_proxy::tests::legacy_v2_submit_bool_wire_cannot_be_accepted_as_v5
  torii_proxy::tests::torii_routing_plan_hint_enforces_native_amx_participant_limit
  torii_proxy::tests::torii_routing_plan_hint_rejects_duplicate_participants_without_deduplication
  torii_proxy::tests::torii_routing_plan_hint_rejects_out_of_order_participants_without_sorting
  kura::tests::configured_pending_control_limits_fail_before_store_creation
  kura::tests::configured_pending_control_count_limits_gate_live_admission
  kura::tests::configured_pending_control_count_limit_rejects_oversized_startup_inventory
  kura::tests::configured_pending_control_shared_bytes_reject_oversized_startup_inventory
  kura::tests::pending_certified_merge_sidecar_is_scoped_to_exact_carrier_round
  kura::tests::pending_certified_merge_work_stops_before_later_malformed_entry
  kura::tests::bounded_pending_merge_hash_scan_filters_orders_and_reports_overflow
  kura::tests::complete_merge_retry_ignores_unrelated_pending_sidecar_capacity
  kura::tests::bounded_pending_merge_selection_skips_committed_prefix_without_underfill
  sumeragi::v2_lane_work::tests::historical_recovery_diagnostics_are_typed_bounded_and_payload_free
  sumeragi::v2_lane_work::tests::native_participant_pruned_carrier_retries_queue_pressure_and_retires_carrier_siblings
  sumeragi::v2_lane_work::tests::merge_leader_candidate_rejects_substitution_outer_epoch_and_oversize_before_journal
)
required_multilane_queue_journal_focus_tests=(
  queue::journal::tests::queue_plan_journal_claim_digest_binds_exact_v4_record_bytes_and_context
  queue::journal::tests::v4_put_rejects_every_noncanonical_admission_context_before_append
  queue::journal::tests::v4_journal_replays_puts_and_exact_removes
  queue::journal::tests::exact_strict_tombstone_removes_once_and_is_idempotent
  queue::journal::tests::exact_strict_tombstone_rejects_stale_plan_without_append
  queue::journal::tests::exact_strict_tombstone_rejects_same_plan_aba_claim_after_compaction_and_restart
  queue::journal::tests::exact_atomic_tombstone_batch_is_one_frame_and_restart_idempotent
  queue::journal::tests::exact_atomic_live_tombstone_batch_rejects_retry_before_append
  queue::journal::tests::exact_atomic_tombstone_batch_rejects_every_later_mismatch_without_append
  queue::journal::tests::exact_atomic_tombstone_batch_parent_sync_failure_replays_whole_frame
  queue::journal::tests::materialized_replay_rejects_later_record_corruption_before_any_callback
  queue::journal::tests::exact_global_binding_tombstone_reconstructs_live_claim_after_restart_and_is_idempotent
  queue::journal::tests::exact_global_binding_tombstone_rejects_wrong_hash_without_append
  queue::journal::tests::exact_global_binding_tombstone_rejects_route_mismatch_and_ordinary_claim
  queue::journal::tests::exact_global_binding_tombstone_rejects_same_plan_aba_after_restart
  queue::journal::tests::exact_global_binding_batch_tombstone_is_atomic_and_constant_scan
  queue::journal::tests::exact_global_binding_batch_rejects_duplicate_absent_and_aba_without_append
  queue::journal::tests::torn_global_binding_remove_batch_replays_all_live_claims
  queue::journal::tests::compaction_recovery_validates_atomic_remove_batch_prefix
  queue::journal::tests::strict_replace_success_shadows_same_key_and_stays_healthy
  queue::journal::tests::strict_replace_forces_preflight_compaction_after_history_reaches_file_limit
  queue::journal::tests::strict_replace_prewrite_failure_is_definitely_not_live_and_healthy
  queue::journal::tests::strict_replace_sync_failure_is_indeterminate_and_replays_new_owner
  queue::journal::tests::strict_replace_parent_sync_failure_is_indeterminate_and_replays_new_owner
  queue::journal::tests::strict_replace_partial_write_is_indeterminate_and_repairs_to_prior_owner
  queue::journal::tests::failed_preflight_compaction_is_definitely_not_live_and_faults_the_journal
  queue::journal::tests::staged_uncommitted_replace_faults_repair_to_prior_owner
  queue::journal::tests::failed_post_durability_compaction_is_indeterminate_and_retains_replacement
  queue::journal::tests::strict_replace_complete_write_ambiguity_replays_new_owner_after_restart
  queue::journal::tests::startup_adopts_and_resynchronizes_complete_presync_remove_before_second_restart
  queue::journal::tests::general_parent_sync_failure_poisoned_until_restart_recovery
  queue::journal::tests::every_recognizable_terminal_v4_prefix_is_repaired_after_valid_frame
  queue::journal::tests::truncate_file_sync_then_parent_failure_is_restart_idempotent
  queue::journal::tests::nonempty_legacy_v1_layout_fails_closed_without_rewrite
  queue::journal::tests::complete_corruption_and_unsupported_versions_fail_without_truncation
  queue::journal::tests::oversized_declared_frame_and_file_fail_before_allocation
  queue::journal::tests::decode_budget_covers_maximum_allocation_dense_instruction_vector
  queue::journal::tests::compressed_frame_is_rejected_before_owned_decompression
  queue::journal::tests::replay_rejects_exactly_one_distinct_identity_above_live_bound
  queue::journal::tests::replay_rejects_transient_distinct_identity_amplification_even_if_final_set_is_small
  queue::journal::tests::compaction_preserves_live_fifo_order_and_uses_v4_frames
  queue::journal::tests::compaction_failure_after_temp_creation_is_reconciled_on_restart
  queue::journal::tests::compaction_rename_then_parent_failure_recovers_replacement_on_restart
  queue::journal::tests::symlinked_or_hardlinked_journal_and_untrusted_compaction_temp_are_rejected
  queue::reservation_journal::tests::complete_v4_bootstrap_is_rejected_without_repair_or_rewrite
  queue::reservation_journal::tests::v5_envelope_rejects_unsupported_record_versions_without_rewrite
  queue::reservation_journal::tests::v5_release_batch_replay_is_atomic_idempotent_and_exact
  queue::reservation_journal::tests::post_sync_append_publication_failure_is_poisoned_and_replayed_on_reopen
  queue::reservation_journal::tests::post_sync_compaction_publication_failure_is_poisoned_and_replayed_on_reopen
  queue::reservation_journal::tests::runtime_commit_requires_live_owner_but_snapshot_recovery_may_restore_commit_barrier
  queue::reservation_journal::tests::prepared_checked_transition_is_bound_to_frame_and_state_generation
  queue::reservation_journal::tests::prepared_checked_transition_rejects_same_generation_cross_state_substitution
  queue::reservation_journal::tests::prepared_checked_transition_binds_exact_ordered_owner_token_coverage
  queue::reservation_journal::tests::checked_transition_result_identity_and_candidate_application_are_atomic
  queue::reservation_journal::tests::checked_transition_generation_overflow_is_rejected_without_mutation
  queue::tests::queue_plan_admission_context_binds_legacy_topology_and_contiguous_generation
  queue::tests::queue_plan_journal_replays_matching_plan_after_restart
  queue::tests::strict_durable_claim_rejects_stale_context_before_ownership_and_binds_exact_record
  queue::tests::strict_durable_claim_retry_survives_height_advance_before_and_after_restart_replay
  queue::tests::strict_durable_claim_rolls_over_to_exact_current_height_and_overlapping_roster
  queue::tests::strict_durable_claim_rollover_recovers_every_replacement_fault_boundary
  queue::tests::strict_durable_claim_retry_does_not_resurrect_removed_queue_ownership
  queue::tests::v4_replay_rejects_same_lane_id_after_incarnation_recreation
  queue::tests::v4_replay_revalidates_original_generation_across_height_advance
  queue::tests::v4_replay_preserves_immutable_admitted_plan_across_router_policy_drift
  queue::tests::strict_queue_plan_journal_admission_replays_exact_transaction_after_restart
  queue::tests::strict_queue_plan_journal_rejects_before_required_startup_installation
  queue::tests::journal_backed_admission_faults_never_acknowledge_and_recover_zero_or_one_owner
  queue::tests::strict_queue_plan_journal_partial_write_is_indeterminate_and_repairs_without_live_record
  queue::tests::strict_queue_plan_journal_sync_failure_is_indeterminate_and_replays_exact_put
  queue::tests::strict_queue_plan_journal_full_write_ambiguity_replays_exact_put
  queue::tests::strict_queue_plan_journal_parent_sync_failure_is_indeterminate_and_faults_queue
  queue::tests::queue_plan_journal_replay_tombstones_only_committed_and_expired_records
  queue::tests::queue_plan_journal_replay_rejects_aggregate_per_user_overflow_without_prefix
  queue::tests::queue_plan_journal_replay_rejects_orphaned_startup_fifo_identity
  queue::tests::exact_queue_plan_rejection_leaves_same_plan_aba_successor_untouched
  queue::tests::strict_global_admission_rejects_self_consistent_noncanonical_request_identity
  queue::tests::globally_bound_guard_drop_restores_exact_fifo_with_absent_registry
  queue::tests::globally_bound_guard_drop_restores_exact_fifo_with_exact_registry
  queue::tests::globally_bound_guard_drop_preserves_claim_for_later_conflict_rejection
  queue::tests::globally_bound_guard_drop_is_terminally_idempotent_across_remove_and_clear_orderings
  queue::tests::globally_bound_guard_drop_restores_during_unwind
  queue::tests::globally_bound_guard_drop_restore_failure_retains_evidence_and_latches_fault
  queue::tests::globally_bound_guard_drop_routing_drift_retains_evidence_and_latches_fault
  queue::tests::queue_plan_journal_install_is_startup_only_and_rejection_does_not_touch_disk
  queue::tests::queue_plan_journal_replay_retains_current_admission_rejection_and_fails_startup
  queue::tests::queue_plan_journal_replay_retains_entrypoint_that_fails_stateless_revalidation
  queue::tests::queue_plan_journal_retains_unverifiable_plan_and_fails_startup
  queue::tests::queue_plan_journal_retains_admitted_plan_when_current_policy_route_lacks_authority
  queue::tests::queue_plan_journal_retains_dataspace_rebind_without_current_authority
  queue::tests::queue_plan_journal_retains_retired_elastic_plan_without_rebinding
  queue::tests::queue_plan_journal_retains_inactive_future_autoscale_plan_fail_closed
  queue::tests::queue_plan_journal_retains_inactive_native_amx_participant_without_rebinding
  queue::tests::lane_reservation_scope_accepts_only_canonical_single_lane_when_nexus_is_disabled
  queue::tests::global_candidate_lease_excludes_autonomous_reservation_until_exact_drop
  queue::tests::stale_reservation_commit_digest_cannot_tombstone_or_forget_live_plan
  queue::tests::stale_reservation_commit_binding_cannot_tombstone_or_forget_live_plan
  queue::tests::installed_plan_journal_reconciles_high_volume_commit_barriers_and_restarts_cleanly
  queue::tests::reservation_restart_fits_ordinary_fifo_around_middle_anchor
  queue::tests::committed_state_with_live_reservation_retains_sole_plan_payload_source
  queue::tests::expired_live_reservation_replays_payload_without_fifo_or_tombstone
  queue::tests::restart_commit_barrier_rejects_mismatched_queue_hash_without_tombstone_or_forget
  queue::tests::restart_commit_barrier_rejects_retargeted_coordinator_without_tombstone_or_forget
  queue::tests::restart_commit_barrier_rejects_same_plan_binding_aba_without_tombstone_or_forget
  queue::tests::restart_after_plan_tombstone_before_forget_commit_is_idempotent
  queue::tests::plan_install_commit_barrier_reconciliation_refreshes_backpressure_snapshot
  queue::tests::globally_bound_reservation_survives_expiry_until_canonical_commit
  queue::tests::lane_reservation_group_diagnostics_rechecks_fault_after_store_lock_handoff
  queue::tests::lane_pending_work_rechecks_durability_fault_after_queue_lock_handoff
  queue::tests::ambiguous_terminal_reservation_appends_fail_closed_for_diagnostics_and_drain
  queue::tests::ambiguous_reservation_compaction_fails_closed_after_terminal_application
  queue::tests::install_replay_reconciliation_fault_publishes_backpressure_after_unlock
  queue::tests::reservation_journal_install_rejects_selection_publication_window
  queue::tests::second_reservation_journal_installer_cannot_touch_its_losing_path
  queue::tests::concurrent_reservation_journal_installers_publish_one_untouched_winner
  queue::tests::missing_replayed_reservation_owns_capacity_until_exact_payload_replay
  queue::tests::missing_replayed_reservation_owns_retained_budget_until_exact_payload_replay
  queue::reservation_journal::tests::configured_frame_limit_rejects_valid_oversized_payload_before_replay
  queue::reservation_journal::tests::configured_file_limit_rejects_oversized_startup_journal_before_scan
  queue::reservation_journal::tests::replay_rejects_more_distinct_owners_than_configured_queue_capacity
  queue::reservation_journal::tests::invalid_runtime_limits_fail_before_creating_a_journal
  queue::reservation_journal::tests::every_execution_group_rejects_4097_members_before_mutating_replay_state
  queue::reservation_journal::tests::every_snapshot_top_level_vector_obeys_configured_owner_capacity
  queue::reservation_journal::tests::runtime_owner_limit_rejects_before_write_and_remains_reopenable
  queue::reservation_journal::tests::stale_forget_release_does_not_undercount_completed_ownership
  queue::reservation_journal::tests::replay_rejects_same_length_valid_content_mutation_on_retained_append_handle
  queue::reservation_journal::tests::ownership_limit_is_checked_before_applying_the_exceeding_prefix
  queue::reservation_journal::tests::ownership_union_counts_tombstones_and_completed_releases_exactly_once
  queue::reservation_journal::tests::deterministic_file_budget_exhaustion_does_not_poison_or_extend_journal
  queue::reservation_journal::tests::decoder_allocation_ceiling_tracks_configured_frame_budget
  queue::reservation_journal::tests::compaction_preserves_valid_owner_state_larger_than_one_execution_group
  queue::reservation_journal::tests::journal_exclusive_owner_lock_blocks_a_second_runtime
  queue::reservation_journal::tests::cached_revision_rejects_an_unlocked_same_length_external_rewrite
  queue::reservation_journal::tests::indexed_replay_matches_reference_vector_transitions_and_ordering
  queue::reservation_journal::tests::indexed_transition_rejections_are_atomic_and_match_reference_replay
  queue::reservation_journal::tests::runtime_semantic_preflight_rejects_invalid_frames_before_durable_append
  queue::reservation_journal::tests::production_replay_handles_many_singleton_frames_with_exact_order
)
required_multilane_data_model_focus_tests=(
  block::consensus::tests::native_amx_grouped_receipts_reject_order_bounds_and_same_route_drift
  block::consensus::tests::native_amx_mixed_role_marker_defers_only_separate_participant_anchor
  block::consensus::tests::native_amx_receipt_negative_corpus_fails_closed
  block::consensus::tests::native_amx_application_evidence_negative_corpus_fails_closed
  block::consensus::tests::native_amx_v2_grouped_participant_settlement_rejects_invalid_source_groups
  block::consensus::tests::autonomous_lane_execution_diagnostics_roundtrip_order_and_bound
  block::consensus::tests::autonomous_lane_execution_conflict_is_explicit_and_fail_closed
  block::consensus_v2::tests::execution_commitment_enforces_native_amx_manifest_shape_and_bound
)
required_multilane_torii_focus_tests=(
  tests_runtime_handlers::torii_proxy_v5_roundtrip_and_forwarding_preserve_transaction_admission_binding
  tests_runtime_handlers::queue_plan_synced_reconciliation_hash_matches_accepted_queue_identity
  tests_runtime_handlers::incoming_submit_queue_plan_synced_without_journal_is_stably_unavailable
  tests_runtime_handlers::incoming_submit_queue_plan_synced_succeeds_with_installed_journal
  tests_runtime_handlers::incoming_torii_proxy_rejects_every_non_v5_schema_before_dispatch
  tests_runtime_handlers::queue_plan_outcome_unknown_survives_both_retryable_completion_orders
  tests_runtime_handlers::queue_plan_outcome_unknown_dominates_nonretryable_failure_in_both_completion_orders
  tests_runtime_handlers::queue_plan_outcome_unknown_rejects_forged_reconciliation_hash
  tests_runtime_handlers::queue_plan_synced_post_dispatch_loss_is_exactly_indeterminate_for_each_transport
  tests_runtime_handlers::queue_plan_synced_p2p_missing_network_is_pre_dispatch
  tests_runtime_handlers::queue_plan_synced_p2p_post_dispatch_channel_loss_is_indeterminate
  tests_runtime_handlers::queue_plan_synced_http_connect_loss_is_pre_dispatch_and_body_loss_is_post_dispatch
  tests_runtime_handlers::queue_plan_synced_http_redirect_to_connect_loss_is_not_followed
  tests_runtime_handlers::queue_plan_synced_before_dispatch_failure_remains_definitely_unavailable
  tests_runtime_handlers::queue_plan_synced_accepts_only_exact_durable_acceptance_evidence
  tests_runtime_handlers::incoming_submit_queue_plan_synced_fails_closed_when_local_peer_and_signer_diverge
  tests_runtime_handlers::queue_plan_synced_accepts_a_reforwarded_certificate_from_an_authoritative_peer
  tests_runtime_handlers::proxied_transaction_submission_preserves_public_accept_and_prefer_contracts
  tests_runtime_handlers::queue_plan_synced_post_admission_or_malformed_500_is_indeterminate
  tests_runtime_handlers::torii_proxy_response_body_limit_caps_hosted_http_and_strict_receipts
  tests_runtime_handlers::queue_plan_synced_certificate_requires_canonical_distinct_authority_quorum
  tests_runtime_handlers::queue_plan_synced_first_quorum_never_waits_for_pending_equivocation_evidence
  tests_runtime_handlers::queue_plan_synced_candidates_use_exact_bound_roster_and_count_local_quorum
  tests_runtime_handlers::queue_plan_synced_real_local_journal_receipt_combines_with_remote_quorum_receipt
  tests_runtime_handlers::queue_plan_synced_admission_context_rejects_height_plan_leg_incarnation_roster_and_threshold_drift
  tests_runtime_handlers::queue_plan_synced_response_bounds_reject_headers_body_encoding_and_decode_amplification
  tests_runtime_handlers::incoming_torii_proxy_rejects_malformed_v4_hop_chain_before_dispatch
  tests_runtime_handlers::queue_plan_synced_certificate_binds_exact_durable_journal_claim
  tests_runtime_handlers::completed_torii_proxy_requests_are_fifo_bounded_and_ttl_pruned
  tests_runtime_handlers::queue_plan_synced_registry_timeout_never_returns_accepted
  tests_runtime_handlers::incoming_queue_plan_synced_exact_retry_survives_height_and_roster_advance
  tests_runtime_handlers::incoming_queue_plan_synced_historical_context_without_owned_claim_fails_closed
  operator_signatures::tests::torii_proxy_signatures_accept_unlisted_peer_keys_without_operator_privileges
  operator_signatures::tests::torii_proxy_signatures_reject_wrong_or_tampered_target
  operator_signatures::tests::torii_proxy_signatures_bind_canonical_request_freshness_and_reject_replay
  operator_signatures::tests::operator_identity_bound_and_torii_proxy_replay_caches_are_partitioned
  operator_signatures::tests::torii_proxy_middleware_exposes_authenticated_unlisted_peer_identity
  router::builder::tests::torii_proxy_peer_witness_is_sealed_as_identity_bound_authentication
  tests_queue_metadata::queue_plan_journal_outcome_unknown_has_stable_code_and_exact_hash
)
required_multilane_torii_shared_focus_tests=(
  route_catalog::tests::internal_torii_proxy_is_the_only_identity_bound_operator_route
)
required_multilane_integration_lib_focus_tests=(
  sandbox::tests::sandboxed_network_start_helper_preserves_optional_developer_skip
  sandbox::tests::sandboxed_network_start_helper_fails_required_release_scenario
)
required_multilane_config_lib_focus_tests=(
  parameters::actual::tests::sumeragi_v2_shared_config_defaults_are_finite_and_deterministic
  parameters::actual::tests::sumeragi_v2_shared_fingerprint_binds_every_runtime_category
  parameters::actual::tests::sumeragi_v2_config_rejects_merge_runtime_limit_boundaries
)
required_multilane_config_runtime_focus_tests=(
  every_merge_runtime_override_reaches_the_actual_config
  tight_valid_merge_runtime_geometry_is_admitted
)
required_multilane_config_fixtures_focus_tests=(
  minimal_config_snapshot
  retired_plan_journal_toggle_fails_during_config_parse_before_runtime_storage
)
readonly expected_multilane_focus_test_count=309
if (( ${#required_multilane_core_focus_tests[@]}
    + ${#required_multilane_queue_journal_focus_tests[@]}
    + ${#required_multilane_config_lib_focus_tests[@]}
    + ${#required_multilane_config_runtime_focus_tests[@]}
    + ${#required_multilane_config_fixtures_focus_tests[@]}
    + ${#required_multilane_data_model_focus_tests[@]}
    + ${#required_multilane_torii_focus_tests[@]}
    + ${#required_multilane_torii_shared_focus_tests[@]}
    + ${#required_multilane_integration_lib_focus_tests[@]}
    != expected_multilane_focus_test_count )); then
  echo "expected exactly ${expected_multilane_focus_test_count} multilane focus tests" >&2
  exit 1
fi
for required_test in "${required_multilane_core_focus_tests[@]}"; do
  if ! grep -Fqx -- "${required_test}: test" <<<"$production_unit_list"; then
    echo "missing required multilane iroha_core focus test: ${required_test}" >&2
    exit 1
  fi
  if grep -Fqx -- "${required_test}: test" <<<"$production_ignored_unit_list"; then
    echo "required multilane iroha_core focus test is ignored: ${required_test}" >&2
    exit 1
  fi
done
for required_test in "${required_multilane_queue_journal_focus_tests[@]}"; do
  if ! grep -Fqx -- "${required_test}: test" <<<"$production_unit_list"; then
    echo "missing required multilane queue-journal focus test: ${required_test}" >&2
    exit 1
  fi
  if grep -Fqx -- "${required_test}: test" <<<"$production_ignored_unit_list"; then
    echo "required multilane queue-journal focus test is ignored: ${required_test}" >&2
    exit 1
  fi
done
for required_test in "${required_multilane_config_lib_focus_tests[@]}"; do
  if ! grep -Fqx -- "${required_test}: test" <<<"$production_config_unit_list"; then
    echo "missing required multilane iroha_config library focus test: ${required_test}" >&2
    exit 1
  fi
  if grep -Fqx -- "${required_test}: test" <<<"$production_config_ignored_unit_list"; then
    echo "required multilane iroha_config library focus test is ignored: ${required_test}" >&2
    exit 1
  fi
done
for required_test in "${required_multilane_config_runtime_focus_tests[@]}"; do
  if ! grep -Fqx -- "${required_test}: test" <<<"$multilane_config_runtime_unit_list"; then
    echo "missing required multilane iroha_config runtime focus test: ${required_test}" >&2
    exit 1
  fi
  if grep -Fqx -- "${required_test}: test" \
    <<<"$multilane_config_runtime_ignored_unit_list"; then
    echo "required multilane iroha_config runtime focus test is ignored: ${required_test}" >&2
    exit 1
  fi
done
for required_test in "${required_multilane_config_fixtures_focus_tests[@]}"; do
  if ! grep -Fqx -- "${required_test}: test" <<<"$multilane_config_fixtures_unit_list"; then
    echo "missing required multilane iroha_config fixture focus test: ${required_test}" >&2
    exit 1
  fi
  if grep -Fqx -- "${required_test}: test" \
    <<<"$multilane_config_fixtures_ignored_unit_list"; then
    echo "required multilane iroha_config fixture focus test is ignored: ${required_test}" >&2
    exit 1
  fi
done
for required_test in "${required_multilane_data_model_focus_tests[@]}"; do
  if ! grep -Fqx -- "${required_test}: test" <<<"$production_data_model_unit_list"; then
    echo "missing required multilane iroha_data_model focus test: ${required_test}" >&2
    exit 1
  fi
  if grep -Fqx -- "${required_test}: test" <<<"$production_data_model_ignored_unit_list"; then
    echo "required multilane iroha_data_model focus test is ignored: ${required_test}" >&2
    exit 1
  fi
done
multilane_torii_unit_list="$(
  run_cargo test --locked --offline -p iroha_torii --lib -- --list
)"
multilane_torii_ignored_unit_list="$(
  run_cargo test --locked --offline -p iroha_torii --lib -- --list --ignored
)"
for required_test in "${required_multilane_torii_focus_tests[@]}"; do
  if ! grep -Fqx -- "${required_test}: test" <<<"$multilane_torii_unit_list"; then
    echo "missing required multilane iroha_torii focus test: ${required_test}" >&2
    exit 1
  fi
  if grep -Fqx -- "${required_test}: test" <<<"$multilane_torii_ignored_unit_list"; then
    echo "required multilane iroha_torii focus test is ignored: ${required_test}" >&2
    exit 1
  fi
done
multilane_torii_shared_unit_list="$(
  run_cargo test --locked --offline -p iroha_torii_shared --lib -- --list
)"
multilane_torii_shared_ignored_unit_list="$(
  run_cargo test --locked --offline -p iroha_torii_shared --lib -- --list --ignored
)"
for required_test in "${required_multilane_torii_shared_focus_tests[@]}"; do
  if ! grep -Fqx -- "${required_test}: test" <<<"$multilane_torii_shared_unit_list"; then
    echo "missing required multilane iroha_torii_shared focus test: ${required_test}" >&2
    exit 1
  fi
  if grep -Fqx -- "${required_test}: test" \
    <<<"$multilane_torii_shared_ignored_unit_list"; then
    echo "required multilane iroha_torii_shared focus test is ignored: ${required_test}" >&2
    exit 1
  fi
done
multilane_integration_lib_unit_list="$(
  run_cargo test --locked --offline -p integration_tests --lib -- --list
)"
multilane_integration_lib_ignored_unit_list="$(
  run_cargo test --locked --offline -p integration_tests --lib -- --list --ignored
)"
for required_test in "${required_multilane_integration_lib_focus_tests[@]}"; do
  if ! grep -Fqx -- "${required_test}: test" <<<"$multilane_integration_lib_unit_list"; then
    echo "missing required multilane integration helper test: ${required_test}" >&2
    exit 1
  fi
  if grep -Fqx -- "${required_test}: test" \
    <<<"$multilane_integration_lib_ignored_unit_list"; then
    echo "required multilane integration helper test is ignored: ${required_test}" >&2
    exit 1
  fi
done

run_multilane_focus_crate_tests() {
  local package="$1"
  shift
  local focus_test
  for focus_test in "$@"; do
    run_cargo test --locked --offline -p "$package" --lib \
      "$focus_test" -- --exact --test-threads=1 || return
  done
}

run_multilane_focus_test_target() {
  local package="$1"
  local test_target="$2"
  shift 2
  local focus_test
  for focus_test in "$@"; do
    run_cargo test --locked --offline -p "$package" --test "$test_target" \
      "$focus_test" -- --exact --test-threads=1 || return
  done
}

append_g_unit_inventory() {
  local leg_id="$1"
  local package="$2"
  shift 2
  local focus_test
  for focus_test in "$@"; do
    printf '%s\t%s\t%s\n' "$leg_id" "$package" "$focus_test" \
      >>"$corridor_g_unit_inventory"
  done
}

require_g_unit_log_results() {
  local focus_test
  for focus_test in "$@"; do
    if [[ "$(grep -Fxc -- "test ${focus_test} ... ok" "$corridor_last_log" || true)" != 1 ]]; then
      echo "G-UNIT corridor log does not contain one passing result for ${focus_test}" >&2
      return 1
    fi
  done
}

# G-UNIT is an execution receipt, not a name-only inventory. Each crate-bound
# leg invokes every exact non-ignored focus test above and archives one
# unambiguous one-test Cargo transcript per entry. The canonical 309-row TSV is
# hashed into the corridor completion and independently revalidated by the
# aggregate receipt writer.
if ((corridor_enabled)); then
  printf '%s\n' $'leg_id\tcrate\ttest' >"$corridor_g_unit_inventory"

  append_g_unit_inventory \
    g-unit-iroha-core iroha_core "${required_multilane_core_focus_tests[@]}"
  run_corridor_leg \
    g-unit-iroha-core cargo-focus "${#required_multilane_core_focus_tests[@]}" \
    'for test in required_multilane_core_focus_tests; do cargo test --locked --offline -p iroha_core --lib "$test" -- --exact --test-threads=1; done' \
    run_multilane_focus_crate_tests \
      iroha_core "${required_multilane_core_focus_tests[@]}"
  require_g_unit_log_results "${required_multilane_core_focus_tests[@]}"

  append_g_unit_inventory \
    g-unit-iroha-core-queue-journal iroha_core \
    "${required_multilane_queue_journal_focus_tests[@]}"
  run_corridor_leg \
    g-unit-iroha-core-queue-journal cargo-focus \
    "${#required_multilane_queue_journal_focus_tests[@]}" \
    'for test in required_multilane_queue_journal_focus_tests; do cargo test --locked --offline -p iroha_core --lib "$test" -- --exact --test-threads=1; done' \
    run_multilane_focus_crate_tests \
      iroha_core "${required_multilane_queue_journal_focus_tests[@]}"
  require_g_unit_log_results \
    "${required_multilane_queue_journal_focus_tests[@]}"

  append_g_unit_inventory \
    g-unit-iroha-config-lib iroha_config \
    "${required_multilane_config_lib_focus_tests[@]}"
  run_corridor_leg \
    g-unit-iroha-config-lib cargo-focus \
    "${#required_multilane_config_lib_focus_tests[@]}" \
    'for test in required_multilane_config_lib_focus_tests; do cargo test --locked --offline -p iroha_config --lib "$test" -- --exact --test-threads=1; done' \
    run_multilane_focus_crate_tests \
      iroha_config "${required_multilane_config_lib_focus_tests[@]}"
  require_g_unit_log_results \
    "${required_multilane_config_lib_focus_tests[@]}"

  append_g_unit_inventory \
    g-unit-iroha-config-runtime iroha_config \
    "${required_multilane_config_runtime_focus_tests[@]}"
  run_corridor_leg \
    g-unit-iroha-config-runtime cargo-focus \
    "${#required_multilane_config_runtime_focus_tests[@]}" \
    'for test in required_multilane_config_runtime_focus_tests; do cargo test --locked --offline -p iroha_config --test sumeragi_v2_merge_runtime_config "$test" -- --exact --test-threads=1; done' \
    run_multilane_focus_test_target \
      iroha_config sumeragi_v2_merge_runtime_config \
      "${required_multilane_config_runtime_focus_tests[@]}"
  require_g_unit_log_results \
    "${required_multilane_config_runtime_focus_tests[@]}"

  append_g_unit_inventory \
    g-unit-iroha-config-fixtures iroha_config \
    "${required_multilane_config_fixtures_focus_tests[@]}"
  run_corridor_leg \
    g-unit-iroha-config-fixtures cargo-focus \
    "${#required_multilane_config_fixtures_focus_tests[@]}" \
    'for test in required_multilane_config_fixtures_focus_tests; do cargo test --locked --offline -p iroha_config --test fixtures "$test" -- --exact --test-threads=1; done' \
    run_multilane_focus_test_target \
      iroha_config fixtures "${required_multilane_config_fixtures_focus_tests[@]}"
  require_g_unit_log_results \
    "${required_multilane_config_fixtures_focus_tests[@]}"

  append_g_unit_inventory \
    g-unit-iroha-data-model iroha_data_model \
    "${required_multilane_data_model_focus_tests[@]}"
  run_corridor_leg \
    g-unit-iroha-data-model cargo-focus \
    "${#required_multilane_data_model_focus_tests[@]}" \
    'for test in required_multilane_data_model_focus_tests; do cargo test --locked --offline -p iroha_data_model --lib "$test" -- --exact --test-threads=1; done' \
    run_multilane_focus_crate_tests \
      iroha_data_model "${required_multilane_data_model_focus_tests[@]}"
  require_g_unit_log_results "${required_multilane_data_model_focus_tests[@]}"

  append_g_unit_inventory \
    g-unit-iroha-torii iroha_torii "${required_multilane_torii_focus_tests[@]}"
  run_corridor_leg \
    g-unit-iroha-torii cargo-focus "${#required_multilane_torii_focus_tests[@]}" \
    'for test in required_multilane_torii_focus_tests; do cargo test --locked --offline -p iroha_torii --lib "$test" -- --exact --test-threads=1; done' \
    run_multilane_focus_crate_tests \
      iroha_torii "${required_multilane_torii_focus_tests[@]}"
  require_g_unit_log_results "${required_multilane_torii_focus_tests[@]}"

  append_g_unit_inventory \
    g-unit-iroha-torii-shared iroha_torii_shared \
    "${required_multilane_torii_shared_focus_tests[@]}"
  run_corridor_leg \
    g-unit-iroha-torii-shared cargo-focus \
    "${#required_multilane_torii_shared_focus_tests[@]}" \
    'for test in required_multilane_torii_shared_focus_tests; do cargo test --locked --offline -p iroha_torii_shared --lib "$test" -- --exact --test-threads=1; done' \
    run_multilane_focus_crate_tests \
      iroha_torii_shared "${required_multilane_torii_shared_focus_tests[@]}"
  require_g_unit_log_results \
    "${required_multilane_torii_shared_focus_tests[@]}"

  append_g_unit_inventory \
    g-unit-integration-tests integration_tests \
    "${required_multilane_integration_lib_focus_tests[@]}"
  run_corridor_leg \
    g-unit-integration-tests cargo-focus \
    "${#required_multilane_integration_lib_focus_tests[@]}" \
    'for test in required_multilane_integration_lib_focus_tests; do cargo test --locked --offline -p integration_tests --lib "$test" -- --exact --test-threads=1; done' \
    run_multilane_focus_crate_tests \
      integration_tests "${required_multilane_integration_lib_focus_tests[@]}"
  require_g_unit_log_results \
    "${required_multilane_integration_lib_focus_tests[@]}"

  if [[ "$(wc -l <"$corridor_g_unit_inventory" | tr -d '[:space:]')" != 310 ]]; then
    echo "G-UNIT inventory must contain one header and exactly 309 focused tests" >&2
    exit 1
  fi
fi

# These are the real-network four-peer acceptance gates. Keep their exact
# harness/name inventory source-bound and non-ignored even though ordinary
# developer runs may opt out inside the test body.
readonly multilane_autoscale_four_peer_release_test="nexus::autoscale_localnet::nexus_autoscale_four_peer_release_lifecycle_recreates_lane_and_rejects_stale_artifacts"
readonly multilane_autoscale_restart_release_test="nexus::autoscale_localnet::nexus_autoscale_certified_merge_recovers_missing_sidecar_after_restart"
readonly multilane_autoscale_drain_release_test="nexus::autoscale_localnet::nexus_autoscale_two_phase_drain_closes_certifies_then_retires_after_restart"
readonly multilane_native_amx_rotating_release_test="native_amx_rotating_validator_fault_soak_preserves_independent_participant_qcs"
readonly multilane_native_amx_grouped_pruning_marker="[multilane-release-native-evidence] grouped_sources=2 durable_manifest=passed body_eviction_recovery=passed authenticated_remote_recovery=passed exact_once=passed"
if [[ "$(grep -Fxc -- "readonly NATIVE_AMX_GROUPED_PRUNING_MARKER=\"${multilane_native_amx_grouped_pruning_marker}\"" scripts/run_nexus_cross_dataspace_atomic_swap.sh || true)" != 1 ]]; then
  echo "mandatory four-peer launcher is not source-bound to grouped Native AMX pruning evidence" >&2
  exit 1
fi
multilane_nexus_release_test_list="$(
  run_cargo test --locked --offline -p integration_tests --test nexus_and_streaming -- --list
)"
multilane_nexus_release_ignored_test_list="$(
  run_cargo test --locked --offline -p integration_tests --test nexus_and_streaming -- --list --ignored
)"
multilane_native_release_test_list="$(
  run_cargo test --locked --offline -p integration_tests --test native_amx_routing -- --list
)"
multilane_native_release_ignored_test_list="$(
  run_cargo test --locked --offline -p integration_tests --test native_amx_routing -- --list --ignored
)"
for required_test_spec in \
  "nexus_and_streaming|${multilane_autoscale_four_peer_release_test}" \
  "nexus_and_streaming|${multilane_autoscale_restart_release_test}" \
  "nexus_and_streaming|${multilane_autoscale_drain_release_test}" \
  "native_amx_routing|${multilane_native_amx_rotating_release_test}"; do
  IFS='|' read -r required_target required_test <<<"$required_test_spec"
  if [[ "$required_target" == "nexus_and_streaming" ]]; then
    required_test_list="$multilane_nexus_release_test_list"
    required_ignored_test_list="$multilane_nexus_release_ignored_test_list"
  else
    required_test_list="$multilane_native_release_test_list"
    required_ignored_test_list="$multilane_native_release_ignored_test_list"
  fi
  if [[ "$(grep -Fxc -- "${required_test}: test" <<<"$required_test_list" || true)" != 1 ]]; then
    echo "missing or renamed mandatory four-peer multilane release test: ${required_test}" >&2
    exit 1
  fi
  if grep -Fqx -- "${required_test}: test" <<<"$required_ignored_test_list"; then
    echo "mandatory four-peer multilane release test is ignored: ${required_test}" >&2
    exit 1
  fi
done

# The source-binding checker derives this same ordered corpus from the formal
# ledger. Keep an independent fixed release count and terminal contract marker
# so the runner and ledger cannot silently agree to drop a mutation together.
readonly expected_multilane_formal_mutation_count=37
observed_multilane_formal_mutation_count="$(
  grep -Ec '^run_mutant [a-z0-9-]+ ' \
    scripts/formal/run_sumeragi_v2_multilane_mutations.sh
)"
if ((observed_multilane_formal_mutation_count
    != expected_multilane_formal_mutation_count)); then
  echo "expected exactly ${expected_multilane_formal_mutation_count} multilane formal mutations, found ${observed_multilane_formal_mutation_count}" >&2
  exit 1
fi
if ! grep -Fqx -- \
  'echo "[tlc] all 37 multilane mutations produced their exact named counterexamples; no deductive proof status was changed"' \
  scripts/formal/run_sumeragi_v2_multilane_mutations.sh; then
  echo "multilane mutation runner lacks the exact 37-mutation completion contract" >&2
  exit 1
fi

readonly expected_typed_rollover_formal_mutation_count=45
observed_typed_rollover_formal_mutation_count="$(
  grep -Ec '^  "[a-z0-9-]+\|typed_rollover_handoff_[a-z0-9_]+_bug[.]cfg\|(12|13)\|\$\{(INVARIANT|TEMPORAL)_MARKER\}"$|^run_case repeated-handoff-after-restart-restore \\$' \
    scripts/formal/run_sumeragi_v2_typed_rollover_handoff_mutations.sh
)"
if ((observed_typed_rollover_formal_mutation_count
    != expected_typed_rollover_formal_mutation_count)); then
  echo "expected exactly ${expected_typed_rollover_formal_mutation_count} typed rollover formal mutations, found ${observed_typed_rollover_formal_mutation_count}" >&2
  exit 1
fi
if ! grep -Fqx -- \
  'echo "[tlc] typed rollover-handoff repaired models and 45-mutant root-anchored V3 matrix passed"' \
  scripts/formal/run_sumeragi_v2_typed_rollover_handoff_mutations.sh; then
  echo "typed rollover mutation runner lacks the exact 45-mutation completion contract" >&2
  exit 1
fi

production_liveness_modules=(
  kura::tests
  kura::lane_geometry::tests
  nexus::lane_relay::tests
  sumeragi::authoritative_runtime_gate_tests
  merge_sidecar::tests
  sumeragi::v2_core::tests
  sumeragi::v2_core::refinement::tests
  sumeragi::v2_core::wal::byte_lifecycle_tests
  sumeragi::v2_core::reducer::source_link_tests
  sumeragi::evidence::tests
  sumeragi::serviced_candidate_store::tests
  sumeragi::v2::tests
  sumeragi::v2_body_store::tests
  sumeragi::v2_block_sync::tests
  sumeragi::v2_apply::tests
  sumeragi::v2_effects::tests
  sumeragi::v2_lane_work::tests
  sumeragi::v2_runtime::tests
  sumeragi::v2_transport::tests
  sumeragi::v2_recovery::tests
  sumeragi::v2_runner::tests
  sumeragi::v2_worker::tests
  sumeragi::status::v2_liveness_watchdog_tests
  zk::kagemusha_finality::tests
  block::consensus_v2::finality::tests
  offline::kagemusha_v4_topup_provenance_tests
  block::consensus_v2::tests
  sumeragi_v2_runner
  peer::run::tests
  peer::shared_byte_budget_tests
  network::tests
  network::inbound_source_memory_bound_tests
  network::handle_update_tests
  consensus_message_control::tests
  network_relay_tests
  tests::relay_fairness
  parameters::actual::tests
  parameters::user::duration_clamp_tests
)
production_liveness_leg_ids=(
  production-kura-progress-durability
  production-kura-lane-geometry
  production-lane-relay-exact-ownership
  production-authoritative-ingress
  production-merge-sidecar
  production-v2-core
  production-v2-core-refinement
  production-v2-core-wal
  production-v2-core-source-link
  production-v2-equivocation-evidence
  production-v2-leader-wire-lifecycle-store
  production-v2-adapter
  production-v2-body-store
  production-v2-block-sync
  production-v2-apply
  production-v2-effects
  production-v2-lane-work
  production-v2-runtime
  production-v2-transport
  production-v2-recovery
  production-v2-runner
  production-v2-worker
  production-v2-watchdog
  production-kagemusha-finality
  production-data-model-v2-finality
  production-data-model-offline-compact-qc
  production-data-model-v2-context-identity
  production-v2-integration-runner
  production-p2p-peer-reliable-flush
  production-p2p-shared-source-byte-geometry
  production-p2p-network-reliable-actor
  production-p2p-source-memory-geometry
  production-p2p-waiter-rank-geometry
  production-irohad-consensus-message-control
  production-irohad-network-relay
  production-irohad-authenticated-via
  production-config-v2-exact-output-geometry
  production-config-v2-exact-output-root-parse
)
if ((corridor_enabled)); then
  printf '%s\n' $'module\ttest' >"$corridor_required_tests"
  for required_test in "${required_production_liveness_tests[@]}"; do
    matched_module=""
    for module in "${production_liveness_modules[@]}"; do
      if [[ "$required_test" == "${module}::"* ]]; then
        matched_module="$module"
        break
      fi
    done
    if [[ -z "$matched_module" ]]; then
      echo "required production test has no corridor module: ${required_test}" >&2
      exit 1
    fi
    printf '%s\t%s\n' "$matched_module" "$required_test" >>"$corridor_required_tests"
  done
fi
for module_index in "${!production_liveness_modules[@]}"; do
  module="${production_liveness_modules[$module_index]}"
  module_leg_id="${production_liveness_leg_ids[$module_index]}"
  module_required_count=0
  for required_test in "${required_production_liveness_tests[@]}"; do
    [[ "$required_test" == "${module}::"* ]] && ((module_required_count += 1))
  done
  if [[ "$module" == sumeragi_v2_runner ]]; then
    module_command="cargo test --locked --offline -p integration_tests --test sumeragi_v2_runner_isolated sumeragi_v2_runner::prepare_qc_split_tests -- --test-threads=1"
    run_corridor_leg \
      "$module_leg_id" cargo-module "$module_required_count" "$module_command" \
      run_cargo test --locked --offline -p integration_tests --test sumeragi_v2_runner_isolated \
        sumeragi_v2_runner::prepare_qc_split_tests -- --test-threads=1
  elif [[ "$module" == peer::run::tests \
    || "$module" == peer::shared_byte_budget_tests \
    || "$module" == network::tests \
    || "$module" == network::inbound_source_memory_bound_tests \
    || "$module" == network::handle_update_tests ]]; then
    module_command="cargo test --locked --offline -p iroha_p2p --lib ${module} -- --test-threads=1"
    run_corridor_leg \
      "$module_leg_id" cargo-module "$module_required_count" "$module_command" \
      run_cargo test --locked --offline -p iroha_p2p --lib "$module" -- --test-threads=1
  elif [[ "$module" == consensus_message_control::tests \
    || "$module" == network_relay_tests \
    || "$module" == tests::relay_fairness ]]; then
    module_command="cargo test --locked --offline -p irohad --bin irohad --features test-network-message-control ${module} -- --test-threads=1"
    run_corridor_leg \
      "$module_leg_id" cargo-module "$module_required_count" "$module_command" \
      run_cargo test --locked --offline -p irohad --bin irohad --features test-network-message-control \
        "$module" -- --test-threads=1
  elif [[ "$module" == parameters::* ]]; then
    module_command="cargo test --locked --offline -p iroha_config --lib ${module} -- --test-threads=1"
    run_corridor_leg \
      "$module_leg_id" cargo-module "$module_required_count" "$module_command" \
      run_cargo test --locked --offline -p iroha_config --lib "$module" -- --test-threads=1
  elif is_production_data_model_module "$module"; then
    module_command="cargo test --locked --offline -p iroha_data_model --lib ${module} -- --test-threads=1"
    run_corridor_leg \
      "$module_leg_id" cargo-module "$module_required_count" "$module_command" \
      run_cargo test --locked --offline -p iroha_data_model --lib "$module" -- --test-threads=1
  else
    module_command="cargo test --locked --offline -p iroha_core --lib ${module} -- --test-threads=1"
    run_corridor_leg \
      "$module_leg_id" cargo-module "$module_required_count" "$module_command" \
      run_cargo test --locked --offline -p iroha_core --lib "$module" -- --test-threads=1
  fi
  if ((corridor_enabled)); then
    for required_test in "${required_production_liveness_tests[@]}"; do
      if [[ "$required_test" == "${module}::"* ]] \
        && [[ "$(grep -Fxc -- "test ${required_test} ... ok" "$corridor_last_log" || true)" != 1 ]]; then
        echo "corridor log does not contain one passing result for ${required_test}" >&2
        exit 1
      fi
    done
  fi
done

required_data_model_status_test="block::consensus_v2::tests::status_validation_accepts_all_ignore_reasons_and_rejects_a_thirteenth_entry"
required_data_model_lane_certificate_test="block::consensus::tests::lane_block_certificate_decodes_atomically_from_slice"
data_model_unit_list="$production_data_model_unit_list"
data_model_ignored_unit_list="$production_data_model_ignored_unit_list"
if ! grep -Fqx -- "${required_data_model_status_test}: test" <<<"$data_model_unit_list"; then
  echo "missing required Sumeragi v2 status-contract test: ${required_data_model_status_test}" >&2
  exit 1
fi
if grep -Fqx -- "${required_data_model_status_test}: test" <<<"$data_model_ignored_unit_list"; then
  echo "required Sumeragi v2 status-contract test is ignored: ${required_data_model_status_test}" >&2
  exit 1
fi
if ! grep -Fqx -- "${required_data_model_lane_certificate_test}: test" <<<"$data_model_unit_list"; then
  echo "missing required atomic lane-certificate decode test: ${required_data_model_lane_certificate_test}" >&2
  exit 1
fi
if grep -Fqx -- "${required_data_model_lane_certificate_test}: test" <<<"$data_model_ignored_unit_list"; then
  echo "required atomic lane-certificate decode test is ignored: ${required_data_model_lane_certificate_test}" >&2
  exit 1
fi
run_corridor_leg \
  status-rust cargo-exact 1 \
  "cargo test --locked --offline -p iroha_data_model --lib ${required_data_model_status_test} -- --test-threads=1" \
  run_cargo test --locked --offline -p iroha_data_model --lib "$required_data_model_status_test" -- --test-threads=1
run_corridor_leg \
  lane-certificate-rust cargo-exact 1 \
  "cargo test --locked --offline -p iroha_data_model --lib ${required_data_model_lane_certificate_test} -- --exact --test-threads=1" \
  run_cargo test --locked --offline -p iroha_data_model --lib "$required_data_model_lane_certificate_test" -- --exact --test-threads=1

run_final_workspace_verification() {
  verify_release_identity "before final source-sealed full workspace verification"
  run_corridor_leg \
    source-sealed-workspace-format command 0 \
    "cargo fmt --all -- --check" \
    run_cargo fmt --all -- --check
  run_corridor_leg \
    source-sealed-legacy-codec-guard command 0 \
    "bash scripts/check_no_legacy_codec.sh" \
    bash scripts/check_no_legacy_codec.sh
  run_corridor_leg \
    source-sealed-workspace-build command 0 \
    "cargo build --locked --offline --workspace" \
    run_cargo build --locked --offline --workspace
  run_corridor_leg \
    source-sealed-workspace-clippy command 0 \
    "cargo clippy --locked --offline --workspace --all-targets -- -D warnings" \
    run_cargo clippy --locked --offline --workspace --all-targets -- -D warnings
  run_corridor_leg \
    source-sealed-workspace-tests command 0 \
    "cargo test --locked --offline --workspace" \
    run_cargo test --locked --offline --workspace
  run_corridor_leg \
    source-sealed-irohad-tests command 0 \
    "cargo test --locked --offline -p irohad --bin irohad --features test-network-message-control" \
    run_cargo test --locked --offline -p irohad --bin irohad --features test-network-message-control
  verify_release_identity "after final source-sealed full workspace verification"
}

# Pin the production-soak execution profile and its serialized evidence schema
# before any real network is started. Cargo's filter succeeds on zero tests, so
# require every exact non-ignored contract before executing it with `--exact`.
required_taira_release_contract_tests=(
  taira_public_localnet::release_execution_profile_accepts_only_the_exact_positive_profile
  taira_public_localnet::release_execution_profile_rejects_wrong_or_blank_build_profiles
  taira_public_localnet::release_execution_profile_rejects_cargo_profile_mismatch
  taira_public_localnet::release_execution_profile_rejects_non_exact_offline_values
  taira_public_localnet::simulation_summary_json_records_release_profile_and_status_evidence
)
taira_release_contract_target="consensus_and_da"
taira_release_contract_list="$(
  run_cargo test --locked --offline -p integration_tests --test "$taira_release_contract_target" -- --list
)"
taira_release_ignored_contract_list="$(
  run_cargo test --locked --offline -p integration_tests --test "$taira_release_contract_target" -- --list --ignored
)"
for taira_contract_index in "${!required_taira_release_contract_tests[@]}"; do
  required_test="${required_taira_release_contract_tests[$taira_contract_index]}"
  if ! grep -Fqx -- "${required_test}: test" <<<"$taira_release_contract_list"; then
    echo "missing required Taira release-evidence contract test: ${required_test}" >&2
    exit 1
  fi
  if grep -Fqx -- "${required_test}: test" <<<"$taira_release_ignored_contract_list"; then
    echo "required Taira release-evidence contract test is ignored: ${required_test}" >&2
    exit 1
  fi
  run_corridor_leg \
    "taira-contract-${taira_contract_index}" cargo-exact 1 \
    "cargo test --locked --offline -p integration_tests --test ${taira_release_contract_target} ${required_test} -- --exact --test-threads=1" \
    run_cargo test --locked --offline -p integration_tests --test "$taira_release_contract_target" \
      "$required_test" -- --exact --test-threads=1
done

# Keep the canonical Rust wire authority and the maintained lightweight SDK
# parsers in the same source-bound corridor. These commands use only checked-in
# fixtures and in-memory responses; dependency installation is deliberately a
# caller prerequisite so this gate never reaches the network.
required_cross_sdk_fixture_tests=(
  sumeragi_v2_cross_sdk_fixtures::shared_sdk_accept_fixtures_are_exact_current_rust_encodings
  sumeragi_v2_cross_sdk_fixtures::shared_sdk_negative_fixtures_fail_rust_structure_or_protocol_validation
)
cross_sdk_fixture_target="iroha_data_model_group_02"
cross_sdk_fixture_list="$(
  run_cargo test --locked --offline -p iroha_data_model --test "$cross_sdk_fixture_target" -- --list
)"
cross_sdk_ignored_fixture_list="$(
  run_cargo test --locked --offline -p iroha_data_model --test "$cross_sdk_fixture_target" -- --list --ignored
)"
for required_test in "${required_cross_sdk_fixture_tests[@]}"; do
  if ! grep -Fqx -- "${required_test}: test" <<<"$cross_sdk_fixture_list"; then
    echo "missing required Rust cross-SDK Sumeragi v2 fixture test: ${required_test}" >&2
    exit 1
  fi
  if grep -Fqx -- "${required_test}: test" <<<"$cross_sdk_ignored_fixture_list"; then
    echo "required Rust cross-SDK Sumeragi v2 fixture test is ignored: ${required_test}" >&2
    exit 1
  fi
done
run_corridor_leg \
  cross-sdk-rust cargo-exact 2 \
  "cargo test --locked --offline -p iroha_data_model --test ${cross_sdk_fixture_target} sumeragi_v2_cross_sdk_fixtures:: -- --test-threads=1" \
  run_cargo test --locked --offline -p iroha_data_model --test "$cross_sdk_fixture_target" \
    sumeragi_v2_cross_sdk_fixtures:: -- --test-threads=1

# The checked-in grouped golden and negative-control corpus must be byte-for-
# byte reproducible by its Rust owner before any downstream SDK consumes it.
run_corridor_leg \
  native-amx-rust-fixture-check command 0 \
  "cargo run --locked --offline -p iroha_data_model --features dev-tools --bin sumeragi_v2_wire_fixtures -- --check" \
  run_cargo run --locked --offline -p iroha_data_model --features dev-tools \
    --bin sumeragi_v2_wire_fixtures -- --check

# Execute every maintained consumer of the Rust-owned grouped Native AMX V2
# golden and negative corpus in the source-sealed production corridor. Each
# suite emits one exact marker that binds its observed test count to both the
# fixture bytes and the fixed suite-source inventory.
readonly native_amx_grouped_parity_harness="ci/run_native_amx_v2_grouped_sdk_parity.sh"
native_amx_grouped_fixture_sha256="$(
  bash "$native_amx_grouped_parity_harness" --fixture-sha256
)"
native_amx_grouped_suite_source_manifest_sha256="$(
  bash "$native_amx_grouped_parity_harness" --suite-source-manifest-sha256
)"
if [[ ! "$native_amx_grouped_fixture_sha256" =~ ^[0-9a-f]{64}$ \
  || ! "$native_amx_grouped_suite_source_manifest_sha256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "grouped Native AMX V2 parity harness returned an invalid source digest" >&2
  exit 1
fi
readonly native_amx_grouped_fixture_sha256
readonly native_amx_grouped_suite_source_manifest_sha256
if [[ "$profile" == "--release" ]]; then
  native_amx_grouped_parity_surfaces=(
    openapi
    python
    javascript
    swift
    kotlin
    java
  )
  native_amx_grouped_parity_test_counts=(
    7
    58
    56
    4
    6
    5
  )
  for native_amx_grouped_parity_index in \
    "${!native_amx_grouped_parity_surfaces[@]}"; do
    native_amx_grouped_parity_surface="${native_amx_grouped_parity_surfaces[$native_amx_grouped_parity_index]}"
    native_amx_grouped_parity_test_count="${native_amx_grouped_parity_test_counts[$native_amx_grouped_parity_index]}"
    run_corridor_leg \
      "native-amx-grouped-${native_amx_grouped_parity_surface}" \
      native-amx-sdk \
      "$native_amx_grouped_parity_test_count" \
      "bash ${native_amx_grouped_parity_harness} ${native_amx_grouped_parity_surface}" \
      bash "$native_amx_grouped_parity_harness" \
        "$native_amx_grouped_parity_surface"
  done
fi

js_status_contract_file="javascript/iroha_js/test/toriiClient.test.js"
required_js_status_contract_tests=(
  "getSumeragiStatusTyped validates and normalizes authoritative v2 status"
  "getSumeragiStatusTyped accepts the local-control liveness blocker"
  "getSumeragiStatusTyped accepts the unsafe-proposal ignore reason"
  "getSumeragiStatusTyped accepts all twelve ignore reasons at the bound"
)
for required_test in "${required_js_status_contract_tests[@]}"; do
  if ! grep -Fq -- "test(\"${required_test}\"," "$js_status_contract_file"; then
    echo "missing required JavaScript Sumeragi v2 status-contract test: ${required_test}" >&2
    exit 1
  fi
done
readonly js_status_pattern='getSumeragiStatusTyped (validates and normalizes authoritative v2 status|accepts the local-control liveness blocker|accepts the unsafe-proposal ignore reason|accepts all twelve ignore reasons at the bound)'
run_corridor_leg \
  status-javascript node 4 \
  "node --test --test-reporter=tap --test-name-pattern=${js_status_pattern} ${js_status_contract_file}" \
  node --test --test-reporter=tap --test-name-pattern="$js_status_pattern" "$js_status_contract_file"

if ! python3 -c 'import pytest, requests' >/dev/null 2>&1; then
  echo "Python Sumeragi v2 status-contract tests require the pinned scripts/requirements.txt dependencies to be installed before this offline gate" >&2
  exit 1
fi
python_status_tests=(
  python/iroha_torii_client/tests/test_client.py::test_get_sumeragi_status_parses_authoritative_v2_snapshot
  python/iroha_torii_client/tests/test_client.py::test_get_sumeragi_status_accepts_local_control_pending_liveness_blocker
  python/iroha_torii_client/tests/test_client.py::test_get_sumeragi_status_accepts_unsafe_proposal_ignore_reason
  python/iroha_torii_client/tests/test_client.py::test_get_sumeragi_status_accepts_all_twelve_ignore_reasons_at_the_bound
)
run_corridor_leg \
  status-python pytest 4 \
  "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider ${python_status_tests[*]}" \
  env PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 \
    python3 -m pytest -q -p no:cacheprovider "${python_status_tests[@]}"

# The release identity must include every checkout source plus the ignored
# workspace lockfile, reject active Git operations and unresolved entries, and
# enforce a clean committed production candidate. Keep this exact inventory in
# the pre-network corridor so weakening identity or sealing cannot silently
# reuse source-bound build or evidence roots.
source_manifest_contract_tests=(
  pytests/scripts/workspace_source_manifest_test.py
  pytests/scripts/seal_workspace_source_test.py
)
source_manifest_contract_log="$(corridor_contract_log_path preflight-source-seal)"
set +e
PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider \
  "${source_manifest_contract_tests[@]}" 2>&1 | tee "$source_manifest_contract_log"
source_manifest_pipeline_status=("${PIPESTATUS[@]}")
set -e
source_manifest_pass_summary="$(
  grep -Ec '^30 passed in [0-9]+([.][0-9]+)?s$' "$source_manifest_contract_log" || true
)"
if ((source_manifest_pipeline_status[0] != 0 || source_manifest_pipeline_status[1] != 0)) \
  || [[ "$source_manifest_pass_summary" != 1 ]]; then
  echo "Sumeragi v2 source-manifest/seal contract preflight did not run exactly 30 passing tests (pytest=${source_manifest_pipeline_status[0]}, tee=${source_manifest_pipeline_status[1]})" >&2
  exit 1
fi
record_corridor_log \
  preflight-source-seal pytest 30 \
  "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider ${source_manifest_contract_tests[*]}" \
  "$source_manifest_contract_log" \
  "${source_manifest_pipeline_status[0]}" "${source_manifest_pipeline_status[1]}"
((corridor_enabled)) || rm -f -- "$source_manifest_contract_log"

# Exercise the shell/evidence contract with a mocked Cargo before spending a
# fresh network attempt. Explicit node IDs make a rename or missing adversarial
# case fail collection, and the final summary rejects skipped/xfail coverage.
seed_launcher_contract_tests=(
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_runs_every_exact_scenario_with_one_start_attempt
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_preserves_prior_invocation_evidence
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_release_profile_uses_32_seeds_per_scenario
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_rejects_zero_test_and_preserves_evidence
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_rejects_ambiguous_test_summary
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_preserves_cargo_failure_through_tee
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_rejects_bundle_tampering_before_completion
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_rejects_symlinked_marker_temp_without_completion
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_marker_durability_failure_is_not_terminal
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_rejects_parent_source_manifest_mismatch
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_rejects_source_drift_before_completion
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_rejects_concurrent_writer_without_clobbering
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_refuses_uninspected_stale_lock
  pytests/scripts/sumeragi_v2_seed_matrix_test.py::test_mocked_seed_matrix_rejects_unsafe_retained_localnet_entries
)
seed_launcher_contract_log="$(corridor_contract_log_path preflight-seed-launcher)"
set +e
PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider \
  "${seed_launcher_contract_tests[@]}" 2>&1 | tee "$seed_launcher_contract_log"
seed_launcher_pipeline_status=("${PIPESTATUS[@]}")
set -e
seed_launcher_pass_summary="$(
  grep -Ec '^14 passed in [0-9]+([.][0-9]+)?s$' "$seed_launcher_contract_log" || true
)"
if ((seed_launcher_pipeline_status[0] != 0 || seed_launcher_pipeline_status[1] != 0)) \
  || [[ "$seed_launcher_pass_summary" != 1 ]]; then
  echo "Sumeragi v2 seed-launcher contract preflight did not run exactly 14 passing tests (pytest=${seed_launcher_pipeline_status[0]}, tee=${seed_launcher_pipeline_status[1]})" >&2
  exit 1
fi
record_corridor_log \
  preflight-seed-launcher pytest 14 \
  "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider ${seed_launcher_contract_tests[*]}" \
  "$seed_launcher_contract_log" \
  "${seed_launcher_pipeline_status[0]}" "${seed_launcher_pipeline_status[1]}"
((corridor_enabled)) || rm -f -- "$seed_launcher_contract_log"

# The chaos launcher must bind its one exact ignored test, full log, clean Git
# identity, and 100,000-height completion before the expensive run starts.
chaos_launcher_contract_files=(
  pytests/scripts/sumeragi_v2_chaos_release_test.py
)
chaos_launcher_contract_log="$(corridor_contract_log_path preflight-chaos-launcher)"
set +e
PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider \
  "${chaos_launcher_contract_files[@]}" 2>&1 | tee "$chaos_launcher_contract_log"
chaos_launcher_pipeline_status=("${PIPESTATUS[@]}")
set -e
chaos_launcher_pass_summary="$(
  grep -Ec '^5 passed in [0-9]+([.][0-9]+)?s$' "$chaos_launcher_contract_log" || true
)"
if ((chaos_launcher_pipeline_status[0] != 0 || chaos_launcher_pipeline_status[1] != 0)) \
  || [[ "$chaos_launcher_pass_summary" != 1 ]]; then
  echo "Sumeragi v2 chaos-launcher contract preflight did not run exactly five passing tests (pytest=${chaos_launcher_pipeline_status[0]}, tee=${chaos_launcher_pipeline_status[1]})" >&2
  exit 1
fi
record_corridor_log \
  preflight-chaos-launcher pytest 5 \
  "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider ${chaos_launcher_contract_files[*]}" \
  "$chaos_launcher_contract_log" \
  "${chaos_launcher_pipeline_status[0]}" "${chaos_launcher_pipeline_status[1]}"
((corridor_enabled)) || rm -f -- "$chaos_launcher_contract_log"

if [[ "$profile" == "--release" ]]; then
  # The production authentication stack is executable release evidence. Run
  # each complete adversarial corpus under the same relocatable SSH verifier
  # and exact source tree that the sealed release will use.
  release_identity_contract_files=(
    pytests/scripts/sumeragi_v2_release_identity_signature_test.py
  )
  release_identity_contract_log="$(corridor_contract_log_path preflight-release-identity)"
  set +e
  SUMERAGI_V2_TEST_RELOCATABLE_SSH_KEYGEN_BIN="$IROHA_RELEASE_SSH_KEYGEN_BIN" \
    PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 \
    python3 -m pytest -q -p no:cacheprovider \
    "${release_identity_contract_files[@]}" 2>&1 | tee "$release_identity_contract_log"
  release_identity_pipeline_status=("${PIPESTATUS[@]}")
  set -e
  release_identity_pass_summary="$(
    grep -Ec '^68 passed in [0-9]+([.][0-9]+)?s( \([0-9]+:[0-5][0-9]:[0-5][0-9]\))?$' \
      "$release_identity_contract_log" || true
  )"
  if ((release_identity_pipeline_status[0] != 0 || release_identity_pipeline_status[1] != 0)) \
    || [[ "$release_identity_pass_summary" != 1 ]]; then
    echo "Sumeragi v2 release-identity preflight did not run exactly 68 passing tests (pytest=${release_identity_pipeline_status[0]}, tee=${release_identity_pipeline_status[1]})" >&2
    exit 1
  fi
  record_corridor_log \
    preflight-release-identity pytest 68 \
    'SUMERAGI_V2_TEST_RELOCATABLE_SSH_KEYGEN_BIN=$IROHA_RELEASE_SSH_KEYGEN_BIN PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider pytests/scripts/sumeragi_v2_release_identity_signature_test.py' \
    "$release_identity_contract_log" \
    "${release_identity_pipeline_status[0]}" "${release_identity_pipeline_status[1]}"

  release_bootstrap_contract_files=(
    pytests/scripts/sumeragi_v2_release_bootstrap_test.py
  )
  release_bootstrap_contract_log="$(corridor_contract_log_path preflight-release-bootstrap)"
  set +e
  PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider \
    "${release_bootstrap_contract_files[@]}" 2>&1 | tee "$release_bootstrap_contract_log"
  release_bootstrap_pipeline_status=("${PIPESTATUS[@]}")
  set -e
  release_bootstrap_pass_summary="$(
    grep -Ec '^82 passed in [0-9]+([.][0-9]+)?s( \([0-9]+:[0-5][0-9]:[0-5][0-9]\))?$' \
      "$release_bootstrap_contract_log" || true
  )"
  if ((release_bootstrap_pipeline_status[0] != 0 || release_bootstrap_pipeline_status[1] != 0)) \
    || [[ "$release_bootstrap_pass_summary" != 1 ]]; then
    echo "Sumeragi v2 release-bootstrap preflight did not run exactly 82 passing tests (pytest=${release_bootstrap_pipeline_status[0]}, tee=${release_bootstrap_pipeline_status[1]})" >&2
    exit 1
  fi
  record_corridor_log \
    preflight-release-bootstrap pytest 82 \
    "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider ${release_bootstrap_contract_files[*]}" \
    "$release_bootstrap_contract_log" \
    "${release_bootstrap_pipeline_status[0]}" "${release_bootstrap_pipeline_status[1]}"

  release_bootstrap_validator_contract_files=(
    pytests/scripts/sumeragi_v2_release_bootstrap_validator_test.py
  )
  release_bootstrap_validator_contract_log="$(
    corridor_contract_log_path preflight-release-bootstrap-validator
  )"
  set +e
  PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider \
    "${release_bootstrap_validator_contract_files[@]}" 2>&1 \
    | tee "$release_bootstrap_validator_contract_log"
  release_bootstrap_validator_pipeline_status=("${PIPESTATUS[@]}")
  set -e
  release_bootstrap_validator_pass_summary="$(
    grep -Ec '^37 passed in [0-9]+([.][0-9]+)?s( \([0-9]+:[0-5][0-9]:[0-5][0-9]\))?$' \
      "$release_bootstrap_validator_contract_log" || true
  )"
  if ((release_bootstrap_validator_pipeline_status[0] != 0 \
    || release_bootstrap_validator_pipeline_status[1] != 0)) \
    || [[ "$release_bootstrap_validator_pass_summary" != 1 ]]; then
    echo "Sumeragi v2 bootstrap-validator preflight did not run exactly 37 passing tests (pytest=${release_bootstrap_validator_pipeline_status[0]}, tee=${release_bootstrap_validator_pipeline_status[1]})" >&2
    exit 1
  fi
  record_corridor_log \
    preflight-release-bootstrap-validator pytest 37 \
    "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider ${release_bootstrap_validator_contract_files[*]}" \
    "$release_bootstrap_validator_contract_log" \
    "${release_bootstrap_validator_pipeline_status[0]}" \
    "${release_bootstrap_validator_pipeline_status[1]}"
fi

# Aggregate receipt validation must reject cross-source and post-completion
# evidence changes before the corridor relies on the final release receipt.
release_receipt_contract_files=(
  pytests/scripts/sumeragi_v2_release_receipt_test.py
  pytests/scripts/sumeragi_v2_prebuilt_bundle_test.py
  pytests/scripts/sumeragi_v2_prebuilt_bundle_shell_test.py
)
release_receipt_contract_log="$(corridor_contract_log_path preflight-release-receipt)"
set +e
PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider \
  "${release_receipt_contract_files[@]}" 2>&1 | tee "$release_receipt_contract_log"
release_receipt_pipeline_status=("${PIPESTATUS[@]}")
set -e
release_receipt_pass_summary="$(
  grep -Ec '^316 passed in [0-9]+([.][0-9]+)?s( \([0-9]+:[0-5][0-9]:[0-5][0-9]\))?$' \
    "$release_receipt_contract_log" || true
)"
if ((release_receipt_pipeline_status[0] != 0 || release_receipt_pipeline_status[1] != 0)) \
  || [[ "$release_receipt_pass_summary" != 1 ]]; then
  echo "Sumeragi v2 aggregate-receipt/bundle contract preflight did not run exactly 316 passing tests (pytest=${release_receipt_pipeline_status[0]}, tee=${release_receipt_pipeline_status[1]})" >&2
  exit 1
fi
record_corridor_log \
  preflight-release-receipt pytest 316 \
  "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider ${release_receipt_contract_files[*]}" \
  "$release_receipt_contract_log" \
  "${release_receipt_pipeline_status[0]}" "${release_receipt_pipeline_status[1]}"
((corridor_enabled)) || rm -f -- "$release_receipt_contract_log"

# Seal the benchmark collector and independent validator themselves before
# trusting a G-SCALE bundle. These tests execute no benchmark and no Cargo
# command; they exercise exact pair ordering, non-weakenable thresholds,
# source binding, bounded artifacts, and every fail-fast collection boundary.
multilane_scaling_contract_files=(
  scripts/tests/validate_multilane_scaling_evidence_test.py
  scripts/tests/run_multilane_scaling_gate_test.py
)
multilane_scaling_contract_log="$(
  corridor_contract_log_path preflight-multilane-scaling
)"
set +e
PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider \
  "${multilane_scaling_contract_files[@]}" 2>&1 | tee "$multilane_scaling_contract_log"
multilane_scaling_pipeline_status=("${PIPESTATUS[@]}")
set -e
multilane_scaling_pass_summary="$(
  grep -Ec '^52 passed in [0-9]+([.][0-9]+)?s( \([0-9]+:[0-5][0-9]:[0-5][0-9]\))?$' \
    "$multilane_scaling_contract_log" || true
)"
if ((multilane_scaling_pipeline_status[0] != 0 \
    || multilane_scaling_pipeline_status[1] != 0)) \
  || [[ "$multilane_scaling_pass_summary" != 1 ]]; then
  echo "G-SCALE runner/validator preflight did not run exactly 52 passing tests (pytest=${multilane_scaling_pipeline_status[0]}, tee=${multilane_scaling_pipeline_status[1]})" >&2
  exit 1
fi
record_corridor_log \
  preflight-multilane-scaling pytest 52 \
  "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider ${multilane_scaling_contract_files[*]}" \
  "$multilane_scaling_contract_log" \
  "${multilane_scaling_pipeline_status[0]}" \
  "${multilane_scaling_pipeline_status[1]}"
((corridor_enabled)) || rm -f -- "$multilane_scaling_contract_log"

# Run the complete fail-closed proof-ledger, Verus-evidence, and TLC-trace
# normalizer contract corpus before trusting any expensive formal result. The
# exact pass count rejects deleted, added, skipped, or xfailed fidelity cases.
proof_fidelity_contract_files=(
  pytests/scripts/sumeragi_v2_proof_ledger_test.py
  pytests/scripts/sumeragi_v2_verus_evidence_test.py
  pytests/scripts/sumeragi_v2_tlc_trace_normalizer_test.py
)
proof_fidelity_contract_log="$(corridor_contract_log_path preflight-proof-fidelity)"
# Collection is source-bound as 3,633 ledger/checker cases (including the
# eight lexically executed case components), 28 pinned-Verus evidence cases,
# and 15 TLC-normalizer cases.
set +e
PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider \
  "${proof_fidelity_contract_files[@]}" 2>&1 | tee "$proof_fidelity_contract_log"
proof_fidelity_pipeline_status=("${PIPESTATUS[@]}")
set -e
proof_fidelity_pass_summary="$(
  grep -Ec '^3676 passed in [0-9]+([.][0-9]+)?s( \([0-9]+:[0-5][0-9]:[0-5][0-9]\))?$' "$proof_fidelity_contract_log" || true
)"
if ((proof_fidelity_pipeline_status[0] != 0 || proof_fidelity_pipeline_status[1] != 0)) \
  || [[ "$proof_fidelity_pass_summary" != 1 ]]; then
  echo "Sumeragi v2 proof-fidelity preflight did not run exactly 3676 passing tests (pytest=${proof_fidelity_pipeline_status[0]}, tee=${proof_fidelity_pipeline_status[1]})" >&2
  exit 1
fi
record_corridor_log \
  preflight-proof-fidelity pytest 3676 \
  "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider ${proof_fidelity_contract_files[*]}" \
  "$proof_fidelity_contract_log" \
  "${proof_fidelity_pipeline_status[0]}" "${proof_fidelity_pipeline_status[1]}"
((corridor_enabled)) || rm -f -- "$proof_fidelity_contract_log"

formal_launcher_contract_files=(
  pytests/scripts/sumeragi_v2_formal_release_test.py
)
formal_launcher_contract_log="$(corridor_contract_log_path preflight-formal-launcher)"
set +e
PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider \
  "${formal_launcher_contract_files[@]}" 2>&1 | tee "$formal_launcher_contract_log"
formal_launcher_pipeline_status=("${PIPESTATUS[@]}")
set -e
formal_launcher_pass_summary="$(
  grep -Ec '^26 passed in [0-9]+([.][0-9]+)?s( \([0-9]+:[0-5][0-9]:[0-5][0-9]\))?$' "$formal_launcher_contract_log" || true
)"
if ((formal_launcher_pipeline_status[0] != 0 || formal_launcher_pipeline_status[1] != 0)) \
  || [[ "$formal_launcher_pass_summary" != 1 ]]; then
  echo "Sumeragi v2 formal-launcher contract preflight did not run exactly 26 passing tests (pytest=${formal_launcher_pipeline_status[0]}, tee=${formal_launcher_pipeline_status[1]})" >&2
  exit 1
fi
record_corridor_log \
  preflight-formal-launcher pytest 26 \
  "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider ${formal_launcher_contract_files[*]}" \
  "$formal_launcher_contract_log" \
  "${formal_launcher_pipeline_status[0]}" "${formal_launcher_pipeline_status[1]}"
((corridor_enabled)) || rm -f -- "$formal_launcher_contract_log"

# Run the complete mocked soak launcher/evidence corpus as one exact file-bound
# preflight. The 42-pass summary rejects missing, added, skipped, or xfailed
# cases before the release corridor can trust the 24-hour evidence path.
taira_soak_contract_files=(
  pytests/scripts/taira_v2_soak_test.py::test_launcher_pins_complete_profile_and_runs_exactly_one_test
  pytests/scripts/taira_v2_soak_test.py::test_launcher_rejects_zero_test_inventory
  pytests/scripts/taira_v2_soak_test.py::test_launcher_rejects_zero_test_execution_output
  pytests/scripts/taira_v2_soak_test.py::test_launcher_rejects_bundle_tampering_before_completion
  pytests/scripts/taira_v2_soak_test.py::test_launcher_rejects_symlinked_marker_temp_without_completion
  pytests/scripts/taira_v2_soak_test.py::test_launcher_marker_durability_failure_is_not_terminal
  pytests/scripts/taira_v2_soak_test.py::test_launcher_rejects_profile_override_arguments_before_cargo
  pytests/scripts/taira_v2_soak_test.py::test_launcher_rejects_a_concurrent_source_bound_soak
  pytests/scripts/taira_v2_soak_test.py::test_launcher_does_not_promote_provisional_evidence_when_validation_fails
  pytests/scripts/taira_v2_soak_evidence_test.py
)
taira_soak_contract_log="$(corridor_contract_log_path preflight-taira-soak)"
set +e
PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider \
  "${taira_soak_contract_files[@]}" 2>&1 | tee "$taira_soak_contract_log"
taira_soak_pipeline_status=("${PIPESTATUS[@]}")
set -e
taira_soak_pass_summary="$(
  grep -Ec '^42 passed in [0-9]+([.][0-9]+)?s$' "$taira_soak_contract_log" || true
)"
if ((taira_soak_pipeline_status[0] != 0 || taira_soak_pipeline_status[1] != 0)) \
  || [[ "$taira_soak_pass_summary" != 1 ]]; then
  echo "Taira v2 soak launcher/evidence preflight did not run exactly 42 passing tests (pytest=${taira_soak_pipeline_status[0]}, tee=${taira_soak_pipeline_status[1]})" >&2
  exit 1
fi
record_corridor_log \
  preflight-taira-soak pytest 42 \
  "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider ${taira_soak_contract_files[*]}" \
  "$taira_soak_contract_log" \
  "${taira_soak_pipeline_status[0]}" "${taira_soak_pipeline_status[1]}"
((corridor_enabled)) || rm -f -- "$taira_soak_contract_log"
publish_corridor_completion() {
  if ((!corridor_enabled)); then
    return
  fi
  if ! localnet_binary_attestation_valid; then
    echo "source-bound localnet binary bundle changed before corridor completion" >&2
    return 1
  fi
  # 38 production modules + 9 G-UNIT groups + 2 data-model contracts
  # + 5 Taira contracts + 1 cross-SDK Rust leg + 1 Native AMX fixture check
  # + 6 grouped SDK parity legs + 2 status SDK legs + 11 contract preflights
  # + 6 final workspace-verification legs.
  readonly expected_corridor_leg_count=81
  if ((corridor_leg_index != expected_corridor_leg_count)); then
    echo "release corridor recorded ${corridor_leg_index} legs, expected ${expected_corridor_leg_count}" >&2
    exit 1
  fi
  corridor_summary_sha256="$(sha256_file "$corridor_summary")"
  corridor_required_tests_sha256="$(sha256_file "$corridor_required_tests")"
  corridor_g_unit_inventory_sha256="$(sha256_file "$corridor_g_unit_inventory")"
  corridor_java_path="$(python3 -I -S -c \
    'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve(strict=True))' \
    "$JAVA_BIN")"
  corridor_java_sha256="$(sha256_file "$corridor_java_path")"
  corridor_cargo_path="$(canonical_executable cargo)"
  corridor_rustc_path="$(canonical_executable rustc)"
  corridor_python_path="$(canonical_executable python3)"
  corridor_node_path="$(canonical_executable node)"
  corridor_swift_path="$(canonical_executable swift)"
  corridor_swift_version="$("$corridor_swift_path" --version)"
  corridor_swift_version="${corridor_swift_version%%$'\n'*}"
  corridor_bash_path="$(canonical_executable bash)"
  corridor_git_path="$(canonical_executable git)"
  corridor_cargo_home="$(canonical_path "$CARGO_HOME")"
  wait_for_external_cargo
  corridor_cargo_version="$("$corridor_cargo_path" --version)"
  corridor_completion_tmp="${corridor_evidence_dir}/.COMPLETED.tsv.$$"
  printf '%s\t%s\n' \
    schema_version 1 \
    head_commit "$release_head_commit" \
    head_tree "$release_head_tree" \
    source_manifest_sha256 "$release_source_manifest_sha256" \
    cargo_lock_sha256 "$release_cargo_lock_sha256" \
    prebuilt_manifest_path \
      "${IROHA_TEST_TARGET_DIR}/.sumeragi-v2-prebuilt-binaries.tsv" \
    prebuilt_manifest_sha256 "$IROHA_RELEASE_PREBUILT_MANIFEST_SHA256" \
    leg_count "$corridor_leg_index" \
    production_required_test_count "$expected_production_liveness_test_count" \
    g_unit_expected_test_count "$expected_multilane_focus_test_count" \
    g_unit_passed_test_count "$expected_multilane_focus_test_count" \
    summary_sha256 "$corridor_summary_sha256" \
    production_required_tests_sha256 "$corridor_required_tests_sha256" \
    g_unit_inventory_sha256 "$corridor_g_unit_inventory_sha256" \
    java_path "$corridor_java_path" \
    java_sha256 "$corridor_java_sha256" \
    cargo_path "$corridor_cargo_path" \
    cargo_sha256 "$(sha256_file "$corridor_cargo_path")" \
    cargo_version "$corridor_cargo_version" \
    rustc_path "$corridor_rustc_path" \
    rustc_sha256 "$(sha256_file "$corridor_rustc_path")" \
    rustc_version "$("$corridor_rustc_path" --version)" \
    python3_path "$corridor_python_path" \
    python3_sha256 "$(sha256_file "$corridor_python_path")" \
    node_path "$corridor_node_path" \
    node_sha256 "$(sha256_file "$corridor_node_path")" \
    swift_path "$corridor_swift_path" \
    swift_sha256 "$(sha256_file "$corridor_swift_path")" \
    swift_version "$corridor_swift_version" \
    bash_path "$corridor_bash_path" \
    bash_sha256 "$(sha256_file "$corridor_bash_path")" \
    git_path "$corridor_git_path" \
    git_sha256 "$(sha256_file "$corridor_git_path")" \
    cargo_home_path "$corridor_cargo_home" \
    repo_cargo_config_sha256 "$(sha256_file "$repo_root/.cargo/config.toml")" \
    native_amx_grouped_fixture_sha256 "$native_amx_grouped_fixture_sha256" \
    native_amx_grouped_suite_source_manifest_sha256 \
      "$native_amx_grouped_suite_source_manifest_sha256" \
    native_amx_grouped_negative_control_count 51 \
    tlc_profile "$SUMERAGI_V2_TLC_PROFILE" \
    tlaps_threads "$SUMERAGI_TLAPS_THREADS" \
    >"$corridor_completion_tmp"
  mv -- "$corridor_completion_tmp" "$corridor_completion_path"
}
verify_release_identity "after release contract preflights"

run_release_scaling_and_formal_gates() {
  scaling_preflight_report="${IROHA_RELEASE_HOST_ROOT}/scaling-validation-preflight.json"
  rm -f -- "$scaling_preflight_report"
  "$IROHA_RELEASE_PYTHON_BIN" -I -S \
    scripts/nexus/validate_multilane_scaling_evidence.py \
    "$IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST" \
    --report "$scaling_preflight_report" \
    --expected-source-revision "$release_head_commit" \
    --expected-workspace-source-sha256 "$release_source_manifest_sha256" \
    --expected-validator-sha256 "$(
      sha256_file scripts/nexus/validate_multilane_scaling_evidence.py
    )" \
    --expected-trial-harness-sha256 \
      "$IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256" \
    --expected-configuration-sha256 \
      "$IROHA_RELEASE_SCALING_CONFIGURATION_SHA256" \
    --expected-irohad-sha256 "$IROHA_RELEASE_SCALING_IROHAD_SHA256" \
    --expected-iroha-cli-sha256 "$IROHA_RELEASE_SCALING_IROHA_CLI_SHA256" \
    --expected-repository-root "$repo_root" \
    --quiet
  if [[ ! -s "$scaling_preflight_report" \
    || -L "$scaling_preflight_report" ]]; then
    echo "source-bound G-SCALE validation did not publish its preflight report" >&2
    exit 1
  fi
  rm -f -- "$scaling_preflight_report"
  verify_release_identity "after source-bound G-SCALE validation"

  # Bind the strict deductive ledger and its backend evidence only after the
  # mandatory fresh-network and fault-soak corridors have completed.
  formal_completion_path_file="${IROHA_RELEASE_HOST_ROOT}/formal-completion-path"
  rm -f -- "$formal_completion_path_file"
  IROHA_FORMAL_COMPLETION_PATH_FILE="$formal_completion_path_file" \
    bash scripts/run_sumeragi_v2_formal_release.sh
  if [[ ! -s "$formal_completion_path_file" ]]; then
    echo "strict formal release gate did not publish its completion path" >&2
    exit 1
  fi
  verify_release_identity "after strict formal corridor"
}

seed_completion_path_file=""
if [[ "$profile" == "--release" ]]; then
  seed_completion_path_file="${IROHA_RELEASE_HOST_ROOT}/seed-matrix-completion-path"
  rm -f -- "$seed_completion_path_file"
fi
IROHA_SEED_MATRIX_COMPLETION_PATH_FILE="$seed_completion_path_file" \
  bash scripts/run_sumeragi_v2_seed_matrix.sh "$profile"
if [[ "$profile" == "--release" && ! -s "$seed_completion_path_file" ]]; then
  echo "release seed matrix did not publish its completion path" >&2
  exit 1
fi
verify_release_identity "after deterministic seed matrix"

if [[ "$profile" == "--release" ]]; then
  multilane_four_peer_completion_path_file="${IROHA_RELEASE_HOST_ROOT}/multilane-four-peer-completion-path"
  rm -f -- "$multilane_four_peer_completion_path_file"
  IROHA_MULTILANE_FOUR_PEER_COMPLETION_PATH_FILE="$multilane_four_peer_completion_path_file" \
    bash scripts/run_nexus_cross_dataspace_atomic_swap.sh \
      --release \
      --capture \
      --test-threads 1 \
      --env IROHA_TEST_ALLOW_REENTRANT_BUILD=0 \
      --multilane-four-peer-release
  if [[ ! -s "$multilane_four_peer_completion_path_file" ]]; then
    echo "mandatory four-peer multilane release gates did not publish a completion path" >&2
    exit 1
  fi
  multilane_four_peer_completion_path="$(<"$multilane_four_peer_completion_path_file")"
  if [[ ! -f "$multilane_four_peer_completion_path" \
    || -L "$multilane_four_peer_completion_path" ]]; then
    echo "four-peer multilane completion must be a regular non-symlink file" >&2
    exit 1
  fi
  for required_line in \
    $'mode\tmandatory-four-peer-multilane-release' \
    $'expected_runs\t4' \
    $'passed_runs\t4' \
    $'failed_runs\t0' \
    $'skipped_runs\t0' \
    $'native_grouped_pruning_evidence\tpassed'; do
    if ! grep -Fqx -- "$required_line" "$multilane_four_peer_completion_path"; then
      echo "four-peer multilane completion is missing exact accounting: ${required_line}" >&2
      exit 1
    fi
  done
  verify_release_identity "after mandatory four-peer multilane release gates"
fi

# G-12P is a distinct real-network gate from the reducer seed matrix above.
# Its launcher starts ten fresh 12-peer networks, uses one canonical seed per
# process, forbids retries, and validates each Cargo transcript as exactly one
# scheduled/passing test before publishing completion accounting.
nexus_cross_completion_path_file="${IROHA_RELEASE_HOST_ROOT:-${repo_root}/target}/nexus-cross-dataspace-completion-path"
rm -f -- "$nexus_cross_completion_path_file"
nexus_cross_args=(
  --capture
  --test-threads
  1
  --env
  IROHA_TEST_ALLOW_REENTRANT_BUILD=0
)
if [[ "$profile" == "--release" ]]; then
  nexus_cross_args+=(--release)
fi
IROHA_NEXUS_CROSS_COMPLETION_PATH_FILE="$nexus_cross_completion_path_file" \
  bash scripts/run_nexus_cross_dataspace_atomic_swap.sh "${nexus_cross_args[@]}"
if [[ ! -s "$nexus_cross_completion_path_file" ]]; then
  echo "G-12P seed matrix did not publish its completion path" >&2
  exit 1
fi
nexus_cross_completion_path="$(<"$nexus_cross_completion_path_file")"
if [[ ! -f "$nexus_cross_completion_path" || -L "$nexus_cross_completion_path" ]]; then
  echo "G-12P seed-matrix completion must be a regular non-symlink file" >&2
  exit 1
fi
for required_line in \
  $'mode\tdeterministic-seed-matrix' \
  $'expected_runs\t10' \
  $'passed_runs\t10' \
  $'failed_runs\t0' \
  $'process_retry_runs\t0'; do
  if ! grep -Fqx -- "$required_line" "$nexus_cross_completion_path"; then
    echo "G-12P seed-matrix completion is missing exact accounting: ${required_line}" >&2
    exit 1
  fi
done
verify_release_identity "after strict G-12P deterministic seed matrix"

if [[ "$profile" == "--release" ]]; then
  nexus_cross_soak_completion_path_file="${IROHA_RELEASE_HOST_ROOT}/nexus-cross-dataspace-soak-completion-path"
  rm -f -- "$nexus_cross_soak_completion_path_file"
  IROHA_NEXUS_CROSS_COMPLETION_PATH_FILE="$nexus_cross_soak_completion_path_file" \
    bash scripts/run_nexus_cross_dataspace_atomic_swap.sh \
      --release \
      --capture \
      --test-threads 1 \
      --env IROHA_TEST_ALLOW_REENTRANT_BUILD=0 \
      --cross-dataspace-fault-soak \
      --cross-dataspace-seed nexus-cross-dataspace-v1-seed-00 \
      --cross-dataspace-soak-duration-secs 7200
  if [[ ! -s "$nexus_cross_soak_completion_path_file" ]]; then
    echo "G-12P two-hour fault soak did not publish its completion path" >&2
    exit 1
  fi
  nexus_cross_soak_completion_path="$(<"$nexus_cross_soak_completion_path_file")"
  if [[ ! -f "$nexus_cross_soak_completion_path" \
    || -L "$nexus_cross_soak_completion_path" ]]; then
    echo "G-12P fault-soak completion must be a regular non-symlink file" >&2
    exit 1
  fi
  for required_line in \
    $'mode\ttwo-hour-fault-soak' \
    $'duration_seconds\t7200' \
    $'expected_runs\t1' \
    $'passed_runs\t1' \
    $'failed_runs\t0' \
    $'process_retry_runs\t0'; do
    if ! grep -Fqx -- "$required_line" "$nexus_cross_soak_completion_path"; then
      echo "G-12P fault-soak completion is missing exact accounting: ${required_line}" >&2
      exit 1
    fi
  done
  verify_release_identity "after G-12P two-hour rotating-validator fault soak"
  run_release_scaling_and_formal_gates
fi

if [[ "$profile" == "--pr" ]]; then
  python3 -I -S scripts/formal/check_sumeragi_v2_proof_ledger.py
  bash scripts/formal/run_sumeragi_v2_harness.sh --unit
  bash scripts/formal/run_sumeragi_v2_harness.sh --fast-network
  bash scripts/formal/run_sumeragi_v2_harness.sh --model-replay
  run_final_workspace_verification
  final_pr_source_manifest_sha256="$(
    python3 -I -S scripts/compute_workspace_source_manifest.py --root "$repo_root"
  )"
  if [[ ! "$final_pr_source_manifest_sha256" =~ ^[0-9a-f]{64}$ ]]; then
    echo "workspace source manifest helper returned an invalid digest after the PR corridor" >&2
    exit 1
  fi
  if [[ "$final_pr_source_manifest_sha256" != "$release_source_manifest_sha256" ]]; then
    echo "workspace sources changed during the PR release corridor" >&2
    exit 1
  fi
  echo "Sumeragi v2 PR gate passed: cross-SDK fixture/status parity, 4 seeds × 5 reducer scenarios (20 runs), strict 10/10 fresh-network G-12P, adversarial simulations, and trace replay" >&2
  exit 0
fi

readonly chaos_completion_path_file="${IROHA_RELEASE_HOST_ROOT}/chaos-completion-path"
rm -f -- "$chaos_completion_path_file"
IROHA_CHAOS_COMPLETION_PATH_FILE="$chaos_completion_path_file" \
  bash scripts/run_sumeragi_v2_100k_chaos.sh
if [[ ! -s "$chaos_completion_path_file" ]]; then
  echo "100,000-height chaos gate did not publish its completion path" >&2
  exit 1
fi
verify_release_identity "after 100,000-height chaos"
pre_soak_source_manifest_sha256="$(
  python3 -I -S scripts/compute_workspace_source_manifest.py --root "$repo_root"
)"
if [[ ! "$pre_soak_source_manifest_sha256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "workspace source manifest helper returned an invalid digest before the Taira production soak" >&2
  exit 1
fi
if [[ "$pre_soak_source_manifest_sha256" != "$release_source_manifest_sha256" ]]; then
  echo "workspace sources changed before the Taira production soak" >&2
  exit 1
fi
readonly taira_completion_path_file="${IROHA_RELEASE_HOST_ROOT}/taira-completion-path"
rm -f -- "$taira_completion_path_file"
IROHA_TAIRA_COMPLETION_PATH_FILE="$taira_completion_path_file" \
  bash scripts/run_taira_v2_24h_soak.sh
if [[ ! -s "$taira_completion_path_file" ]]; then
  echo "Taira production soak did not publish its completion path" >&2
  exit 1
fi
verify_release_identity "after Taira production soak"

final_release_source_manifest_sha256="$(
  python3 -I -S scripts/compute_workspace_source_manifest.py --root "$repo_root"
)"
if [[ ! "$final_release_source_manifest_sha256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "workspace source manifest helper returned an invalid digest after the production release corridor" >&2
  exit 1
fi
if [[ "$final_release_source_manifest_sha256" != "$release_source_manifest_sha256" ]]; then
  echo "workspace sources changed during the production release corridor" >&2
  exit 1
fi
# Revalidate the deductive evidence after every long-running gate so a TLA+
# or production-source edit during chaos or soak execution cannot inherit
# stale TLAPS/Verus/cross-tool success.
cross_tool_obligations="$(
  python3 -I -S scripts/formal/check_sumeragi_v2_proof_ledger.py \
    --print-cross-tool-obligations
)"
final_proof_evidence_args=(
  --release
  --evidence target/formal/sumeragi_v2/proof_evidence.json
  --verus-evidence target/formal/sumeragi_v2/verus_evidence.json
)
readonly cross_tool_evidence_path="target/formal/sumeragi_v2/cross_tool_evidence.json"
if [[ -n "$cross_tool_obligations" ]]; then
  if [[ ! -f "$cross_tool_evidence_path" || -L "$cross_tool_evidence_path" ]]; then
    echo "cross_tool_proved obligations require regular cross-tool evidence" >&2
    exit 1
  fi
  final_proof_evidence_args+=(
    --cross-tool-evidence "$cross_tool_evidence_path"
  )
elif [[ -e "$cross_tool_evidence_path" || -L "$cross_tool_evidence_path" ]]; then
  echo "dormant cross-tool evidence must be absent" >&2
  exit 1
fi
python3 -I -S scripts/formal/check_sumeragi_v2_proof_ledger.py \
  "${final_proof_evidence_args[@]}"
verify_release_identity "after final proof-evidence validation"

run_final_workspace_verification
publish_corridor_completion
verify_release_identity "after final corridor completion publication"

seed_completion_path="$(<"$seed_completion_path_file")"
formal_completion_path="$(<"$formal_completion_path_file")"
chaos_completion_path="$(<"$chaos_completion_path_file")"
taira_completion_path="$(<"$taira_completion_path_file")"
verify_release_identity "before aggregate release receipt publication"
"$IROHA_RELEASE_PYTHON_BIN" -I -S scripts/write_sumeragi_v2_release_receipt.py \
  --candidate-identity "$IROHA_RELEASE_CANDIDATE_IDENTITY_PATH" \
  --sealed-identity "$IROHA_RELEASE_EXPECTED_IDENTITY_PATH" \
  --release-root "$repo_root" \
  --bootstrap-completion "$SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION" \
  --bootstrap-evidence-dir "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR" \
  --bootstrap-identity "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY" \
  --bootstrap-attestation "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION" \
  --bootstrap-transcript "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_TRANSCRIPT" \
  --expected-bootstrap-completion-sha256 \
    "$SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256" \
  --bootstrap-candidate-root "$IROHA_RELEASE_BOOTSTRAP_CANDIDATE_ROOT" \
  --bootstrap-runner "$IROHA_RELEASE_BOOTSTRAP_RUNNER" \
  --signature-attestation "$IROHA_RELEASE_SIGNATURE_ATTESTATION" \
  --signature-transcript "$IROHA_RELEASE_SIGNATURE_TRANSCRIPT" \
  --signature-raw-commit "$IROHA_RELEASE_SIGNATURE_RAW_COMMIT" \
  --signature-cargo-lock "$IROHA_RELEASE_SIGNATURE_CARGO_LOCK" \
  --signature-allowed-signers "$IROHA_RELEASE_SIGNATURE_ALLOWED_SIGNERS" \
  --signature-revocation "$IROHA_RELEASE_SIGNATURE_REVOCATION" \
  --signature-git "$IROHA_RELEASE_GIT_BIN" \
  --signature-ssh-keygen "$IROHA_RELEASE_SSH_KEYGEN_BIN" \
  --expected-git-sha256 "$IROHA_RELEASE_EXPECTED_GIT_SHA256" \
  --expected-ssh-keygen-sha256 "$IROHA_RELEASE_EXPECTED_SSH_KEYGEN_SHA256" \
  --expected-allowed-signers-sha256 "$IROHA_RELEASE_EXPECTED_ALLOWED_SIGNERS_SHA256" \
  --expected-revocation-sha256 "$IROHA_RELEASE_EXPECTED_REVOCATION_SHA256" \
  --expected-signer-fingerprint "$IROHA_RELEASE_EXPECTED_SIGNER_FINGERPRINT" \
  --corridor-completion "$corridor_completion_path" \
  --formal-completion "$formal_completion_path" \
  --seed-completion "$seed_completion_path" \
  --chaos-completion "$chaos_completion_path" \
  --taira-completion "$taira_completion_path" \
  --g4p-completion "$multilane_four_peer_completion_path" \
  --g12-seed-completion "$nexus_cross_completion_path" \
  --g12-fault-soak-completion "$nexus_cross_soak_completion_path" \
  --scaling-evidence-manifest "$IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST" \
  --expected-scaling-trial-harness-sha256 \
    "$IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256" \
  --expected-scaling-configuration-sha256 \
    "$IROHA_RELEASE_SCALING_CONFIGURATION_SHA256" \
  --expected-scaling-irohad-sha256 "$IROHA_RELEASE_SCALING_IROHAD_SHA256" \
  --expected-scaling-iroha-cli-sha256 "$IROHA_RELEASE_SCALING_IROHA_CLI_SHA256" \
  --repository-root "$repo_root" \
  --output "$IROHA_RELEASE_AGGREGATE_RECEIPT_PATH"

  echo "Sumeragi v2 production release gates passed, including exact 309/309 G-UNIT, strict 10/10 G-12P, the two-hour G-12P fault soak, sealed G-SCALE evidence, 100,000 heights, and the 24-hour Taira soak; receipt=${IROHA_RELEASE_AGGREGATE_RECEIPT_PATH}" >&2
