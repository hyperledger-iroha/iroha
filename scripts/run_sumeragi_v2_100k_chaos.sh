#!/usr/bin/env bash
# Run and attest the exact 100,000-height Sumeragi v2 chaos gate.

set -euo pipefail

if (($#)); then
  echo "usage: $0" >&2
  exit 2
fi

readonly repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

identity_field() {
  python3 -c \
    'import json, sys; print(json.loads(sys.argv[1])[sys.argv[2]])' \
    "$1" "$2"
}

expected_identity="$(
  python3 scripts/compute_workspace_source_manifest.py \
    --root "$repo_root" --release-identity-json
)"
if [[ -n "${IROHA_RELEASE_EXPECTED_IDENTITY_PATH:-}" ]]; then
  parent_identity="$(<"$IROHA_RELEASE_EXPECTED_IDENTITY_PATH")"
  if [[ "$expected_identity" != "$parent_identity" ]]; then
    echo "100,000-height chaos source identity disagrees with the parent release" >&2
    exit 1
  fi
fi

readonly source_manifest_sha256="$(
  identity_field "$expected_identity" workspace_source_manifest_sha256
)"
readonly head_commit="$(identity_field "$expected_identity" head_commit)"
readonly head_tree="$(identity_field "$expected_identity" head_tree)"
readonly cargo_lock_sha256="$(identity_field "$expected_identity" cargo_lock_sha256)"
if [[ -n "${IROHA_RELEASE_SOURCE_MANIFEST_SHA256:-}" \
  && "$IROHA_RELEASE_SOURCE_MANIFEST_SHA256" != "$source_manifest_sha256" ]]; then
  echo "100,000-height chaos source manifest disagrees with the parent release" >&2
  exit 1
fi

verify_identity() {
  local checkpoint="$1"
  local observed
  if ! observed="$(
    python3 scripts/compute_workspace_source_manifest.py \
      --root "$repo_root" --release-identity-json
  )"; then
    echo "failed to compute chaos source identity at ${checkpoint}" >&2
    return 1
  fi
  if [[ "$observed" != "$expected_identity" ]]; then
    echo "100,000-height chaos source identity changed at ${checkpoint}" >&2
    return 1
  fi
  if [[ "${IROHA_RELEASE_SEALED_WORKTREE:-0}" == 1 ]]; then
    python3 scripts/seal_workspace_source.py \
      --verify --root "$repo_root" --writable target
  fi
}

readonly evidence_root="${SUMERAGI_V2_CHAOS_EVIDENCE_DIR:-${repo_root}/target/sumeragi-v2-release/${source_manifest_sha256}/evidence/chaos-100k}"
mkdir -p -- "$evidence_root"
readonly evidence_lock="${evidence_root}/.chaos-100k.lock"
if ! mkdir -- "$evidence_lock"; then
  echo "another 100,000-height chaos gate owns ${evidence_lock}" >&2
  exit 1
fi
cleanup_lock() {
  local status=$?
  rm -f -- "${evidence_lock}/owner"
  rmdir -- "$evidence_lock" || status=1
  trap - EXIT
  exit "$status"
}
trap cleanup_lock EXIT
printf 'pid=%s\nhead_commit=%s\nsource_manifest_sha256=%s\n' \
  "$$" "$head_commit" "$source_manifest_sha256" >"${evidence_lock}/owner"

invocation_dir="$(mktemp -d "${evidence_root}/invocation.XXXXXX")"
readonly invocation_dir
readonly run_log="${invocation_dir}/chaos-100k.log"
readonly invocation_attestation="${invocation_dir}/invocation.tsv"
readonly completion_attestation="${invocation_dir}/COMPLETED.tsv"
printf '%s\t%s\n' \
  schema_version 1 \
  head_commit "$head_commit" \
  head_tree "$head_tree" \
  source_manifest_sha256 "$source_manifest_sha256" \
  cargo_lock_sha256 "$cargo_lock_sha256" \
  expected_heights 100000 \
  permissioned_heights 50000 \
  npos_heights 50000 \
  >"$invocation_attestation"

verify_identity "before execution"
set +e
bash scripts/formal/run_sumeragi_v2_harness.sh --chaos-100k \
  2>&1 | tee "$run_log"
pipeline_status=("${PIPESTATUS[@]}")
set -e
if ((pipeline_status[0] != 0 || pipeline_status[1] != 0)); then
  echo "100,000-height chaos command failed (harness=${pipeline_status[0]}, tee=${pipeline_status[1]})" >&2
  exit 1
fi
verify_identity "after execution"

running_one="$(grep -Fxc 'running 1 test' "$run_log" || true)"
passing_one="$(
  grep -Ec '^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in .+$' \
    "$run_log" || true
)"
if [[ "$running_one" != 1 || "$passing_one" != 1 ]] \
  || ! grep -Fq \
    'test accelerated_100_000_block_chaos_preserves_chain_prefix ... ok' \
    "$run_log"; then
  echo "100,000-height chaos output does not prove exactly one passing release test" >&2
  exit 1
fi

log_sha256="$(hash_file "$run_log")"
completion_tmp="${invocation_dir}/.COMPLETED.tsv.$$"
printf '%s\t%s\n' \
  schema_version 1 \
  head_commit "$head_commit" \
  head_tree "$head_tree" \
  source_manifest_sha256 "$source_manifest_sha256" \
  cargo_lock_sha256 "$cargo_lock_sha256" \
  completed_heights 100000 \
  log_sha256 "$log_sha256" \
  >"$completion_tmp"
mv -- "$completion_tmp" "$completion_attestation"
if ! verify_identity "after completion attestation"; then
  rm -f -- "$completion_attestation"
  exit 1
fi

if [[ -n "${IROHA_CHAOS_COMPLETION_PATH_FILE:-}" ]]; then
  completion_path_tmp="${IROHA_CHAOS_COMPLETION_PATH_FILE}.$$"
  printf '%s\n' "$completion_attestation" >"$completion_path_tmp"
  mv -- "$completion_path_tmp" "$IROHA_CHAOS_COMPLETION_PATH_FILE"
fi
echo "100,000-height chaos gate passed; completion=${completion_attestation}" >&2
