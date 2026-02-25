#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

export NORITO_SKIP_BINDINGS_SYNC="${NORITO_SKIP_BINDINGS_SYNC:-1}"

cargo test -p iroha --lib batch_verification_ -- --nocapture
cargo test -p iroha --lib get_sumeragi_status_wire_rejects_ -- --nocapture
cargo test -p iroha --lib get_cross_lane_transfer_proofs_ -- --nocapture

echo "[nexus] cross-lane proof filters passed"
