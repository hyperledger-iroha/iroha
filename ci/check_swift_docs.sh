#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

echo "[swift-docs] running Swift documentation lint"
python3 scripts/swift_doc_lint.py "$@"
echo "[swift-docs] lint passed"
