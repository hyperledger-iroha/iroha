#!/usr/bin/env bash
# Run and attest the exact 100,000-height certificate-supplied reducer chaos gate.

set -euo pipefail

if (($#)); then
  echo "usage: $0" >&2
  exit 2
fi

readonly repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
source "${repo_root}/scripts/sumeragi_v2_release_process_policy.sh"

release_root_input_count=0
for release_root_name in \
  CARGO_TARGET_DIR \
  IROHA_RELEASE_ARTIFACT_ROOT \
  IROHA_RELEASE_CANCEL_REQUEST_PATH; do
  if [[ -n "${!release_root_name:-}" ]]; then
    release_root_input_count=$((release_root_input_count + 1))
  fi
done
if ((release_root_input_count != 0 && release_root_input_count != 3)); then
  echo "CARGO_TARGET_DIR, IROHA_RELEASE_ARTIFACT_ROOT, and IROHA_RELEASE_CANCEL_REQUEST_PATH must be supplied all-or-none" >&2
  exit 2
fi
if ((release_root_input_count == 0)); then
  chaos_invocation_root="$(
    mktemp -d /private/tmp/iroha-sumeragi-v2-chaos-100k.XXXXXX
  )"
  mkdir -m 0700 -- \
    "$chaos_invocation_root/target" \
    "$chaos_invocation_root/artifacts"
  export CARGO_TARGET_DIR="$chaos_invocation_root/target"
  export IROHA_RELEASE_ARTIFACT_ROOT="$chaos_invocation_root/artifacts"
  export IROHA_RELEASE_CANCEL_REQUEST_PATH="$chaos_invocation_root/cancel-request.json"
fi
require_external_cargo_target_dir "$repo_root"
require_external_release_artifact_root "$repo_root"
require_disjoint_release_roots "$repo_root"
release_gate_boundary "chaos-100k:entry" || exit $?

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
export IROHA_RELEASE_SOURCE_MANIFEST_SHA256="$source_manifest_sha256"

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
      --verify --root "$repo_root" --no-writable-paths
  fi
}

evidence_root="${SUMERAGI_V2_CHAOS_EVIDENCE_DIR:-${IROHA_RELEASE_ARTIFACT_ROOT}/sumeragi-v2-release/${source_manifest_sha256}/evidence/chaos-100k}"
require_release_artifact_path "$evidence_root" || exit $?
mkdir -p -- "$evidence_root"
evidence_root="$(cd -- "$evidence_root" && pwd -P)"
require_release_artifact_directory "$evidence_root" || exit $?
readonly evidence_root
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
  schema_version 2 \
  head_commit "$head_commit" \
  head_tree "$head_tree" \
  source_manifest_sha256 "$source_manifest_sha256" \
  cargo_lock_sha256 "$cargo_lock_sha256" \
  expected_heights 100000 \
  permissioned_heights 50000 \
  npos_heights 50000 \
  restart_interval 64 \
  duplicate_interval 32 \
  under_quorum_interval 97 \
  certificate_source external_fixture \
  >"$invocation_attestation"

verify_identity "before execution"
release_gate_boundary "chaos-100k:harness:before" || exit $?
set +e
bash scripts/formal/run_sumeragi_v2_harness.sh --chaos-100k \
  2>&1 | tee "$run_log"
pipeline_status=("${PIPESTATUS[@]}")
set -e
release_gate_boundary "chaos-100k:harness:after" || exit $?
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
readonly chaos_completion_marker='SUMERAGI_V2_CHAOS_COMPLETED permissioned_heights=50000 npos_heights=50000 total_heights=100000 supplied_commit_qcs=100000 supplied_tcs=75000 finalized_validators=400000 wal_append_restarts=314 fetch_restarts=312 store_restarts=312 validation_restarts=312 application_restarts=312 stale_generation_rejections=1562 deferred_fetch_completions=400936 deferred_store_completions=400624 deferred_validation_completions=400312 deferred_application_completions=400000 duplicate_commit_qcs=3124 reordered_commit_batches=75000 reordered_tc_batches=75000 insufficient_dual_qcs=1030 count_only_qcs=515 power_only_qcs=515 restart_interval=64 duplicate_interval=32 under_quorum_interval=97 certificate_source=external_fixture'
readonly chaos_test_prefix='test accelerated_100_000_block_chaos_preserves_chain_prefix ... '
readonly chaos_test_completion_line="${chaos_test_prefix}ok"
completion_marker_lines="$(grep -Fxc -- "$chaos_completion_marker" "$run_log" || true)"
completion_test_lines="$(grep -Fxc -- "$chaos_test_completion_line" "$run_log" || true)"
if [[ "$running_one" != 1 || "$passing_one" != 1 ]] \
  || [[ "$completion_marker_lines" != 1 ]] \
  || [[ "$completion_test_lines" != 1 ]] \
  || [[ "$(grep -Fc -- "$chaos_test_prefix" "$run_log" || true)" != 1 ]]; then
  echo "100,000-height chaos output does not prove exactly one passing release test" >&2
  exit 1
fi

log_sha256="$(hash_file "$run_log")"
completion_body="$(printf '%s\t%s\n' \
  schema_version 2 \
  head_commit "$head_commit" \
  head_tree "$head_tree" \
  source_manifest_sha256 "$source_manifest_sha256" \
  cargo_lock_sha256 "$cargo_lock_sha256" \
  permissioned_heights 50000 \
  npos_heights 50000 \
  completed_heights 100000 \
  supplied_commit_qcs 100000 \
  supplied_tcs 75000 \
  finalized_validators 400000 \
  wal_append_restarts 314 \
  fetch_restarts 312 \
  store_restarts 312 \
  validation_restarts 312 \
  application_restarts 312 \
  stale_generation_rejections 1562 \
  deferred_fetch_completions 400936 \
  deferred_store_completions 400624 \
  deferred_validation_completions 400312 \
  deferred_application_completions 400000 \
  duplicate_commit_qcs 3124 \
  reordered_commit_batches 75000 \
  reordered_tc_batches 75000 \
  insufficient_dual_qcs 1030 \
  count_only_qcs 515 \
  power_only_qcs 515 \
  restart_interval 64 \
  duplicate_interval 32 \
  under_quorum_interval 97 \
  certificate_source external_fixture \
  log_sha256 "$log_sha256")"
marker_publish_args=(
  --output "$completion_attestation"
  --maximum-bytes 131072
)
if [[ -n "${IROHA_CHAOS_COMPLETION_PATH_FILE:-}" ]]; then
  completion_pointer_parent="${IROHA_CHAOS_COMPLETION_PATH_FILE%/*}"
  require_release_artifact_path "$completion_pointer_parent" || exit $?
  require_release_artifact_directory "$completion_pointer_parent" || exit $?
  marker_publish_args+=(
    --pointer "$IROHA_CHAOS_COMPLETION_PATH_FILE"
  )
fi
verify_identity "immediately before completion publication"
release_gate_boundary "chaos-100k:completion-publication:before" || exit $?
printf '%s\n' "$completion_body" |
  python3 -I -S "${repo_root}/scripts/publish_release_marker.py" \
    "${marker_publish_args[@]}"
publication_status=0
release_gate_boundary "chaos-100k:completion-publication:after" \
  || publication_status=$?
if ((publication_status != 0)) \
  || ! verify_identity "after completion publication"; then
  rm -f -- "$completion_attestation"
  if [[ -n "${IROHA_CHAOS_COMPLETION_PATH_FILE:-}" ]]; then
    rm -f -- "$IROHA_CHAOS_COMPLETION_PATH_FILE"
  fi
  if ((publication_status != 0)); then
    exit "$publication_status"
  fi
  exit 1
fi
echo "100,000-height chaos gate passed; completion=${completion_attestation}" >&2
