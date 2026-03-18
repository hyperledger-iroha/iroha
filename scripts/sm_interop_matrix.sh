#!/usr/bin/env bash
# Run the SM interop matrix against the configured CLI tools.
#
# Usage:
#   scripts/sm_interop_matrix.sh [extra cargo args...]
#   IROHA_SM_CLI="openssl /opt/tongsuo/bin/openssl" scripts/sm_interop_matrix.sh
#
# If `IROHA_SM_CLI` is unset the harness will probe for `openssl` and `tongsuo`
# in PATH, skipping any tools that are unavailable.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

pushd "${ROOT_DIR}" >/dev/null

NORITO_SKIP_BINDINGS_SYNC=1 cargo test \
  -p iroha_crypto \
  --test sm_cli_matrix \
  --features sm \
  "$@"

popd >/dev/null
