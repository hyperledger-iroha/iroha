#!/usr/bin/env bash
set -euo pipefail

if ! python3 scripts/check_python_fixtures.py --quiet >/dev/null 2>&1; then
    python3 scripts/check_python_fixtures.py
    exit 1
fi
