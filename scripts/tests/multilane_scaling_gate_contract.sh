#!/usr/bin/env bash
# Fast shell-surface contract for the fixed V1 G-SCALE runner.
#
# This test performs no benchmark and invokes no Cargo command.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SHELL_RUNNER="${REPO_ROOT}/scripts/nexus/run_multilane_scaling_gate.sh"
PYTHON_RUNNER="${REPO_ROOT}/scripts/nexus/run_multilane_scaling_gate.py"
PYTHON_BIN="${PYTHON_BIN:-python3}"

bash -n "${SHELL_RUNNER}"
runner_help="$(bash "${SHELL_RUNNER}" --help)"
python_help="$("${PYTHON_BIN}" "${PYTHON_RUNNER}" --help)"
if [[ "${runner_help}" != "${python_help}" ]]; then
  echo "shell runner does not expose the canonical Python entrypoint" >&2
  exit 1
fi

for required in \
  "--launch-input-fd" \
  "--launch-input-sha256" \
  "--seed-fd"; do
  if ! grep -Fq -- "${required}" <<<"${runner_help}"; then
    echo "runner help omits required option: ${required}" >&2
    exit 1
  fi
done

for forbidden in \
  "--trial-command" \
  "--seed-namespace" \
  "--scaling-evidence-manifest" \
  "--report" \
  "--min-throughput-ratio" \
  "--max-p95-latency-ratio"; do
  if grep -Fq -- "${forbidden}" <<<"${runner_help}"; then
    echo "runner exposes forbidden gate override: ${forbidden}" >&2
    exit 1
  fi
done

if "${PYTHON_BIN}" "${PYTHON_RUNNER}" --report /tmp/retired-scaling-report.json >/dev/null 2>&1; then
  echo "runner accepted retired independent-report arguments" >&2
  exit 1
fi

"${PYTHON_BIN}" -m py_compile "${PYTHON_RUNNER}"
echo "[g-scale-contract] shell surface passed"
