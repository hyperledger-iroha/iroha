#!/usr/bin/env bash
# Helper for selectively updating Cargo.lock entries required by the SM feature spike.
# Usage: run from the workspace root after obtaining approval per docs/source/crypto/sm_lock_refresh_plan.md.

set -euo pipefail

if [[ ! -f Cargo.toml ]]; then
  echo "Run this script from the repository root." >&2
  exit 1
fi

echo "Refreshing Cargo.lock for sm2/sm3/sm4/rfc6979..."
cargo update \
  -p sm2 \
  -p sm3 \
  -p sm4 \
  -p rfc6979

echo "Generating dependency tree for audit..."
cargo tree -p iroha_crypto --features sm > target/sm_dep_tree.txt

echo "Done. Attach Cargo.lock diff and target/sm_dep_tree.txt to the approval PR."
