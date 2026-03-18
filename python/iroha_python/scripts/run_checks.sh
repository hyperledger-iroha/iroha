#!/usr/bin/env bash
# Run linting, type checks, and tests for the Python SDK.
#
# Usage:
#   ./python/iroha_python/scripts/run_checks.sh
#
# The script expects the `iroha-python[dev]` extras to be installed so that
# `ruff`, `mypy`, and `pytest` are available. Set SKIP_LINT=1, SKIP_TESTS=1, or
# SKIP_FIXTURES=1 to skip individual phases when iterating locally.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

SKIP_LINT="${SKIP_LINT:-0}"
SKIP_TESTS="${SKIP_TESTS:-0}"
SKIP_FIXTURES="${SKIP_FIXTURES:-0}"
PYTHON_BIN="${PYTHON_BIN:-python3}"
IROHA_PYTHON_ROOT="python/iroha_python"
IROHA_PYTHON_SRC="${IROHA_PYTHON_ROOT}/src/iroha_python"
IROHA_PYTHON_TESTS="python/iroha_python/tests"
TORII_CLIENT_SRC="python/iroha_torii_client"
TORII_CLIENT_TESTS="python/iroha_torii_client/tests"
IROHA_PYTHON_RUFF_CFG="${IROHA_PYTHON_ROOT}/pyproject.toml"
TORII_RUFF_CFG="${TORII_CLIENT_SRC}/pyproject.toml"

run() {
    echo "+ $*"
    "$@"
}

cd "${REPO_ROOT}"

if [[ "${SKIP_LINT}" != "1" ]]; then
    if ! "${PYTHON_BIN}" -m ruff --version >/dev/null 2>&1; then
        echo "ruff is not installed. Install with 'pip install iroha-python[dev]'." >&2
        exit 1
    fi
    (
        cd "${REPO_ROOT}/${IROHA_PYTHON_ROOT}"
        run "${PYTHON_BIN}" -m ruff check "src/iroha_python" "tests"
    )
    (
        cd "${REPO_ROOT}/${TORII_CLIENT_SRC}"
        run "${PYTHON_BIN}" -m ruff check "."
    )

    if ! "${PYTHON_BIN}" -m mypy --version >/dev/null 2>&1; then
        echo "mypy is not installed. Install with 'pip install iroha-python[dev]'." >&2
        exit 1
    fi
    run "${PYTHON_BIN}" -m mypy --config-file "${IROHA_PYTHON_RUFF_CFG}" "${IROHA_PYTHON_SRC}" "${IROHA_PYTHON_TESTS}"
    run "${PYTHON_BIN}" -m mypy --config-file "${TORII_RUFF_CFG}" "${TORII_CLIENT_SRC}"
fi

if [[ "${SKIP_TESTS}" != "1" ]]; then
    if ! "${PYTHON_BIN}" -m pytest --version >/dev/null 2>&1; then
        echo "pytest is not installed. Install with 'pip install iroha-python[dev]'." >&2
        exit 1
    fi
    run "${PYTHON_BIN}" -m pytest "${IROHA_PYTHON_TESTS}" -q
    run "${PYTHON_BIN}" -m pytest "${TORII_CLIENT_TESTS}" -q
fi

if [[ "${SKIP_FIXTURES}" != "1" ]]; then
    if ! "${PYTHON_BIN}" scripts/check_python_fixtures.py --quiet >/dev/null 2>&1; then
        echo "[error] python fixture parity check failed" >&2
        run "${PYTHON_BIN}" scripts/check_python_fixtures.py
        exit 1
    fi
    echo "[ok] Python fixtures match canonical source"
    run scripts/check_sm2_sdk_fixtures.py --quiet
fi
