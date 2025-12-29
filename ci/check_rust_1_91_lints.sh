#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

EXTRA_LINTS=(-Dwarnings -Wdangling_pointers_from_locals -Winteger_to_ptr_transmutes -Wsemicolon_in_expressions_from_macros)

if [[ -n "${RUSTFLAGS:-}" ]]; then
  export RUSTFLAGS="${RUSTFLAGS} ${EXTRA_LINTS[*]}"
else
  export RUSTFLAGS="${EXTRA_LINTS[*]}"
fi

echo "[rust-1.91] running cargo check with stricter lint gates..."
cargo check --workspace --all-targets "$@"
