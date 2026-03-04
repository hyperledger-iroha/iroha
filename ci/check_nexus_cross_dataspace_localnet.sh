#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

# Run the deterministic cross-dataspace all-or-nothing swap proof.
scripts/run_nexus_cross_dataspace_atomic_swap.sh --capture

echo "[nexus] cross-dataspace localnet proof passed"
