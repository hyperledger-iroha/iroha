#!/usr/bin/env bash
# Execute the source-bound Sumeragi v2 PR or production-release corridor.

set -euo pipefail

profile="${1:---pr}"
if [[ $# -gt 1 ]] || [[ "$profile" != "--pr" && "$profile" != "--release" ]]; then
  echo "usage: $0 [--pr|--release]" >&2
  exit 2
fi

readonly repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
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
  python3 -c \
    'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve(strict=True))' \
    "$executable"
}

canonical_path() {
  python3 -c \
    'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve(strict=True))' \
    "$1"
}

release_identity_json() {
  python3 scripts/compute_workspace_source_manifest.py \
    --root "$repo_root" \
    --release-identity-json
}

identity_field() {
  local identity_path="$1"
  local field="$2"
  python3 -c \
    'import json, sys; print(json.load(open(sys.argv[1], encoding="utf-8"))[sys.argv[2]])' \
    "$identity_path" "$field"
}

# Production evidence never executes in the caller's mutable checkout. Require
# one committed source tree, reproduce it in a unique detached worktree, copy
# the separately-bound ignored Cargo.lock, and remove source write permission.
# The complete corridor then re-enters this script from that sealed source.
if [[ "$profile" == "--release" && "${IROHA_RELEASE_SEALED_WORKTREE:-0}" != 1 ]]; then
  release_git_bin="$(canonical_executable git)" || {
    echo "a resolved Git executable is required for the release corridor" >&2
    exit 1
  }
  release_python_bin="$(canonical_executable python3)" || {
    echo "a resolved Python executable is required for the release corridor" >&2
    exit 1
  }
  release_bash_bin="$(canonical_executable bash)" || {
    echo "a resolved Bash executable is required for the release corridor" >&2
    exit 1
  }
  readonly release_git_bin release_python_bin release_bash_bin
  export PATH="$(dirname "$release_git_bin"):$(dirname "$release_python_bin"):$(dirname "$release_bash_bin"):${PATH}"
  candidate_identity_json="$(release_identity_json)"
  candidate_manifest_sha256="$(
    python3 -c 'import json, sys; print(json.loads(sys.argv[1])["workspace_source_manifest_sha256"])' \
      "$candidate_identity_json"
  )"
  candidate_head="$(
    python3 -c 'import json, sys; print(json.loads(sys.argv[1])["head_commit"])' \
      "$candidate_identity_json"
  )"
  readonly candidate_identity_json candidate_manifest_sha256 candidate_head

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
      echo "unsupported formal release host: $(uname -s)-$(uname -m)" >&2
      exit 1
      ;;
  esac
  readonly release_tlapm_platform release_verus_platform
  readonly release_verus_sha256 release_cargo_verus_sha256
  readonly release_tlapm_root="${repo_root}/target/tlapm/toolchains/763bf3c1826d77a4cf206f43d5aa16775da1da33/${release_tlapm_platform}"
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
    || "$($release_tlapm_bin --version 2>&1)" != "763bf3c" \
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

  readonly release_host_base="/tmp/iroha-sumeragi-v2-release-host-${UID}/${candidate_manifest_sha256}"
  mkdir -p -- "$release_host_base"
  release_invocation_root="$(mktemp -d "${release_host_base}/invocation.XXXXXX")"
  readonly release_invocation_root
  readonly sealed_repo_root="${release_invocation_root}/source"
  readonly release_host_root="${release_invocation_root}/output"
  readonly candidate_identity_path="${release_invocation_root}/candidate-identity.json"
  readonly sealed_identity_path="${release_invocation_root}/sealed-identity.json"
  readonly aggregate_receipt_path_file="${release_host_root}/release-receipt-path"
  mkdir -p -- \
    "$release_host_root/workspace-target" \
    "$release_host_root/tmp" \
    "$release_host_root/cache" \
    "$release_host_root/cargo-home"
  for cargo_cache in registry git; do
    if [[ -d "${inherited_cargo_cache_home}/${cargo_cache}" ]]; then
      ln -s "$(canonical_path "${inherited_cargo_cache_home}/${cargo_cache}")" \
        "$release_host_root/cargo-home/${cargo_cache}"
    fi
  done
  printf '%s\n' "$candidate_identity_json" >"$candidate_identity_path"

  sealed_worktree_added=0
  cleanup_sealed_worktree() {
    local status=$?
    if ((sealed_worktree_added)); then
      if [[ -d "$sealed_repo_root" ]]; then
        python3 "$repo_root/scripts/seal_workspace_source.py" \
          --unseal --root "$sealed_repo_root" >/dev/null 2>&1 || status=1
      fi
      "$release_git_bin" -C "$repo_root" worktree remove --force "$sealed_repo_root" \
        >/dev/null 2>&1 || status=1
    fi
    trap - EXIT
    exit "$status"
  }
  trap cleanup_sealed_worktree EXIT

  "$release_git_bin" worktree add --detach "$sealed_repo_root" "$candidate_head"
  sealed_worktree_added=1
  cp -- "$repo_root/Cargo.lock" "$sealed_repo_root/Cargo.lock"
  reproduced_identity_json="$(
    python3 "$sealed_repo_root/scripts/compute_workspace_source_manifest.py" \
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
  python3 "$sealed_repo_root/scripts/seal_workspace_source.py" \
    --seal --root "$sealed_repo_root" --writable target
  python3 "$sealed_repo_root/scripts/seal_workspace_source.py" \
    --verify --root "$sealed_repo_root" --writable target
  # Keep both digests. The candidate manifest records the original checkout;
  # the sealed manifest may change because the manifest intentionally binds all
  # permission bits. Every child build/evidence item uses the sealed digest,
  # while the final receipt also binds candidate HEAD/tree/Cargo.lock.
  sealed_identity_json="$(
    python3 "$sealed_repo_root/scripts/compute_workspace_source_manifest.py" \
      --root "$sealed_repo_root" \
      --release-identity-json
  )"
  printf '%s\n' "$sealed_identity_json" >"$sealed_identity_path"
  chmod 0444 "$candidate_identity_path" "$sealed_identity_path"

  set +e
  IROHA_RELEASE_SEALED_WORKTREE=1 \
    IROHA_RELEASE_SEALED_ROOT="$sealed_repo_root" \
    IROHA_RELEASE_HOST_ROOT="$release_host_root" \
    IROHA_RELEASE_WORKSPACE_TARGET="$release_host_root/workspace-target" \
    IROHA_RELEASE_INVOCATION_ROOT="$release_invocation_root" \
    IROHA_RELEASE_CANDIDATE_IDENTITY_PATH="$candidate_identity_path" \
    IROHA_RELEASE_EXPECTED_IDENTITY_PATH="$sealed_identity_path" \
    IROHA_RELEASE_AGGREGATE_RECEIPT_PATH_FILE="$aggregate_receipt_path_file" \
    IROHA_RELEASE_TLAPM_BIN="$release_tlapm_bin" \
    IROHA_RELEASE_TLAPM_STDLIB="$release_tlapm_stdlib" \
    IROHA_RELEASE_TLA2TOOLS_JAR="$release_tla2tools_jar" \
    IROHA_RELEASE_VERUS_DIR="$release_verus_dir" \
    IROHA_RELEASE_JAVA_BIN="$release_java_bin" \
    IROHA_RELEASE_GIT_BIN="$release_git_bin" \
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
    if [[ ! -s "$aggregate_receipt_path_file" ]]; then
      echo "sealed release corridor passed without an aggregate receipt" >&2
      sealed_status=1
    else
      echo "aggregate release receipt: $(<"$aggregate_receipt_path_file")" >&2
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
    || -z "${IROHA_RELEASE_AGGREGATE_RECEIPT_PATH_FILE:-}" \
    || ! -f "$IROHA_RELEASE_EXPECTED_IDENTITY_PATH" \
    || ! -f "$IROHA_RELEASE_CANDIDATE_IDENTITY_PATH" ]]; then
    echo "production release corridor must run from its sealed detached worktree" >&2
    return 1
  fi
  local observed_target expected_target
  if [[ ! -L "$repo_root/target" ]]; then
    echo "sealed release target is not the external output symlink at ${checkpoint}" >&2
    return 1
  fi
  observed_target="$(
    python3 -c 'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve(strict=True))' \
      "$repo_root/target"
  )"
  expected_target="$(
    python3 -c 'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve(strict=True))' \
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
  if ! python3 scripts/seal_workspace_source.py \
    --verify --root "$repo_root" --writable target; then
    echo "release source seal changed at ${checkpoint}" >&2
    return 1
  fi
}

verify_release_identity "release corridor entry"

# Both profiles are release evidence. Bind the complete corridor, including the
# PR matrix, to one checkout manifest and one content-addressed build root.
release_source_manifest_sha256="$(
  python3 scripts/compute_workspace_source_manifest.py --root "$repo_root"
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
  export PATH="${IROHA_RELEASE_VERUS_DIR}:$(dirname "$IROHA_RELEASE_GIT_BIN"):$(dirname "$IROHA_RELEASE_CARGO_BIN"):$(dirname "$IROHA_RELEASE_RUSTC_BIN"):$(dirname "$IROHA_RELEASE_PYTHON_BIN"):$(dirname "$IROHA_RELEASE_NODE_BIN"):$(dirname "$IROHA_RELEASE_BASH_BIN"):${PATH}"
  for pinned_tool in git cargo rustc python3 node bash; do
    pinned_variable="IROHA_RELEASE_${pinned_tool^^}_BIN"
    [[ "$pinned_tool" == python3 ]] && pinned_variable=IROHA_RELEASE_PYTHON_BIN
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
  observed_test_count="$(python3 - "$kind" "$log_path" <<'PY'
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
        re.fullmatch(r"([0-9]+) passed in [0-9]+(?:\.[0-9]+)?s", line)
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
  sumeragi::v2_core::refinement::tests::retransmit_may_reconstruct_one_final_decision_body_stage
  sumeragi::v2_core::refinement::tests::source_linked_effective_lock_body_kernels_reject_adversarial_inputs
  sumeragi::v2_core::reducer::source_link_tests::retransmit_body_stage_requires_an_exact_durable_decision_capability
  sumeragi::v2_core::reducer::source_link_tests::replay_refinement_binds_the_complete_durable_fifo
  sumeragi::v2_core::reducer::source_link_tests::replay_refinement_rejects_malformed_post_states_even_with_the_right_first_effect
  sumeragi::authoritative_runtime_gate_tests::anonymous_and_non_roster_v2_sources_share_one_bounded_lane
  sumeragi::authoritative_runtime_gate_tests::byzantine_v2_source_cannot_consume_honest_ingress_reservations_or_service_turns
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_minimum_capacity_admits_timeout_votes
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_prepare_vote_cannot_consume_commit_progress_reservation
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_reservation_potential_does_not_increase_on_service
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_reserves_timeout_vote_bytes_behind_auxiliary_pressure
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_rejects_timeout_vote_larger_than_its_byte_reserve
  sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_saturated_peer_cannot_block_an_empty_validator_timeout
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
  sumeragi::v2::tests::deferred_service_cursor_advances_across_busy_front_requeue
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
  sumeragi::v2_effects::tests::runtime_step_dispatches_entire_effect_batch_before_returning
  sumeragi::v2_effects::tests::failed_view_cleanup_keeps_stale_fetch_and_requires_restart
  sumeragi::v2_effects::tests::view_cleanup_rejects_inconsistent_protected_request_before_lock_mutation
  sumeragi::v2_effects::tests::view_cleanup_second_cancellation_failure_commits_no_fetch_retirement
  sumeragi::v2_lane_work::tests::direct_decision_quiesces_losing_lane_and_retransmission_work
  sumeragi::v2_lane_work::tests::native_amx_signing_guard_capacity_preserves_small_product
  sumeragi::v2_lane_work::tests::native_amx_signing_guard_capacity_preserves_exact_hard_boundary
  sumeragi::v2_lane_work::tests::native_amx_signing_guard_capacity_caps_deployed_localnet_oversized_product
  sumeragi::v2_lane_work::tests::native_amx_signing_guard_capacity_rejects_usize_overflow
  sumeragi::v2_lane_work::tests::native_amx_adapter_opens_with_bounded_production_like_limits
  sumeragi::v2_lane_work::tests::persisted_lane_session_uses_only_selected_qc_signer_pops
  sumeragi::v2_lane_work::tests::planner_view_one_binds_rotated_global_leader_to_fresh_lane_view
  sumeragi::v2_lane_work::tests::enabled_nexus_binds_independent_lane_author_distinct_from_global_leader
  sumeragi::v2_lane_work::tests::lane_work_stays_quiescent_until_the_exact_global_prepare_lock
  sumeragi::v2_lane_work::tests::global_body_lock_replacement_requires_higher_prepare_round_and_exact_subject
  sumeragi::v2_lane_work::tests::superseded_commit_protected_lane_session_cannot_retransmit
  sumeragi::v2_lane_work::tests::same_body_binds_after_prepare_lock_advances_beyond_header_view
  sumeragi::v2_runtime::tests::retiring_exact_body_completion_releases_a_capacity_one_ingress_slot
  sumeragi::v2_runtime::tests::exact_authenticated_progress_retransmission_is_queue_coalesced
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
  sumeragi::v2_recovery::tests::all_hash_only_snapshot_recovers_exact_authenticated_successor
  sumeragi::v2_recovery::tests::finalized_tip_derives_one_idempotent_successor_context
  sumeragi::v2_runner::tests::same_tag_higher_lock_retires_all_local_proposal_owners
  sumeragi::v2_runner::tests::first_same_subject_lock_preserves_pending_local_proposal_events
  sumeragi::v2_runner::tests::higher_same_subject_lock_keeps_one_local_proposal_owner
  sumeragi::v2_runner::tests::late_old_rejection_cannot_arm_heartbeat_for_replacement_lock
  sumeragi::v2_runner::tests::decision_retires_local_work_before_prepared_delivery
  sumeragi::v2_runner::tests::finalized_rollover_closes_ingress_before_successor_replay
  sumeragi::v2_runner::tests::successor_activation_is_published_only_after_ingress_is_open
  sumeragi::v2_runner::tests::complete_tip_recovery_uses_the_same_live_successor_boundary
  sumeragi::v2_runner::tests::successor_startup_failure_stays_running_and_fails_closed_without_activation
  sumeragi::v2_runner::tests::status_guard_retains_failure_snapshot_and_clears_clean_shutdown
  sumeragi::v2_worker::tests::fetch_consumer_rebind_preserves_live_or_queued_reconstruction_owner
  sumeragi::v2_worker::tests::invalid_fetch_consumer_rebind_fails_closed_without_consuming_owner
  sumeragi::v2_worker::tests::locked_candidate_requests_coalesce_by_immutable_subject
  sumeragi::v2_worker::tests::locked_candidate_completion_uses_latest_consumer_without_reloading
  sumeragi::v2_worker::tests::locked_candidate_consumer_rebind_rejects_stale_or_regressive_tags
  sumeragi::v2_worker::tests::locked_candidate_duplicate_or_wrong_completion_is_rejected
  sumeragi::v2_worker::tests::higher_different_lock_replaces_load_and_retires_stale_completion
  sumeragi::v2_worker::tests::superseded_locked_candidate_failure_starts_latest_acquisition
  sumeragi::v2_worker::tests::unavailable_locked_candidate_waits_for_matching_durable_store
  sumeragi::v2_worker::tests::outbound_payload_retention_is_constant_across_many_view_changes
  sumeragi::v2_worker::tests::late_stale_proposal_signature_cannot_restore_pruned_outbound_payload
  sumeragi::v2_worker::tests::nonzero_view_proposal_intent_replays_through_production_services
  sumeragi::v2_worker::tests::decision_retires_candidate_and_outbound_work_but_keeps_exact_sidecar_deferral
  sumeragi::v2_worker::tests::worker_completion_is_retained_behind_a_full_runtime_fifo
  sumeragi::v2_worker::tests::production_drain_publishes_worker_completion_behind_full_runtime_fifo
  sumeragi::v2_worker::tests::successful_auxiliary_drain_republishes_cleared_completion_ownership
  sumeragi::v2_worker::tests::auxiliary_completion_drain_is_batch_bounded
  sumeragi::status::v2_liveness_watchdog_tests::blocker_classifier_has_stable_specific_precedence
  sumeragi::status::v2_liveness_watchdog_tests::locked_candidate_load_overlay_precedes_commit_quorum_diagnosis
  sumeragi::status::v2_liveness_watchdog_tests::aged_queue_without_service_debt_does_not_claim_scheduler_starvation
  sumeragi::status::v2_liveness_watchdog_tests::network_ingress_service_clock_distinguishes_stopped_and_active_scans
  sumeragi::status::v2_liveness_watchdog_tests::repeated_tc_reconstruction_of_same_locked_commit_pool_does_not_reset_height_clock
  sumeragi::status::v2_liveness_watchdog_tests::apply_waiting_on_merge_sidecar_is_application_pending_not_body_unavailable
  sumeragi::status::v2_liveness_watchdog_tests::effect_completion_overlay_preserves_capacity_age_and_service_debt
  sumeragi::status::v2_liveness_watchdog_tests::live_effect_completion_observer_survives_stopped_runner_and_clears_stale_depth
  sumeragi::status::v2_liveness_watchdog_tests::successor_handoff_is_visible_until_the_exact_successor_becomes_active_once
  sumeragi::status::v2_liveness_watchdog_tests::completed_predecessor_work_alone_never_claims_successor_activation
  sumeragi::status::v2_liveness_watchdog_tests::complete_tip_recovery_publishes_one_exact_live_successor_boundary
  sumeragi::status::v2_liveness_watchdog_tests::rejected_successor_activation_never_mutates_or_replaces_the_predecessor
  sumeragi::status::v2_liveness_watchdog_tests::successor_handoff_rejects_every_incomplete_predecessor_witness
  sumeragi::status::v2_liveness_watchdog_tests::successor_startup_overlays_never_cross_the_height_context_boundary
)
readonly expected_production_liveness_test_count=166
if (( ${#required_production_liveness_tests[@]} != expected_production_liveness_test_count )); then
  echo "expected exactly ${expected_production_liveness_test_count} production Sumeragi v2 liveness tests, found ${#required_production_liveness_tests[@]}" >&2
  exit 1
fi
production_unit_list="$(cargo test --locked -p iroha_core --lib -- --list)"
production_ignored_unit_list="$(
  cargo test --locked -p iroha_core --lib -- --list --ignored
)"
for required_test in "${required_production_liveness_tests[@]}"; do
  if ! grep -Fqx -- "${required_test}: test" <<<"$production_unit_list"; then
    echo "missing required production Sumeragi v2 liveness test: ${required_test}" >&2
    exit 1
  fi
  if grep -Fqx -- "${required_test}: test" <<<"$production_ignored_unit_list"; then
    echo "required production Sumeragi v2 liveness test is ignored: ${required_test}" >&2
    exit 1
  fi
done
production_liveness_modules=(
  sumeragi::authoritative_runtime_gate_tests
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
)
production_liveness_leg_ids=(
  production-authoritative-ingress
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
  module_command="cargo test --locked -p iroha_core --lib ${module} -- --test-threads=1"
  run_corridor_leg \
    "$module_leg_id" cargo-module "$module_required_count" "$module_command" \
    cargo test --locked -p iroha_core --lib "$module" -- --test-threads=1
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
run_corridor_leg \
  status-rust cargo-exact 1 \
  "cargo test --locked -p iroha_data_model --lib ${required_data_model_status_test} -- --test-threads=1" \
  cargo test --locked -p iroha_data_model --lib "$required_data_model_status_test" -- --test-threads=1
verify_release_identity "after production liveness modules"

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
)
seed_launcher_contract_log="$(corridor_contract_log_path preflight-seed-launcher)"
set +e
PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider \
  "${seed_launcher_contract_tests[@]}" 2>&1 | tee "$seed_launcher_contract_log"
seed_launcher_pipeline_status=("${PIPESTATUS[@]}")
set -e
seed_launcher_pass_summary="$(
  grep -Ec '^10 passed in [0-9]+([.][0-9]+)?s$' "$seed_launcher_contract_log" || true
)"
if ((seed_launcher_pipeline_status[0] != 0 || seed_launcher_pipeline_status[1] != 0)) \
  || [[ "$seed_launcher_pass_summary" != 1 ]]; then
  echo "Sumeragi v2 seed-launcher contract preflight did not run exactly ten passing tests (pytest=${seed_launcher_pipeline_status[0]}, tee=${seed_launcher_pipeline_status[1]})" >&2
  exit 1
fi
record_corridor_log \
  preflight-seed-launcher pytest 10 \
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
  grep -Ec '^37 passed in [0-9]+([.][0-9]+)?s$' "$release_receipt_contract_log" || true
)"
if ((release_receipt_pipeline_status[0] != 0 || release_receipt_pipeline_status[1] != 0)) \
  || [[ "$release_receipt_pass_summary" != 1 ]]; then
  echo "Sumeragi v2 aggregate-receipt contract preflight did not run exactly 37 passing tests (pytest=${release_receipt_pipeline_status[0]}, tee=${release_receipt_pipeline_status[1]})" >&2
  exit 1
fi
record_corridor_log \
  preflight-release-receipt pytest 37 \
  "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider ${release_receipt_contract_files[*]}" \
  "$release_receipt_contract_log" \
  "${release_receipt_pipeline_status[0]}" "${release_receipt_pipeline_status[1]}"
((corridor_enabled)) || rm -f -- "$release_receipt_contract_log"

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
  grep -Ec '^12 passed in [0-9]+([.][0-9]+)?s$' "$formal_launcher_contract_log" || true
)"
if ((formal_launcher_pipeline_status[0] != 0 || formal_launcher_pipeline_status[1] != 0)) \
  || [[ "$formal_launcher_pass_summary" != 1 ]]; then
  echo "Sumeragi v2 formal-launcher contract preflight did not run exactly twelve passing tests (pytest=${formal_launcher_pipeline_status[0]}, tee=${formal_launcher_pipeline_status[1]})" >&2
  exit 1
fi
record_corridor_log \
  preflight-formal-launcher pytest 12 \
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
  readonly expected_corridor_leg_count=29
  if ((corridor_leg_index != expected_corridor_leg_count)); then
    echo "release corridor recorded ${corridor_leg_index} legs, expected ${expected_corridor_leg_count}" >&2
    exit 1
  fi
  corridor_summary_sha256="$(sha256_file "$corridor_summary")"
  corridor_required_tests_sha256="$(sha256_file "$corridor_required_tests")"
  corridor_java_path="$(python3 -c \
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
  # Fail before 128 real-network runs when the strict deductive ledger or its
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
  python3 scripts/formal/check_sumeragi_v2_proof_ledger.py
  bash scripts/formal/run_sumeragi_v2_harness.sh --unit
  bash scripts/formal/run_sumeragi_v2_harness.sh --fast-network
  bash scripts/formal/run_sumeragi_v2_harness.sh --model-replay
  final_pr_source_manifest_sha256="$(
    python3 scripts/compute_workspace_source_manifest.py --root "$repo_root"
  )"
  if [[ ! "$final_pr_source_manifest_sha256" =~ ^[0-9a-f]{64}$ ]]; then
    echo "workspace source manifest helper returned an invalid digest after the PR corridor" >&2
    exit 1
  fi
  if [[ "$final_pr_source_manifest_sha256" != "$release_source_manifest_sha256" ]]; then
    echo "workspace sources changed during the PR release corridor" >&2
    exit 1
  fi
  echo "Sumeragi v2 PR gate passed: cross-SDK fixture/status parity, 4 seeds × 4 scenarios (16 runs), reducer invariants, adversarial simulations, and trace replay" >&2
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
  python3 scripts/compute_workspace_source_manifest.py --root "$repo_root"
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
  python3 scripts/compute_workspace_source_manifest.py --root "$repo_root"
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
# edit during chaos or soak execution cannot inherit stale TLAPS success.
python3 scripts/formal/check_sumeragi_v2_proof_ledger.py \
  --release \
  --evidence target/formal/sumeragi_v2/proof_evidence.json
verify_release_identity "after final proof-evidence validation"

seed_completion_path="$(<"$seed_completion_path_file")"
formal_completion_path="$(<"$formal_completion_path_file")"
chaos_completion_path="$(<"$chaos_completion_path_file")"
taira_completion_path="$(<"$taira_completion_path_file")"
release_receipt_root="${release_source_bound_root}/evidence/release"
mkdir -p -- "$release_receipt_root"
release_receipt_dir="$(mktemp -d "${release_receipt_root}/invocation.XXXXXX")"
release_receipt_path="${release_receipt_dir}/RELEASE_COMPLETED.json"
release_receipt_partial="${release_receipt_dir}/.RELEASE_COMPLETED.partial.json"
receipt_pointer_tmp="${IROHA_RELEASE_AGGREGATE_RECEIPT_PATH_FILE}.$$"
terminal_receipt_published=0
cleanup_unpublished_terminal_receipt() {
  local status=$?
  if ((terminal_receipt_published == 0)); then
    rm -f -- \
      "$release_receipt_partial" \
      "$release_receipt_path" \
      "$receipt_pointer_tmp" \
      "$IROHA_RELEASE_AGGREGATE_RECEIPT_PATH_FILE"
  fi
  trap - EXIT
  exit "$status"
}
trap cleanup_unpublished_terminal_receipt EXIT
python3 scripts/write_sumeragi_v2_release_receipt.py \
  --candidate-identity "$IROHA_RELEASE_CANDIDATE_IDENTITY_PATH" \
  --sealed-identity "$IROHA_RELEASE_EXPECTED_IDENTITY_PATH" \
  --corridor-completion "$corridor_completion_path" \
  --formal-completion "$formal_completion_path" \
  --seed-completion "$seed_completion_path" \
  --chaos-completion "$chaos_completion_path" \
  --taira-completion "$taira_completion_path" \
  --output "$release_receipt_partial"
verify_release_identity "before aggregate release receipt promotion"
mv -- "$release_receipt_partial" "$release_receipt_path"
canonical_release_receipt="$(
  python3 -c 'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve(strict=True))' \
    "$release_receipt_path"
)"
printf '%s\n' "$canonical_release_receipt" >"$receipt_pointer_tmp"
mv -- "$receipt_pointer_tmp" "$IROHA_RELEASE_AGGREGATE_RECEIPT_PATH_FILE"
terminal_receipt_published=1
trap - EXIT

echo "Sumeragi v2 production release gates passed, including 100,000 heights and the 24-hour Taira soak; receipt=${canonical_release_receipt}" >&2
