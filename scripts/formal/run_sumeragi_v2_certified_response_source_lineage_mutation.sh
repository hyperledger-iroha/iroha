#!/usr/bin/env bash
# Run the bounded certified-response source-lineage mutation pair.

set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/formal/sumeragi_v2"
readonly MODEL="SumeragiV2CertifiedResponseSourceLineageMutation.tla"
readonly SANY_SUCCESS_MARKER="Semantic processing of module SumeragiV2CertifiedResponseSourceLineageMutation"
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:?TLA2TOOLS_JAR must name the authenticated external tool}"
source "${REPO_ROOT}/scripts/formal/sumeragi_v2_tlc_result_contract.sh"

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
"$JAVA_BIN" -version >/dev/null 2>&1 || {
  echo "a working Java runtime is required" >&2
  exit 1
}

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-certified-response-lineage.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT

(
  cd "$FORMAL_DIR"
  "$JAVA_BIN" -cp "$TLA2TOOLS_JAR" tla2sany.SANY "$MODEL"
) >"${run_dir}/sany.log" 2>&1
sany_last_nonblank="$(awk 'NF { line = $0 } END { print line }' "${run_dir}/sany.log")"
[[ "$sany_last_nonblank" == "$SANY_SUCCESS_MARKER" ]] || {
  echo "SANY did not end at the expected semantic-processing marker" >&2
  cat "${run_dir}/sany.log" >&2
  exit 1
}
echo "[sany] certified-response source-lineage mutation parsed with pinned TLA2Tools v${TLA2TOOLS_VERSION}"

common=(
  "$JAVA_BIN" -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -workers 1 -fp 96 -seed 139154308881391968
)

run_case() {
  local label="$1"
  local config="$2"
  local expected_status="$3"
  shift 3
  local log="${run_dir}/${label}.log"
  local actual_status
  local expected_diagnostic
  local expected_diagnostic_count=0
  local initial_state_counterexample=0
  local primary_diagnostic_count
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
  elif [[ "$expected_status" -eq 12 || "$expected_status" -eq 13 ]]; then
    sumeragi_v2_tlc_assert_terminal "$label" "$log"
  fi
  for marker in "$@"; do
    case "$marker" in
      "Invariant "*)
        expected_diagnostic="Error: ${marker}"
        if [[ "$marker" == *" is violated by the initial state" ]]; then
          expected_diagnostic="${expected_diagnostic}:"
          initial_state_counterexample=1
        fi
        sumeragi_v2_tlc_assert_exact_line \
          "$label" "$log" "$expected_diagnostic"
        expected_diagnostic_count=$((expected_diagnostic_count + 1))
        ;;
      "Action property "*|"Temporal properties were violated.")
        expected_diagnostic="Error: ${marker}"
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
  if [[ "$expected_status" -eq 12 || "$expected_status" -eq 13 ]]; then
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
    if [[ "$initial_state_counterexample" -eq 0 ]]; then
      sumeragi_v2_tlc_assert_nonzero_state_space "$label" "$log"
    fi
  fi
  echo "[tlc] ${label}: expected status ${expected_status}"
}

run_case embedded-cited-signer-surrogate-fixed \
  certified_response_source_lineage_fixed.cfg 0 \
  "TLC2 Version 2.19" \
  "Model checking completed. No error has been found." \
  "3 states generated, 2 distinct states found, 0 states left on queue." \
  "The depth of the complete state graph search is 2."

run_case outer-transport-source-mutant \
  certified_response_source_lineage_outer_source_bug.cfg 12 \
  "TLC2 Version 2.19" \
  "Invariant DecisionRecoveryOwnerRetained is violated." \
  "State 2: <ScheduleCertifiedResponse" \
  '/\ stage = "FetchCertifiedBody"' \
  '/\ candidateScheduled = TRUE' \
  "2 states generated, 2 distinct states found, 0 states left on queue." \
  "The depth of the complete state graph search is 2."

echo "[tlc] embedded cited-signer surrogate retained certified-response ownership"
echo "[tlc] outer untrusted transport source orphaned the Decision recovery owner"
