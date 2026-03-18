#!/usr/bin/env bash

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd -- "${ROOT}"

if ! command -v python3 >/dev/null 2>&1; then
	echo "error: python3 is required for the tests-added guard." >&2
	exit 1
fi

if [[ "${TEST_GUARD_ALLOW:-0}" == 1 ]]; then
	echo "warning: TEST_GUARD_ALLOW=1 set; skipping tests-added guard." >&2
	exit 0
fi

python3 ci/check_tests_guard.py "$@"
