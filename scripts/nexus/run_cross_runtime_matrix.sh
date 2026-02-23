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

while [[ $# -gt 0 ]]; do
  case "$1" in
    --cross-runs)
      CROSS_RUNS="$2"
      shift 2
      ;;
    --runtime-runs)
      RUNTIME_RUNS="$2"
      shift 2
      ;;
    --cross-soak-iterations)
      CROSS_SOAK_ITERATIONS="$2"
      shift 2
      ;;
    --runtime-bench-iterations)
      RUNTIME_BENCH_ITERATIONS="$2"
      shift 2
      ;;
    --cross-timeout-s)
      CROSS_TIMEOUT_S="$2"
      shift 2
      ;;
    --runtime-timeout-s)
      RUNTIME_TIMEOUT_S="$2"
      shift 2
      ;;
    --target-dir)
      TARGET_DIR="$2"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --cargo-jobs)
      CARGO_JOBS="$2"
      shift 2
      ;;
    --test-threads)
      TEST_THREADS="$2"
      shift 2
      ;;
    --prefer-test-binary)
      PREFER_TEST_BINARY=true
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

mkdir -p "$OUTPUT_DIR"

CROSS_CSV="${OUTPUT_DIR}/cross_dataspace_matrix.csv"
RUNTIME_CSV="${OUTPUT_DIR}/runtime_registration_matrix.csv"
SUMMARY_TXT="${OUTPUT_DIR}/matrix_summary.txt"
printf "run,exit,timed_out,duration_s,soak_passed,soak_total,soak_fallbacks,soak_retries,swap_nonconverged_fallbacks\n" > "$CROSS_CSV"
printf "run,exit,timed_out,duration_s,pass_rate,submitter_delta_max,cluster_delta_max,note_zero_delta\n" > "$RUNTIME_CSV"
: > "$SUMMARY_TXT"

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
  shift
  if [[ "$timeout_s" -gt 0 ]]; then
    python3 - "$timeout_s" "$@" <<'PY'
import subprocess
import sys

timeout_s = int(sys.argv[1])
command = sys.argv[2:]
try:
    completed = subprocess.run(command, check=False, timeout=timeout_s)
except subprocess.TimeoutExpired:
    sys.exit(124)
sys.exit(completed.returncode)
PY
  else
    "$@"
  fi
}

resolve_mod_test_binary() {
  local deps_dir="${TARGET_DIR}/debug/deps"
  local best_path=""
  local best_mtime=0
  local candidate mtime
  if [[ ! -d "${deps_dir}" ]]; then
    return 1
  fi
  while IFS= read -r candidate; do
    mtime=$(stat -f '%m' "${candidate}" 2>/dev/null || printf '0')
    if [[ -z "${best_path}" ]] || [[ "${mtime}" -gt "${best_mtime}" ]]; then
      best_path="${candidate}"
      best_mtime="${mtime}"
    fi
  done < <(find "${deps_dir}" -maxdepth 1 -type f -name 'mod-*' -perm -111)
  if [[ -z "${best_path}" ]]; then
    return 1
  fi
  printf '%s\n' "${best_path}"
}

run_cross() {
  local run_index="$1"
  local log_path="${OUTPUT_DIR}/cross_run_${run_index}.log"
  local start_ts end_ts duration_s code
  local timed_out=0
  local test_bin
  local -a command
  local soak_line soak_passed soak_total soak_fallbacks soak_retries swap_nonconverged
  build_env_vars
  if [[ "${PREFER_TEST_BINARY}" == true ]]; then
    test_bin=$(resolve_mod_test_binary || true)
    if [[ -z "${test_bin}" ]]; then
      echo "Unable to locate executable mod-* test binary under ${TARGET_DIR}/debug/deps" >&2
      exit 2
    fi
    command=("${test_bin}" "cross_dataspace_atomic_swap_is_all_or_nothing" "--nocapture" "--test-threads=${TEST_THREADS}")
  else
    command=(cargo test -p integration_tests --test mod cross_dataspace_atomic_swap_is_all_or_nothing -- --nocapture --test-threads="${TEST_THREADS}")
  fi
  start_ts=$(date +%s)
  if run_with_timeout "${CROSS_TIMEOUT_S}" env "${ENV_VARS[@]}" "${command[@]}" >"${log_path}" 2>&1; then
    code=0
  else
    code=$?
  fi
  if [[ "${code}" -eq 124 ]]; then
    timed_out=1
  fi
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
  local start_ts end_ts duration_s code
  local timed_out=0
  local test_bin
  local -a command
  local pass_rate submitter_max cluster_max note_zero_delta
  build_env_vars
  if [[ "${PREFER_TEST_BINARY}" == true ]]; then
    test_bin=$(resolve_mod_test_binary || true)
    if [[ -z "${test_bin}" ]]; then
      echo "Unable to locate executable mod-* test binary under ${TARGET_DIR}/debug/deps" >&2
      exit 2
    fi
    command=("${test_bin}" "runtime_nexus_registration_reports_lane_lifecycle_costs" "--nocapture" "--test-threads=${TEST_THREADS}")
  else
    command=(cargo test -p integration_tests --test mod runtime_nexus_registration_reports_lane_lifecycle_costs -- --nocapture --test-threads="${TEST_THREADS}")
  fi
  start_ts=$(date +%s)
  if run_with_timeout "${RUNTIME_TIMEOUT_S}" env "${ENV_VARS[@]}" "${command[@]}" >"${log_path}" 2>&1; then
    code=0
  else
    code=$?
  fi
  if [[ "${code}" -eq 124 ]]; then
    timed_out=1
  fi
  end_ts=$(date +%s)
  duration_s=$((end_ts - start_ts))

  pass_rate=$(rg -N "\[registration-perf\] iterations=" "${log_path}" | tail -n 1 | sed -nE 's/.*pass_rate=([0-9.]+)%.*/\1/p' || true)
  submitter_max=$(rg -N "submitter commit height delta min/avg/max" "${log_path}" | tail -n 1 | sed -nE 's/.*= [0-9]+\/[0-9.]+\/([0-9]+).*/\1/p' || true)
  cluster_max=$(rg -N "cluster max commit height delta min/avg/max" "${log_path}" | tail -n 1 | sed -nE 's/.*= [0-9]+\/[0-9.]+\/([0-9]+).*/\1/p' || true)
  if rg -q "expected for control-plane /v1/nexus/lifecycle state mutation" "${log_path}"; then
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

echo "Output directory: ${OUTPUT_DIR}"
echo "Cross runs enabled: ${RUN_CROSS}, runs: ${CROSS_RUNS}, soak iterations: ${CROSS_SOAK_ITERATIONS}, timeout: ${CROSS_TIMEOUT_S}s"
echo "Runtime runs enabled: ${RUN_RUNTIME}, runs: ${RUNTIME_RUNS}, bench iterations: ${RUNTIME_BENCH_ITERATIONS}, timeout: ${RUNTIME_TIMEOUT_S}s"
echo "Target dir: ${TARGET_DIR}"
echo "Cargo jobs: ${CARGO_JOBS}"
echo "Test threads: ${TEST_THREADS}"
echo "Prefer test binary: ${PREFER_TEST_BINARY}"
echo "Skip build: ${SKIP_BUILD}"
echo "Skip bindings sync: ${SKIP_BINDINGS_SYNC}"
echo "Continue on failure: ${CONTINUE_ON_FAILURE}"

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

echo "CROSS_CSV=${CROSS_CSV}"
echo "RUNTIME_CSV=${RUNTIME_CSV}"
cat "${CROSS_CSV}"
cat "${RUNTIME_CSV}"
cross_summary_line | tee -a "${SUMMARY_TXT}"
runtime_summary_line | tee -a "${SUMMARY_TXT}"
echo "SUMMARY_TXT=${SUMMARY_TXT}"
