#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

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

readonly proof_ledger="docs/formal/sumeragi_v2/proof_coverage.json"
readonly proof_evidence="target/formal/sumeragi_v2/proof_evidence.json"
readonly verus_evidence="target/formal/sumeragi_v2/verus_evidence.json"
readonly cross_tool_evidence="target/formal/sumeragi_v2/cross_tool_evidence.json"
cross_tool_obligations="$(
  python3 scripts/formal/check_sumeragi_v2_proof_ledger.py \
    --ledger "$proof_ledger" \
    --print-cross-tool-obligations
)"
readonly cross_tool_obligations
# A previous invocation must not make a dormant or failed generation look
# current. The canonical document is recreated only after both component
# evidence files have passed their fresh runs.
rm -f -- "$cross_tool_evidence"

python3 scripts/formal/check_sumeragi_v2_proof_ledger.py
bash scripts/formal/run_sumeragi_v2_tlaps.sh
if [[ -z "$cross_tool_obligations" ]]; then
  python3 scripts/formal/check_sumeragi_v2_proof_ledger.py \
    --release \
    --evidence "$proof_evidence"
fi
bash scripts/formal/run_sumeragi_v2_service_rank_mutation.sh
bash scripts/formal/run_sumeragi_v2_productive_mutation.sh
bash scripts/formal/run_sumeragi_v2_candidate_restart_mutation.sh
bash scripts/formal/run_sumeragi_v2_commit_import_provenance_mutations.sh
bash scripts/formal/run_sumeragi_v2_restart_locked_fetch_order_mutation.sh
bash scripts/formal/run_sumeragi_v2_persist_install_generation_mutation.sh
bash scripts/formal/run_sumeragi_v2_persist_install_validation_mutation.sh
bash scripts/formal/run_sumeragi_v2_apply_authority_mutation.sh
bash scripts/formal/run_sumeragi_v2_replay_locked_body_carrier_mutation.sh
bash scripts/formal/run_sumeragi_v2_certificate_ref_recovery_mutation.sh
bash scripts/formal/run_sumeragi_v2_certified_response_source_lineage_mutation.sh
bash scripts/formal/run_sumeragi_v2_certified_response_identity_separation_mutation.sh
bash scripts/formal/run_sumeragi_v2_progress_mutations.sh
bash scripts/formal/run_sumeragi_v2_begin_timeout_ready_mutation.sh
bash scripts/formal/run_sumeragi_v2_command_execution_ready_mutation.sh
bash scripts/formal/run_sumeragi_v2_post_decision_timeout_mutation.sh
bash scripts/formal/run_sumeragi_v2_decision_recovery_lifecycle_mutation.sh
bash scripts/formal/run_sumeragi_v2_certified_response_registration_mutation.sh
bash scripts/formal/run_sumeragi_v2_effect_capacity_ownership_mutation.sh
bash scripts/formal/run_sumeragi_v2_ingress_causal_freshness_mutation.sh
bash scripts/formal/run_sumeragi_v2_liveness_ownership_mutations.sh
bash scripts/formal/run_sumeragi_v2_serve_scheduler_ordinal_mutations.sh
bash scripts/formal/run_sumeragi_v2_indexed_service_activation_mutations.sh
bash scripts/formal/run_sumeragi_v2_adequate_leader_readiness_mutations.sh
bash scripts/formal/run_sumeragi_v2_indexed_height_mutation.sh
bash scripts/formal/run_sumeragi_v2_item_carrier_typing_mutation.sh
bash scripts/formal/run_sumeragi_v2_reply_writer_deadline_mutations.sh
bash scripts/formal/run_sumeragi_v2_historical_discovery_occurrence_rank_mutation.sh
bash scripts/formal/run_sumeragi_v2_typed_rollover_handoff_mutations.sh
bash scripts/formal/run_sumeragi_v2_tlc.sh ci
bash scripts/formal/check_sumeragi_v2_replay_trace.sh
bash scripts/verify_sumeragi_v2.sh
python3 scripts/formal/sumeragi_v2_verus_evidence.py validate \
  --root "$repo_root" \
  --evidence "$verus_evidence"
if [[ -n "$cross_tool_obligations" ]]; then
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
python3 scripts/formal/check_sumeragi_v2_proof_ledger.py "${release_args[@]}"

echo "Sumeragi v2 formal gate passed: source-bound TLAPS, all registered adversarial scheduler/readiness/indexed-height/item-carrier/reply-writer/recovery/ownership mutations, bounded TLC, trace replay, and production Verus"
