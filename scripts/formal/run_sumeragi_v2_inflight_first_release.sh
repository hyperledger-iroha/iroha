#!/usr/bin/env bash
set -euo pipefail

# Bounded TLC evidence for the in-flight first-release carrier kernel.  This is
# a standalone safety corpus: it deliberately does not claim a Rust-to-TLA
# refinement theorem or amend the source-bound multilane release matrix.

if (($#)); then
  echo "usage: $0" >&2
  exit 2
fi

readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/formal/sumeragi_v2"
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-${REPO_ROOT}/target/tla2tools/1.7.4/tla2tools.jar}"
readonly MODULE="SumeragiV2InFlightFirstRelease.tla"
[[ -f "$TLA2TOOLS_JAR" ]] || { echo "missing TLA2Tools jar: $TLA2TOOLS_JAR" >&2; exit 1; }

if [[ -n "${JAVA_BIN:-}" ]]; then
  resolved_java="${REPO_ROOT}/scripts/formal/resolve_java.sh"
  JAVA_BIN="$(bash "$resolved_java" "$JAVA_BIN")"
else
  JAVA_BIN="$(bash "${REPO_ROOT}/scripts/formal/resolve_java.sh")"
fi
readonly JAVA_BIN

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-first-release.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT
common=("$JAVA_BIN" -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC -cleanup -workers 1)

run_positive() {
  local log="$run_dir/fixed.log"
  (cd "$FORMAL_DIR" && "${common[@]}" -metadir "$run_dir/fixed" \
    -config inflight_first_release_fixed.cfg "$MODULE") >"$log" 2>&1
  cat "$log"
  grep -Fqx "Model checking completed. No error has been found." "$log" || {
    echo "first-release fixed model did not complete successfully" >&2; exit 1; }
  echo "[tlc] first-release fixed: no bounded safety counterexample"
}

run_mutant() {
  local config="$1"
  local invariant="$2"
  local log="$run_dir/${config}.log"
  set +e
  (cd "$FORMAL_DIR" && "${common[@]}" -metadir "$run_dir/${config}" \
    -config "$config" "$MODULE") >"$log" 2>&1
  local status=$?
  set -e
  if [[ "$status" -ne 12 ]] || ! grep -Fq "Invariant ${invariant} is violated." "$log"; then
    cat "$log" >&2
    echo "${config} did not produce ${invariant}" >&2
    exit 1
  fi
  echo "[tlc] first-release mutation ${config}: ${invariant}"
}

run_positive
run_mutant inflight_first_release_reservation_before_selected_queue_plan_bug.cfg MLSelectedQueuePlanV4ConjunctionBeforeReservationV5
run_mutant inflight_first_release_kura_before_reservation_bug.cfg MLReservationV5BeforeKuraActive
run_mutant inflight_first_release_ready_authorization_before_input_bug.cfg MLExecutionInputBeforeReadyAuthorization
run_mutant inflight_first_release_ready_signature_before_authorization_bug.cfg MLReadyAuthorizationBeforeLocalSignature
run_mutant inflight_first_release_ready_qc_before_signatures_bug.cfg MLLocalSignaturesBeforeDurableReadyQc
run_mutant inflight_first_release_crash_drops_durable_bug.cfg MLCrashDurableFactsRecoverable
run_mutant inflight_first_release_crash_retains_volatile_body_bug.cfg MLVolatileSessionLostOnCrash
run_mutant inflight_first_release_payload_conflict_bug.cfg MLPayloadSchemaV2CarriesExactAdmissionPreimage
run_mutant inflight_first_release_lane_commit_scope_conflict_bug.cfg MLCommitAndReleaseRetainExactScope
run_mutant inflight_first_release_release_scope_conflict_bug.cfg MLCommitAndReleaseRetainExactScope
run_mutant inflight_first_release_duplicate_apply_bug.cfg MLExactlyOnceCarrierApplication
run_mutant inflight_first_release_reservation_commit_before_carrier_bug.cfg MLPostCarrierCommitCleanupOrder
run_mutant inflight_first_release_plan_tombstone_before_reservation_commit_bug.cfg MLPostCarrierCommitCleanupOrder
run_mutant inflight_first_release_forget_commit_before_plan_tombstone_bug.cfg MLPostCarrierCommitCleanupOrder
run_mutant inflight_first_release_release_pending_before_retirement_bug.cfg MLReleaseStageOrder
run_mutant inflight_first_release_release_prepare_before_pending_bug.cfg MLReleaseStageOrder
run_mutant inflight_first_release_released_claims_before_prepare_bug.cfg MLReleaseStageOrder
run_mutant inflight_first_release_release_complete_before_released_bug.cfg MLReleaseStageOrder
run_mutant inflight_first_release_forget_release_before_fifo_bug.cfg MLReleaseStageOrder
run_mutant inflight_first_release_oversize_selected_queue_plan_bug.cfg MLQueuePlanV4SelectedConjunctionBound4096

echo "[tlc] first-release corpus complete: bounded abstract evidence only; no production refinement claim"
