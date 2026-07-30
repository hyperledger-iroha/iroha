#!/usr/bin/env bash
# Run and attest the complete strict Sumeragi v2 formal release gate.

set -euo pipefail

if (($#)); then
  echo "usage: $0" >&2
  exit 2
fi

readonly repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

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
      --verify --root "$repo_root" --writable target
  fi
}

readonly evidence_root="${SUMERAGI_V2_FORMAL_EVIDENCE_DIR:-${repo_root}/target/sumeragi-v2-release/${source_manifest_sha256}/evidence/formal}"
mkdir -p -- "$evidence_root"
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
readonly multilane_apalache_evidence_copy="${invocation_dir}/multilane_apalache_evidence.tsv"
readonly harness_lock_copy="${invocation_dir}/harness-Cargo.lock"
readonly toolchain_copy="${invocation_dir}/formal-toolchain.tsv"
readonly tlaps_resource_jsonl_copy="${invocation_dir}/tlaps_resource.jsonl"
readonly tlaps_resource_summary_copy="${invocation_dir}/tlaps_resource_summary.json"
readonly completion_attestation="${invocation_dir}/COMPLETED.tsv"
readonly final_marker="Sumeragi v2 formal gate passed: source-bound TLAPS, adversarial scheduler/post-decision/recovery/effect-capacity/ingress-causal-freshness/liveness-ownership/historical-discovery-rank mutations, bounded TLC, trace replay, and production Verus"
readonly source_ledger="formal/sumeragi_v2/proof_coverage.json"
cross_tool_obligations="$(
  python3 scripts/formal/check_sumeragi_v2_proof_ledger.py \
    --ledger "$source_ledger" \
    --print-cross-tool-obligations
)"
readonly cross_tool_obligations

verify_identity "before execution"
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
verify_identity "after execution"

marker_count="$(grep -Fxc -- "$final_marker" "$gate_log" || true)"
last_line="$(tail -n 1 "$gate_log")"
if [[ "$marker_count" != 1 || "$last_line" != "$final_marker" ]]; then
  echo "strict formal release log lacks its one exact final success marker" >&2
  exit 1
fi

readonly source_evidence="target/formal/sumeragi_v2/proof_evidence.json"
readonly source_verus_evidence="target/formal/sumeragi_v2/verus_evidence.json"
readonly source_verus_log="target/formal/sumeragi_v2/verus.log"
readonly source_cross_tool_evidence="target/formal/sumeragi_v2/cross_tool_evidence.json"
readonly source_multilane_apalache_evidence="target/formal/sumeragi_v2/multilane_apalache_evidence.tsv"
readonly source_tlaps_resource_jsonl="target/formal/sumeragi_v2/tlaps_resource.jsonl"
readonly source_tlaps_resource_summary="target/formal/sumeragi_v2/tlaps_resource_summary.json"
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
completion_tmp="${invocation_dir}/.COMPLETED.tsv.$$"
printf '%s\t%s\n' \
  schema_version 1 \
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
  harness_cargo_lock_sha256 "$harness_cargo_lock_sha256" \
  formal_toolchain_sha256 "$formal_toolchain_sha256" \
  tlaps_resource_jsonl_sha256 "$tlaps_resource_jsonl_sha256" \
  tlaps_resource_summary_sha256 "$tlaps_resource_summary_sha256" \
  >"$completion_tmp"
if [[ -n "$cross_tool_obligations" ]]; then
  printf '%s\t%s\n' \
    cross_tool_evidence_sha256 "$(hash_file "$cross_tool_evidence_copy")" \
    >>"$completion_tmp"
fi
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
echo "strict Sumeragi v2 formal release gate passed; completion=${completion_attestation}" >&2
