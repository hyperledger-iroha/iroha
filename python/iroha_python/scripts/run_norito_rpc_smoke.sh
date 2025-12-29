#!/usr/bin/env bash
# Run the Norito RPC smoke tests (unit-level parity checks).
#
# Usage:
#   python/iroha_python/scripts/run_norito_rpc_smoke.sh
#   PYTHON_BIN=/path/to/python python/iroha_python/scripts/run_norito_rpc_smoke.sh
#
# The script expects `pytest` to be available in the selected interpreter.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
PYTHON_BIN="${PYTHON_BIN:-python3}"

if ! "${PYTHON_BIN}" -m pytest --version >/dev/null 2>&1; then
    echo "pytest is required. Install dev extras: 'pip install iroha-python[dev]'." >&2
    exit 1
fi

cd "${REPO_ROOT}"

echo "+ ${PYTHON_BIN} -m pytest python/iroha_python/tests/test_norito_rpc.py -q"
"${PYTHON_BIN}" -m pytest python/iroha_python/tests/test_norito_rpc.py -q

echo "[ok] Norito RPC smoke tests passed"
