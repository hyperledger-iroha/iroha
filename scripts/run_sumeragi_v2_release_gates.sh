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
fi
readonly inherited_cargo_cache_home="${CARGO_HOME:-${HOME:-}/.cargo}"
cd "$repo_root"
# Every real-network leg in this parent shell must fail rather than translate a
# socket/sandbox denial into a successful developer skip.
export IROHA_TEST_REQUIRE_NETWORK=1
unset TEST_NETWORK_BIN_IROHAD KAGAMI_BIN CARGO_BIN_EXE_iroha3d CARGO_BIN_EXE_kagami
unset TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL TEST_NETWORK_BIN_IROHA CARGO_BIN_EXE_iroha
unset TEST_NETWORK_IROHAD_FEATURES TEST_NETWORK_CARGO
unset IROHA_TEST_SKIP_BUILD IROHA_TEST_ALLOW_REENTRANT_BUILD
unset IROHA_TEST_TARGET_DIR IROHA_TEST_BUILD_PROFILE IROHA_TEST_BUILD_TIMEOUT_MS PROFILE
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
    || ! -f "$IROHA_RELEASE_SIGNATURE_REVOCATION" ]]; then
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
  if [[ ! "$IROHA_RELEASE_EXPECTED_SIGNER_FINGERPRINT" =~ ^SHA256:[A-Za-z0-9+/]{43}$ ]]; then
    echo "signed release fingerprint trust anchor changed at ${checkpoint}" >&2
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
export IROHA_TEST_TARGET_DIR="${release_source_bound_root}/programs"
export IROHA_TEST_SKIP_BUILD=0
export IROHA_TEST_ALLOW_REENTRANT_BUILD=1
export IROHA_TEST_BUILD_TIMEOUT_MS=3600
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
  export SUMERAGI_TLAPS_THREADS=4
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
if kind.startswith("cargo-"):
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
elif kind == "command":
    print(0)
else:
    raise SystemExit(f"unknown corridor leg kind: {kind}")
PY
)" || {
    echo "release corridor leg ${leg_id} has invalid test output" >&2
    return 1
  }
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
  kura::lane_geometry::tests::first_release_retirement_promotes_then_rejects_complete_autonomous_rewrite
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
  sumeragi::v2_core::tests::higher_tc_lock_prunes_superseded_commit_retransmission
  sumeragi::v2_core::tests::same_lock_tc_resigns_local_commit_and_rebuilds_quorum_without_self_delivery
  sumeragi::v2_core::tests::tc_highest_prepare_missed_locally_persists_historical_commit_after_validation
  sumeragi::v2_core::tests::historical_locked_commit_replays_only_after_exact_tc_lock_installation
  sumeragi::v2_core::tests::higher_conflicting_prepare_intent_fences_historical_commit_reconstruction
  sumeragi::v2_core::tests::higher_same_subject_prepare_allows_historical_commit_reconstruction
  sumeragi::v2_core::tests::replay_does_not_resign_commit_superseded_by_higher_tc_lock
  sumeragi::v2_core::tests::replay_resigns_current_proposal_prepare_then_historical_locked_commit_fifo
  sumeragi::v2_core::tests::replay_resigns_current_timeout_then_historical_locked_commit_fifo
  sumeragi::v2_core::tests::current_view_commit_waits_for_the_exact_durable_lock
  sumeragi::v2_core::tests::decision_retains_in_flight_body_pipeline_without_duplicate_fetch
  sumeragi::v2_core::tests::timeout_elapsed_cannot_start_durable_timeout_after_decision
  sumeragi::v2_core::tests::quorum_completing_timeout_vote_cannot_form_tc_after_decision
  sumeragi::v2_core::tests::commit_qc_cannot_overtake_timeout_frontier
  sumeragi::v2_core::refinement::tests::retransmit_may_reconstruct_one_final_decision_body_stage
  sumeragi::v2_core::refinement::tests::source_linked_effective_lock_body_kernels_reject_adversarial_inputs
  sumeragi::v2_core::refinement::tests::applied_successor_kernel_rejects_foreign_same_height_authority_and_status_mutations
  sumeragi::v2_core::refinement::tests::recovered_successor_kernel_keeps_complete_tip_and_snapshot_authority_disjoint
  sumeragi::v2_core::refinement::tests::successor_startup_lifecycle_preserves_running_on_failure_and_separates_restart_sources
  sumeragi::v2_core::refinement::tests::durable_intent_accepts_a_stale_event_only_as_an_empty_owner_stutter
  sumeragi::v2_core::refinement::tests::durable_timeout_boundary_preserves_record_and_successor_owner_rounds
  sumeragi::v2_core::refinement::tests::two_stage_relay_retry_kernel_rejects_source_rotation_eligibility_and_fifo_mutations
  sumeragi::v2_core::reducer::source_link_tests::retransmit_body_stage_requires_an_exact_durable_decision_capability
  sumeragi::v2_core::reducer::source_link_tests::replay_refinement_binds_the_complete_durable_fifo
  sumeragi::v2_core::reducer::source_link_tests::replay_refinement_rejects_malformed_post_states_even_with_the_right_first_effect
  sumeragi::authoritative_runtime_gate_tests::anonymous_and_non_roster_v2_sources_share_one_bounded_lane
  sumeragi::authoritative_runtime_gate_tests::byzantine_v2_source_cannot_consume_honest_ingress_reservations_or_service_turns
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_completion_bound_overflow_fails_closed
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_completion_corridor_survives_ordinary_progress_and_timeout_saturation
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_completion_owner_is_source_isolated_and_queue_scoped
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_coalesces_semantic_request_and_attaches_independent_routes
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_exact_ownership_carrier_tracks_route_actions_and_cursors
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_projection_distinguishes_identical_bytes_from_distinct_origins
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
  sumeragi::authoritative_runtime_gate_tests::roster_origin_relay_completion_has_untrusted_count_and_byte_owner
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_wire_index_keeps_untrusted_origins_distinct
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_reserves_same_source_transport_completion_behind_auxiliary_pressure
  sumeragi::authoritative_runtime_gate_tests::v2_ingress_rejects_capacity_without_per_validator_progress_reservations
  sumeragi::authoritative_runtime_gate_tests::transport_reply_route_construction_is_fallible_and_target_bound
  merge_sidecar::tests::exact_active_delivery_retry_preserves_decreasing_chunk_rank
  merge_sidecar::tests::alternate_source_progress_and_reconnect_preserve_independent_cursors
  merge_sidecar::tests::equal_ordinal_different_tenure_alternate_source_is_rejected_atomically
  merge_sidecar::tests::inactive_source_teardown_releases_budget_and_reconnect_resumes_cursor
  merge_sidecar::tests::later_delivery_preserves_the_current_source_cursor
  merge_sidecar::tests::later_delivery_while_chunk_is_in_flight_waits_for_flush_before_next_emit
  merge_sidecar::tests::late_old_exact_item_receipt_completes_reconnected_attempt_once
  merge_sidecar::tests::later_delivery_updates_pending_work_without_losing_materialized_output
  merge_sidecar::tests::reconnect_during_materialization_keeps_old_authorization_but_emits_new_tenure
  merge_sidecar::tests::conflicting_server_request_id_reuse_is_rejected_before_materialization
  merge_sidecar::tests::failed_materialization_releases_rate_gate_for_exact_retry
  merge_sidecar::tests::response_materialization_requires_and_consumes_its_exact_admission_gate
  merge_sidecar::tests::inactive_reply_route_is_rejected_before_server_gate_admission
  merge_sidecar::tests::completed_source_later_and_reconnect_stay_terminal_while_sibling_progresses
  merge_sidecar::tests::exact_delivery_retry_rematerializes_after_rate_gate_expiry
  merge_sidecar::tests::completed_source_does_not_block_a_new_alternate_source
  merge_sidecar::tests::configured_route_source_capacity_bounds_semantic_attempts
  merge_sidecar::tests::configured_source_geometry_reserves_more_than_eight_independent_attempts
  merge_sidecar::tests::fifth_gate_from_one_hub_is_rejected_while_another_hub_progresses
  merge_sidecar::tests::third_session_from_one_hub_is_rejected_while_another_hub_progresses
  merge_sidecar::tests::source_byte_overflow_is_rejected_while_another_hub_progresses
  merge_sidecar::tests::completed_short_session_replacement_cannot_starve_an_older_long_session
  merge_sidecar::tests::route_retirement_between_admission_and_enqueue_releases_all_response_reservations
  merge_sidecar::tests::saturated_materializer_does_not_erase_same_request_alternate_session
  merge_sidecar::tests::saturated_materializer_does_not_erase_same_request_alternate_bytes
  merge_sidecar::tests::partitioned_materialization_preserves_rejected_source_resume_cursor
  merge_sidecar::tests::sidecar_flush_refinement_advances_only_exact_source_chunk
  merge_sidecar::tests::authenticated_source_limits_are_fixed_at_four_gates_two_sessions_and_sixteen_mibibytes
  merge_sidecar::tests::cached_sidecar_payload_objects_scale_with_chunks_not_sources
  merge_sidecar::tests::sidecar_admission_matches_the_cached_arc_without_changing_ownership
  sumeragi::v2::tests::deferred_locked_commit_delivery_tracks_generation_after_tc
  sumeragi::v2::tests::prelock_current_commit_is_readmitted_after_exact_lock_persistence
  sumeragi::v2::tests::tc_reset_readmits_exact_locked_commit_once_per_generation
  sumeragi::v2::tests::tc_promoted_historical_commit_is_fsynced_before_sign_and_status
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
  sumeragi::v2::tests::deferred_adapter_rejects_foreign_and_replayed_capabilities_before_reducer_step
  sumeragi::v2::tests::deferred_authenticated_retry_retains_exact_original_and_effective_tags
  sumeragi::v2::tests::deferred_ordinal_exhaustion_fails_adapter_closed_before_wrap
  sumeragi::v2::tests::deferred_service_debt_overflow_is_typed_and_fail_closed
  sumeragi::v2::tests::deferred_service_evidence_rejects_every_owner_and_rank_mutation
  sumeragi::v2::tests::deferred_zero_ordinal_is_exact_single_use_and_never_reminted
  sumeragi::v2::tests::authenticated_deferred_service_rejects_same_kind_envelope_swap_before_reducer
  sumeragi::v2_block_sync::tests::discovery_outputs_only_normal_commit_qc_ingress_and_waits_for_enqueue
  sumeragi::v2_block_sync::tests::catch_up_is_strictly_sequential_across_contexts
  sumeragi::v2_block_sync::tests::historical_body_comes_from_kura_and_only_a_certified_signer_can_serve
  sumeragi::v2_apply::tests::committed_merge_reservation_rejects_bare_norito
  sumeragi::v2_effects::tests::retained_locked_body_survives_same_lock_view_churn_before_fetch_adopts_it
  sumeragi::v2_effects::tests::higher_different_lock_releases_retained_cache_before_replacement_staging
  sumeragi::v2_effects::tests::higher_round_same_subject_reuses_only_the_view_independent_locked_cache
  sumeragi::v2_effects::tests::queued_protected_store_keeps_one_work_id_across_repeated_tcs
  sumeragi::v2_effects::tests::tc_body_rebind_preserves_the_exact_fetch_until_reconstruction_completes
  sumeragi::v2_effects::tests::tc_body_rebind_preserves_certified_request_ownership_through_signed_response
  sumeragi::v2_effects::tests::tc_body_rebind_uses_the_effective_local_lock_when_the_tc_omits_or_lowers_it
  sumeragi::v2_effects::tests::enter_view_rejects_a_tc_high_without_an_effective_protected_body
  sumeragi::v2_effects::tests::tc_body_rebind_retags_a_queued_body_available_completion
  sumeragi::v2_effects::tests::tc_body_rebind_retires_a_superseded_completion_and_releases_capacity
  sumeragi::v2_effects::tests::serialized_runtime_rebinds_busy_deferred_body_completion_before_service
  sumeragi::v2_effects::tests::tc_body_rebind_cancels_fetch_superseded_by_a_higher_different_qc
  sumeragi::v2_effects::tests::same_tag_higher_lock_retires_reproposal_round_ownership_before_staging
  sumeragi::v2_effects::tests::same_tag_higher_lock_retires_fetch_store_and_validation_owners
  sumeragi::v2_effects::tests::first_lock_retires_unlocked_fetch_store_and_validation_owners
  sumeragi::v2_effects::tests::first_lock_retires_queued_store_validation_and_local_proposal_completions
  sumeragi::v2_effects::tests::lock_reconciliation_rejects_same_round_conflict_and_late_lower_lock
  sumeragi::v2_effects::tests::failed_lock_cleanup_keeps_exact_owner_and_requires_restart
  sumeragi::v2_effects::tests::lock_cleanup_rejects_inconsistent_certified_request_before_mutation
  sumeragi::v2_effects::tests::lock_cleanup_status_failure_preserves_committed_replacement
  sumeragi::v2_effects::tests::higher_round_same_subject_preserves_current_proposal_pipeline_with_same_tag
  sumeragi::v2_effects::tests::decision_installed_by_same_runtime_step_retires_stale_terminal_effects
  sumeragi::v2_effects::tests::decision_installed_by_same_runtime_step_keeps_exact_commit_and_body_work
  sumeragi::v2_effects::tests::decision_installation_frees_losing_capacity_before_fetch
  sumeragi::v2_effects::tests::failed_decision_cleanup_keeps_losing_owner_and_requires_restart
  sumeragi::v2_effects::tests::decision_cleanup_fetch_failure_preserves_exact_local_pipeline_consumer
  sumeragi::v2_effects::tests::decision_cleanup_rejects_inconsistent_certified_request_before_mutation
  sumeragi::v2_effects::tests::decision_rebinds_exact_local_validation_to_reducer_progress
  sumeragi::v2_effects::tests::decision_preserves_current_tag_local_proposal_for_direct_apply
  sumeragi::v2_effects::tests::decision_commitment_mismatch_fails_closed_before_apply
  sumeragi::v2_effects::tests::stale_generation_local_completion_uses_durable_recovery
  sumeragi::v2_effects::tests::missing_merge_sidecar_retains_exact_validation_until_retry
  sumeragi::v2_effects::tests::decided_apply_retries_after_exact_merge_sidecar_recovery
  sumeragi::v2_effects::tests::production_certified_body_request_rejects_locally_conflicting_qc_without_fail_close
  sumeragi::v2_effects::tests::production_commit_certificate_response_conflict_keeps_discovery_outstanding_and_runtime_open
  sumeragi::v2_effects::tests::proposal_a_distinct_prepare_qc_b_and_timeout_sign_progress_at_capacity_two
  sumeragi::v2_effects::tests::serialized_runtime_emits_proposal_a_prepare_qc_b_timeout_capacity_trace
  sumeragi::v2_effects::tests::full_capacity_certified_fetch_remains_missing_and_retransmit_later_adopts_it
  sumeragi::v2_effects::tests::certified_request_pressure_leaves_higher_authority_upgrade_for_retransmission
  sumeragi::v2_effects::tests::reconstructible_new_certified_fetch_acquires_ownership_after_retransmission
  sumeragi::v2_effects::tests::production_capacity_saturation_admits_response_and_reconstructible_fetch
  sumeragi::v2_effects::tests::durable_sign_preemption_orders_speculative_certified_and_locked_fetches
  sumeragi::v2_effects::tests::retained_producer_suffix_allows_exact_payload_chunk_to_release_fetch_capacity
  sumeragi::v2_effects::tests::retained_producer_suffix_allows_exact_certified_response_to_release_fetch_capacity
  sumeragi::v2_effects::tests::retained_effect_batch_rejects_overtaking_and_oversize_before_partial_dispatch
  sumeragi::v2_effects::tests::retained_effect_tail_is_fifo_and_refilters_after_durable_decision
  sumeragi::v2_effects::tests::pending_work_producer_inventory_is_exhaustive_and_source_linked
  sumeragi::v2_effects::tests::reconciled_decision_rejects_same_round_subject_commitment_drift
  sumeragi::v2_effects::tests::runtime_step_dispatches_entire_effect_batch_before_returning
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
  sumeragi::v2_lane_work::tests::native_amx_signing_guard_capacity_preserves_small_product
  sumeragi::v2_lane_work::tests::native_amx_signing_guard_capacity_preserves_exact_hard_boundary
  sumeragi::v2_lane_work::tests::native_amx_signing_guard_capacity_caps_deployed_localnet_oversized_product
  sumeragi::v2_lane_work::tests::native_amx_signing_guard_capacity_rejects_usize_overflow
  sumeragi::v2_lane_work::tests::native_amx_adapter_opens_with_bounded_production_like_limits
  sumeragi::v2_lane_work::tests::native_amx_request_rejects_inactive_reply_route_before_signing
  sumeragi::v2_lane_work::tests::persisted_lane_session_uses_only_selected_qc_signer_pops
  sumeragi::v2_lane_work::tests::same_proposal_shortcut_rejects_unvalidated_certificate_variants
  sumeragi::v2_lane_work::tests::planner_view_one_binds_rotated_global_leader_to_fresh_lane_view
  sumeragi::v2_lane_work::tests::enabled_nexus_binds_independent_lane_author_distinct_from_global_leader
  sumeragi::v2_lane_work::tests::lane_work_stays_quiescent_until_the_exact_global_prepare_lock
  sumeragi::v2_lane_work::tests::global_body_lock_replacement_requires_higher_prepare_round_and_exact_subject
  sumeragi::v2_lane_work::tests::superseded_commit_protected_lane_session_cannot_retransmit
  sumeragi::v2_lane_work::tests::same_body_binds_after_prepare_lock_advances_beyond_header_view
  sumeragi::v2_lane_work::tests::validator_storage_platform_gate_rejects_voters_and_allows_observers
  sumeragi::v2_lane_work::tests::durable_lane_certificate_is_one_atomic_kura_backed_response
  sumeragi::v2_lane_work::tests::durable_lane_certificate_serves_rotated_validator_after_pressure
  sumeragi::v2_lane_work::tests::historical_certificate_survives_successor_lock_decision_persistence_and_restart
  sumeragi::v2_lane_work::tests::carrier_replacement_filters_persistence_and_output_sources_together
  sumeragi::v2_lane_work::tests::duplicate_reply_effect_preserves_exact_source_delivery
  sumeragi::v2_lane_work::tests::reply_effect_rejects_missing_or_retargeted_route_set
  sumeragi::v2_lane_work::tests::duplicate_reply_effect_updates_only_later_delivery_from_same_source
  sumeragi::v2_lane_work::tests::duplicate_reply_effect_retains_alternate_sources_across_source_update
  sumeragi::v2_lane_work::tests::temporarily_unserviceable_effect_requeues_behind_later_reserved_work
  sumeragi::v2_lane_work::tests::retired_sidecar_route_between_drain_and_lane_queue_preserves_live_sibling
  sumeragi::v2_lane_work::tests::durable_lane_certificate_coalescing_preserves_alternate_ingress_owners
  sumeragi::v2_runtime::tests::retiring_exact_body_completion_releases_a_capacity_one_ingress_slot
  sumeragi::v2_runtime::tests::exact_authenticated_progress_retransmission_is_queue_coalesced
  sumeragi::v2_runtime::tests::commit_certificate_response_coalesces_with_exact_busy_deferred_qc
  sumeragi::v2_runtime::tests::completion_retries_coalesce_across_ingress_and_busy_deferred_ownership
  sumeragi::v2_runtime::tests::body_available_rebind_rejects_uninstalled_destination_without_mutation
  sumeragi::v2_runtime::tests::body_available_rebind_coalesces_exact_busy_deferred_destination_owner
  sumeragi::v2_runtime::tests::body_available_rebind_destination_conflicts_and_duplicates_fail_closed_before_mutation
  sumeragi::v2_runtime::tests::duplicate_body_available_rebind_and_retirement_fail_closed_before_mutation
  sumeragi::v2_runtime::tests::unbound_direct_vote_authentication_is_recoverable_and_becomes_admissible_after_validation
  sumeragi::v2_runtime::tests::conflicting_body_pipeline_evidence_fails_closed_before_body_available_pruning
  sumeragi::v2_runtime::tests::conflicting_local_and_validated_receipts_do_not_coalesce
  sumeragi::v2_runtime::tests::production_busy_transfer_retains_exact_validation_evidence_for_retry_and_cleanup
  sumeragi::v2_runtime::tests::body_pipeline_retirement_spans_ingress_and_busy_deferred_owners_and_rejects_duplicates
  sumeragi::v2_runtime::tests::decision_retires_proposal_owners_but_preserves_body_and_application_completions
  sumeragi::v2_runtime::tests::decision_retires_stale_local_completion_for_durable_recovery
  sumeragi::v2_runtime::tests::progress_cursor_decision_preserves_outer_ingress_completion_until_apply
  sumeragi::v2_runtime::tests::decision_cleanup_preserves_unique_busy_deferred_completion
  sumeragi::v2_runtime::tests::decision_commitment_mismatch_fails_closed_before_retirement
  sumeragi::v2_runtime::tests::successor_activation_snapshot_requires_armed_live_clocks
  sumeragi::v2_runtime::tests::production_ingress_pop_uses_shared_selector_for_every_ready_mask
  sumeragi::v2_runtime::tests::network_admission_uses_exact_normal_and_progress_reservations
  sumeragi::v2_runtime::tests::serviceable_adapter_debt_drains_one_macro_step_before_new_work
  sumeragi::v2_runtime::tests::serviceable_adapter_debt_runs_without_runtime_ingress
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
  sumeragi::v2_recovery::tests::all_hash_only_snapshot_recovers_exact_authenticated_successor
  sumeragi::v2_recovery::tests::finalized_tip_derives_one_idempotent_successor_context
  sumeragi::v2_recovery::tests::successor_rejects_foreign_same_height_predecessor_and_mismatched_receipt
  sumeragi::v2_runner::tests::same_tag_higher_lock_retires_all_local_proposal_owners
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
  sumeragi::v2_runner::tests::runner_dispatch_rejects_certified_sidecar_chunk_without_reply_route
  sumeragi::v2_runner::tests::runner_dispatch_rejects_durable_response_without_reply_routes
  sumeragi::v2_runner::tests::first_same_subject_lock_preserves_pending_local_proposal_events
  sumeragi::v2_runner::tests::higher_same_subject_lock_keeps_one_local_proposal_owner
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
  sumeragi::v2_worker::tests::fetch_consumer_rebind_preserves_live_or_queued_reconstruction_owner
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
  sumeragi::v2_worker::tests::exact_output_coalescing_preserves_distinct_fair_ingress_admissions
  sumeragi::v2_worker::tests::same_tenure_updates_and_reconnect_preserve_current_item
  sumeragi::v2_worker::tests::closed_sidecar_source_reconnect_retries_current_item_while_sibling_backpressures
  sumeragi::v2_worker::tests::closed_sidecar_reconnect_is_capacity_checked_then_retries_current_item
  sumeragi::v2_worker::tests::later_delivery_cannot_requeue_pending_or_unapplied_sidecar_flush_but_other_attempts_progress
  sumeragi::v2_worker::tests::mixed_source_retry_retains_terminal_flush_target_without_resetting_live_siblings
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
  sumeragi::v2_worker::tests::sidecar_flush_ack_identity_mismatch_fails_closed
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
  sumeragi::status::v2_liveness_watchdog_tests::blocker_classifier_has_stable_specific_precedence
  sumeragi::status::v2_liveness_watchdog_tests::current_view_timeout_path_supersedes_prepare_but_not_any_locked_commit
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
  sumeragi::status::v2_liveness_watchdog_tests::rejected_running_successor_failure_projection_still_latches_restart_required
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
  peer::run::tests::dispatch_worker_shutdown_drains_reliable_old_generation_to_actor
  peer::run::tests::full_write_without_flush_ack_closes_actor_witness_and_retries_on_replacement
  network::tests::authenticated_source_count_share_is_checked_and_never_zero
  network::tests::actor_progress_bypasses_full_deferred_owner_and_waits_for_writer_flush
  network::tests::actor_progress_lease_survives_topology_transition
  network::tests::actor_progress_retries_exactly_once_on_peer_writer_replacement
  network::tests::actor_progress_retry_round_robin_bypasses_partitioned_target
  network::tests::cap_one_blocked_source_cannot_prevent_live_source_service
  network::tests::actor_progress_lease_survives_debug_packet_loss_until_delivery_retries
  network::tests::actor_broadcast_retry_targets_only_failed_peers
  network::tests::reliable_subscriber_is_single_consumer_under_clone_budget_pressure
  network::tests::reconnecting_peer_cannot_multiply_retained_source_credits
  network::tests::progress_budget_preserves_fifo_for_three_registered_producers
  network::tests::reliable_progress_class_matches_actor_reservations_exactly
  network::tests::reply_flush_identity_binds_ticket_tenure_source_payload_and_delivery_occurrence
  network::tests::reply_flush_test_fixture_binds_exact_canonical_post_and_opaque_actor
  network::tests::peer_message_mints_actor_global_delivery_ordinals_across_connection_tenures
  network::tests::reply_route_survives_peer_message_clone_mapping_and_split
  network::tests::peer_message_rehydration_rejects_second_reply_route_without_retargeting
  network::tests::reply_source_key_groups_relay_origins_and_orders_actor_instances
  network::tests::reply_route_source_updates_are_ordinal_monotonic_and_target_scoped
  network::tests::dependent_test_fixture_mints_opaque_tenures_and_delivery_ordinals
  network::tests::cancelled_newer_hub_cannot_erase_older_independent_route_attempt
  network::tests::dependent_fixture_models_bounded_actor_global_multi_hub_ownership
  network::tests::reply_route_pruning_retains_equal_ordinal_tenure_tombstone
  network::tests::reply_route_binding_rejects_evicted_tombstone_collision
  network::tests::reply_route_set_isolates_sources_preserves_cursors_and_prunes_retired_capacity
  network::tests::route_cancelled_between_preflight_and_admission_retires_without_queue_ownership
  network::tests::reply_admission_rejects_retargeting_foreign_handles_and_wrong_tickets
  network::tests::reply_actor_admission_does_not_complete_writer_flush_ack
  network::tests::reply_flush_ack_cancellation_between_precheck_and_budget_lock_returns_none
  network::tests::retired_reply_tenure_closes_flush_ack_without_false_completion
  network::tests::reply_flush_test_fixture_controls_success_and_close_without_false_receipts
  network::tests::reply_flush_ack_completes_only_after_peer_writer_flush
  network::tests::progress_ticket_rejects_a_different_same_length_payload
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
  network::tests::accepted_draining_generation_delivers_reliable_progress_after_replacement
  network::tests::targetized_broadcast_coalesces_only_the_same_digest_and_membership
  network::tests::distinct_broadcast_residual_is_target_isolated_and_its_rank_decreases
  network::tests::exact_broadcast_retry_coalesces_but_distinct_and_direct_requests_do_not
  network::tests::removed_membership_cancels_only_old_broadcast_debt_across_readd
  network::tests::cancelled_target_child_with_pending_flush_ack_releases_exactly_once
  network::tests::requested_topology_is_not_authority_and_closed_fanout_returns_all_targets
  network::tests::reliable_delivery_waits_for_its_route_subscriber
  network::tests::closed_reliable_subscriber_transfers_actor_pending_backlog_to_replacement
  network::tests::network_actor_drop_retires_routes_and_only_its_waiters
  network::inbound_source_memory_bound_tests::reliable_actor_waiter_geometry_rejects_zero_and_combined_overflow
  network::handle_update_tests::configured_producer_geometry_gives_six_same_source_waiters_decreasing_ranks
  consensus_message_control::tests::controlled_v2_admission_preserves_distinct_relay_identity
  consensus_message_control::tests::failed_release_clears_in_flight_ownership_and_latches_fatal
  consensus_message_control::tests::fatal_controller_rejects_an_unchanged_command_poll
  consensus_message_control::tests::retired_release_finishes_drain_without_claiming_delivery
  network_relay_tests::obsolete_sumeragi_relay_message_completes_as_delivered
  network_relay_tests::test_control_hold_release_preserves_live_route_and_retires_canceled_reentry
  tests::relay_fairness::daemon_source_credit_layers_over_upstream_and_preserves_the_ninth_exact_owner
  tests::relay_fairness::saturated_sumeragi_dispatch_does_not_hold_normal_worker_permits
  tests::relay_fairness::real_inner_ingress_retry_preserves_a_copies_and_bounds_b_service_rank
  tests::relay_fairness::same_source_safety_and_shared_high_credits_cross_daemon_without_head_of_line_wait
  tests::relay_fairness::base_one_four_sources_reserve_both_upstream_lanes_without_head_of_line_wait
  genesis_bootstrap::tests::pending_reply_count_uses_shared_per_source_waiter_geometry
  genesis_bootstrap::tests::genesis_request_fanout_deduplicates_same_source_targets
  genesis_bootstrap::tests::bootstrapper_clones_cannot_multiply_listener_producers
  genesis_bootstrap::tests::bootstrapper_clones_cannot_multiply_fetch_fanouts
  parameters::actual::tests::sumeragi_v2_exact_output_geometry_checks_every_arithmetic_boundary
  parameters::user::duration_clamp_tests::sumeragi_v2_exact_output_geometry_accepts_network_source_boundary
  parameters::user::duration_clamp_tests::sumeragi_v2_exact_output_geometry_accepts_equal_capacity_boundary
  parameters::user::duration_clamp_tests::sumeragi_v2_exact_output_geometry_rejects_unreservable_network_sources
)
readonly expected_production_liveness_test_count=465
if (( ${#required_production_liveness_tests[@]} != expected_production_liveness_test_count )); then
  echo "expected exactly ${expected_production_liveness_test_count} production Sumeragi v2 liveness tests, found ${#required_production_liveness_tests[@]}" >&2
  exit 1
fi
production_unit_list="$(cargo test --locked -p iroha_core --lib -- --list)"
production_ignored_unit_list="$(
  cargo test --locked -p iroha_core --lib -- --list --ignored
)"
production_integration_unit_list="$(
  cargo test --locked -p integration_tests --test sumeragi_v2_runner_isolated -- --list
)"
production_integration_ignored_unit_list="$(
  cargo test --locked -p integration_tests --test sumeragi_v2_runner_isolated -- --list --ignored
)"
# This source-bound corridor intentionally exercises `iroha_p2p`'s production
# default feature set (`default = []`). Feature-gated QUIC first-packet geometry
# tests remain useful transport regressions, but are not claimed by this
# twenty-nine-module pre-network inventory.
production_p2p_unit_list="$(cargo test --locked -p iroha_p2p --lib -- --list)"
production_p2p_ignored_unit_list="$(
  cargo test --locked -p iroha_p2p --lib -- --list --ignored
)"
production_irohad_unit_list="$(
  cargo test --locked -p irohad --bin irohad --features test-network-message-control -- --list
)"
production_irohad_ignored_unit_list="$(
  cargo test --locked -p irohad --bin irohad --features test-network-message-control -- --list --ignored
)"
production_config_unit_list="$(cargo test --locked -p iroha_config --lib -- --list)"
production_config_ignored_unit_list="$(
  cargo test --locked -p iroha_config --lib -- --list --ignored
)"
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
    || "$required_test" == tests::relay_fairness::* \
    || "$required_test" == genesis_bootstrap::tests::* ]]; then
    required_unit_list="$production_irohad_unit_list"
    required_ignored_unit_list="$production_irohad_ignored_unit_list"
  elif [[ "$required_test" == parameters::* ]]; then
    required_unit_list="$production_config_unit_list"
    required_ignored_unit_list="$production_config_ignored_unit_list"
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
production_liveness_modules=(
  kura::tests
  kura::lane_geometry::tests
  nexus::lane_relay::tests
  sumeragi::authoritative_runtime_gate_tests
  merge_sidecar::tests
  sumeragi::v2_core::tests
  sumeragi::v2_core::refinement::tests
  sumeragi::v2_core::reducer::source_link_tests
  sumeragi::v2::tests
  sumeragi::v2_block_sync::tests
  sumeragi::v2_apply::tests
  sumeragi::v2_effects::tests
  sumeragi::v2_lane_work::tests
  sumeragi::v2_runtime::tests
  sumeragi::v2_recovery::tests
  sumeragi::v2_runner::tests
  sumeragi::v2_worker::tests
  sumeragi::status::v2_liveness_watchdog_tests
  sumeragi_v2_runner
  peer::run::tests
  peer::shared_byte_budget_tests
  network::tests
  network::inbound_source_memory_bound_tests
  network::handle_update_tests
  consensus_message_control::tests
  network_relay_tests
  tests::relay_fairness
  genesis_bootstrap::tests
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
  production-v2-core-source-link
  production-v2-adapter
  production-v2-block-sync
  production-v2-apply
  production-v2-effects
  production-v2-lane-work
  production-v2-runtime
  production-v2-recovery
  production-v2-runner
  production-v2-worker
  production-v2-watchdog
  production-v2-integration-runner
  production-p2p-peer-reliable-flush
  production-p2p-shared-source-byte-geometry
  production-p2p-network-reliable-actor
  production-p2p-source-memory-geometry
  production-p2p-waiter-rank-geometry
  production-irohad-consensus-message-control
  production-irohad-network-relay
  production-irohad-authenticated-via
  production-irohad-genesis-reply-geometry
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
    module_command="cargo test --locked -p integration_tests --test sumeragi_v2_runner_isolated sumeragi_v2_runner::prepare_qc_split_tests -- --test-threads=1"
    run_corridor_leg \
      "$module_leg_id" cargo-module "$module_required_count" "$module_command" \
      cargo test --locked -p integration_tests --test sumeragi_v2_runner_isolated \
        sumeragi_v2_runner::prepare_qc_split_tests -- --test-threads=1
  elif [[ "$module" == peer::run::tests \
    || "$module" == peer::shared_byte_budget_tests \
    || "$module" == network::tests \
    || "$module" == network::inbound_source_memory_bound_tests \
    || "$module" == network::handle_update_tests ]]; then
    module_command="cargo test --locked -p iroha_p2p --lib ${module} -- --test-threads=1"
    run_corridor_leg \
      "$module_leg_id" cargo-module "$module_required_count" "$module_command" \
      cargo test --locked -p iroha_p2p --lib "$module" -- --test-threads=1
  elif [[ "$module" == consensus_message_control::tests \
    || "$module" == network_relay_tests \
    || "$module" == tests::relay_fairness \
    || "$module" == genesis_bootstrap::tests ]]; then
    module_command="cargo test --locked -p irohad --bin irohad --features test-network-message-control ${module} -- --test-threads=1"
    run_corridor_leg \
      "$module_leg_id" cargo-module "$module_required_count" "$module_command" \
      cargo test --locked -p irohad --bin irohad --features test-network-message-control \
        "$module" -- --test-threads=1
  elif [[ "$module" == parameters::* ]]; then
    module_command="cargo test --locked -p iroha_config --lib ${module} -- --test-threads=1"
    run_corridor_leg \
      "$module_leg_id" cargo-module "$module_required_count" "$module_command" \
      cargo test --locked -p iroha_config --lib "$module" -- --test-threads=1
  else
    module_command="cargo test --locked -p iroha_core --lib ${module} -- --test-threads=1"
    run_corridor_leg \
      "$module_leg_id" cargo-module "$module_required_count" "$module_command" \
      cargo test --locked -p iroha_core --lib "$module" -- --test-threads=1
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
data_model_unit_list="$(cargo test --locked -p iroha_data_model --lib -- --list)"
data_model_ignored_unit_list="$(
  cargo test --locked -p iroha_data_model --lib -- --list --ignored
)"
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
  "cargo test --locked -p iroha_data_model --lib ${required_data_model_status_test} -- --test-threads=1" \
  cargo test --locked -p iroha_data_model --lib "$required_data_model_status_test" -- --test-threads=1
run_corridor_leg \
  lane-certificate-rust cargo-exact 1 \
  "cargo test --locked -p iroha_data_model --lib ${required_data_model_lane_certificate_test} -- --exact --test-threads=1" \
  cargo test --locked -p iroha_data_model --lib "$required_data_model_lane_certificate_test" -- --exact --test-threads=1
verify_release_identity "before source-sealed full workspace verification"
run_corridor_leg \
  source-sealed-workspace-clippy command 0 \
  "cargo clippy --workspace --all-targets -- -D warnings" \
  cargo clippy --workspace --all-targets -- -D warnings
run_corridor_leg \
  source-sealed-workspace-tests command 0 \
  "cargo test --locked --workspace" \
  cargo test --locked --workspace
run_corridor_leg \
  source-sealed-irohad-tests command 0 \
  "cargo test --locked -p irohad --bin irohad --features test-network-message-control" \
  cargo test --locked -p irohad --bin irohad --features test-network-message-control
verify_release_identity "after source-sealed full workspace verification"

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
  cargo test --locked -p integration_tests --test "$taira_release_contract_target" -- --list
)"
taira_release_ignored_contract_list="$(
  cargo test --locked -p integration_tests --test "$taira_release_contract_target" -- --list --ignored
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
    "cargo test --locked -p integration_tests --test ${taira_release_contract_target} ${required_test} -- --exact --test-threads=1" \
    cargo test --locked -p integration_tests --test "$taira_release_contract_target" \
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
  cargo test --locked -p iroha_data_model --test "$cross_sdk_fixture_target" -- --list
)"
cross_sdk_ignored_fixture_list="$(
  cargo test --locked -p iroha_data_model --test "$cross_sdk_fixture_target" -- --list --ignored
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
  "cargo test --locked -p iroha_data_model --test ${cross_sdk_fixture_target} sumeragi_v2_cross_sdk_fixtures:: -- --test-threads=1" \
  cargo test --locked -p iroha_data_model --test "$cross_sdk_fixture_target" \
    sumeragi_v2_cross_sdk_fixtures:: -- --test-threads=1

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
  grep -Ec '^11 passed in [0-9]+([.][0-9]+)?s$' "$seed_launcher_contract_log" || true
)"
if ((seed_launcher_pipeline_status[0] != 0 || seed_launcher_pipeline_status[1] != 0)) \
  || [[ "$seed_launcher_pass_summary" != 1 ]]; then
  echo "Sumeragi v2 seed-launcher contract preflight did not run exactly 11 passing tests (pytest=${seed_launcher_pipeline_status[0]}, tee=${seed_launcher_pipeline_status[1]})" >&2
  exit 1
fi
record_corridor_log \
  preflight-seed-launcher pytest 11 \
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
    grep -Ec '^71 passed in [0-9]+([.][0-9]+)?s( \([0-9]+:[0-5][0-9]:[0-5][0-9]\))?$' \
      "$release_bootstrap_contract_log" || true
  )"
  if ((release_bootstrap_pipeline_status[0] != 0 || release_bootstrap_pipeline_status[1] != 0)) \
    || [[ "$release_bootstrap_pass_summary" != 1 ]]; then
    echo "Sumeragi v2 release-bootstrap preflight did not run exactly 71 passing tests (pytest=${release_bootstrap_pipeline_status[0]}, tee=${release_bootstrap_pipeline_status[1]})" >&2
    exit 1
  fi
  record_corridor_log \
    preflight-release-bootstrap pytest 71 \
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
)
release_receipt_contract_log="$(corridor_contract_log_path preflight-release-receipt)"
set +e
PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider \
  "${release_receipt_contract_files[@]}" 2>&1 | tee "$release_receipt_contract_log"
release_receipt_pipeline_status=("${PIPESTATUS[@]}")
set -e
release_receipt_pass_summary="$(
  grep -Ec '^189 passed in [0-9]+([.][0-9]+)?s( \([0-9]+:[0-5][0-9]:[0-5][0-9]\))?$' \
    "$release_receipt_contract_log" || true
)"
if ((release_receipt_pipeline_status[0] != 0 || release_receipt_pipeline_status[1] != 0)) \
  || [[ "$release_receipt_pass_summary" != 1 ]]; then
  echo "Sumeragi v2 aggregate-receipt contract preflight did not run exactly 189 passing tests (pytest=${release_receipt_pipeline_status[0]}, tee=${release_receipt_pipeline_status[1]})" >&2
  exit 1
fi
record_corridor_log \
  preflight-release-receipt pytest 189 \
  "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider ${release_receipt_contract_files[*]}" \
  "$release_receipt_contract_log" \
  "${release_receipt_pipeline_status[0]}" "${release_receipt_pipeline_status[1]}"
((corridor_enabled)) || rm -f -- "$release_receipt_contract_log"

# Run the complete fail-closed proof-ledger, Verus-evidence, and TLC-trace
# normalizer contract corpus before trusting any expensive formal result. The
# exact pass count rejects deleted, added, skipped, or xfailed fidelity cases.
proof_fidelity_contract_files=(
  pytests/scripts/sumeragi_v2_proof_ledger_test.py
  pytests/scripts/sumeragi_v2_verus_evidence_test.py
  pytests/scripts/sumeragi_v2_tlc_trace_normalizer_test.py
)
proof_fidelity_contract_log="$(corridor_contract_log_path preflight-proof-fidelity)"
set +e
PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider \
  "${proof_fidelity_contract_files[@]}" 2>&1 | tee "$proof_fidelity_contract_log"
proof_fidelity_pipeline_status=("${PIPESTATUS[@]}")
set -e
proof_fidelity_pass_summary="$(
  grep -Ec '^1044 passed in [0-9]+([.][0-9]+)?s( \([0-9]+:[0-5][0-9]:[0-5][0-9]\))?$' "$proof_fidelity_contract_log" || true
)"
if ((proof_fidelity_pipeline_status[0] != 0 || proof_fidelity_pipeline_status[1] != 0)) \
  || [[ "$proof_fidelity_pass_summary" != 1 ]]; then
  echo "Sumeragi v2 proof-fidelity preflight did not run exactly 1044 passing tests (pytest=${proof_fidelity_pipeline_status[0]}, tee=${proof_fidelity_pipeline_status[1]})" >&2
  exit 1
fi
record_corridor_log \
  preflight-proof-fidelity pytest 1044 \
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
  grep -Ec '^16 passed in [0-9]+([.][0-9]+)?s$' "$formal_launcher_contract_log" || true
)"
if ((formal_launcher_pipeline_status[0] != 0 || formal_launcher_pipeline_status[1] != 0)) \
  || [[ "$formal_launcher_pass_summary" != 1 ]]; then
  echo "Sumeragi v2 formal-launcher contract preflight did not run exactly 16 passing tests (pytest=${formal_launcher_pipeline_status[0]}, tee=${formal_launcher_pipeline_status[1]})" >&2
  exit 1
fi
record_corridor_log \
  preflight-formal-launcher pytest 16 \
  "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider ${formal_launcher_contract_files[*]}" \
  "$formal_launcher_contract_log" \
  "${formal_launcher_pipeline_status[0]}" "${formal_launcher_pipeline_status[1]}"
((corridor_enabled)) || rm -f -- "$formal_launcher_contract_log"

# Run the complete mocked soak launcher/evidence corpus as one exact file-bound
# preflight. The 39-pass summary rejects missing, added, skipped, or xfailed
# cases before the release corridor can trust the 24-hour evidence path.
taira_soak_contract_files=(
  pytests/scripts/taira_v2_soak_test.py
  pytests/scripts/taira_v2_soak_evidence_test.py
)
taira_soak_contract_log="$(corridor_contract_log_path preflight-taira-soak)"
set +e
PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider \
  "${taira_soak_contract_files[@]}" 2>&1 | tee "$taira_soak_contract_log"
taira_soak_pipeline_status=("${PIPESTATUS[@]}")
set -e
taira_soak_pass_summary="$(
  grep -Ec '^39 passed in [0-9]+([.][0-9]+)?s$' "$taira_soak_contract_log" || true
)"
if ((taira_soak_pipeline_status[0] != 0 || taira_soak_pipeline_status[1] != 0)) \
  || [[ "$taira_soak_pass_summary" != 1 ]]; then
  echo "Taira v2 soak launcher/evidence preflight did not run exactly 39 passing tests (pytest=${taira_soak_pipeline_status[0]}, tee=${taira_soak_pipeline_status[1]})" >&2
  exit 1
fi
record_corridor_log \
  preflight-taira-soak pytest 39 \
  "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider ${taira_soak_contract_files[*]}" \
  "$taira_soak_contract_log" \
  "${taira_soak_pipeline_status[0]}" "${taira_soak_pipeline_status[1]}"
((corridor_enabled)) || rm -f -- "$taira_soak_contract_log"
if ((corridor_enabled)); then
  readonly expected_corridor_leg_count=53
  if ((corridor_leg_index != expected_corridor_leg_count)); then
    echo "release corridor recorded ${corridor_leg_index} legs, expected ${expected_corridor_leg_count}" >&2
    exit 1
  fi
  corridor_summary_sha256="$(sha256_file "$corridor_summary")"
  corridor_required_tests_sha256="$(sha256_file "$corridor_required_tests")"
  corridor_java_path="$(python3 -I -S -c \
    'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve(strict=True))' \
    "$JAVA_BIN")"
  corridor_java_sha256="$(sha256_file "$corridor_java_path")"
  corridor_cargo_path="$(canonical_executable cargo)"
  corridor_rustc_path="$(canonical_executable rustc)"
  corridor_python_path="$(canonical_executable python3)"
  corridor_node_path="$(canonical_executable node)"
  corridor_bash_path="$(canonical_executable bash)"
  corridor_git_path="$(canonical_executable git)"
  corridor_cargo_home="$(canonical_path "$CARGO_HOME")"
  corridor_completion_tmp="${corridor_evidence_dir}/.COMPLETED.tsv.$$"
  printf '%s\t%s\n' \
    schema_version 1 \
    head_commit "$release_head_commit" \
    head_tree "$release_head_tree" \
    source_manifest_sha256 "$release_source_manifest_sha256" \
    cargo_lock_sha256 "$release_cargo_lock_sha256" \
    leg_count "$corridor_leg_index" \
    production_required_test_count "$expected_production_liveness_test_count" \
    summary_sha256 "$corridor_summary_sha256" \
    production_required_tests_sha256 "$corridor_required_tests_sha256" \
    java_path "$corridor_java_path" \
    java_sha256 "$corridor_java_sha256" \
    cargo_path "$corridor_cargo_path" \
    cargo_sha256 "$(sha256_file "$corridor_cargo_path")" \
    cargo_version "$("$corridor_cargo_path" --version)" \
    rustc_path "$corridor_rustc_path" \
    rustc_sha256 "$(sha256_file "$corridor_rustc_path")" \
    rustc_version "$("$corridor_rustc_path" --version)" \
    python3_path "$corridor_python_path" \
    python3_sha256 "$(sha256_file "$corridor_python_path")" \
    node_path "$corridor_node_path" \
    node_sha256 "$(sha256_file "$corridor_node_path")" \
    bash_path "$corridor_bash_path" \
    bash_sha256 "$(sha256_file "$corridor_bash_path")" \
    git_path "$corridor_git_path" \
    git_sha256 "$(sha256_file "$corridor_git_path")" \
    cargo_home_path "$corridor_cargo_home" \
    repo_cargo_config_sha256 "$(sha256_file "$repo_root/.cargo/config.toml")" \
    tlc_profile "$SUMERAGI_V2_TLC_PROFILE" \
    tlaps_threads "$SUMERAGI_TLAPS_THREADS" \
    >"$corridor_completion_tmp"
  mv -- "$corridor_completion_tmp" "$corridor_completion_path"
fi
verify_release_identity "after release contract preflights"

if [[ "$profile" == "--release" ]]; then
  # Fail before 160 real-network runs when the strict deductive ledger or its
  # source-bound backend evidence is not release-complete.
  formal_completion_path_file="${IROHA_RELEASE_HOST_ROOT}/formal-completion-path"
  rm -f -- "$formal_completion_path_file"
  IROHA_FORMAL_COMPLETION_PATH_FILE="$formal_completion_path_file" \
    bash scripts/run_sumeragi_v2_formal_release.sh
  if [[ ! -s "$formal_completion_path_file" ]]; then
    echo "strict formal release gate did not publish its completion path" >&2
    exit 1
  fi
  verify_release_identity "after strict formal corridor"
fi

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

if [[ "$profile" == "--pr" ]]; then
  python3 -I -S scripts/formal/check_sumeragi_v2_proof_ledger.py
  bash scripts/formal/run_sumeragi_v2_harness.sh --unit
  bash scripts/formal/run_sumeragi_v2_harness.sh --fast-network
  bash scripts/formal/run_sumeragi_v2_harness.sh --model-replay
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
  echo "Sumeragi v2 PR gate passed: cross-SDK fixture/status parity, 4 seeds × 5 scenarios (20 runs), reducer invariants, adversarial simulations, and trace replay" >&2
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
  --repository-root "$repo_root" \
  --output "$IROHA_RELEASE_AGGREGATE_RECEIPT_PATH"

echo "Sumeragi v2 production release gates passed, including 100,000 heights and the 24-hour Taira soak; receipt=${IROHA_RELEASE_AGGREGATE_RECEIPT_PATH}" >&2
