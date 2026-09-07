#!/usr/bin/env bash
set -euo pipefail

# Bounded TLC evidence for the in-flight first-release carrier kernel.  This is
# a standalone safety corpus: it deliberately does not claim a Rust-to-TLA
# refinement theorem or amend the source-bound multilane release matrix.
# Requires the pinned TLA2TOOLS_JAR and working Java (optionally JAVA_BIN).
# Retain every invocation under SUMERAGI_V2_FORMAL_EVIDENCE_DIR when supplied;
# otherwise announce a retained private temp directory for standalone runs.

if (($#)); then
  echo "usage: $0" >&2
  exit 2
fi

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/formal/sumeragi_v2"
readonly MODULE="SumeragiV2InFlightFirstRelease.tla"
source "${REPO_ROOT}/scripts/formal/sumeragi_v2_tlc_result_contract.sh"
run_dir="$(python3 -I -S "${REPO_ROOT}/scripts/formal/sumeragi_v2_tlc_artifacts.py" init \
  --parent "${SUMERAGI_V2_FORMAL_EVIDENCE_DIR:-${TMPDIR:-/tmp}}" \
  --runner "${BASH_SOURCE[0]}" --expected-cases 23 \
  --support "${REPO_ROOT}/scripts/formal/sumeragi_v2_tlc_result_contract.sh" \
  --support "${REPO_ROOT}/scripts/formal/resolve_java.sh")"
readonly run_dir
readonly ARTIFACT_RECORDER="$run_dir/support/sumeragi_v2_tlc_artifacts.py"
echo "[tlc] retained artifacts: $run_dir" >&2
finish_artifacts() {
  local status=$?
  trap - EXIT
  echo "[tlc] retained artifacts: $run_dir" >&2
  python3 -I -S "$ARTIFACT_RECORDER" finish --run-dir "$run_dir" --status "$status" \
    || exit 1
  exit "$status"
}
trap finish_artifacts EXIT
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:?TLA2TOOLS_JAR must name the authenticated external tool}"

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

[[ -f "$TLA2TOOLS_JAR" ]] || {
  echo "pinned TLA2Tools v${TLA2TOOLS_VERSION} is required at ${TLA2TOOLS_JAR}" >&2
  exit 1
}
actual_sha256="$(hash_file "$TLA2TOOLS_JAR")"
[[ "$actual_sha256" == "$TLA2TOOLS_SHA256" ]] || {
  echo "TLA2Tools checksum mismatch" >&2
  echo "expected: ${TLA2TOOLS_SHA256}" >&2
  echo "actual:   ${actual_sha256}" >&2
  exit 1
}

if [[ -n "${JAVA_BIN:-}" ]]; then
  resolved_java="${REPO_ROOT}/scripts/formal/resolve_java.sh"
  JAVA_BIN="$(bash "$resolved_java" "$JAVA_BIN")"
else
  JAVA_BIN="$(bash "${REPO_ROOT}/scripts/formal/resolve_java.sh")"
fi
readonly JAVA_BIN

python3 -I -S "$ARTIFACT_RECORDER" tools --run-dir "$run_dir" \
  --java "$JAVA_BIN" --jar "$TLA2TOOLS_JAR" --jar-sha256 "$TLA2TOOLS_SHA256"

common=("$JAVA_BIN" -XX:+UseParallelGC -cp "$run_dir/tools/tla2tools.jar" tlc2.TLC -cleanup -workers 1)

run_positive() {
  local log="$run_dir/cases/fixed/stdout.log"
  local status
  set +e
  python3 -I -S "$ARTIFACT_RECORDER" capture --run-dir "$run_dir" \
    --name fixed --formal-dir "$FORMAL_DIR" --module "$MODULE" \
    --config inflight_first_release_fixed.cfg --expectation fixed-success -- \
    "${common[@]}" -metadir "$run_dir/cases/fixed/tlc" \
    -config inflight_first_release_fixed.cfg "$MODULE"
  status=$?
  set -e
  cat "$log"
  sumeragi_v2_tlc_assert_fixed_success "first-release-fixed" "$log" "$status"
  grep -Fqx "Model checking completed. No error has been found." "$log" || {
    echo "first-release fixed model did not complete successfully" >&2; exit 1; }
  python3 -I -S "$ARTIFACT_RECORDER" accept --run-dir "$run_dir" --name fixed
  echo "[tlc] first-release fixed: no bounded safety counterexample"
}

run_mutant() {
  local config="$1"
  local invariant="$2"
  local log="$run_dir/cases/${config}/stdout.log"
  local invariant_marker="Error: Invariant ${invariant} is violated."
  local primary_diagnostic_count
  set +e
  python3 -I -S "$ARTIFACT_RECORDER" capture --run-dir "$run_dir" \
    --name "$config" --formal-dir "$FORMAL_DIR" --module "$MODULE" \
    --config "$config" --expectation "$invariant" -- \
    "${common[@]}" -metadir "$run_dir/cases/${config}/tlc" \
    -config "$config" "$MODULE"
  local status=$?
  set -e
  if [[ "$status" -ne 12 ]]; then
    cat "$log" >&2
    echo "${config} did not produce ${invariant}" >&2
    exit 1
  fi
  sumeragi_v2_tlc_assert_nonzero_state_space "$config" "$log"
  sumeragi_v2_tlc_assert_exact_line \
    "$config" "$log" "$invariant_marker"
  sumeragi_v2_tlc_assert_exact_line \
    "$config" "$log" "Error: The behavior up to this point is:"
  primary_diagnostic_count="$(
    grep -Ec \
      "$SUMERAGI_V2_TLC_PRIMARY_DIAGNOSTIC_PATTERN" \
      "$log" || true
  )"
  [[ "$primary_diagnostic_count" -eq 1 ]] || {
    cat "$log" >&2
    echo "${config} emitted ${primary_diagnostic_count} primary failure diagnostics" >&2
    exit 1
  }
  sumeragi_v2_tlc_assert_terminal "$config" "$log"
  python3 -I -S "$ARTIFACT_RECORDER" accept --run-dir "$run_dir" --name "$config"
  echo "[tlc] first-release mutation ${config}: ${invariant}"
}

run_positive
run_mutant inflight_first_release_reservation_before_selected_queue_plan_bug.cfg MLSelectedQueuePlanV1ConjunctionBeforeReservationV1
run_mutant inflight_first_release_kura_before_reservation_bug.cfg MLReservationV1BeforeKuraActive
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
run_mutant inflight_first_release_commit_prefix_skipped_key_bug.cfg MLPostCarrierCommitCleanupOrder
run_mutant inflight_first_release_commit_prefix_decrease_bug.cfg MLPostCarrierCommitCleanupOrder
run_mutant inflight_first_release_release_pending_before_retirement_bug.cfg MLReleaseStageOrder
run_mutant inflight_first_release_release_prepare_before_pending_bug.cfg MLReleaseStageOrder
run_mutant inflight_first_release_released_claims_before_prepare_bug.cfg MLReleaseStageOrder
run_mutant inflight_first_release_release_complete_before_released_bug.cfg MLReleaseStageOrder
run_mutant inflight_first_release_forget_release_before_fifo_bug.cfg MLReleaseStageOrder
run_mutant inflight_first_release_oversize_selected_queue_plan_bug.cfg MLQueuePlanV1SelectedConjunctionBound4096

echo "[tlc] first-release corpus complete: bounded abstract evidence only; no production refinement claim"
