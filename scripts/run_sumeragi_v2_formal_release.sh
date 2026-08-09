#!/usr/bin/env bash
# Run and attest the complete strict Sumeragi v2 formal release gate.
#
# The canonical completion is published only after the proof checker derives,
# archives, and revalidates the production trace-extraction theorem
# certificate. Missing production theorem links therefore leave the useful
# formal archive in place but cannot mint a release completion capability.

set -euo pipefail

if (($#)); then
  echo "usage: $0" >&2
  exit 2
fi

readonly repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
source "${repo_root}/scripts/sumeragi_v2_release_process_policy.sh"

if [[ -z "${CARGO_TARGET_DIR:-}" \
  && -z "${IROHA_RELEASE_ARTIFACT_ROOT:-}" \
  && -z "${IROHA_RELEASE_CANCEL_REQUEST_PATH:-}" ]]; then
  formal_invocation_root="$(
    mktemp -d /private/tmp/iroha-sumeragi-v2-formal.XXXXXX
  )"
  mkdir -m 0700 -- \
    "$formal_invocation_root/target" \
    "$formal_invocation_root/artifacts"
  export CARGO_TARGET_DIR="$formal_invocation_root/target"
  export IROHA_RELEASE_ARTIFACT_ROOT="$formal_invocation_root/artifacts"
  export IROHA_RELEASE_CANCEL_REQUEST_PATH="$formal_invocation_root/cancel-request.json"
elif [[ -z "${CARGO_TARGET_DIR:-}" \
  || -z "${IROHA_RELEASE_ARTIFACT_ROOT:-}" \
  || -z "${IROHA_RELEASE_CANCEL_REQUEST_PATH:-}" ]]; then
  echo "formal release requires CARGO_TARGET_DIR, IROHA_RELEASE_ARTIFACT_ROOT, and IROHA_RELEASE_CANCEL_REQUEST_PATH together" >&2
  exit 2
fi
require_external_cargo_target_dir "$repo_root"
require_external_release_artifact_root "$repo_root"
require_disjoint_release_roots "$repo_root"
release_gate_boundary "formal-release:entry" || exit $?

if [[ -n "${JAVA_BIN:-}" ]]; then
  JAVA_BIN="$(scripts/formal/resolve_java.sh "$JAVA_BIN")"
else
  JAVA_BIN="$(scripts/formal/resolve_java.sh)"
fi
export JAVA_BIN

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

canonical_path() {
  python3 -c \
    'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve(strict=True))' \
    "$1"
}

expected_identity="$(
  python3 scripts/compute_workspace_source_manifest.py \
    --root "$repo_root" --release-identity-json
)"
if [[ -n "${IROHA_RELEASE_EXPECTED_IDENTITY_PATH:-}" ]]; then
  parent_identity="$(<"$IROHA_RELEASE_EXPECTED_IDENTITY_PATH")"
  if [[ "$expected_identity" != "$parent_identity" ]]; then
    echo "formal release source identity disagrees with the parent release" >&2
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
  echo "formal release source manifest disagrees with the parent release" >&2
  exit 1
fi

verify_identity() {
  local checkpoint="$1"
  local observed
  if ! observed="$(
    python3 scripts/compute_workspace_source_manifest.py \
      --root "$repo_root" --release-identity-json
  )"; then
    echo "failed to compute formal release identity at ${checkpoint}" >&2
    return 1
  fi
  if [[ "$observed" != "$expected_identity" ]]; then
    echo "formal release source identity changed at ${checkpoint}" >&2
    return 1
  fi
  if [[ "${IROHA_RELEASE_SEALED_WORKTREE:-0}" == 1 ]]; then
    python3 scripts/seal_workspace_source.py \
      --verify --root "$repo_root" --no-writable-paths
  fi
}

formal_work_root="${SUMERAGI_V2_FORMAL_EVIDENCE_DIR:-${IROHA_RELEASE_ARTIFACT_ROOT}/formal/sumeragi_v2}"
if [[ "$formal_work_root" != /* ]]; then
  echo "formal evidence root must be absolute" >&2
  exit 2
fi
mkdir -p -m 0700 -- "$formal_work_root"
formal_work_root="$(canonical_path "$formal_work_root")"
require_release_artifact_directory "$formal_work_root"
readonly formal_work_root
export SUMERAGI_V2_FORMAL_EVIDENCE_DIR="$formal_work_root"

readonly evidence_root="${IROHA_RELEASE_ARTIFACT_ROOT}/sumeragi-v2-release/${source_manifest_sha256}/evidence/formal"
mkdir -p -- "$evidence_root"
require_release_artifact_directory "$evidence_root"
readonly evidence_lock="${evidence_root}/.formal-release.lock"
if ! mkdir -- "$evidence_lock"; then
  echo "another strict formal release gate owns ${evidence_lock}" >&2
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
readonly gate_log="${invocation_dir}/formal-gate.log"
readonly ledger_copy="${invocation_dir}/proof_coverage.json"
readonly evidence_copy="${invocation_dir}/proof_evidence.json"
readonly verus_evidence_copy="${invocation_dir}/verus_evidence.json"
readonly verus_log_copy="${invocation_dir}/verus.log"
readonly cross_tool_evidence_copy="${invocation_dir}/cross_tool_evidence.json"
readonly production_trace_extraction_evidence_copy="${invocation_dir}/production_trace_extraction_evidence.json"
readonly multilane_apalache_evidence_copy="${invocation_dir}/multilane_apalache_evidence.tsv"
readonly harness_lock_copy="${invocation_dir}/harness-Cargo.lock"
readonly toolchain_copy="${invocation_dir}/formal-toolchain.tsv"
readonly tlaps_resource_jsonl_copy="${invocation_dir}/tlaps_resource.jsonl"
readonly tlaps_resource_summary_copy="${invocation_dir}/tlaps_resource_summary.json"
readonly completion_attestation="${invocation_dir}/COMPLETED.tsv"
readonly final_marker="Sumeragi v2 formal gate passed: source-bound TLAPS, all registered adversarial scheduler/readiness/indexed-height/item-carrier/reply-writer/recovery/ownership mutations, bounded TLC, trace replay, and production Verus"
readonly source_ledger="formal/sumeragi_v2/proof_coverage.json"
readonly source_evidence="${formal_work_root}/proof_evidence.json"
readonly source_verus_evidence="${formal_work_root}/verus_evidence.json"
readonly source_verus_log="${formal_work_root}/verus.log"
readonly source_cross_tool_evidence="${formal_work_root}/cross_tool_evidence.json"
readonly source_production_trace_extraction_evidence="${formal_work_root}/production_trace_extraction_evidence.json"
readonly source_multilane_apalache_evidence="${formal_work_root}/multilane_apalache_evidence.tsv"
readonly source_tlaps_resource_jsonl="${formal_work_root}/tlaps_resource.jsonl"
readonly source_tlaps_resource_summary="${formal_work_root}/tlaps_resource_summary.json"
cross_tool_obligations="$(
  python3 scripts/formal/check_sumeragi_v2_proof_ledger.py \
    --ledger "$source_ledger" \
    --print-cross-tool-obligations
)"
readonly cross_tool_obligations

verify_identity "before execution"
release_gate_boundary "formal-release:before-formal-gates" || exit $?
# Every copied artifact must be published by this invocation. Remove the exact
# shared-target outputs first so a skipped producer cannot satisfy the release
# checks with evidence left by an earlier run.
rm -f -- \
  "$source_evidence" \
  "$source_verus_evidence" \
  "$source_verus_log" \
  "$source_cross_tool_evidence" \
  "$source_production_trace_extraction_evidence" \
  "$source_multilane_apalache_evidence" \
  "$source_tlaps_resource_jsonl" \
  "$source_tlaps_resource_summary"
readonly SUMERAGI_TLAPS_THREADS=1
export SUMERAGI_TLAPS_THREADS
set +e
bash ci/check_sumeragi_formal.sh 2>&1 | tee "$gate_log"
pipeline_status=("${PIPESTATUS[@]}")
set -e
if ((pipeline_status[0] != 0 || pipeline_status[1] != 0)); then
  echo "strict formal release command failed (gate=${pipeline_status[0]}, tee=${pipeline_status[1]})" >&2
  exit 1
fi
release_gate_boundary "formal-release:after-formal-gates-natural-completion" || exit $?
verify_identity "after execution"

marker_count="$(grep -Fxc -- "$final_marker" "$gate_log" || true)"
last_line="$(tail -n 1 "$gate_log")"
if [[ "$marker_count" != 1 || "$last_line" != "$final_marker" ]]; then
  echo "strict formal release log lacks its one exact final success marker" >&2
  exit 1
fi

if [[ ! -f "$source_ledger" || -L "$source_ledger" \
  || ! -f "$source_evidence" || -L "$source_evidence" \
  || ! -f "$source_verus_evidence" || -L "$source_verus_evidence" \
  || ! -f "$source_verus_log" || -L "$source_verus_log" \
  || ! -f "$source_multilane_apalache_evidence" \
  || -L "$source_multilane_apalache_evidence" \
  || ! -f "$source_tlaps_resource_jsonl" || -L "$source_tlaps_resource_jsonl" \
  || ! -f "$source_tlaps_resource_summary" || -L "$source_tlaps_resource_summary" ]]; then
  echo "strict formal release gate did not produce regular TLAPS/Verus/multilane Apalache evidence files" >&2
  exit 1
fi
if [[ -n "$cross_tool_obligations" ]]; then
  if [[ ! -f "$source_cross_tool_evidence" || -L "$source_cross_tool_evidence" ]]; then
    echo "strict formal release gate did not produce required cross-tool evidence" >&2
    exit 1
  fi
elif [[ -e "$source_cross_tool_evidence" || -L "$source_cross_tool_evidence" ]]; then
  echo "strict formal release gate produced forbidden dormant cross-tool evidence" >&2
  exit 1
fi
cp -- "$source_ledger" "${ledger_copy}.partial"
mv -- "${ledger_copy}.partial" "$ledger_copy"
cp -- "$source_evidence" "${evidence_copy}.partial"
mv -- "${evidence_copy}.partial" "$evidence_copy"
cp -- "$source_verus_evidence" "${verus_evidence_copy}.partial"
mv -- "${verus_evidence_copy}.partial" "$verus_evidence_copy"
cp -- "$source_verus_log" "${verus_log_copy}.partial"
mv -- "${verus_log_copy}.partial" "$verus_log_copy"
cp -- "$source_multilane_apalache_evidence" "${multilane_apalache_evidence_copy}.partial"
mv -- "${multilane_apalache_evidence_copy}.partial" "$multilane_apalache_evidence_copy"
cp -- "$source_tlaps_resource_jsonl" "${tlaps_resource_jsonl_copy}.partial"
mv -- "${tlaps_resource_jsonl_copy}.partial" "$tlaps_resource_jsonl_copy"
cp -- "$source_tlaps_resource_summary" "${tlaps_resource_summary_copy}.partial"
mv -- "${tlaps_resource_summary_copy}.partial" "$tlaps_resource_summary_copy"
if [[ -n "$cross_tool_obligations" ]]; then
  cp -- "$source_cross_tool_evidence" "${cross_tool_evidence_copy}.partial"
  mv -- "${cross_tool_evidence_copy}.partial" "$cross_tool_evidence_copy"
fi
readonly source_harness_lock="scripts/formal/sumeragi_v2_harness.lock"
if [[ ! -f "$source_harness_lock" || -L "$source_harness_lock" ]]; then
  echo "strict formal release gate lacks its regular pinned harness lock" >&2
  exit 1
fi
cp -- "$source_harness_lock" "${harness_lock_copy}.partial"
mv -- "${harness_lock_copy}.partial" "$harness_lock_copy"

java_path="$(canonical_path "$JAVA_BIN")"
tlapm_path="$(canonical_path "$TLAPM_BIN")"
tla2tools_path="$(canonical_path "$TLA2TOOLS_JAR")"
verus_path="$(canonical_path "$(command -v verus)")"
cargo_verus_path="$(canonical_path "$(command -v cargo-verus)")"
toolchain_tmp="${toolchain_copy}.partial"
printf '%s\t%s\n' \
  schema_version 1 \
  java_path "$java_path" \
  java_sha256 "$(hash_file "$java_path")" \
  tlapm_path "$tlapm_path" \
  tlapm_sha256 "$(hash_file "$tlapm_path")" \
  tla2tools_path "$tla2tools_path" \
  tla2tools_sha256 "$(hash_file "$tla2tools_path")" \
  verus_path "$verus_path" \
  verus_sha256 "$(hash_file "$verus_path")" \
  cargo_verus_path "$cargo_verus_path" \
  cargo_verus_sha256 "$(hash_file "$cargo_verus_path")" \
  tlc_profile ci \
  tlaps_threads 1 \
  >"$toolchain_tmp"
mv -- "$toolchain_tmp" "$toolchain_copy"

# Revalidate the exact archived pair after every formal leg, not merely the
# TLAPS evidence generated near the beginning of the CI script.
python3 scripts/formal/sumeragi_v2_verus_evidence.py validate \
  --root "$repo_root" \
  --evidence "$verus_evidence_copy" \
  --log "$verus_log_copy"
archived_release_args=(
  --ledger "$ledger_copy"
  --release
  --evidence "$evidence_copy"
  --verus-evidence "$verus_evidence_copy"
  --verus-log "$verus_log_copy"
)
if [[ -n "$cross_tool_obligations" ]]; then
  archived_release_args+=(--cross-tool-evidence "$cross_tool_evidence_copy")
fi
python3 scripts/formal/check_sumeragi_v2_proof_ledger.py "${archived_release_args[@]}"
verify_identity "after archived proof validation"

if [[ -z "$cross_tool_obligations" ]]; then
  echo "production trace-extraction certification requires linked cross-tool evidence" >&2
  exit 1
fi
python3 scripts/formal/check_sumeragi_v2_proof_ledger.py \
  --ledger "$ledger_copy" \
  --evidence "$evidence_copy" \
  --verus-evidence "$verus_evidence_copy" \
  --verus-log "$verus_log_copy" \
  --cross-tool-evidence "$cross_tool_evidence_copy" \
  --write-production-trace-extraction-evidence \
  "$source_production_trace_extraction_evidence"
if [[ ! -f "$source_production_trace_extraction_evidence" \
  || -L "$source_production_trace_extraction_evidence" ]]; then
  echo "production trace-extraction checker did not produce a regular certificate" >&2
  exit 1
fi
cp -- "$source_production_trace_extraction_evidence" \
  "${production_trace_extraction_evidence_copy}.partial"
mv -- "${production_trace_extraction_evidence_copy}.partial" \
  "$production_trace_extraction_evidence_copy"
certificate_release_args=(
  "${archived_release_args[@]}"
  --production-trace-extraction-evidence
  "$production_trace_extraction_evidence_copy"
)
python3 scripts/formal/check_sumeragi_v2_proof_ledger.py \
  "${certificate_release_args[@]}"
verify_identity "after production trace-extraction certification"

gate_log_sha256="$(hash_file "$gate_log")"
proof_coverage_sha256="$(hash_file "$ledger_copy")"
proof_evidence_sha256="$(hash_file "$evidence_copy")"
verus_evidence_sha256="$(hash_file "$verus_evidence_copy")"
verus_log_sha256="$(hash_file "$verus_log_copy")"
multilane_apalache_evidence_sha256="$(hash_file "$multilane_apalache_evidence_copy")"
harness_cargo_lock_sha256="$(hash_file "$harness_lock_copy")"
formal_toolchain_sha256="$(hash_file "$toolchain_copy")"
tlaps_resource_jsonl_sha256="$(hash_file "$tlaps_resource_jsonl_copy")"
tlaps_resource_summary_sha256="$(hash_file "$tlaps_resource_summary_copy")"
cross_tool_evidence_sha256="$(hash_file "$cross_tool_evidence_copy")"
production_trace_extraction_evidence_sha256="$(
  hash_file "$production_trace_extraction_evidence_copy"
)"
completion_tmp="${invocation_dir}/.COMPLETED.tsv.$$"
release_gate_boundary "formal-release:before-completion-publication" || exit $?
printf '%s\t%s\n' \
  schema_version 2 \
  head_commit "$head_commit" \
  head_tree "$head_tree" \
  source_manifest_sha256 "$source_manifest_sha256" \
  cargo_lock_sha256 "$cargo_lock_sha256" \
  formal_gate_log_sha256 "$gate_log_sha256" \
  proof_coverage_sha256 "$proof_coverage_sha256" \
  proof_evidence_sha256 "$proof_evidence_sha256" \
  verus_evidence_sha256 "$verus_evidence_sha256" \
  verus_log_sha256 "$verus_log_sha256" \
  multilane_apalache_evidence_sha256 "$multilane_apalache_evidence_sha256" \
  cross_tool_evidence_sha256 "$cross_tool_evidence_sha256" \
  production_trace_extraction_evidence_sha256 \
    "$production_trace_extraction_evidence_sha256" \
  harness_cargo_lock_sha256 "$harness_cargo_lock_sha256" \
  formal_toolchain_sha256 "$formal_toolchain_sha256" \
  tlaps_resource_jsonl_sha256 "$tlaps_resource_jsonl_sha256" \
  tlaps_resource_summary_sha256 "$tlaps_resource_summary_sha256" \
  >"$completion_tmp"
mv -- "$completion_tmp" "$completion_attestation"
if ! verify_identity "after completion attestation"; then
  rm -f -- "$completion_attestation"
  exit 1
fi

if [[ -n "${IROHA_FORMAL_COMPLETION_PATH_FILE:-}" ]]; then
  completion_path_tmp="${IROHA_FORMAL_COMPLETION_PATH_FILE}.$$"
  printf '%s\n' "$completion_attestation" >"$completion_path_tmp"
  mv -- "$completion_path_tmp" "$IROHA_FORMAL_COMPLETION_PATH_FILE"
fi
release_gate_boundary "formal-release:after-completion-publication" || exit $?
echo "strict Sumeragi v2 formal release gate passed; completion=${completion_attestation}" >&2
