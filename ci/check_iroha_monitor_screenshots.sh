#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT_DIR=$(cd "${SCRIPT_DIR}/.." && pwd)

python3 "${ROOT_DIR}/scripts/check_iroha_monitor_screenshots.py" \
  --dir "${ROOT_DIR}/docs/source/images/iroha_monitor_demo" \
  --record "${ROOT_DIR}/docs/source/images/iroha_monitor_demo/checksums.json"
