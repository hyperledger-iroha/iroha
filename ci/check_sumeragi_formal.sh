#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
source "${repo_root}/scripts/sumeragi_v2_release_process_policy.sh"

if [[ -z "${CARGO_TARGET_DIR:-}" \
  || -z "${IROHA_RELEASE_ARTIFACT_ROOT:-}" \
  || -z "${IROHA_RELEASE_CANCEL_REQUEST_PATH:-}" \
  || -z "${SUMERAGI_V2_FORMAL_EVIDENCE_DIR:-}" ]]; then
  echo "formal CI must run through the external-target formal release wrapper" >&2
  exit 2
fi
require_external_cargo_target_dir "$repo_root"
require_external_release_artifact_root "$repo_root"
require_disjoint_release_roots "$repo_root"
require_release_artifact_directory "$SUMERAGI_V2_FORMAL_EVIDENCE_DIR"
release_gate_boundary "formal:entry" || exit $?

run_formal_gate() {
  local gate="$1"
  shift
  release_gate_boundary "formal:${gate}:before" || return $?
  "$@"
  release_gate_boundary "formal:${gate}:after-natural-completion" || return $?
}

run_formal_script() {
  local script="$1"
  shift
  local gate="${script##*/}"
  run_formal_gate "${gate%.sh}" bash "$script" "$@"
}

if (($#)); then
  echo "usage: $0" >&2
  exit 2
fi

if [[ -n "${JAVA_BIN:-}" ]]; then
  JAVA_BIN="$(scripts/formal/resolve_java.sh "$JAVA_BIN")"
else
  JAVA_BIN="$(scripts/formal/resolve_java.sh)"
fi
export JAVA_BIN

readonly proof_ledger="formal/sumeragi_v2/proof_coverage.json"
readonly proof_evidence="${SUMERAGI_V2_FORMAL_EVIDENCE_DIR}/proof_evidence.json"
readonly verus_evidence="${SUMERAGI_V2_FORMAL_EVIDENCE_DIR}/verus_evidence.json"
readonly cross_tool_evidence="${SUMERAGI_V2_FORMAL_EVIDENCE_DIR}/cross_tool_evidence.json"
cross_tool_obligations="$(
  run_formal_gate proof-obligations \
    python3 scripts/formal/check_sumeragi_v2_proof_ledger.py \
    --ledger "$proof_ledger" \
    --print-cross-tool-obligations
)"
readonly cross_tool_obligations
# A previous invocation must not make a dormant or failed generation look
# current. The canonical document is recreated only after both component
# evidence files have passed their fresh runs.
rm -f -- "$cross_tool_evidence"

run_formal_gate proof-ledger \
  python3 scripts/formal/check_sumeragi_v2_proof_ledger.py
run_formal_script scripts/formal/run_sumeragi_v2_tlaps.sh
if [[ -z "$cross_tool_obligations" ]]; then
  run_formal_gate proof-ledger-release-without-cross-tool \
    python3 scripts/formal/check_sumeragi_v2_proof_ledger.py \
    --release \
    --evidence "$proof_evidence"
fi
run_formal_script scripts/formal/run_sumeragi_v2_service_rank_mutation.sh
run_formal_script scripts/formal/run_sumeragi_v2_productive_mutation.sh
run_formal_script scripts/formal/run_sumeragi_v2_candidate_restart_mutation.sh
run_formal_script scripts/formal/run_sumeragi_v2_commit_import_provenance_mutations.sh
run_formal_script scripts/formal/run_sumeragi_v2_restart_locked_fetch_order_mutation.sh
run_formal_script scripts/formal/run_sumeragi_v2_persist_install_generation_mutation.sh
run_formal_script scripts/formal/run_sumeragi_v2_persist_install_validation_mutation.sh
run_formal_script scripts/formal/run_sumeragi_v2_apply_authority_mutation.sh
run_formal_script scripts/formal/run_sumeragi_v2_replay_locked_body_carrier_mutation.sh
run_formal_script scripts/formal/run_sumeragi_v2_certificate_ref_recovery_mutation.sh
run_formal_script scripts/formal/run_sumeragi_v2_certified_response_source_lineage_mutation.sh
run_formal_script scripts/formal/run_sumeragi_v2_certified_response_identity_separation_mutation.sh
run_formal_script scripts/formal/run_sumeragi_v2_progress_mutations.sh
run_formal_script scripts/formal/run_sumeragi_v2_begin_timeout_ready_mutation.sh
run_formal_script scripts/formal/run_sumeragi_v2_command_execution_ready_mutation.sh
run_formal_script scripts/formal/run_sumeragi_v2_post_decision_timeout_mutation.sh
run_formal_script scripts/formal/run_sumeragi_v2_decision_recovery_lifecycle_mutation.sh
run_formal_script scripts/formal/run_sumeragi_v2_certified_response_registration_mutation.sh
run_formal_script scripts/formal/run_sumeragi_v2_effect_capacity_ownership_mutation.sh
run_formal_script scripts/formal/run_sumeragi_v2_applied_phase_admission_mutations.sh
run_formal_script scripts/formal/run_sumeragi_v2_durable_validate_lifecycle_mutations.sh
run_formal_script scripts/formal/run_sumeragi_v2_ingress_causal_freshness_mutation.sh
run_formal_script scripts/formal/run_sumeragi_v2_liveness_ownership_mutations.sh
run_formal_script scripts/formal/run_sumeragi_v2_serve_scheduler_ordinal_mutations.sh
run_formal_script scripts/formal/run_sumeragi_v2_indexed_service_activation_mutations.sh
run_formal_script scripts/formal/run_sumeragi_v2_adequate_leader_readiness_mutations.sh
run_formal_script scripts/formal/run_sumeragi_v2_indexed_height_mutation.sh
run_formal_script scripts/formal/run_sumeragi_v2_item_carrier_typing_mutation.sh
run_formal_script scripts/formal/run_sumeragi_v2_reply_writer_deadline_mutations.sh
run_formal_script scripts/formal/run_sumeragi_v2_historical_discovery_occurrence_rank_mutation.sh
run_formal_script scripts/formal/run_sumeragi_v2_typed_rollover_handoff_mutations.sh
run_formal_script scripts/formal/run_sumeragi_v2_tlc.sh ci
run_formal_script scripts/formal/check_sumeragi_v2_replay_trace.sh
run_formal_script scripts/verify_sumeragi_v2.sh
run_formal_gate verus-evidence-validation \
  python3 scripts/formal/sumeragi_v2_verus_evidence.py validate \
  --root "$repo_root" \
  --evidence "$verus_evidence"
if [[ -n "$cross_tool_obligations" ]]; then
  run_formal_gate cross-tool-evidence \
    python3 scripts/formal/check_sumeragi_v2_proof_ledger.py \
    --ledger "$proof_ledger" \
    --evidence "$proof_evidence" \
    --verus-evidence "$verus_evidence" \
    --write-cross-tool-evidence "$cross_tool_evidence"
fi
release_args=(
  --ledger "$proof_ledger"
  --release
  --evidence "$proof_evidence"
  --verus-evidence "$verus_evidence"
)
if [[ -n "$cross_tool_obligations" ]]; then
  release_args+=(--cross-tool-evidence "$cross_tool_evidence")
fi
run_formal_gate final-proof-ledger-release \
  python3 scripts/formal/check_sumeragi_v2_proof_ledger.py "${release_args[@]}"

echo "Sumeragi v2 formal gate passed: source-bound TLAPS, all registered adversarial scheduler/readiness/indexed-height/item-carrier/reply-writer/recovery/ownership mutations, bounded TLC, trace replay, and production Verus"
