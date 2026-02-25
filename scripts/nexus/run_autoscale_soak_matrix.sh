#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: run_autoscale_soak_matrix.sh [OPTIONS]

Run the Nexus autoscale ignored soak test in a deterministic seed matrix and
aggregate per-run artifacts.

Options:
  --runs <N>                 Number of runs (default: 3)
  --seeds <CSV>              Comma-separated seeds (default: generated from --seed-prefix)
  --seed-prefix <VALUE>      Prefix used when generating seeds (default: autoscale-seed)
  --output-dir <PATH>        Output directory (default: /tmp/iroha_autoscale_soak_matrix)
  --target-dir <PATH>        CARGO_TARGET_DIR (default: /tmp/iroha_target_autoscale_soak_matrix)
  --cargo-jobs <N>           CARGO_BUILD_JOBS (default: 1)
  --test-threads <N>         --test-threads for cargo test (default: 1)
  --force-fail-cycle <N>     Set IROHA_AUTOSCALE_SOAK_FORCE_FAIL_CYCLE for all runs (default: 0)
  --no-skip-bindings-sync    Do not set NORITO_SKIP_BINDINGS_SYNC=1
  --continue-on-failure      Continue matrix runs even if a run fails
  -h, --help                 Show this help

Outputs:
  <output-dir>/run_<N>/artifacts/autoscale_soak_summary.json
  <output-dir>/run_<N>/artifacts/autoscale_soak_events.jsonl
  <output-dir>/run_<N>/cargo.log
  <output-dir>/autoscale_soak_runs.csv
  <output-dir>/autoscale_soak_aggregate.json
  <output-dir>/autoscale_soak_aggregate.csv
EOF
}

require_option_value() {
  local flag="$1"
  local value="${2-}"
  if [[ -z "$value" ]] || [[ "$value" == --* ]]; then
    echo "Missing value for ${flag}" >&2
    exit 2
  fi
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

require_command() {
  local cmd="$1"
  if ! command -v "${cmd}" >/dev/null 2>&1; then
    echo "Required command not found: ${cmd}" >&2
    exit 2
  fi
}

RUNS=3
SEEDS_CSV=""
SEED_PREFIX="autoscale-seed"
OUTPUT_DIR="/tmp/iroha_autoscale_soak_matrix"
TARGET_DIR="/tmp/iroha_target_autoscale_soak_matrix"
CARGO_JOBS=1
TEST_THREADS=1
FORCE_FAIL_CYCLE=0
SKIP_BINDINGS_SYNC=true
CONTINUE_ON_FAILURE=false
TEST_FILTER="nexus::autoscale_localnet::nexus_autoscale_soak_expand_contract_cycles_in_localnet"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --runs)
      require_option_value "--runs" "${2-}"
      RUNS="$2"
      shift 2
      ;;
    --seeds)
      require_option_value "--seeds" "${2-}"
      SEEDS_CSV="$2"
      shift 2
      ;;
    --seed-prefix)
      require_option_value "--seed-prefix" "${2-}"
      SEED_PREFIX="$2"
      shift 2
      ;;
    --output-dir)
      require_option_value "--output-dir" "${2-}"
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --target-dir)
      require_option_value "--target-dir" "${2-}"
      TARGET_DIR="$2"
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
    --force-fail-cycle)
      require_option_value "--force-fail-cycle" "${2-}"
      FORCE_FAIL_CYCLE="$2"
      shift 2
      ;;
    --no-skip-bindings-sync)
      SKIP_BINDINGS_SYNC=false
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

require_command "python3"
require_positive_int "--runs" "$RUNS"
require_positive_int "--cargo-jobs" "$CARGO_JOBS"
require_positive_int "--test-threads" "$TEST_THREADS"
require_nonnegative_int "--force-fail-cycle" "$FORCE_FAIL_CYCLE"

mkdir -p "$OUTPUT_DIR"

declare -a SEEDS
if [[ -n "$SEEDS_CSV" ]]; then
  IFS=',' read -r -a SEEDS <<< "$SEEDS_CSV"
  if [[ "${#SEEDS[@]}" -lt "$RUNS" ]]; then
    echo "Expected at least ${RUNS} seeds but got ${#SEEDS[@]}" >&2
    exit 2
  fi
else
  for run in $(seq 1 "$RUNS"); do
    SEEDS+=("${SEED_PREFIX}-${run}")
  done
fi

RUNS_CSV="${OUTPUT_DIR}/autoscale_soak_runs.csv"
AGGREGATE_JSON="${OUTPUT_DIR}/autoscale_soak_aggregate.json"
AGGREGATE_CSV="${OUTPUT_DIR}/autoscale_soak_aggregate.csv"

printf "run,seed,exit_code,summary_path,events_path,log_path\n" > "$RUNS_CSV"

had_failure=false
for run in $(seq 1 "$RUNS"); do
  seed="${SEEDS[$((run-1))]}"
  run_dir="${OUTPUT_DIR}/run_${run}"
  artifacts_dir="${run_dir}/artifacts"
  summary_path="${artifacts_dir}/autoscale_soak_summary.json"
  events_path="${artifacts_dir}/autoscale_soak_events.jsonl"
  log_path="${run_dir}/cargo.log"
  mkdir -p "$artifacts_dir"

  env_vars=(
    "CARGO_TARGET_DIR=${TARGET_DIR}"
    "CARGO_BUILD_JOBS=${CARGO_JOBS}"
    "IROHA_AUTOSCALE_SOAK_SEED=${seed}"
    "IROHA_AUTOSCALE_SOAK_ARTIFACT_DIR=${artifacts_dir}"
  )
  if [[ "$SKIP_BINDINGS_SYNC" == true ]]; then
    env_vars+=("NORITO_SKIP_BINDINGS_SYNC=1")
  fi
  if [[ "$FORCE_FAIL_CYCLE" -gt 0 ]]; then
    env_vars+=("IROHA_AUTOSCALE_SOAK_FORCE_FAIL_CYCLE=${FORCE_FAIL_CYCLE}")
  fi

  echo "[autoscale-soak-matrix] run ${run}/${RUNS} seed=${seed}"
  set +e
  env "${env_vars[@]}" \
    cargo test -p integration_tests --test mod "$TEST_FILTER" -- --ignored --nocapture --test-threads="$TEST_THREADS" \
    >"$log_path" 2>&1
  exit_code=$?
  set -e

  if [[ ! -f "$summary_path" ]] || [[ ! -f "$events_path" ]]; then
    echo "[autoscale-soak-matrix] missing artifacts for run ${run}: ${summary_path} ${events_path}" >&2
    if [[ "$exit_code" -eq 0 ]]; then
      exit_code=3
    fi
  fi
  if [[ "$exit_code" -ne 0 ]]; then
    had_failure=true
  fi

  printf "%s,%s,%s,%s,%s,%s\n" \
    "$run" "$seed" "$exit_code" "$summary_path" "$events_path" "$log_path" \
    >> "$RUNS_CSV"

  if [[ "$exit_code" -ne 0 ]] && [[ "$CONTINUE_ON_FAILURE" != true ]]; then
    break
  fi
done

python3 - "$RUNS_CSV" "$AGGREGATE_JSON" "$AGGREGATE_CSV" <<'PY'
import csv
import json
import math
import pathlib
import sys
from collections import Counter

runs_csv = pathlib.Path(sys.argv[1])
aggregate_json = pathlib.Path(sys.argv[2])
aggregate_csv = pathlib.Path(sys.argv[3])

rows = []
with runs_csv.open("r", encoding="utf-8") as f:
    reader = csv.DictReader(f)
    rows = list(reader)

retry_distribution = Counter()
cycle_count_distribution = Counter()
max_attempt_by_run = []
expansion_samples = []
contraction_samples = []
pass_count = 0
fail_count = 0

for row in rows:
    run = int(row["run"])
    seed = row["seed"]
    exit_code = int(row["exit_code"])
    summary_path = pathlib.Path(row["summary_path"])
    events_path = pathlib.Path(row["events_path"])
    summary = {}

    if summary_path.is_file():
        summary = json.loads(summary_path.read_text(encoding="utf-8"))

    final_result = str(summary.get("final_result", "missing"))
    if final_result == "pass" and exit_code == 0:
        pass_count += 1
    else:
        fail_count += 1

    retries_used_total = int(summary.get("retries_used_total", 0) or 0)
    retry_distribution[str(retries_used_total)] += 1
    cycles_completed = int(summary.get("cycles_completed", 0) or 0)
    cycle_count_distribution[str(cycles_completed)] += 1
    max_attempt = int(summary.get("max_attempt_used_in_any_cycle", 0) or 0)
    max_attempt_by_run.append(
        {
            "run": run,
            "seed": seed,
            "max_attempt_used_in_any_cycle": max_attempt,
            "final_result": final_result,
            "exit_code": exit_code,
        }
    )

    if events_path.is_file():
        for line in events_path.read_text(encoding="utf-8").splitlines():
            if not line.strip():
                continue
            event = json.loads(line)
            event_type = event.get("event_type")
            if event_type == "expansion_result":
                value = event.get("expansion_time_s")
                if isinstance(value, (int, float)):
                    expansion_samples.append(float(value))
            elif event_type == "contraction_result":
                value = event.get("contraction_time_s")
                if isinstance(value, (int, float)):
                    contraction_samples.append(float(value))


def quantile(values, q):
    if not values:
        return 0.0
    sorted_values = sorted(values)
    rank = max(0, math.ceil(q * len(sorted_values)) - 1)
    return sorted_values[rank]


aggregate = {
    "runs_total": len(rows),
    "pass_count": pass_count,
    "fail_count": fail_count,
    "retry_distribution": dict(sorted(retry_distribution.items(), key=lambda item: int(item[0]))),
    "max_attempt_by_run": max_attempt_by_run,
    "cycle_count_distribution": dict(
        sorted(cycle_count_distribution.items(), key=lambda item: int(item[0]))
    ),
    "expansion_time_s_p50": quantile(expansion_samples, 0.5),
    "expansion_time_s_p95": quantile(expansion_samples, 0.95),
    "contraction_time_s_p50": quantile(contraction_samples, 0.5),
    "contraction_time_s_p95": quantile(contraction_samples, 0.95),
}

aggregate_json.write_text(json.dumps(aggregate, indent=2, sort_keys=True) + "\n", encoding="utf-8")

with aggregate_csv.open("w", encoding="utf-8", newline="") as f:
    writer = csv.writer(f)
    writer.writerow(["metric", "value"])
    writer.writerow(["runs_total", aggregate["runs_total"]])
    writer.writerow(["pass_count", aggregate["pass_count"]])
    writer.writerow(["fail_count", aggregate["fail_count"]])
    writer.writerow(["expansion_time_s_p50", aggregate["expansion_time_s_p50"]])
    writer.writerow(["expansion_time_s_p95", aggregate["expansion_time_s_p95"]])
    writer.writerow(["contraction_time_s_p50", aggregate["contraction_time_s_p50"]])
    writer.writerow(["contraction_time_s_p95", aggregate["contraction_time_s_p95"]])
    writer.writerow(["retry_distribution", json.dumps(aggregate["retry_distribution"], sort_keys=True)])
    writer.writerow(
        ["cycle_count_distribution", json.dumps(aggregate["cycle_count_distribution"], sort_keys=True)]
    )

print(f"[autoscale-soak-matrix] aggregate-json={aggregate_json}")
print(f"[autoscale-soak-matrix] aggregate-csv={aggregate_csv}")
PY

echo "[autoscale-soak-matrix] runs-csv=${RUNS_CSV}"
echo "[autoscale-soak-matrix] aggregate-json=${AGGREGATE_JSON}"
echo "[autoscale-soak-matrix] aggregate-csv=${AGGREGATE_CSV}"

if [[ "$had_failure" == true ]]; then
  exit 1
fi
