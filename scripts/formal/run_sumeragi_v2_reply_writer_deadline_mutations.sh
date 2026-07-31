#!/usr/bin/env bash
# Run the orthogonal exact-reply writer-deadline mutation matrix.

set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly TLAPM_COMMIT="3ab43c7ff31db4ced850619d4746fa4c841a7681"
readonly TLAPM_TLAPS_SHA256="5cc604533e49792c1c3d050a38d845d08d9c209879ca20c86de04975bc4bc563"
readonly EXPECTED_JAVA_VERSION='openjdk version "21.0.12"'
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/formal/sumeragi_v2"
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-${REPO_ROOT}/target/tla2tools/${TLA2TOOLS_VERSION}/tla2tools.jar}"
source "${REPO_ROOT}/scripts/formal/sumeragi_v2_tlc_result_contract.sh"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) readonly TLAPM_PLATFORM="x86_64-linux-gnu" ;;
  Darwin-arm64) readonly TLAPM_PLATFORM="arm64-darwin" ;;
  *)
    echo "unsupported TLAPM host: $(uname -s)-$(uname -m)" >&2
    exit 1
    ;;
esac
readonly TLAPM_STDLIB="${TLAPM_STDLIB:-${REPO_ROOT}/target/tlapm/toolchains/${TLAPM_COMMIT}/${TLAPM_PLATFORM}/tlapm/lib/tlapm/stdlib}"

if [[ -n "${JAVA_BIN:-}" ]]; then
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh" "$JAVA_BIN")"
else
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh")"
fi
readonly JAVA_BIN="$resolved_java_bin"

if (($#)); then
  echo "usage: $0" >&2
  exit 2
fi

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

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-reply-writer-deadline.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT

for module in \
  SumeragiV2ReplyWriterDeadline \
  SumeragiV2ReplyWriterDeadlineMutation \
  SumeragiV2ReplyWriterDeadlineProofs; do
  (
    cd "$FORMAL_DIR"
    "$JAVA_BIN" "-DTLA-Library=${TLAPM_STDLIB}" \
      -cp "$TLA2TOOLS_JAR" tla2sany.SANY "${module}.tla"
  ) >"${run_dir}/${module}.sany.log" 2>&1
  sany_last_nonblank="$(
    awk 'NF { line = $0 } END { print line }' \
      "${run_dir}/${module}.sany.log"
  )"
  expected_marker="Semantic processing of module ${module}"
  [[ "$sany_last_nonblank" == "$expected_marker" ]] || {
    echo "${module}: SANY did not end at the expected marker" >&2
    cat "${run_dir}/${module}.sany.log" >&2
    exit 1
  }
done
echo "[sany] reply-writer deadline models parsed with frozen Java 21.0.12"

readonly DORMANT_CLOCK_MODULE="SumeragiV2DormantReplyClockMutation"
(
  cd "$FORMAL_DIR"
  "$JAVA_BIN" "-DTLA-Library=${TLAPM_STDLIB}" \
    -cp "$TLA2TOOLS_JAR" tla2sany.SANY "${DORMANT_CLOCK_MODULE}.tla"
) >"${run_dir}/${DORMANT_CLOCK_MODULE}.sany.log" 2>&1
dormant_clock_sany_last_nonblank="$(
  awk 'NF { line = $0 } END { print line }' \
    "${run_dir}/${DORMANT_CLOCK_MODULE}.sany.log"
)"
dormant_clock_expected_marker=\
"Semantic processing of module ${DORMANT_CLOCK_MODULE}"
[[ "$dormant_clock_sany_last_nonblank" == \
     "$dormant_clock_expected_marker" ]] || {
  echo "${DORMANT_CLOCK_MODULE}: SANY did not end at the expected marker" >&2
  cat "${run_dir}/${DORMANT_CLOCK_MODULE}.sany.log" >&2
  exit 1
}
echo "[sany] dormant exact-reply clock mutation parsed with frozen Java 21.0.12"

common=(
  "$JAVA_BIN" -XX:+UseParallelGC "-DTLA-Library=${TLAPM_STDLIB}"
  -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -deadlock -workers 1 -fp 71 -seed 729469033612990334
)

run_case() {
  local label="$1"
  local model="$2"
  local config="$3"
  local expected_status="$4"
  shift 4
  local log="${run_dir}/${label}.log"
  local actual_status
  local expected_diagnostic
  local expected_diagnostic_count=0
  local primary_diagnostic_count
  set +e
  (
    cd "$FORMAL_DIR"
    "${common[@]}" -metadir "${run_dir}/${label}" \
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
    sumeragi_v2_tlc_assert_terminal "$label" "$log"
  fi
  for marker in "$@"; do
    case "$marker" in
      "Invariant "*)
        expected_diagnostic="Error: ${marker}"
        sumeragi_v2_tlc_assert_exact_line \
          "$label" "$log" "$expected_diagnostic"
        expected_diagnostic_count=$((expected_diagnostic_count + 1))
        ;;
      "Error: Invariant "*|"Error: Action property "*|\
        "Temporal properties were violated.")
        expected_diagnostic="$marker"
        if [[ "$marker" == "Temporal properties were violated." ]]; then
          expected_diagnostic="Error: ${marker}"
        fi
        sumeragi_v2_tlc_assert_exact_line \
          "$label" "$log" "$expected_diagnostic"
        expected_diagnostic_count=$((expected_diagnostic_count + 1))
        ;;
      *)
        if ! grep -Fq "$marker" "$log"; then
          echo "${label} missed expected marker: ${marker}" >&2
          cat "$log" >&2
          exit 1
        fi
        ;;
    esac
  done
  if [[ "$expected_status" -ne 0 ]]; then
    [[ "$expected_diagnostic_count" -eq 1 ]] || {
      echo "${label} did not declare exactly one failure diagnostic" >&2
      cat "$log" >&2
      exit 1
    }
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
    sumeragi_v2_tlc_assert_nonzero_state_space "$label" "$log"
  fi
  echo "[tlc] ${label}: expected status ${expected_status}"
}

run_case deadline-local-termination-fixed \
  SumeragiV2ReplyWriterDeadline.tla \
  reply_writer_deadline_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "331 states generated, 164 distinct states found, 0 states left on queue." \
  "The depth of the complete state graph search is 20."
run_case deadline-responsive-writer-fixed \
  SumeragiV2ReplyWriterDeadline.tla \
  reply_writer_deadline_responsive_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "331 states generated, 164 distinct states found, 0 states left on queue." \
  "The depth of the complete state graph search is 20."
run_case deadline-mutation-fixed \
  SumeragiV2ReplyWriterDeadlineMutation.tla \
  reply_writer_deadline_mutation_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "331 states generated, 164 distinct states found, 0 states left on queue." \
  "The depth of the complete state graph search is 20."

readonly INVARIANT_MARKER="Error: Invariant ReplyWriterDeadlineInvariant is violated."
readonly ACTION_MARKER="Error: Action property MutationWriterFlushObservationMonotonicity is violated."

mutation_cases=(
  "queue-retry-reset|reply_writer_deadline_retry_reset_bug.cfg|12|7 states generated, 7 distinct states found, 2 states left on queue.|The depth of the complete state graph search is 4."
  "timeout-as-flush|reply_writer_deadline_timeout_as_flush_bug.cfg|12|27 states generated, 22 distinct states found, 12 states left on queue.|The depth of the complete state graph search is 5."
  "closed-attempt-growth|reply_writer_deadline_closed_attempt_bug.cfg|12|22 states generated, 21 distinct states found, 12 states left on queue.|The depth of the complete state graph search is 5."
  "reconnect-attempt-reset|reply_writer_deadline_reconnect_reset_bug.cfg|12|67 states generated, 42 distinct states found, 19 states left on queue.|The depth of the complete state graph search is 6."
  "uncapped-attempt|reply_writer_deadline_uncapped_attempt_bug.cfg|12|225 states generated, 116 distinct states found, 11 states left on queue.|The depth of the complete state graph search is 11."
  "topology-deadline|reply_writer_deadline_topology_deadline_bug.cfg|12|6 states generated, 6 distinct states found, 2 states left on queue.|The depth of the complete state graph search is 3."
  "replacement-termination|reply_writer_deadline_replacement_kill_bug.cfg|12|66 states generated, 40 distinct states found, 17 states left on queue.|The depth of the complete state graph search is 6."
  "timeout-beats-ready-flush|reply_writer_deadline_timeout_beats_flush_bug.cfg|12|98 states generated, 52 distinct states found, 13 states left on queue.|The depth of the complete state graph search is 7."
  "wrong-timeout-attempt-flush|reply_writer_deadline_wrong_attempt_flush_bug.cfg|12|136 states generated, 66 distinct states found, 10 states left on queue.|The depth of the complete state graph search is 9."
  "inactive-close-beats-ready-flush|reply_writer_deadline_close_ready_flush_bug.cfg|12|55 states generated, 39 distinct states found, 19 states left on queue.|The depth of the complete state graph search is 6."
  "retirement-beats-ready-flush|reply_writer_deadline_retire_ready_flush_bug.cfg|12|46 states generated, 35 distinct states found, 17 states left on queue.|The depth of the complete state graph search is 6."
  "erase-ready-flush-witness|reply_writer_deadline_erase_ready_witness_bug.cfg|13|46 states generated, 34 distinct states found, 17 states left on queue.|The depth of the complete state graph search is 6."
)

for case_spec in "${mutation_cases[@]}"; do
  IFS='|' read -r label config expected_status state_marker depth_marker <<<"$case_spec"
  if [[ "$expected_status" -eq 12 ]]; then
    violation_marker="$INVARIANT_MARKER"
  else
    violation_marker="$ACTION_MARKER"
  fi
  run_case "$label" \
    SumeragiV2ReplyWriterDeadlineMutation.tla "$config" "$expected_status" \
    "$violation_marker" "$state_marker" "$depth_marker"
done

run_case dormant-reply-clock-fixed \
  SumeragiV2DormantReplyClockMutation.tla \
  dormant_reply_clock_fixed.cfg 0 \
  "Model checking completed. No error has been found."

run_case all-due-reply-clock-freeze \
  SumeragiV2DormantReplyClockMutation.tla \
  dormant_reply_clock_all_due_bug.cfg 13 \
  "Temporal properties were violated." \
  "Stuttering"

echo "[tlc] reply-writer deadline mutation matrix passed"
echo "[tlc] dormant exact-reply packets stay retained without freezing the next-view clock"
