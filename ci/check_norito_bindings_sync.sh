#!/usr/bin/env bash
# Run every cross-language Norito parity lane with capability skips forbidden.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! command -v python3 >/dev/null 2>&1; then
  echo "[norito-bindings] error: Python 3 is required" >&2
  exit 1
fi

export NORITO_BINDINGS_CHECK_ALL="1"
export NORITO_JAVA_STRICT="1"
export NORITO_KOTLIN_STRICT="1"

cd "${repo_root}"
exec python3 scripts/check_norito_bindings_sync.py
