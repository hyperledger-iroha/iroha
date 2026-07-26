#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PY_SCRIPT="$SCRIPT_DIR/capture_matrix.py"

if [[ ! -f "$PY_SCRIPT" ]]; then
  echo "[fastpq] missing capture_matrix.py ($PY_SCRIPT)" >&2
  exit 1
fi

exec python3 "$PY_SCRIPT" --repo-root "$REPO_ROOT" "$@"
