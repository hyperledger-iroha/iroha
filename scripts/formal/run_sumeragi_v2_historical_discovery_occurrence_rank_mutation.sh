#!/usr/bin/env bash
# Run the historical-discovery count-first occurrence-rank mutation pair.

set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly EXPECTED_JAVA_VERSION='openjdk version "21.0.12"'
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/formal/sumeragi_v2"
readonly MODEL="SumeragiV2HistoricalDiscoveryOccurrenceRankMutation.tla"
readonly MODULE="${MODEL%.tla}"
readonly FIXED_CONFIG="historical_discovery_occurrence_rank_fixed.cfg"
readonly MUTANT_CONFIG="historical_discovery_plain_minimum_bug.cfg"
readonly SANY_SUCCESS_MARKER="Semantic processing of module ${MODULE}"
readonly FIXED_SUCCESS_MARKER="Model checking completed. No error has been found."
readonly MUTANT_FIRST_MARKER="Error: Invariant RankNeverIncreases is violated."
readonly RETAINED_NEGATIVE_CONTROL="INVARIANT LowerServiceStrictlyDescends"
readonly CONTINUATION_MODEL="SumeragiV2HistoricalProducerContinuationMutation.tla"
readonly CONTINUATION_MODULE="${CONTINUATION_MODEL%.tla}"
readonly CONTINUATION_FIXED_CONFIG="historical_producer_continuation_fixed.cfg"
readonly CONTINUATION_MUTANT_CONFIG="historical_producer_continuation_voter_only_bug.cfg"
readonly CONTINUATION_SANY_SUCCESS_MARKER="Semantic processing of module ${CONTINUATION_MODULE}"
readonly CONTINUATION_TEMPORAL_MARKER="Error: Temporal properties were violated."
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-${REPO_ROOT}/target/tla2tools/${TLA2TOOLS_VERSION}/tla2tools.jar}"
source "${REPO_ROOT}/scripts/formal/sumeragi_v2_tlc_result_contract.sh"

if (($#)); then
  echo "usage: $0" >&2
  exit 2
fi

if [[ -n "${JAVA_BIN:-}" ]]; then
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh" "$JAVA_BIN")"
else
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh")"
fi
readonly JAVA_BIN="$resolved_java_bin"

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
java_version="$("$JAVA_BIN" -version 2>&1)"
grep -Fq "$EXPECTED_JAVA_VERSION" <<<"$java_version" || {
  echo "frozen Java 21.0.12 is required" >&2
  printf '%s\n' "$java_version" >&2
  exit 1
}

grep -Fqx "$RETAINED_NEGATIVE_CONTROL" \
  "${FORMAL_DIR}/${MUTANT_CONFIG}" || {
  echo "${MUTANT_CONFIG} must retain the exact ${RETAINED_NEGATIVE_CONTROL} negative control" >&2
  exit 1
}

run_dir="$(
  mktemp -d \
    "${TMPDIR:-/tmp}/sumeragi-historical-discovery-occurrence-rank.XXXXXX"
)"
trap 'rm -rf -- "$run_dir"' EXIT

(
  cd "$FORMAL_DIR"
  "$JAVA_BIN" -cp "$TLA2TOOLS_JAR" tla2sany.SANY "$MODEL"
) >"${run_dir}/sany.log" 2>&1
sany_last_nonblank="$(
  awk 'NF { line = $0 } END { print line }' "${run_dir}/sany.log"
)"
[[ "$sany_last_nonblank" == "$SANY_SUCCESS_MARKER" ]] || {
  echo "SANY did not end at the exact semantic-processing marker" >&2
  cat "${run_dir}/sany.log" >&2
  exit 1
}
[[ "$(grep -Fxc "$SANY_SUCCESS_MARKER" "${run_dir}/sany.log" || true)" == 1 ]] || {
  echo "SANY did not emit exactly one semantic-processing marker" >&2
  cat "${run_dir}/sany.log" >&2
  exit 1
}
echo "[sany] historical-discovery occurrence-rank mutation parsed with frozen Java 21.0.12"

(
  cd "$FORMAL_DIR"
  "$JAVA_BIN" -cp "$TLA2TOOLS_JAR" tla2sany.SANY "$CONTINUATION_MODEL"
) >"${run_dir}/continuation-sany.log" 2>&1
continuation_sany_last_nonblank="$(
  awk 'NF { line = $0 } END { print line }' \
    "${run_dir}/continuation-sany.log"
)"
[[ "$continuation_sany_last_nonblank" == "$CONTINUATION_SANY_SUCCESS_MARKER" ]] || {
  echo "historical producer-continuation SANY did not end at the exact semantic-processing marker" >&2
  cat "${run_dir}/continuation-sany.log" >&2
  exit 1
}
[[ "$(grep -Fxc "$CONTINUATION_SANY_SUCCESS_MARKER" \
  "${run_dir}/continuation-sany.log" || true)" == 1 ]] || {
  echo "historical producer-continuation SANY did not emit exactly one semantic-processing marker" >&2
  cat "${run_dir}/continuation-sany.log" >&2
  exit 1
}
echo "[sany] historical producer-continuation mutation parsed with frozen Java 21.0.12"

common=(
  "$JAVA_BIN" -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -workers 1 -fp 89 -seed 351472986013
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
  [[ "$actual_status" -eq "$expected_status" ]] || {
    echo "${label} returned TLC status ${actual_status}, expected ${expected_status}" >&2
    cat "$log" >&2
    exit 1
  }
  if [[ "$expected_status" -eq 0 ]]; then
    sumeragi_v2_tlc_assert_fixed_success "$label" "$log" "$actual_status"
  else
    sumeragi_v2_tlc_assert_nonzero_state_space "$label" "$log"
    sumeragi_v2_tlc_assert_terminal "$label" "$log"
  fi
  printf '%s\n' "$log"
}

fixed_log="$(run_tlc repaired-count-first "$FIXED_CONFIG" 0)"
for marker in \
  "TLC2 Version 2.19" \
  "Computed 1 initial states..." \
  "$FIXED_SUCCESS_MARKER"; do
  [[ "$(grep -Fc "$marker" "$fixed_log" || true)" == 1 ]] || {
    echo "repaired-count-first did not emit exactly one success marker: ${marker}" >&2
    cat "$fixed_log" >&2
    exit 1
  }
done
[[ "$(grep -Ec '^(Error:|Deadlock reached[.])' "$fixed_log" || true)" == 0 ]] || {
  echo "repaired-count-first emitted an error diagnostic" >&2
  cat "$fixed_log" >&2
  exit 1
}
echo "[tlc] repaired count-first occurrence rank completed with status 0"

mutant_log="$(run_tlc plain-minimum-mutant "$MUTANT_CONFIG" 12)"
first_invariant_marker="$(
  grep -m1 -E '^Error: Invariant .+ is violated[.]$' "$mutant_log" || true
)"
[[ "$first_invariant_marker" == "$MUTANT_FIRST_MARKER" ]] || {
  echo "plain-minimum-mutant first invariant marker was not ${MUTANT_FIRST_MARKER}" >&2
  cat "$mutant_log" >&2
  exit 1
}
[[ "$(grep -Fxc "$MUTANT_FIRST_MARKER" "$mutant_log" || true)" == 1 ]] || {
  echo "plain-minimum-mutant did not emit exactly one ${MUTANT_FIRST_MARKER} marker" >&2
  cat "$mutant_log" >&2
  exit 1
}
mutant_primary_diagnostic_count="$(
  grep -Ec \
    "$SUMERAGI_V2_TLC_PRIMARY_DIAGNOSTIC_PATTERN" \
    "$mutant_log" || true
)"
[[ "$mutant_primary_diagnostic_count" -eq 1 ]] || {
  echo "plain-minimum-mutant emitted ${mutant_primary_diagnostic_count} primary failure diagnostics" >&2
  cat "$mutant_log" >&2
  exit 1
}
[[ "$(grep -Fxc "Error: The behavior up to this point is:" "$mutant_log" || true)" == 1 ]] || {
  echo "plain-minimum-mutant did not emit exactly one counterexample trace marker" >&2
  cat "$mutant_log" >&2
  exit 1
}
echo "[tlc] plain-minimum mutant failed first at RankNeverIncreases with status 12"
echo "[tlc] exact terminal contract passed; LowerServiceStrictlyDescends remained an enabled negative control"

continuation_fixed_log="$(
  run_tlc historical-producer-continuation-fixed \
    "$CONTINUATION_FIXED_CONFIG" 0 "$CONTINUATION_MODEL"
)"
[[ "$(grep -Fxc "$FIXED_SUCCESS_MARKER" "$continuation_fixed_log" || true)" == 1 ]] || {
  echo "historical producer-continuation repair missed its exact success marker" >&2
  cat "$continuation_fixed_log" >&2
  exit 1
}

continuation_mutant_log="$(
  run_tlc historical-producer-continuation-voter-only \
    "$CONTINUATION_MUTANT_CONFIG" 13 "$CONTINUATION_MODEL"
)"
sumeragi_v2_tlc_assert_exact_line \
  "historical-producer-continuation-voter-only" \
  "$continuation_mutant_log" "$CONTINUATION_TEMPORAL_MARKER"
[[ "$(grep -Ec \
  '^Error: (Invariant |Action property |Temporal properties were violated[.]$|Deadlock reached([.]|$))' \
  "$continuation_mutant_log" || true)" == 1 ]] || {
  echo "historical producer-continuation mutant did not emit exactly one primary diagnostic" >&2
  cat "$continuation_mutant_log" >&2
  exit 1
}
grep -Fq "Stuttering" "$continuation_mutant_log" || {
  echo "historical producer-continuation mutant missed its stuttering lasso marker" >&2
  cat "$continuation_mutant_log" >&2
  exit 1
}
echo "[tlc] historical non-voter continuation produced exact temporal status 13"
