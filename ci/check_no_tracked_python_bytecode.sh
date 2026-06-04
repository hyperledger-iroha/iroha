#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${CHECK_NO_TRACKED_PYTHON_BYTECODE_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"

cd "${ROOT_DIR}"
tracked_bytecode=()
while IFS= read -r path; do
  if [[ -e "${path}" ]]; then
    tracked_bytecode+=("${path}")
  fi
done < <(git ls-files -ci --exclude-standard -- '*.pyc' '*__pycache__*')

if (( ${#tracked_bytecode[@]} > 0 )); then
  echo "error: tracked Python bytecode/cache artifacts are ignored by .gitignore and must be removed:" >&2
  printf '%s\n' "${tracked_bytecode[@]}" >&2
  exit 1
fi
