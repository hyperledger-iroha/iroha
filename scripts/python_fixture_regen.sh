#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

echo "[python-fixtures] delegating to the canonical Norito RPC owner"
exec "${CARGO_BIN:-cargo}" run --locked -p xtask --bin xtask -- \
  norito-rpc-fixtures "$@"
