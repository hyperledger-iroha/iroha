#!/usr/bin/env bash
# Run the Sumeragi V2 G-SCALE evidence orchestrator.
#
# Prerequisites and all required paths are documented by --help. PYTHON_BIN may
# select a Python 3.11+ interpreter. The runner creates a new artifact directory
# and never overwrites an existing evidence bundle.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PYTHON_BIN="${PYTHON_BIN:-python3}"

exec "${PYTHON_BIN}" "${SCRIPT_DIR}/run_multilane_scaling_gate.py" "$@"
