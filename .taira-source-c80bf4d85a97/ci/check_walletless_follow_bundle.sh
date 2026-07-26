#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

"${ROOT_DIR}/scripts/social/package_walletless_bundle.sh"

BUNDLE="${ROOT_DIR}/artifacts/walletless_follow_bundle.tar.gz"
SUM="${BUNDLE}.sha256"

if [[ ! -f "${BUNDLE}" ]]; then
  echo "bundle not found at ${BUNDLE}" >&2
  exit 1
fi

if [[ ! -f "${SUM}" ]]; then
  echo "sha256 sidecar not found at ${SUM}" >&2
  exit 1
fi

echo "walletless follow bundle packaged successfully"
