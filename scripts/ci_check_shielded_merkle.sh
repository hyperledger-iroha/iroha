#!/usr/bin/env bash
set -euo pipefail

# Smoke-check shielded commitment Merkle determinism.
# - Computes a few roots via the Rust tests and ensures the results are stable.
# - Intended to be run on x86_64 and aarch64 in CI. For parity checks, compare
#   the printed hex digests across runners.

echo "[merkle] computing shielded empty roots and small trees.."
cargo test -p iroha_crypto --test merkle_shielded_vectors -- --nocapture

cat <<'NOTE'
To enable strict cross-arch parity:
 - Fill constants in crates/iroha_crypto/tests/merkle_shielded_golden.rs (HEX_*),
   and run in both CI jobs. The tests will then assert the exact byte strings.
NOTE
