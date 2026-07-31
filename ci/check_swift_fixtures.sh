#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "${repo_root}"
echo "[swift-fixtures] verifying Swift fixture parity"
python3 scripts/check_swift_fixtures.py --quiet
echo "[swift-fixtures] parity confirmed"
