#!/usr/bin/env bash
# Sole subprocess-safe entry point for tools that accept only one Cargo binary.
# Requires an absolute owner-private external CARGO_TARGET_DIR. An optional
# IROHA_RELEASE_CANCEL_REQUEST_PATH is checked only between natural commands.
# shellcheck source-path=SCRIPTDIR
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
PROCESS_POLICY="${SCRIPT_DIR}/sumeragi_v2_release_process_policy.sh"
if [[ ! -f "${PROCESS_POLICY}" || -L "${PROCESS_POLICY}" ]]; then
  echo "error: shared release process policy is unavailable or symbolic." >&2
  exit 2
fi
# shellcheck source=sumeragi_v2_release_process_policy.sh
source "${PROCESS_POLICY}"

if [[ "$#" == 1 && ( "$1" == "-h" || "$1" == "--help" ) ]]; then
  cat <<'EOF'
Usage: scripts/sumeragi_v2_release_cargo_proxy.sh <cargo-subcommand> <arguments...>

Run one supported Cargo command through the authenticated Cargo 1.93.1 binary,
--locked/--offline/-j1 and no-interference release policy. The caller must set
CARGO_TARGET_DIR to a fresh owner-private directory below /private/tmp,
IROHA_RELEASE_CARGO_BIN to its absolute canonical executable, and keep both
outside this source tree.
EOF
  exit 0
fi

require_external_cargo_target_dir "${REPO_ROOT}"
run_cargo "$@"
