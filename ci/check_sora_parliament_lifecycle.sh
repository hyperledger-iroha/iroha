#!/usr/bin/env bash
set -euo pipefail

readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd -- "$REPO_ROOT"

readonly PARLIAMENT_TARGET_DIR="${CARGO_TARGET_DIR:-${REPO_ROOT}/target/sora-parliament-lifecycle}"
readonly PARLIAMENT_DAEMON="${PARLIAMENT_TARGET_DIR}/debug/iroha3d"

# Keep the proof-valid fixture providers in one dedicated debug-only build lane.
# The providers themselves reject optimized compilation, and this script never
# replaces or consumes a release binary.
export CARGO_TARGET_DIR="$PARLIAMENT_TARGET_DIR"
export NORITO_SKIP_BINDINGS_SYNC=1

cargo build --locked \
  -p irohad \
  --bin iroha3d \
  --features test-network-parliament-signers

if [[ ! -x "$PARLIAMENT_DAEMON" ]]; then
  echo "feature-isolated Parliament daemon was not built at $PARLIAMENT_DAEMON" >&2
  exit 1
fi

export TEST_NETWORK_BIN_IROHAD_PARLIAMENT_SIGNERS="$PARLIAMENT_DAEMON"
export IROHA_TEST_SKIP_BUILD=1
export IROHA_TEST_REQUIRE_NETWORK=1
export IROHA_FAIL_ON_SANDBOX_SKIP=1
export IROHA_TEST_NETWORK_START_ATTEMPTS=1

cargo test --locked \
  -p integration_tests \
  --features parliament-test-signers \
  --test sora_parliament_lifecycle_smoke \
  -- \
  --nocapture \
  --test-threads=1
