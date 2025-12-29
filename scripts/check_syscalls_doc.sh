#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
GEN="$ROOT_DIR/scripts/gen_syscall_doc.py"
OUT="$ROOT_DIR/docs/source/ivm_syscalls_generated.md"
TMP="$(mktemp)"

python3 "$GEN" > "$TMP"

if ! diff -u "$OUT" "$TMP" >/dev/null; then
  echo "Syscall docs are out of date. To regenerate, run: make docs-syscalls" >&2
  echo "Diff:" >&2
  diff -u "$OUT" "$TMP" || true
  rm -f "$TMP"
  exit 1
fi

rm -f "$TMP"
echo "Syscall docs are up to date."

