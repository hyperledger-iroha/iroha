#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

bash ci/check_sumeragi_v2_multilane_release_inventory.sh

# Release/PR evidence must exercise a source-current daemon on a real network.
# Explicit binary overrides bypass the test-network source fingerprint check.
unset TEST_NETWORK_BIN_IROHAD KAGAMI_BIN CARGO_BIN_EXE_irohad CARGO_BIN_EXE_kagami
unset TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL TEST_NETWORK_BIN_IROHA CARGO_BIN_EXE_iroha
unset TEST_NETWORK_IROHAD_FEATURES TEST_NETWORK_CARGO
export IROHA_TEST_REQUIRE_NETWORK=1
export IROHA_TEST_NETWORK_START_ATTEMPTS=1
export IROHA_TEST_SKIP_BUILD=0
export IROHA_TEST_ALLOW_REENTRANT_BUILD=1

# Run ten fresh, deterministic 12-peer networks. The launcher validates every
# seed transcript as exactly one scheduled/passing test and publishes a
# 10/10, zero-retry completion record.
cross_dataspace_args=(--capture --no-skip-build)
if [[ -n "${NEXUS_CROSS_DATASPACE_EVIDENCE_DIR:-}" ]]; then
  if [[ "$NEXUS_CROSS_DATASPACE_EVIDENCE_DIR" != /* ]]; then
    echo "NEXUS_CROSS_DATASPACE_EVIDENCE_DIR must be absolute" >&2
    exit 1
  fi
  cross_dataspace_args+=(
    --evidence-dir "$NEXUS_CROSS_DATASPACE_EVIDENCE_DIR"
  )
fi
scripts/run_nexus_cross_dataspace_atomic_swap.sh "${cross_dataspace_args[@]}"

echo "[nexus] strict 10/10 cross-dataspace localnet seed matrix passed"
