#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
GEN="$ROOT_DIR/scripts/gen_syscall_doc.py"

python3 "$GEN" --check
echo "Syscall docs are up to date."
