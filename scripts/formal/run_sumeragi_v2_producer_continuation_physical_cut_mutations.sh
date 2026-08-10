#!/usr/bin/env bash
# Run exact-ingress, producer-continuation, and adequate-periodic cut pairs.

set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly EXPECTED_JAVA_VERSION='openjdk version "21.0.12"'
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/formal/sumeragi_v2"
readonly MODEL="SumeragiV2ProducerContinuationPhysicalCutMutation.tla"
readonly CURRENT_FIXED_CONFIG="current_ingress_physical_cut_fixed.cfg"
readonly CURRENT_CHURN_BUG_CONFIG="current_ingress_replenishment_churn_bug.cfg"
readonly CONTINUATION_FIXED_CONFIG="producer_continuation_physical_cut_fixed.cfg"
readonly CONTINUATION_LOGICAL_BUG_CONFIG="producer_continuation_logical_only_replay_bug.cfg"
readonly TIMEOUT_FIXED_CONFIG="producer_continuation_timeout_cut_fixed.cfg"
readonly TIMEOUT_LOGICAL_BUG_CONFIG="producer_continuation_timeout_cut_logical_minimum_bug.cfg"
readonly PERIODIC_MODEL="SumeragiV2AdequateLeaderPeriodicPrefixMutation.tla"
readonly PERIODIC_FIXED_CONFIG="adequate_leader_periodic_prefix_fixed.cfg"
readonly PERIODIC_HIDDEN_BUG_CONFIG="adequate_leader_periodic_hidden_prefix_bug.cfg"
readonly PERIODIC_REPLENISHMENT_BUG_CONFIG="adequate_leader_periodic_replenishment_bug.cfg"
readonly TEMPORAL_VIOLATION_DIAGNOSTIC="Error: Temporal properties were violated."
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:?TLA2TOOLS_JAR must name the authenticated external tool}"
source "${REPO_ROOT}/scripts/formal/sumeragi_v2_tlc_result_contract.sh"

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
if [[ -n "${JAVA_BIN:-}" ]]; then
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh" "$JAVA_BIN")"
else
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh")"
fi
readonly JAVA_BIN="$resolved_java_bin"
java_version="$($JAVA_BIN -version 2>&1)"
grep -Fq "$EXPECTED_JAVA_VERSION" <<<"$java_version" || {
  echo "frozen Java 21.0.12 is required" >&2
  printf '%s\n' "$java_version" >&2
  exit 1
}

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-producer-cut.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT

(
  cd "$FORMAL_DIR"
  "$JAVA_BIN" -cp "$TLA2TOOLS_JAR" tla2sany.SANY "$MODEL"
) >"${run_dir}/sany.log" 2>&1
readonly SANY_SUCCESS_MARKER="Semantic processing of module ${MODEL%.tla}"
sany_last_nonblank="$(awk 'NF { line = $0 } END { print line }' "${run_dir}/sany.log")"
[[ "$sany_last_nonblank" == "$SANY_SUCCESS_MARKER" ]] || {
  echo "producer-continuation physical-cut model missed the exact SANY marker" >&2
  cat "${run_dir}/sany.log" >&2
  exit 1
}
[[ "$(grep -Fxc "$SANY_SUCCESS_MARKER" "${run_dir}/sany.log" || true)" == 1 ]] || {
  echo "producer-continuation physical-cut model emitted an ambiguous SANY marker" >&2
  cat "${run_dir}/sany.log" >&2
  exit 1
}

(
  cd "$FORMAL_DIR"
  "$JAVA_BIN" -cp "$TLA2TOOLS_JAR" tla2sany.SANY "$PERIODIC_MODEL"
) >"${run_dir}/periodic-sany.log" 2>&1
readonly PERIODIC_SANY_SUCCESS_MARKER="Semantic processing of module ${PERIODIC_MODEL%.tla}"
periodic_sany_last_nonblank="$(awk 'NF { line = $0 } END { print line }' "${run_dir}/periodic-sany.log")"
[[ "$periodic_sany_last_nonblank" == "$PERIODIC_SANY_SUCCESS_MARKER" ]] || {
  echo "adequate-leader periodic-prefix model missed the exact SANY marker" >&2
  cat "${run_dir}/periodic-sany.log" >&2
  exit 1
}
[[ "$(grep -Fxc "$PERIODIC_SANY_SUCCESS_MARKER" "${run_dir}/periodic-sany.log" || true)" == 1 ]] || {
  echo "adequate-leader periodic-prefix model emitted an ambiguous SANY marker" >&2
  cat "${run_dir}/periodic-sany.log" >&2
  exit 1
}

common=(
  "$JAVA_BIN" -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -workers 1 -fp 73 -seed 815760958143817463
)

run_tlc() {
  local label="$1"
  local config="$2"
  local expected_status="$3"
  local model="${4:-$MODEL}"
  local log="${run_dir}/${label}.log"
  local actual_status
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
    sumeragi_v2_tlc_assert_nonzero_state_space "$label" "$log"
    sumeragi_v2_tlc_assert_terminal "$label" "$log"
  fi
  printf '%s\n' "$log"
}

current_fixed_log="$(run_tlc current-ingress-fixed "$CURRENT_FIXED_CONFIG" 0)"
continuation_fixed_log="$(run_tlc continuation-cut-fixed "$CONTINUATION_FIXED_CONFIG" 0)"
timeout_fixed_log="$(run_tlc timeout-cut-fixed "$TIMEOUT_FIXED_CONFIG" 0)"
periodic_fixed_log="$(
  run_tlc adequate-periodic-fixed "$PERIODIC_FIXED_CONFIG" 0 "$PERIODIC_MODEL"
)"
for log in \
  "$current_fixed_log" \
  "$continuation_fixed_log" \
  "$timeout_fixed_log" \
  "$periodic_fixed_log"; do
  sumeragi_v2_tlc_assert_exact_line \
    "producer-continuation repaired case" "$log" \
    "Model checking completed. No error has been found."
  [[ "$(grep -Ec '^(Error:|Deadlock reached[.])' "$log" || true)" == 0 ]] || {
    echo "producer-continuation repaired case emitted a failure diagnostic" >&2
    cat "$log" >&2
    exit 1
  }
done

periodic_hidden_bug_log="$(
  run_tlc adequate-periodic-hidden \
    "$PERIODIC_HIDDEN_BUG_CONFIG" 12 "$PERIODIC_MODEL"
)"
sumeragi_v2_tlc_assert_exact_line \
  "adequate-leader hidden periodic prefix" "$periodic_hidden_bug_log" \
  "Error: Invariant FiniteOwnerEpisodeStartsAfterPeriodicPrefixDrains is violated."
[[ "$(grep -Ec "$SUMERAGI_V2_TLC_PRIMARY_DIAGNOSTIC_PATTERN" "$periodic_hidden_bug_log" || true)" == 1 ]] || {
  echo "adequate-leader hidden periodic prefix emitted an ambiguous primary diagnostic" >&2
  cat "$periodic_hidden_bug_log" >&2
  exit 1
}

current_bug_log="$(run_tlc current-ingress-churn "$CURRENT_CHURN_BUG_CONFIG" 13)"
timeout_bug_log="$(run_tlc timeout-logical-minimum "$TIMEOUT_LOGICAL_BUG_CONFIG" 13)"
for log in "$current_bug_log" "$timeout_bug_log"; do
  sumeragi_v2_tlc_assert_exact_line \
    "producer-continuation lasso" "$log" \
    "$TEMPORAL_VIOLATION_DIAGNOSTIC"
  grep -Fq "Stuttering" "$log" || {
    echo "producer-continuation lasso missed the TLC Stuttering marker" >&2
    cat "$log" >&2
    exit 1
  }
  [[ "$(grep -Ec "$SUMERAGI_V2_TLC_PRIMARY_DIAGNOSTIC_PATTERN" "$log" || true)" == 1 ]] || {
    echo "producer-continuation lasso emitted an ambiguous primary diagnostic" >&2
    cat "$log" >&2
    exit 1
  }
done

periodic_replenishment_bug_log="$(
  run_tlc adequate-periodic-replenishment \
    "$PERIODIC_REPLENISHMENT_BUG_CONFIG" 13 "$PERIODIC_MODEL"
)"
sumeragi_v2_tlc_assert_exact_line \
  "adequate-leader periodic replenishment lasso" \
  "$periodic_replenishment_bug_log" \
  "$TEMPORAL_VIOLATION_DIAGNOSTIC"
grep -Fq "Back to state" \
  "$periodic_replenishment_bug_log" || {
  echo "adequate-leader periodic replenishment missed the lasso back-edge" >&2
  cat "$periodic_replenishment_bug_log" >&2
  exit 1
}
[[ "$(grep -Ec "$SUMERAGI_V2_TLC_PRIMARY_DIAGNOSTIC_PATTERN" "$periodic_replenishment_bug_log" || true)" == 1 ]] || {
  echo "adequate-leader periodic replenishment emitted an ambiguous primary diagnostic" >&2
  cat "$periodic_replenishment_bug_log" >&2
  exit 1
}

continuation_bug_log="$(
  run_tlc continuation-logical-only "$CONTINUATION_LOGICAL_BUG_CONFIG" 12
)"
sumeragi_v2_tlc_assert_exact_line \
  "producer-continuation provenance mutant" "$continuation_bug_log" \
  "Error: Invariant CausalSuccessorRetainsPostCutPhysicalRoot is violated."
[[ "$(grep -Ec "$SUMERAGI_V2_TLC_PRIMARY_DIAGNOSTIC_PATTERN" "$continuation_bug_log" || true)" == 1 ]] || {
  echo "producer-continuation provenance mutant emitted an ambiguous primary diagnostic" >&2
  cat "$continuation_bug_log" >&2
  exit 1
}

echo "[tlc] repaired exact-ingress, continuation, timeout-cut, and periodic-prefix selectors passed"
echo "[tlc] replenishment, hidden-prefix, logical-only, and post-timeout-cut mutants failed as intended"
