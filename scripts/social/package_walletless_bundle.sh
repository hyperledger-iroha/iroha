#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SRC_DIR="${ROOT_DIR}/examples/walletless-follow-game"
ARTIFACT_DIR="${ROOT_DIR}/artifacts"
BUNDLE_PATH="${ARTIFACT_DIR}/walletless_follow_bundle.tar.gz"

mkdir -p "${ARTIFACT_DIR}"

tar -czf "${BUNDLE_PATH}" -C "${SRC_DIR}" .
if command -v shasum >/dev/null 2>&1; then
  shasum -a 256 "${BUNDLE_PATH}" >"${BUNDLE_PATH}.sha256"
elif command -v sha256sum >/dev/null 2>&1; then
  sha256sum "${BUNDLE_PATH}" >"${BUNDLE_PATH}.sha256"
fi

echo "Bundle written to ${BUNDLE_PATH}"
