#!/usr/bin/env bash
# Run the deterministic typed rollover-handoff fixed model and mutation matrix.
#
# Prerequisites: pinned TLA2Tools v1.7.4, Java 21.0.12, and the pinned TLAPM
# standard library. JAVA_BIN, TLA2TOOLS_JAR, and TLAPM_STDLIB may override
# their default paths. This runner invokes SANY and TLC only, never TLAPM.
# All TLC metadirectories and logs are temporary and are removed on exit.

set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly TLAPM_COMMIT="3ab43c7ff31db4ced850619d4746fa4c841a7681"
readonly TLAPM_TLAPS_SHA256="5cc604533e49792c1c3d050a38d845d08d9c209879ca20c86de04975bc4bc563"
readonly EXPECTED_JAVA_VERSION='openjdk version "21.0.12"'
readonly TLC_FP_INDEX="96"
readonly TLC_SEED="139154308881391968"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/docs/formal/sumeragi_v2"
readonly FIXED_MODEL="SumeragiV2TypedRolloverHandoff.tla"
readonly MUTATION_MODEL="SumeragiV2TypedRolloverHandoffMutation.tla"
readonly LIVENESS_MUTATION_MODEL="SumeragiV2TypedRolloverHandoffLivenessMutation.tla"
readonly REPEATED_HANDOFF_MUTATION_MODEL="SumeragiV2TypedRolloverHandoffRepeatedHandoffMutation.tla"
readonly REPEATED_HANDOFF_MUTATION_CONFIG="typed_rollover_handoff_repeated_handoff_after_restart_restore_bug.cfg"
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-${REPO_ROOT}/target/tla2tools/${TLA2TOOLS_VERSION}/tla2tools.jar}"
source "${REPO_ROOT}/scripts/formal/sumeragi_v2_tlc_result_contract.sh"

usage() {
  cat <<USAGE
usage: $0 [--help]

Parse the typed rollover-handoff base, mutation, and proof modules with SANY,
then run the three repaired TLC corridors and all 45 deterministic mutation
configurations.

Environment overrides:
  JAVA_BIN       Java 21.0.12 executable or containing directory
  TLA2TOOLS_JAR  pinned TLA2Tools v${TLA2TOOLS_VERSION} jar
  TLAPM_STDLIB   pinned TLAPM ${TLAPM_COMMIT} standard-library directory
USAGE
}

if (($#)); then
  if [[ "$#" -eq 1 && "$1" == "--help" ]]; then
    usage
    exit 0
  fi
  usage >&2
  exit 2
fi

if [[ -n "${JAVA_BIN:-}" ]]; then
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh" "$JAVA_BIN")"
else
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh")"
fi
readonly JAVA_BIN="$resolved_java_bin"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) readonly TLAPM_PLATFORM="x86_64-linux-gnu" ;;
  Darwin-arm64) readonly TLAPM_PLATFORM="arm64-darwin" ;;
  *)
    echo "unsupported TLAPM host: $(uname -s)-$(uname -m)" >&2
    exit 1
    ;;
esac
readonly TLAPM_STDLIB="${TLAPM_STDLIB:-${REPO_ROOT}/target/tlapm/toolchains/${TLAPM_COMMIT}/${TLAPM_PLATFORM}/tlapm/lib/tlapm/stdlib}"

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

[[ -f "${TLAPM_STDLIB}/TLAPS.tla" ]] || {
  echo "pinned TLAPM ${TLAPM_COMMIT} standard library is required at ${TLAPM_STDLIB}" >&2
  exit 1
}
actual_sha256="$(hash_file "${TLAPM_STDLIB}/TLAPS.tla")"
[[ "$actual_sha256" == "$TLAPM_TLAPS_SHA256" ]] || {
  echo "pinned TLAPM standard-library checksum mismatch for TLAPS.tla" >&2
  echo "expected: ${TLAPM_TLAPS_SHA256}" >&2
  echo "actual:   ${actual_sha256}" >&2
  exit 1
}

java_version="$($JAVA_BIN -version 2>&1)"
grep -Fq "$EXPECTED_JAVA_VERSION" <<<"$java_version" || {
  echo "frozen Java 21.0.12 is required" >&2
  printf '%s\n' "$java_version" >&2
  exit 1
}

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-typed-rollover-handoff.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT

run_sany() {
  local module="$1"
  local log="${run_dir}/${module}.sany.log"
  local actual_status
  local sany_last_nonblank
  local expected_marker
  set +e
  (
    cd "$FORMAL_DIR"
    "$JAVA_BIN" "-DTLA-Library=${TLAPM_STDLIB}" \
      -cp "$TLA2TOOLS_JAR" tla2sany.SANY "${module}.tla"
  ) >"$log" 2>&1
  actual_status=$?
  set -e
  if [[ "$actual_status" -ne 0 ]]; then
    echo "${module}: SANY returned status ${actual_status}" >&2
    cat "$log" >&2
    exit 1
  fi
  sany_last_nonblank="$(awk 'NF { line = $0 } END { print line }' "$log")"
  expected_marker="Semantic processing of module ${module}"
  [[ "$sany_last_nonblank" == "$expected_marker" ]] || {
    echo "${module}: SANY did not end at the expected marker" >&2
    cat "$log" >&2
    exit 1
  }
  echo "[sany] ${module}: parsed"
}

for module in \
  SumeragiV2TypedRolloverHandoff \
  SumeragiV2TypedRolloverHandoffMutation \
  SumeragiV2TypedRolloverHandoffLivenessMutation \
  SumeragiV2TypedRolloverHandoffRepeatedHandoffMutation \
  SumeragiV2TypedRolloverHandoffProofs; do
  run_sany "$module"
done

common=(
  "$JAVA_BIN" -XX:+UseParallelGC "-DTLA-Library=${TLAPM_STDLIB}"
  -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -workers 1 -fp "$TLC_FP_INDEX" -seed "$TLC_SEED"
)

run_case() {
  local label="$1"
  local model="$2"
  local config="$3"
  local expected_status="$4"
  shift 4
  local log="${run_dir}/${label}.log"
  local metadir="${run_dir}/${label}/states"
  local actual_status
  local primary_diagnostic_count
  mkdir -p "$metadir"
  set +e
  (
    cd "$FORMAL_DIR"
    "${common[@]}" -metadir "$metadir" \
      -config "$config" "$model"
  ) >"$log" 2>&1
  actual_status=$?
  set -e
  if [[ "$actual_status" -ne "$expected_status" ]]; then
    echo "${label} returned TLC status ${actual_status}, expected ${expected_status}" >&2
    cat "$log" >&2
    exit 1
  fi
  if [[ "$expected_status" -eq 0 ]]; then
    sumeragi_v2_tlc_assert_fixed_success "$label" "$log" "$actual_status"
  else
    sumeragi_v2_tlc_assert_nonzero_state_space "$label" "$log"
    sumeragi_v2_tlc_assert_terminal "$label" "$log"
  fi
  for marker in "$@"; do
    if [[ "$(grep -Fxc "$marker" "$log" || true)" != 1 ]]; then
      echo "${label} did not emit exactly one expected marker: ${marker}" >&2
      cat "$log" >&2
      exit 1
    fi
  done
  if [[ "$expected_status" -ne 0 ]]; then
    primary_diagnostic_count="$(
      grep -Ec \
        "$SUMERAGI_V2_TLC_PRIMARY_DIAGNOSTIC_PATTERN" \
        "$log" || true
    )"
    [[ "$primary_diagnostic_count" -eq 1 ]] || {
      echo "${label} emitted ${primary_diagnostic_count} primary failure diagnostics" >&2
      cat "$log" >&2
      exit 1
    }
  fi
  echo "[tlc] ${label}: expected status ${expected_status}"
}

run_case typed-rollover-fixed \
  "$FIXED_MODEL" typed_rollover_handoff_fixed.cfg 0 \
  "Model checking completed. No error has been found."
run_case typed-rollover-responsive-durable-liveness \
  "$FIXED_MODEL" typed_rollover_handoff_responsive_durable_liveness.cfg 0 \
  "Model checking completed. No error has been found."
run_case typed-rollover-responsive-restart-restore-liveness \
  "$FIXED_MODEL" typed_rollover_handoff_responsive_restart_restore_liveness.cfg 0 \
  "Model checking completed. No error has been found."

readonly INVARIANT_MARKER="Error: Invariant TypedRolloverSafetyInvariant is violated."
readonly TEMPORAL_MARKER="Error: Temporal properties were violated."
readonly EXPECTED_MATRIX_MUTATION_COUNT=42
readonly EXPECTED_LIVENESS_MUTATION_COUNT=2
readonly EXPECTED_MUTATION_COUNT=45

mutation_cases=(
  "accept-semantic-invalid-lifecycle-state|typed_rollover_handoff_accept_semantic_invalid_lifecycle_state_bug.cfg|12|${INVARIANT_MARKER}"
  "active-state-roll|typed_rollover_handoff_active_state_roll_bug.cfg|12|${INVARIANT_MARKER}"
  "changed-roster-without-generation-advance|typed_rollover_handoff_changed_roster_without_generation_advance_bug.cfg|12|${INVARIANT_MARKER}"
  "clean-state-slot-v3-persistence-failure|typed_rollover_handoff_clean_state_slot_v3_persistence_failure_bug.cfg|12|${INVARIANT_MARKER}"
  "clean-crash-after-lifecycle-root-v3-commit|typed_rollover_handoff_clean_crash_after_lifecycle_root_v3_commit_bug.cfg|12|${INVARIANT_MARKER}"
  "clean-foreign-owner-reject|typed_rollover_handoff_clean_foreign_owner_reject_bug.cfg|12|${INVARIANT_MARKER}"
  "clean-late-enqueue-reject|typed_rollover_handoff_clean_late_enqueue_reject_bug.cfg|12|${INVARIANT_MARKER}"
  "clean-predecessor-artifact-reject|typed_rollover_handoff_clean_predecessor_artifact_reject_bug.cfg|12|${INVARIANT_MARKER}"
  "clean-predecessor-context-reject|typed_rollover_handoff_clean_predecessor_context_reject_bug.cfg|12|${INVARIANT_MARKER}"
  "clean-wrong-successor-reject|typed_rollover_handoff_clean_wrong_successor_reject_bug.cfg|12|${INVARIANT_MARKER}"
  "cleanup-before-root-parent-resync|typed_rollover_handoff_cleanup_before_root_parent_resync_bug.cfg|12|${INVARIANT_MARKER}"
  "cleanup-before-semantic-validation|typed_rollover_handoff_cleanup_before_validation_bug.cfg|12|${INVARIANT_MARKER}"
  "cleanup-retains-inactive-slot|typed_rollover_handoff_cleanup_retains_inactive_slot_bug.cfg|12|${INVARIANT_MARKER}"
  "cross-service-transport-owner-pair|typed_rollover_handoff_cross_service_transport_owner_pair_bug.cfg|12|${INVARIANT_MARKER}"
  "crossed-lifecycle-root-shape|typed_rollover_handoff_crossed_root_shape_bug.cfg|12|${INVARIANT_MARKER}"
  "epoch-overflow|typed_rollover_handoff_epoch_overflow_bug.cfg|12|${INVARIANT_MARKER}"
  "epoch-use-before-persist|typed_rollover_handoff_epoch_use_before_persist_bug.cfg|12|${INVARIANT_MARKER}"
  "forged-authenticated-close-prefix|typed_rollover_handoff_forged_authenticated_close_prefix_bug.cfg|12|${INVARIANT_MARKER}"
  "foreign-candidate-ignored|typed_rollover_handoff_foreign_candidate_ignored_bug.cfg|12|${INVARIANT_MARKER}"
  "foreign-receipt|typed_rollover_handoff_foreign_receipt_bug.cfg|12|${INVARIANT_MARKER}"
  "foreign-successor|typed_rollover_handoff_foreign_successor_bug.cfg|12|${INVARIANT_MARKER}"
  "service-generation-overflow|typed_rollover_handoff_generation_overflow_bug.cfg|12|${INVARIANT_MARKER}"
  "late-callback|typed_rollover_handoff_late_callback_bug.cfg|12|${INVARIANT_MARKER}"
  "late-enqueue|typed_rollover_handoff_late_enqueue_bug.cfg|12|${INVARIANT_MARKER}"
  "lose-requester-incarnation-after-crash|typed_rollover_handoff_lose_requester_incarnation_after_crash_bug.cfg|12|${INVARIANT_MARKER}"
  "missing-root-selected-state|typed_rollover_handoff_missing_selected_state_bug.cfg|12|${INVARIANT_MARKER}"
  "predecessor-artifact-accept|typed_rollover_handoff_predecessor_artifact_accept_bug.cfg|12|${INVARIANT_MARKER}"
  "predecessor-context-accept|typed_rollover_handoff_predecessor_context_accept_bug.cfg|12|${INVARIANT_MARKER}"
  "premature-mint|typed_rollover_handoff_premature_mint_bug.cfg|12|${INVARIANT_MARKER}"
  "preserve-process-receipt-across-crash|typed_rollover_handoff_preserve_process_receipt_across_crash_bug.cfg|12|${INVARIANT_MARKER}"
  "publish-memory-before-lifecycle-root-v3-commit|typed_rollover_handoff_publish_memory_before_lifecycle_root_v3_commit_bug.cfg|12|${INVARIANT_MARKER}"
  "retry-loss|typed_rollover_handoff_retry_loss_bug.cfg|12|${INVARIANT_MARKER}"
  "reuse-root-selected-state-slot|typed_rollover_handoff_reuse_root_selected_state_slot_bug.cfg|12|${INVARIANT_MARKER}"
  "root-commit-before-state-slot|typed_rollover_handoff_root_commit_before_state_slot_bug.cfg|12|${INVARIANT_MARKER}"
  "root-generation-overflow|typed_rollover_handoff_root_generation_overflow_bug.cfg|12|${INVARIANT_MARKER}"
  "same-roster-generation-roll|typed_rollover_handoff_same_roster_generation_roll_bug.cfg|12|${INVARIANT_MARKER}"
  "skip-bootstrap-crash-history|typed_rollover_handoff_skip_bootstrap_crash_history_bug.cfg|12|${INVARIANT_MARKER}"
  "skip-lifecycle-root-v3-crash-history|typed_rollover_handoff_skip_lifecycle_root_v3_crash_history_bug.cfg|12|${INVARIANT_MARKER}"
  "split-lifecycle-generation-hash|typed_rollover_handoff_split_generation_hash_bug.cfg|12|${INVARIANT_MARKER}"
  "recover-uncommitted-state-slot|typed_rollover_handoff_recover_uncommitted_state_slot_bug.cfg|12|${INVARIANT_MARKER}"
  "untyped-force|typed_rollover_handoff_untyped_force_bug.cfg|12|${INVARIANT_MARKER}"
  "wrong-bootstrap-lifecycle-projection|typed_rollover_handoff_wrong_bootstrap_lifecycle_projection_bug.cfg|12|${INVARIANT_MARKER}"
)

liveness_mutation_cases=(
  "missing-validated-cleanup-fairness|typed_rollover_handoff_missing_validated_cleanup_fairness_bug.cfg|13|${TEMPORAL_MARKER}"
  "missing-worker-clear-fairness|typed_rollover_handoff_missing_worker_clear_fairness_bug.cfg|13|${TEMPORAL_MARKER}"
)

actual_configs=("${FORMAL_DIR}"/typed_rollover_handoff_*_bug.cfg)
if [[ "${#mutation_cases[@]}" -ne "$EXPECTED_MATRIX_MUTATION_COUNT" ]]; then
  echo "typed rollover shared mutation matrix must contain exactly ${EXPECTED_MATRIX_MUTATION_COUNT} cases; found ${#mutation_cases[@]}" >&2
  exit 1
fi
if [[ "${#liveness_mutation_cases[@]}" -ne "$EXPECTED_LIVENESS_MUTATION_COUNT" ]]; then
  echo "typed rollover liveness mutation matrix must contain exactly ${EXPECTED_LIVENESS_MUTATION_COUNT} cases; found ${#liveness_mutation_cases[@]}" >&2
  exit 1
fi
if [[ "${#actual_configs[@]}" -ne "$EXPECTED_MUTATION_COUNT" ]]; then
  echo "found ${#actual_configs[@]} typed rollover mutation configs; expected ${EXPECTED_MUTATION_COUNT}" >&2
  printf '%s\n' "${actual_configs[@]##*/}" >&2
  exit 1
fi
for actual_path in "${actual_configs[@]}"; do
  actual_config="${actual_path##*/}"
  config_is_expected=false
  if [[ "$actual_config" == "$REPEATED_HANDOFF_MUTATION_CONFIG" ]]; then
    config_is_expected=true
  fi
  for case_spec in "${mutation_cases[@]}"; do
    case_tail="${case_spec#*|}"
    expected_config="${case_tail%%|*}"
    if [[ "$actual_config" == "$expected_config" ]]; then
      config_is_expected=true
      break
    fi
  done
  for case_spec in "${liveness_mutation_cases[@]}"; do
    case_tail="${case_spec#*|}"
    expected_config="${case_tail%%|*}"
    if [[ "$actual_config" == "$expected_config" ]]; then
      config_is_expected=true
      break
    fi
  done
  if [[ "$config_is_expected" != true ]]; then
    echo "unexpected typed rollover mutation config: ${actual_config}" >&2
    exit 1
  fi
done

for case_spec in "${liveness_mutation_cases[@]}"; do
  IFS='|' read -r label config expected_status expected_marker <<<"$case_spec"
  liveness_log="${run_dir}/${label}.log"
  run_case "$label" "$LIVENESS_MUTATION_MODEL" "$config" \
    "$expected_status" "$expected_marker" "Stuttering"
  if grep -Fq "Error: Invariant " "$liveness_log"; then
    echo "${label} was misclassified as an invariant failure" >&2
    cat "$liveness_log" >&2
    exit 1
  fi
done

for case_spec in "${mutation_cases[@]}"; do
  IFS='|' read -r label config expected_status expected_marker <<<"$case_spec"
  run_case "$label" "$MUTATION_MODEL" "$config" "$expected_status" "$expected_marker"
done

run_case repeated-handoff-after-restart-restore \
  "$REPEATED_HANDOFF_MUTATION_MODEL" \
  "$REPEATED_HANDOFF_MUTATION_CONFIG" 12 \
  "$INVARIANT_MARKER"

echo "[tlc] typed rollover-handoff repaired models and 45-mutant root-anchored V3 matrix passed"
