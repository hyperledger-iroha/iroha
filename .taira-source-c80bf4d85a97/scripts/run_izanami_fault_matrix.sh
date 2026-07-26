#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/run_izanami_fault_matrix.sh [--out DIR] [--izanami-cmd CMD] [-- EXTRA_IZANAMI_ARGS...]

Runs one-fault-at-a-time Izanami sweeps and records a per-scenario summary for:
  crash-only
  wipe-only
  spam-only
  network-only
  cpu-only
  disk-only
  full-mix

Examples:
  scripts/run_izanami_fault_matrix.sh
  scripts/run_izanami_fault_matrix.sh --out dist/izanami-fault-matrix -- --duration 300s --tps 25
  scripts/run_izanami_fault_matrix.sh --izanami-cmd "cargo run -p izanami --bin izanami --" -- --seed 7
EOF
}

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out_dir="${root_dir}/dist/izanami-fault-matrix-$(date +%Y%m%d-%H%M%S)"
izanami_cmd='cargo run -p izanami --bin izanami --'
declare -a extra_args=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --out)
      out_dir="$2"
      shift 2
      ;;
    --izanami-cmd)
      izanami_cmd="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    --)
      shift
      extra_args=("$@")
      break
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

mkdir -p "$out_dir"
summary_tsv="${out_dir}/summary.tsv"
summary_md="${out_dir}/summary.md"

printf 'scenario\texit_code\tstatus\tlog\n' >"$summary_tsv"
cat >"$summary_md" <<'EOF'
# Izanami Fault Matrix

| Scenario | Exit Code | Status | Log |
| --- | ---: | --- | --- |
EOF

default_args=(
  --allow-net
  --peers 4
  --faulty 1
  --duration 120s
  --tps 15
  --submitters 1
)

disable_all_faults=(
  --fault-enable-crash-restart=false
  --fault-enable-wipe-storage=false
  --fault-enable-spam-invalid-transactions=false
  --fault-enable-network-latency=false
  --fault-enable-network-partition=false
  --fault-enable-cpu-stress=false
  --fault-enable-disk-saturation=false
)

run_scenario() {
  local scenario="$1"
  shift
  local -a scenario_flags=("$@")
  local log_path="${out_dir}/${scenario}.log"
  local status_label="ok"
  local exit_code=0

  echo "==> ${scenario}" | tee "${log_path}"
  echo "command: ${izanami_cmd} ${default_args[*]} ${scenario_flags[*]} ${extra_args[*]}" \
    | tee -a "${log_path}"

  pushd "$root_dir" >/dev/null
  set +e
  # shellcheck disable=SC2206
  local -a cmd_parts=(${izanami_cmd})
  "${cmd_parts[@]}" "${default_args[@]}" "${scenario_flags[@]}" "${extra_args[@]}" \
    2>&1 | tee -a "${log_path}"
  exit_code=${PIPESTATUS[0]}
  set -e
  popd >/dev/null

  if [[ $exit_code -ne 0 ]]; then
    status_label="failed"
  fi

  printf '%s\t%s\t%s\t%s\n' "${scenario}" "${exit_code}" "${status_label}" "${log_path}" >>"$summary_tsv"
  printf '| %s | %s | %s | `%s` |\n' "${scenario}" "${exit_code}" "${status_label}" "${log_path}" >>"$summary_md"

  local summary_line
  summary_line="$(rg 'izanami::summary' "${log_path}" | tail -n 1 || true)"
  if [[ -n "${summary_line}" ]]; then
    {
      echo
      echo "## ${scenario}"
      echo
      echo '```text'
      echo "${summary_line}"
      echo '```'
    } >>"$summary_md"
  fi
}

run_scenario "crash-only" \
  "${disable_all_faults[@]}" \
  --fault-enable-crash-restart=true

run_scenario "wipe-only" \
  "${disable_all_faults[@]}" \
  --fault-enable-wipe-storage=true

run_scenario "spam-only" \
  "${disable_all_faults[@]}" \
  --fault-enable-spam-invalid-transactions=true

run_scenario "network-only" \
  "${disable_all_faults[@]}" \
  --fault-enable-network-latency=true \
  --fault-enable-network-partition=true

run_scenario "cpu-only" \
  "${disable_all_faults[@]}" \
  --fault-enable-cpu-stress=true

run_scenario "disk-only" \
  "${disable_all_faults[@]}" \
  --fault-enable-disk-saturation=true

run_scenario "full-mix"

cat <<EOF
fault matrix complete
summary: ${summary_md}
table:   ${summary_tsv}
logs:    ${out_dir}
EOF
