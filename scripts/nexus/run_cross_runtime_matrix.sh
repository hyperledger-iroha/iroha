#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: run_cross_runtime_matrix.sh [OPTIONS]

Run Nexus cross-dataspace and runtime-registration integration tests in a matrix
and emit CSV summaries.

Options:
  --cross-runs <N>                 Number of cross-dataspace test runs (default: 5, 0 disables)
  --runtime-runs <N>               Number of runtime-registration test runs (default: 5, 0 disables)
  --cross-soak-iterations <N>      Override IROHA_NEXUS_CROSS_SOAK_ITERATIONS (default: 10)
  --runtime-bench-iterations <N>   Override IROHA_NEXUS_RUNTIME_BENCH_ITERATIONS (default: 5)
  --cross-timeout-s <N>            Timeout in seconds for one cross run (default: 0, disabled)
  --runtime-timeout-s <N>          Timeout in seconds for one runtime run (default: 0, disabled)
  --target-dir <PATH>              Set CARGO_TARGET_DIR (default: /tmp/iroha_target_cross_opt_rolling)
  --output-dir <PATH>              Matrix output directory (default: /tmp/iroha_gapfill_matrix)
  --cargo-jobs <N>                 Set CARGO_BUILD_JOBS for cargo test (default: 1)
  --test-threads <N>               --test-threads for cargo test (default: 1)
  --prefer-test-binary             Prefer running prebuilt test binary from <target-dir>/debug/deps/mod-*
  --summary-json-rows              Include per-run row arrays in matrix_summary.json
  --skip-cross                     Skip all cross-dataspace runs
  --skip-runtime                   Skip all runtime-registration runs
  --no-skip-bindings-sync          Do not set NORITO_SKIP_BINDINGS_SYNC=1
  --no-skip-build                  Do not set IROHA_TEST_SKIP_BUILD=1
  --continue-on-failure            Keep running and record failed rows
  -h, --help                       Show this help

Outputs:
  <output-dir>/cross_dataspace_matrix.csv
  <output-dir>/runtime_registration_matrix.csv
  <output-dir>/matrix_summary.txt
  <output-dir>/matrix_summary.json
  <output-dir>/cross_run_<N>.log
  <output-dir>/runtime_run_<N>.log
EOF
}

require_positive_int() {
  local name="$1"
  local value="$2"
  if [[ ! "$value" =~ ^[0-9]+$ ]] || [[ "$value" -lt 1 ]]; then
    echo "Invalid ${name}: ${value} (expected positive integer)" >&2
    exit 2
  fi
}

require_nonnegative_int() {
  local name="$1"
  local value="$2"
  if [[ ! "$value" =~ ^[0-9]+$ ]]; then
    echo "Invalid ${name}: ${value} (expected non-negative integer)" >&2
    exit 2
  fi
}

require_option_value() {
  local flag="$1"
  local value="${2-}"
  local allow_option_like="${3-false}"
  if [[ -z "${value}" ]]; then
    echo "Missing value for ${flag}" >&2
    exit 2
  fi
  if [[ "${allow_option_like}" != true ]] && [[ "${value}" == --* ]]; then
    echo "Missing value for ${flag}" >&2
    exit 2
  fi
}

require_command() {
  local cmd="$1"
  if ! command -v "${cmd}" >/dev/null 2>&1; then
    echo "Required command not found: ${cmd}" >&2
    exit 2
  fi
}

CROSS_RUNS=5
RUNTIME_RUNS=5
CROSS_SOAK_ITERATIONS=10
RUNTIME_BENCH_ITERATIONS=5
TARGET_DIR="/tmp/iroha_target_cross_opt_rolling"
OUTPUT_DIR="/tmp/iroha_gapfill_matrix"
TEST_THREADS=1
SKIP_BUILD=true
CONTINUE_ON_FAILURE=false
CROSS_TIMEOUT_S=0
RUNTIME_TIMEOUT_S=0
RUN_CROSS=true
RUN_RUNTIME=true
CARGO_JOBS=1
SKIP_BINDINGS_SYNC=true
PREFER_TEST_BINARY=false
SUMMARY_JSON_ROWS=false
CACHED_CROSS_TEST_BIN=""
CACHED_RUNTIME_TEST_BIN=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --cross-runs)
      require_option_value "--cross-runs" "${2-}"
      CROSS_RUNS="$2"
      shift 2
      ;;
    --runtime-runs)
      require_option_value "--runtime-runs" "${2-}"
      RUNTIME_RUNS="$2"
      shift 2
      ;;
    --cross-soak-iterations)
      require_option_value "--cross-soak-iterations" "${2-}"
      CROSS_SOAK_ITERATIONS="$2"
      shift 2
      ;;
    --runtime-bench-iterations)
      require_option_value "--runtime-bench-iterations" "${2-}"
      RUNTIME_BENCH_ITERATIONS="$2"
      shift 2
      ;;
    --cross-timeout-s)
      require_option_value "--cross-timeout-s" "${2-}"
      CROSS_TIMEOUT_S="$2"
      shift 2
      ;;
    --runtime-timeout-s)
      require_option_value "--runtime-timeout-s" "${2-}"
      RUNTIME_TIMEOUT_S="$2"
      shift 2
      ;;
    --target-dir)
      require_option_value "--target-dir" "${2-}" true
      TARGET_DIR="$2"
      shift 2
      ;;
    --output-dir)
      require_option_value "--output-dir" "${2-}" true
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --cargo-jobs)
      require_option_value "--cargo-jobs" "${2-}"
      CARGO_JOBS="$2"
      shift 2
      ;;
    --test-threads)
      require_option_value "--test-threads" "${2-}"
      TEST_THREADS="$2"
      shift 2
      ;;
    --prefer-test-binary)
      PREFER_TEST_BINARY=true
      shift
      ;;
    --summary-json-rows)
      SUMMARY_JSON_ROWS=true
      shift
      ;;
    --skip-cross)
      RUN_CROSS=false
      shift
      ;;
    --skip-runtime)
      RUN_RUNTIME=false
      shift
      ;;
    --no-skip-bindings-sync)
      SKIP_BINDINGS_SYNC=false
      shift
      ;;
    --no-skip-build)
      SKIP_BUILD=false
      shift
      ;;
    --continue-on-failure)
      CONTINUE_ON_FAILURE=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

require_nonnegative_int "--cross-runs" "$CROSS_RUNS"
require_nonnegative_int "--runtime-runs" "$RUNTIME_RUNS"
require_positive_int "--cross-soak-iterations" "$CROSS_SOAK_ITERATIONS"
require_positive_int "--runtime-bench-iterations" "$RUNTIME_BENCH_ITERATIONS"
require_nonnegative_int "--cross-timeout-s" "$CROSS_TIMEOUT_S"
require_nonnegative_int "--runtime-timeout-s" "$RUNTIME_TIMEOUT_S"
require_positive_int "--cargo-jobs" "$CARGO_JOBS"
require_positive_int "--test-threads" "$TEST_THREADS"

if [[ "$CROSS_RUNS" -eq 0 ]]; then
  RUN_CROSS=false
fi
if [[ "$RUNTIME_RUNS" -eq 0 ]]; then
  RUN_RUNTIME=false
fi
if [[ "$RUN_CROSS" != true ]] && [[ "$RUN_RUNTIME" != true ]]; then
  echo "No runs selected: enable at least one of cross/runtime runs." >&2
  exit 2
fi

require_command "python3"
require_command "rg"

mkdir -p "$OUTPUT_DIR"

CROSS_CSV="${OUTPUT_DIR}/cross_dataspace_matrix.csv"
RUNTIME_CSV="${OUTPUT_DIR}/runtime_registration_matrix.csv"
SUMMARY_TXT="${OUTPUT_DIR}/matrix_summary.txt"
SUMMARY_JSON="${OUTPUT_DIR}/matrix_summary.json"
printf "run,exit,timed_out,duration_s,soak_passed,soak_total,soak_fallbacks,soak_retries,swap_nonconverged_fallbacks\n" > "$CROSS_CSV"
printf "run,exit,timed_out,duration_s,pass_rate,submitter_delta_max,cluster_delta_max,note_zero_delta\n" > "$RUNTIME_CSV"
: > "$SUMMARY_TXT"
: > "$SUMMARY_JSON"

ENV_VARS=()

build_env_vars() {
  ENV_VARS=(
    "CARGO_TARGET_DIR=${TARGET_DIR}"
    "CARGO_BUILD_JOBS=${CARGO_JOBS}"
    "IROHA_NEXUS_CROSS_SOAK_ITERATIONS=${CROSS_SOAK_ITERATIONS}"
    "IROHA_NEXUS_RUNTIME_BENCH_ITERATIONS=${RUNTIME_BENCH_ITERATIONS}"
  )
  if [[ "$SKIP_BUILD" == true ]]; then
    ENV_VARS+=("IROHA_TEST_SKIP_BUILD=1")
  fi
  if [[ "$SKIP_BINDINGS_SYNC" == true ]]; then
    ENV_VARS+=("NORITO_SKIP_BINDINGS_SYNC=1")
  fi
}

run_with_timeout() {
  local timeout_s="$1"
  local timeout_marker="$2"
  shift 2
  rm -f "${timeout_marker}"
  if [[ "$timeout_s" -gt 0 ]]; then
    MATRIX_TIMEOUT_MARKER="${timeout_marker}" python3 - "$timeout_s" "$@" <<'PY'
import os
import signal
import subprocess
import sys

timeout_s = int(sys.argv[1])
command = sys.argv[2:]
timeout_marker = os.environ.get("MATRIX_TIMEOUT_MARKER")

process = subprocess.Popen(command, start_new_session=True)
try:
    sys.exit(process.wait(timeout=timeout_s))
except subprocess.TimeoutExpired:
    if timeout_marker:
        with open(timeout_marker, "w", encoding="utf-8") as marker:
            marker.write("1\n")
    pgid = None
    try:
        pgid = os.getpgid(process.pid)
    except Exception:
        pgid = None
    if pgid is not None:
        try:
            os.killpg(pgid, signal.SIGTERM)
        except Exception:
            pass
    else:
        try:
            process.terminate()
        except Exception:
            pass
    try:
        process.wait(timeout=2)
    except subprocess.TimeoutExpired:
        if pgid is not None:
            try:
                os.killpg(pgid, signal.SIGKILL)
            except Exception:
                pass
        else:
            try:
                process.kill()
            except Exception:
                pass
        process.wait()
    sys.exit(124)
PY
  else
    "$@"
  fi
}

file_mtime_epoch() {
  local file_path="$1"
  local mtime
  if mtime=$(stat -f '%m' "${file_path}" 2>/dev/null); then
    printf '%s\n' "${mtime}"
    return 0
  fi
  if mtime=$(stat -c '%Y' "${file_path}" 2>/dev/null); then
    printf '%s\n' "${mtime}"
    return 0
  fi
  printf '0\n'
}

list_mod_test_binaries_by_mtime_desc() {
  local deps_dir="${TARGET_DIR}/debug/deps"
  local candidate mtime
  if [[ ! -d "${deps_dir}" ]]; then
    return 1
  fi
  while IFS= read -r candidate; do
    mtime=$(file_mtime_epoch "${candidate}")
    printf '%s\t%s\n' "${mtime}" "${candidate}"
  done < <(find "${deps_dir}" -maxdepth 1 -type f -name 'mod-*' -perm -111)
}

resolve_mod_test_binary_for_filter() {
  local test_filter="$1"
  local candidate list_output expected_test_id
  expected_test_id=$(expected_test_id_for_filter "${test_filter}")

  while IFS=$'\t' read -r _mtime candidate; do
    list_output=$("${candidate}" "${test_filter}" --list 2>/dev/null || true)
    while IFS= read -r listed_line; do
      [[ "${listed_line}" == *": test" ]] || continue
      local listed_test
      listed_test="${listed_line%: test}"
      if [[ "${listed_test}" == "${expected_test_id}" ]] || [[ "${listed_test}" == *"::${test_filter}" ]]; then
        printf '%s\n' "${candidate}"
        return 0
      fi
    done < <(printf '%s\n' "${list_output}")
    if [[ -n "${expected_test_id}" ]] && printf '%s\n' "${list_output}" | rg -q -F "${expected_test_id}: test"; then
      printf '%s\n' "${candidate}"
      return 0
    fi
  done < <(list_mod_test_binaries_by_mtime_desc | sort -rn -k1,1)

  return 1
}

expected_test_id_for_filter() {
  local test_filter="$1"
  case "${test_filter}" in
    cross_dataspace_atomic_swap_is_all_or_nothing)
      printf '%s\n' "nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing"
      ;;
    runtime_nexus_registration_reports_lane_lifecycle_costs)
      printf '%s\n' "nexus::runtime_dataspace_registration_perf::runtime_nexus_registration_reports_lane_lifecycle_costs"
      ;;
    *)
      printf '%s\n' "${test_filter}"
      ;;
  esac
}

require_resolved_test_binary() {
  local test_filter="$1"
  local test_bin

  test_bin=$(resolve_mod_test_binary_for_filter "${test_filter}" || true)
  if [[ -z "${test_bin}" ]]; then
    echo "Unable to locate executable mod-* test binary under ${TARGET_DIR}/debug/deps containing test filter '${test_filter}'" >&2
    exit 2
  fi
  printf '%s\n' "${test_bin}"
}

run_cross() {
  local run_index="$1"
  local log_path="${OUTPUT_DIR}/cross_run_${run_index}.log"
  local timeout_marker="${OUTPUT_DIR}/.cross_run_${run_index}.timed_out"
  local start_ts end_ts duration_s code
  local timed_out=0
  local test_bin test_filter
  local -a command
  local soak_line soak_passed soak_total soak_fallbacks soak_retries swap_nonconverged
  test_filter="cross_dataspace_atomic_swap_is_all_or_nothing"
  build_env_vars
  if [[ "${PREFER_TEST_BINARY}" == true ]]; then
    if [[ -z "${CACHED_CROSS_TEST_BIN}" ]]; then
      CACHED_CROSS_TEST_BIN=$(require_resolved_test_binary "${test_filter}")
    fi
    test_bin="${CACHED_CROSS_TEST_BIN}"
    echo "[matrix] resolved test binary for ${test_filter}: ${test_bin}"
    command=("${test_bin}" "${test_filter}" "--nocapture" "--test-threads=${TEST_THREADS}")
  else
    command=(cargo test -p integration_tests --test mod "${test_filter}" -- --nocapture --test-threads="${TEST_THREADS}")
  fi
  start_ts=$(date +%s)
  if run_with_timeout "${CROSS_TIMEOUT_S}" "${timeout_marker}" env "${ENV_VARS[@]}" "${command[@]}" >"${log_path}" 2>&1; then
    code=0
  else
    code=$?
  fi
  if [[ -f "${timeout_marker}" ]]; then
    timed_out=1
  fi
  rm -f "${timeout_marker}"
  end_ts=$(date +%s)
  duration_s=$((end_ts - start_ts))

  soak_line=$(rg -N "\[health\]" "${log_path}" | tail -n 1 || true)
  soak_passed=$(printf "%s" "${soak_line}" | sed -nE 's/.*soak_passed=([0-9]+)\/([0-9]+).*/\1/p')
  soak_total=$(printf "%s" "${soak_line}" | sed -nE 's/.*soak_passed=([0-9]+)\/([0-9]+).*/\2/p')
  soak_fallbacks=$(printf "%s" "${soak_line}" | sed -nE 's/.*soak_fallbacks=([0-9]+).*/\1/p')
  soak_retries=$(printf "%s" "${soak_line}" | sed -nE 's/.*soak_retries=([0-9]+).*/\1/p')
  swap_nonconverged=$(printf "%s" "${soak_line}" | sed -nE 's/.*swap_nonconverged_fallbacks=([0-9]+).*/\1/p')

  printf "%s,%s,%s,%s,%s,%s,%s,%s,%s\n" \
    "${run_index}" "${code}" "${timed_out}" "${duration_s}" \
    "${soak_passed:-}" "${soak_total:-}" "${soak_fallbacks:-}" "${soak_retries:-}" "${swap_nonconverged:-}" \
    >> "${CROSS_CSV}"

  if [[ "${code}" -ne 0 ]] && [[ "${CONTINUE_ON_FAILURE}" != true ]]; then
    if [[ "${timed_out}" -eq 1 ]]; then
      echo "cross run ${run_index} timed out after ${CROSS_TIMEOUT_S}s; tail follows:" >&2
    else
      echo "cross run ${run_index} failed; tail follows:" >&2
    fi
    tail -n 120 "${log_path}" >&2
    exit "${code}"
  fi
}

run_runtime() {
  local run_index="$1"
  local log_path="${OUTPUT_DIR}/runtime_run_${run_index}.log"
  local timeout_marker="${OUTPUT_DIR}/.runtime_run_${run_index}.timed_out"
  local start_ts end_ts duration_s code
  local timed_out=0
  local test_bin test_filter
  local -a command
  local pass_rate submitter_max cluster_max note_zero_delta
  test_filter="runtime_nexus_registration_reports_lane_lifecycle_costs"
  build_env_vars
  if [[ "${PREFER_TEST_BINARY}" == true ]]; then
    if [[ -z "${CACHED_RUNTIME_TEST_BIN}" ]]; then
      CACHED_RUNTIME_TEST_BIN=$(require_resolved_test_binary "${test_filter}")
    fi
    test_bin="${CACHED_RUNTIME_TEST_BIN}"
    echo "[matrix] resolved test binary for ${test_filter}: ${test_bin}"
    command=("${test_bin}" "${test_filter}" "--nocapture" "--test-threads=${TEST_THREADS}")
  else
    command=(cargo test -p integration_tests --test mod "${test_filter}" -- --nocapture --test-threads="${TEST_THREADS}")
  fi
  start_ts=$(date +%s)
  if run_with_timeout "${RUNTIME_TIMEOUT_S}" "${timeout_marker}" env "${ENV_VARS[@]}" "${command[@]}" >"${log_path}" 2>&1; then
    code=0
  else
    code=$?
  fi
  if [[ -f "${timeout_marker}" ]]; then
    timed_out=1
  fi
  rm -f "${timeout_marker}"
  end_ts=$(date +%s)
  duration_s=$((end_ts - start_ts))

  pass_rate=$(rg -N "\[registration-perf\] iterations=" "${log_path}" | tail -n 1 | sed -nE 's/.*pass_rate=([0-9.]+)%.*/\1/p' || true)
  submitter_max=$(rg -N "submitter commit height delta min/avg/max" "${log_path}" | tail -n 1 | sed -nE 's/.*= [0-9]+\/[0-9.]+\/([0-9]+).*/\1/p' || true)
  cluster_max=$(rg -N "cluster max commit height delta min/avg/max" "${log_path}" | tail -n 1 | sed -nE 's/.*= [0-9]+\/[0-9.]+\/([0-9]+).*/\1/p' || true)
  if rg -q "expected for control-plane /v2/nexus/lifecycle state mutation" "${log_path}"; then
    note_zero_delta=1
  else
    note_zero_delta=0
  fi

  printf "%s,%s,%s,%s,%s,%s,%s,%s\n" \
    "${run_index}" "${code}" "${timed_out}" "${duration_s}" \
    "${pass_rate:-}" "${submitter_max:-}" "${cluster_max:-}" "${note_zero_delta}" \
    >> "${RUNTIME_CSV}"

  if [[ "${code}" -ne 0 ]] && [[ "${CONTINUE_ON_FAILURE}" != true ]]; then
    if [[ "${timed_out}" -eq 1 ]]; then
      echo "runtime run ${run_index} timed out after ${RUNTIME_TIMEOUT_S}s; tail follows:" >&2
    else
      echo "runtime run ${run_index} failed; tail follows:" >&2
    fi
    tail -n 120 "${log_path}" >&2
    exit "${code}"
  fi
}

cross_summary_line() {
  awk -F, '
    NR == 1 { next }
    {
      runs++
      fail += ($2 != 0)
      timed_out += $3
      duration_sum += $4
      if (duration_min == "" || $4 < duration_min) duration_min = $4
      if (duration_max == "" || $4 > duration_max) duration_max = $4
      soak_passed_sum += $5
      soak_total_sum += $6
      soak_fallback_sum += $7
      soak_retry_sum += $8
      swap_nonconverged_sum += $9
    }
    END {
      if (runs == 0) {
        print "cross_summary runs=0 fail=0 timed_out=0 duration_s[min/avg/max]=0/0/0 soak_pass_rate=0.00% soak_fallbacks=0 soak_retries=0 swap_nonconverged_fallbacks=0"
      } else {
        duration_avg = duration_sum / runs
        if (soak_total_sum > 0) {
          soak_pass_rate = (soak_passed_sum * 100.0) / soak_total_sum
        } else {
          soak_pass_rate = 0.0
        }
        printf "cross_summary runs=%d fail=%d timed_out=%d duration_s[min/avg/max]=%d/%.2f/%d soak_pass_rate=%.2f%% soak_fallbacks=%d soak_retries=%d swap_nonconverged_fallbacks=%d\n",
          runs, fail, timed_out, duration_min, duration_avg, duration_max, soak_pass_rate, soak_fallback_sum, soak_retry_sum, swap_nonconverged_sum
      }
    }
  ' "${CROSS_CSV}"
}

runtime_summary_line() {
  awk -F, '
    NR == 1 { next }
    {
      runs++
      fail += ($2 != 0)
      timed_out += $3
      duration_sum += $4
      if (duration_min == "" || $4 < duration_min) duration_min = $4
      if (duration_max == "" || $4 > duration_max) duration_max = $4
      pass_rate_sum += $5
      submitter_delta_max = ($6 > submitter_delta_max) ? $6 : submitter_delta_max
      cluster_delta_max = ($7 > cluster_delta_max) ? $7 : cluster_delta_max
      note_zero_delta_sum += $8
    }
    END {
      if (runs == 0) {
        print "runtime_summary runs=0 fail=0 timed_out=0 duration_s[min/avg/max]=0/0/0 pass_rate_avg=0.00 submitter_delta_max=0 cluster_delta_max=0 note_zero_delta_rows=0"
      } else {
        duration_avg = duration_sum / runs
        pass_rate_avg = pass_rate_sum / runs
        printf "runtime_summary runs=%d fail=%d timed_out=%d duration_s[min/avg/max]=%d/%.2f/%d pass_rate_avg=%.2f submitter_delta_max=%d cluster_delta_max=%d note_zero_delta_rows=%d\n",
          runs, fail, timed_out, duration_min, duration_avg, duration_max, pass_rate_avg, submitter_delta_max, cluster_delta_max, note_zero_delta_sum
      }
    }
  ' "${RUNTIME_CSV}"
}

write_json_summary() {
  local include_rows_flag=0
  if [[ "${SUMMARY_JSON_ROWS}" == true ]]; then
    include_rows_flag=1
  fi
  MATRIX_TARGET_DIR="${TARGET_DIR}" \
  MATRIX_OUTPUT_DIR="${OUTPUT_DIR}" \
  MATRIX_CROSS_RUNS="${CROSS_RUNS}" \
  MATRIX_RUNTIME_RUNS="${RUNTIME_RUNS}" \
  MATRIX_CROSS_SOAK_ITERATIONS="${CROSS_SOAK_ITERATIONS}" \
  MATRIX_RUNTIME_BENCH_ITERATIONS="${RUNTIME_BENCH_ITERATIONS}" \
  MATRIX_CROSS_TIMEOUT_S="${CROSS_TIMEOUT_S}" \
  MATRIX_RUNTIME_TIMEOUT_S="${RUNTIME_TIMEOUT_S}" \
  MATRIX_CARGO_JOBS="${CARGO_JOBS}" \
  MATRIX_TEST_THREADS="${TEST_THREADS}" \
  MATRIX_PREFER_TEST_BINARY="${PREFER_TEST_BINARY}" \
  MATRIX_SUMMARY_JSON_ROWS="${SUMMARY_JSON_ROWS}" \
  MATRIX_SKIP_BUILD="${SKIP_BUILD}" \
  MATRIX_SKIP_BINDINGS_SYNC="${SKIP_BINDINGS_SYNC}" \
  MATRIX_CONTINUE_ON_FAILURE="${CONTINUE_ON_FAILURE}" \
  MATRIX_RUN_CROSS="${RUN_CROSS}" \
  MATRIX_RUN_RUNTIME="${RUN_RUNTIME}" \
  python3 - "${CROSS_CSV}" "${RUNTIME_CSV}" "${SUMMARY_JSON}" "${include_rows_flag}" <<'PY'
import csv
import json
import os
import sys
from datetime import datetime, timezone

cross_csv, runtime_csv, out_json = sys.argv[1], sys.argv[2], sys.argv[3]
include_rows = sys.argv[4] == "1"

def to_int(value):
    if value is None:
        return 0
    value = str(value).strip()
    if value == "":
        return 0
    return int(float(value))

def to_float(value):
    if value is None:
        return 0.0
    value = str(value).strip()
    if value == "":
        return 0.0
    return float(value)

def load_rows(path):
    with open(path, newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))

cross_rows = load_rows(cross_csv)
runtime_rows = load_rows(runtime_csv)

def summarize_cross(rows):
    runs = len(rows)
    if runs == 0:
        return {
            "runs": 0,
            "success": 0,
            "fail": 0,
            "exit_code_counts": {},
            "timed_out": 0,
            "duration_s": {"min": 0, "avg": 0.0, "max": 0},
            "soak_pass_rate_percent": 0.0,
            "soak_fallbacks_total": 0,
            "soak_retries_total": 0,
            "swap_nonconverged_fallbacks_total": 0,
        }

    durations = [to_int(r.get("duration_s")) for r in rows]
    exit_codes = [to_int(r.get("exit")) for r in rows]
    fail = sum(1 for code in exit_codes if code != 0)
    success = runs - fail
    timed_out = sum(to_int(r.get("timed_out")) for r in rows)
    soak_passed = sum(to_int(r.get("soak_passed")) for r in rows)
    soak_total = sum(to_int(r.get("soak_total")) for r in rows)
    soak_pass_rate = (soak_passed * 100.0 / soak_total) if soak_total else 0.0
    exit_code_counts = {}
    for code in exit_codes:
        key = str(code)
        exit_code_counts[key] = exit_code_counts.get(key, 0) + 1
    return {
        "runs": runs,
        "success": success,
        "fail": fail,
        "exit_code_counts": exit_code_counts,
        "timed_out": timed_out,
        "duration_s": {
            "min": min(durations),
            "avg": sum(durations) / runs,
            "max": max(durations),
        },
        "soak_pass_rate_percent": soak_pass_rate,
        "soak_fallbacks_total": sum(to_int(r.get("soak_fallbacks")) for r in rows),
        "soak_retries_total": sum(to_int(r.get("soak_retries")) for r in rows),
        "swap_nonconverged_fallbacks_total": sum(to_int(r.get("swap_nonconverged_fallbacks")) for r in rows),
    }

def summarize_runtime(rows):
    runs = len(rows)
    if runs == 0:
        return {
            "runs": 0,
            "success": 0,
            "fail": 0,
            "exit_code_counts": {},
            "timed_out": 0,
            "duration_s": {"min": 0, "avg": 0.0, "max": 0},
            "pass_rate_avg_percent": 0.0,
            "submitter_delta_max": 0,
            "cluster_delta_max": 0,
            "note_zero_delta_rows": 0,
        }

    durations = [to_int(r.get("duration_s")) for r in rows]
    exit_codes = [to_int(r.get("exit")) for r in rows]
    fail = sum(1 for code in exit_codes if code != 0)
    success = runs - fail
    exit_code_counts = {}
    for code in exit_codes:
        key = str(code)
        exit_code_counts[key] = exit_code_counts.get(key, 0) + 1
    return {
        "runs": runs,
        "success": success,
        "fail": fail,
        "exit_code_counts": exit_code_counts,
        "timed_out": sum(to_int(r.get("timed_out")) for r in rows),
        "duration_s": {
            "min": min(durations),
            "avg": sum(durations) / runs,
            "max": max(durations),
        },
        "pass_rate_avg_percent": sum(to_float(r.get("pass_rate")) for r in rows) / runs,
        "submitter_delta_max": max(to_int(r.get("submitter_delta_max")) for r in rows),
        "cluster_delta_max": max(to_int(r.get("cluster_delta_max")) for r in rows),
        "note_zero_delta_rows": sum(to_int(r.get("note_zero_delta")) for r in rows),
    }

payload = {
    "generated_at_utc": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
    "config": {
        "target_dir": os.environ.get("MATRIX_TARGET_DIR", ""),
        "output_dir": os.environ.get("MATRIX_OUTPUT_DIR", ""),
        "cross_runs": to_int(os.environ.get("MATRIX_CROSS_RUNS", "0")),
        "runtime_runs": to_int(os.environ.get("MATRIX_RUNTIME_RUNS", "0")),
        "cross_soak_iterations": to_int(os.environ.get("MATRIX_CROSS_SOAK_ITERATIONS", "0")),
        "runtime_bench_iterations": to_int(os.environ.get("MATRIX_RUNTIME_BENCH_ITERATIONS", "0")),
        "cross_timeout_s": to_int(os.environ.get("MATRIX_CROSS_TIMEOUT_S", "0")),
        "runtime_timeout_s": to_int(os.environ.get("MATRIX_RUNTIME_TIMEOUT_S", "0")),
        "cargo_jobs": to_int(os.environ.get("MATRIX_CARGO_JOBS", "0")),
        "test_threads": to_int(os.environ.get("MATRIX_TEST_THREADS", "0")),
        "prefer_test_binary": os.environ.get("MATRIX_PREFER_TEST_BINARY", "") == "true",
        "summary_json_rows": os.environ.get("MATRIX_SUMMARY_JSON_ROWS", "") == "true",
        "skip_build": os.environ.get("MATRIX_SKIP_BUILD", "") == "true",
        "skip_bindings_sync": os.environ.get("MATRIX_SKIP_BINDINGS_SYNC", "") == "true",
        "continue_on_failure": os.environ.get("MATRIX_CONTINUE_ON_FAILURE", "") == "true",
        "run_cross": os.environ.get("MATRIX_RUN_CROSS", "") == "true",
        "run_runtime": os.environ.get("MATRIX_RUN_RUNTIME", "") == "true",
    },
    "cross": summarize_cross(cross_rows),
    "runtime": summarize_runtime(runtime_rows),
}

if include_rows:
    def typed_cross_row(row):
        return {
            "run": to_int(row.get("run")),
            "exit": to_int(row.get("exit")),
            "timed_out": to_int(row.get("timed_out")),
            "duration_s": to_int(row.get("duration_s")),
            "soak_passed": to_int(row.get("soak_passed")),
            "soak_total": to_int(row.get("soak_total")),
            "soak_fallbacks": to_int(row.get("soak_fallbacks")),
            "soak_retries": to_int(row.get("soak_retries")),
            "swap_nonconverged_fallbacks": to_int(row.get("swap_nonconverged_fallbacks")),
        }

    def typed_runtime_row(row):
        return {
            "run": to_int(row.get("run")),
            "exit": to_int(row.get("exit")),
            "timed_out": to_int(row.get("timed_out")),
            "duration_s": to_int(row.get("duration_s")),
            "pass_rate": to_float(row.get("pass_rate")),
            "submitter_delta_max": to_int(row.get("submitter_delta_max")),
            "cluster_delta_max": to_int(row.get("cluster_delta_max")),
            "note_zero_delta": to_int(row.get("note_zero_delta")),
        }

    payload["rows"] = {
        "cross": [typed_cross_row(row) for row in cross_rows],
        "runtime": [typed_runtime_row(row) for row in runtime_rows],
    }

with open(out_json, "w", encoding="utf-8") as f:
    json.dump(payload, f, indent=2, sort_keys=True)
    f.write("\n")
PY
}

FINALIZED_OUTPUTS=false
finalize_outputs() {
  if [[ "${FINALIZED_OUTPUTS}" == true ]]; then
    return 0
  fi
  FINALIZED_OUTPUTS=true
  set +e
  echo "CROSS_CSV=${CROSS_CSV}"
  echo "RUNTIME_CSV=${RUNTIME_CSV}"
  cat "${CROSS_CSV}"
  cat "${RUNTIME_CSV}"
  cross_summary_line | tee -a "${SUMMARY_TXT}"
  runtime_summary_line | tee -a "${SUMMARY_TXT}"
  write_json_summary
  echo "SUMMARY_TXT=${SUMMARY_TXT}"
  echo "SUMMARY_JSON=${SUMMARY_JSON}"
  return 0
}

echo "Output directory: ${OUTPUT_DIR}"
echo "Cross runs enabled: ${RUN_CROSS}, runs: ${CROSS_RUNS}, soak iterations: ${CROSS_SOAK_ITERATIONS}, timeout: ${CROSS_TIMEOUT_S}s"
echo "Runtime runs enabled: ${RUN_RUNTIME}, runs: ${RUNTIME_RUNS}, bench iterations: ${RUNTIME_BENCH_ITERATIONS}, timeout: ${RUNTIME_TIMEOUT_S}s"
echo "Target dir: ${TARGET_DIR}"
echo "Cargo jobs: ${CARGO_JOBS}"
echo "Test threads: ${TEST_THREADS}"
echo "Prefer test binary: ${PREFER_TEST_BINARY}"
echo "Summary JSON include rows: ${SUMMARY_JSON_ROWS}"
echo "Skip build: ${SKIP_BUILD}"
echo "Skip bindings sync: ${SKIP_BINDINGS_SYNC}"
echo "Continue on failure: ${CONTINUE_ON_FAILURE}"
trap finalize_outputs EXIT

if [[ "${RUN_CROSS}" == true ]]; then
  for i in $(seq 1 "${CROSS_RUNS}"); do
    echo "[matrix] cross run ${i}/${CROSS_RUNS}"
    run_cross "${i}"
  done
else
  echo "[matrix] cross runs skipped"
fi

if [[ "${RUN_RUNTIME}" == true ]]; then
  for i in $(seq 1 "${RUNTIME_RUNS}"); do
    echo "[matrix] runtime run ${i}/${RUNTIME_RUNS}"
    run_runtime "${i}"
  done
else
  echo "[matrix] runtime runs skipped"
fi
