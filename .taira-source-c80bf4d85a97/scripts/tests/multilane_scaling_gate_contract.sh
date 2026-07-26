#!/usr/bin/env bash
# Fast shell-surface contract for the Sumeragi V2 G-SCALE runner.
#
# This test performs no benchmark and invokes no Cargo command.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SHELL_RUNNER="${REPO_ROOT}/scripts/nexus/run_multilane_scaling_gate.sh"
PYTHON_RUNNER="${REPO_ROOT}/scripts/nexus/run_multilane_scaling_gate.py"
VALIDATOR="${REPO_ROOT}/scripts/nexus/validate_multilane_scaling_evidence.py"
PYTHON_BIN="${PYTHON_BIN:-python3}"

bash -n "${SHELL_RUNNER}"
runner_help="$(bash "${SHELL_RUNNER}" --help)"
validator_help="$("${PYTHON_BIN}" "${VALIDATOR}" --help)"

for required in \
  "--trial-command" \
  "--seed-namespace" \
  "--offered-load-tps" \
  "--max-queue-depth" \
  "--max-index-entries" \
  "--max-memory-bytes" \
  "--max-disk-bytes"; do
  if ! grep -Fq -- "${required}" <<<"${runner_help}"; then
    echo "runner help omits required option: ${required}" >&2
    exit 1
  fi
done

for forbidden in \
  "--runs" \
  "--skip" \
  "--continue-on-failure" \
  "--min-throughput-ratio" \
  "--max-p95-latency-ratio"; do
  if grep -Fq -- "${forbidden}" <<<"${runner_help}"; then
    echo "runner exposes forbidden gate override: ${forbidden}" >&2
    exit 1
  fi
done

if ! grep -Fq -- "--report" <<<"${validator_help}"; then
  echo "validator help omits --report" >&2
  exit 1
fi

"${PYTHON_BIN}" -m py_compile "${PYTHON_RUNNER}" "${VALIDATOR}"
echo "[g-scale-contract] shell surface passed"
