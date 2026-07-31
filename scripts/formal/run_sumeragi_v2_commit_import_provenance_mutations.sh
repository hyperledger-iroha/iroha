#!/usr/bin/env bash
# Run exact historical Commit-import provenance mutation pairs.

set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly EXPECTED_JAVA_VERSION='openjdk version "21.0.12"'
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/formal/sumeragi_v2"
readonly MODEL="SumeragiV2CommitImportProvenanceMutation.tla"
readonly FIXED_EXECUTION_CONFIG="commit_import_provenance_fixed.cfg"
readonly EXECUTION_BUG_CONFIG="commit_import_provenance_execution_bug.cfg"
readonly FIXED_SUCCESSOR_CONFIG="commit_import_successor_fixed.cfg"
readonly SUCCESSOR_BUG_CONFIG="commit_import_successor_replacement_bug.cfg"
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-${REPO_ROOT}/target/tla2tools/${TLA2TOOLS_VERSION}/tla2tools.jar}"
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

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-commit-import-provenance.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT

(
  cd "$FORMAL_DIR"
  "$JAVA_BIN" -cp "$TLA2TOOLS_JAR" tla2sany.SANY "$MODEL"
) >"${run_dir}/sany.log" 2>&1
readonly SANY_SUCCESS_MARKER="Semantic processing of module ${MODEL%.tla}"
sany_last_nonblank="$(awk 'NF { line = $0 } END { print line }' "${run_dir}/sany.log")"
[[ "$sany_last_nonblank" == "$SANY_SUCCESS_MARKER" ]] || {
  echo "Commit-import provenance model did not reach the exact SANY marker" >&2
  cat "${run_dir}/sany.log" >&2
  exit 1
}
[[ "$(grep -Fxc "$SANY_SUCCESS_MARKER" "${run_dir}/sany.log" || true)" == 1 ]] || {
  echo "Commit-import provenance model emitted an ambiguous SANY marker" >&2
  cat "${run_dir}/sany.log" >&2
  exit 1
}

common=(
  "$JAVA_BIN" -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -workers 1 -fp 73 -seed 576538917624817903
)

run_tlc() {
  local label="$1"
  local config="$2"
  local expected_status="$3"
  local log="${run_dir}/${label}.log"
  local actual_status
  set +e
  (
    cd "$FORMAL_DIR"
    "${common[@]}" -metadir "${run_dir}/${label}" \
      -config "$config" "$MODEL"
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

assert_exact_invariant_counterexample() {
  local label="$1"
  local log="$2"
  local invariant_marker="$3"
  local primary_diagnostic_count
  sumeragi_v2_tlc_assert_exact_line "$label" "$log" "$invariant_marker"
  sumeragi_v2_tlc_assert_exact_line \
    "$label" "$log" "Error: The behavior up to this point is:"
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
}

fixed_execution_log="$(run_tlc fixed-execution "$FIXED_EXECUTION_CONFIG" 0)"
fixed_successor_log="$(run_tlc fixed-successor "$FIXED_SUCCESSOR_CONFIG" 0)"
for log in "$fixed_execution_log" "$fixed_successor_log"; do
  [[ "$(grep -Fxc "Model checking completed. No error has been found." "$log" || true)" == 1 ]] || {
    echo "repaired Commit-import provenance case missed exact completion" >&2
    cat "$log" >&2
    exit 1
  }
  [[ "$(grep -Ec '^(Error:|Deadlock reached[.])' "$log" || true)" == 0 ]] || {
    echo "repaired Commit-import provenance case emitted a failure diagnostic" >&2
    cat "$log" >&2
    exit 1
  }
done

execution_bug_log="$(run_tlc execution-guard-removed "$EXECUTION_BUG_CONFIG" 12)"
readonly EXECUTION_INVARIANT_MARKER="Error: Invariant ExecutedImportsHaveExactLineage is violated."
assert_exact_invariant_counterexample \
  "execution-guard-removed" "$execution_bug_log" \
  "$EXECUTION_INVARIANT_MARKER"

successor_bug_log="$(run_tlc successor-origin-replaced "$SUCCESSOR_BUG_CONFIG" 12)"
readonly SUCCESSOR_INVARIANT_MARKER="Error: Invariant ProducedSuccessorsRetainExactLineage is violated."
assert_exact_invariant_counterexample \
  "successor-origin-replaced" "$successor_bug_log" \
  "$SUCCESSOR_INVARIANT_MARKER"

echo "[tlc] repaired Commit-import execution and causal-successor provenance models passed"
echo "[tlc] missing execution guard and successor-origin replacement produced exact invariant status 12"
