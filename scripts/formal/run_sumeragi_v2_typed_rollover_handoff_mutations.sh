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
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-${REPO_ROOT}/target/tla2tools/${TLA2TOOLS_VERSION}/tla2tools.jar}"

usage() {
  cat <<USAGE
usage: $0 [--help]

Parse the typed rollover-handoff base, mutation, and proof modules with SANY,
then run the fixed TLC model and all 20 deterministic mutation configurations.

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
  for marker in "$@"; do
    if ! grep -Fq "$marker" "$log"; then
      echo "${label} missed expected marker: ${marker}" >&2
      cat "$log" >&2
      exit 1
    fi
  done
  echo "[tlc] ${label}: expected status ${expected_status}"
}

run_case typed-rollover-fixed \
  "$FIXED_MODEL" typed_rollover_handoff_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "228 states generated, 131 distinct states found, 0 states left on queue." \
  "The depth of the complete state graph search is 14."

readonly INVARIANT_MARKER="Error: Invariant TypedRolloverSafetyInvariant is violated."
readonly FOREIGN_OWNER_ACTION_MARKER="Error: Action property ForeignOwnerCandidateActionProperty is violated."
readonly TORN_HISTORY_ACTION_MARKER="Error: Action property TornHighWaterHistoryActionProperty is violated."

mutation_cases=(
  "clean-foreign-owner-reject|typed_rollover_handoff_clean_foreign_owner_reject_bug.cfg|12|${INVARIANT_MARKER}"
  "clean-high-water-persistence-failure|typed_rollover_handoff_clean_high_water_persistence_failure_bug.cfg|12|${INVARIANT_MARKER}"
  "clean-late-enqueue-reject|typed_rollover_handoff_clean_late_enqueue_reject_bug.cfg|12|${INVARIANT_MARKER}"
  "clean-lifecycle-snapshot-persistence-failure|typed_rollover_handoff_clean_lifecycle_snapshot_persistence_failure_bug.cfg|12|${INVARIANT_MARKER}"
  "clean-predecessor-artifact-reject|typed_rollover_handoff_clean_predecessor_artifact_reject_bug.cfg|12|${INVARIANT_MARKER}"
  "clean-predecessor-context-reject|typed_rollover_handoff_clean_predecessor_context_reject_bug.cfg|12|${INVARIANT_MARKER}"
  "clean-wrong-successor-reject|typed_rollover_handoff_clean_wrong_successor_reject_bug.cfg|12|${INVARIANT_MARKER}"
  "foreign-candidate-ignored|typed_rollover_handoff_foreign_candidate_ignored_bug.cfg|13|${FOREIGN_OWNER_ACTION_MARKER}"
  "foreign-receipt|typed_rollover_handoff_foreign_receipt_bug.cfg|12|${INVARIANT_MARKER}"
  "foreign-successor|typed_rollover_handoff_foreign_successor_bug.cfg|12|${INVARIANT_MARKER}"
  "high-water-ahead-open|typed_rollover_handoff_high_water_ahead_open_bug.cfg|13|${TORN_HISTORY_ACTION_MARKER}"
  "high-water-skip|typed_rollover_handoff_high_water_skip_bug.cfg|12|${INVARIANT_MARKER}"
  "late-callback|typed_rollover_handoff_late_callback_bug.cfg|12|${INVARIANT_MARKER}"
  "late-enqueue|typed_rollover_handoff_late_enqueue_bug.cfg|12|${INVARIANT_MARKER}"
  "omit-lifecycle-snapshot-torn-history|typed_rollover_handoff_omit_lifecycle_snapshot_torn_history_bug.cfg|12|${INVARIANT_MARKER}"
  "predecessor-artifact-accept|typed_rollover_handoff_predecessor_artifact_accept_bug.cfg|12|${INVARIANT_MARKER}"
  "predecessor-context-accept|typed_rollover_handoff_predecessor_context_accept_bug.cfg|12|${INVARIANT_MARKER}"
  "premature-mint|typed_rollover_handoff_premature_mint_bug.cfg|12|${INVARIANT_MARKER}"
  "retry-loss|typed_rollover_handoff_retry_loss_bug.cfg|12|${INVARIANT_MARKER}"
  "untyped-force|typed_rollover_handoff_untyped_force_bug.cfg|12|${INVARIANT_MARKER}"
)

actual_configs=("${FORMAL_DIR}"/typed_rollover_handoff_*_bug.cfg)
if [[ "${#actual_configs[@]}" -ne "${#mutation_cases[@]}" ]]; then
  echo "found ${#actual_configs[@]} typed rollover mutation configs; expected ${#mutation_cases[@]}" >&2
  printf '%s\n' "${actual_configs[@]##*/}" >&2
  exit 1
fi
for actual_path in "${actual_configs[@]}"; do
  actual_config="${actual_path##*/}"
  config_is_expected=false
  for case_spec in "${mutation_cases[@]}"; do
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

for case_spec in "${mutation_cases[@]}"; do
  IFS='|' read -r label config expected_status expected_marker <<<"$case_spec"
  run_case "$label" "$MUTATION_MODEL" "$config" "$expected_status" "$expected_marker"
done

echo "[tlc] typed rollover-handoff fixed model and 20-mutant matrix passed"
