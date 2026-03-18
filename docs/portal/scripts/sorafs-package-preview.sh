#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail

BUILD_DIR="${BUILD_DIR:-build}"
ARTIFACT_DIR="${ARTIFACT_DIR:-artifacts}"
SORA_DIR="${SORA_DIR:-${ARTIFACT_DIR}/sorafs}"
CONFIG_PATH=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --config)
      CONFIG_PATH="$2"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

mkdir -p "${SORA_DIR}"

ARCHIVE="${ARTIFACT_DIR}/preview-site.tar.gz"
CHECKSUM_MANIFEST="${ARTIFACT_DIR}/checksums.sha256"

if [[ ! -f "${ARCHIVE}" ]]; then
  echo "expected ${ARCHIVE} to exist (generated earlier in the workflow)" >&2
  exit 1
fi

if [[ ! -f "${CHECKSUM_MANIFEST}" ]]; then
  echo "expected ${CHECKSUM_MANIFEST} to exist (generated earlier in the workflow)" >&2
  exit 1
fi

TORII_URL="${SORA_FS_PREVIEW_TORII_URL:-}"
AUTHORITY="${SORA_FS_PREVIEW_AUTHORITY:-}"
PRIVATE_KEY="${SORA_FS_PREVIEW_PRIVATE_KEY:-}"
SUBMITTED_EPOCH="${SORA_FS_PREVIEW_SUBMITTED_EPOCH:-}"

if [[ -n "${CONFIG_PATH}" ]]; then
  echo "Using preview config ${CONFIG_PATH}"
  if [[ ! -f "${CONFIG_PATH}" ]]; then
    echo "preview config not found at ${CONFIG_PATH}" >&2
    exit 1
  fi
  readarray -t config_values < <(node <<'NODE' "${CONFIG_PATH}")
const fs = require('node:fs');
const path = process.argv[1];
const data = JSON.parse(fs.readFileSync(path, 'utf8'));
function emit(key) {
  const value = data[key];
  if (typeof value === 'undefined' || value === null || value === '') {
    console.log('');
  } else {
    console.log(String(value));
  }
}
emit('torii_url');
emit('authority');
emit('private_key');
emit('submitted_epoch');
NODE
  TORII_URL="${config_values[0]}"
  AUTHORITY="${config_values[1]}"
  PRIVATE_KEY="${config_values[2]}"
  SUBMITTED_EPOCH="${config_values[3]}"
fi

CAR_OUT="${SORA_DIR}/preview.car"
PLAN_OUT="${SORA_DIR}/preview.plan.json"
CAR_SUMMARY="${SORA_DIR}/preview.car.summary.json"

MANIFEST_OUT="${SORA_DIR}/preview.manifest.to"
MANIFEST_JSON="${SORA_DIR}/preview.manifest.json"

echo "Packing SoraFS CAR archive from ${ARCHIVE}"
cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  car pack \
  --input="${ARCHIVE}" \
  --car-out="${CAR_OUT}" \
  --plan-out="${PLAN_OUT}" \
  --summary-out="${CAR_SUMMARY}"

echo "Building Norito manifest"
cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest build \
  --summary="${CAR_SUMMARY}" \
  --manifest-out="${MANIFEST_OUT}" \
  --manifest-json-out="${MANIFEST_JSON}"

if [[ -n "${TORII_URL}" ]] && \
   [[ -n "${AUTHORITY}" ]] && \
   [[ -n "${PRIVATE_KEY}" ]] && \
   [[ -n "${SUBMITTED_EPOCH}" ]]; then
  echo "Submitting manifest to SoraFS preview environment"
  SUBMIT_SUMMARY="${SORA_DIR}/preview.submit.summary.json"
  SUBMIT_RESPONSE="${SORA_DIR}/preview.submit.response.json"
  cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
    manifest submit \
    --manifest="${MANIFEST_OUT}" \
    --torii-url="${TORII_URL}" \
    --submitted-epoch="${SUBMITTED_EPOCH}" \
    --chunk-plan="${PLAN_OUT}" \
    --authority="${AUTHORITY}" \
    --private-key="${PRIVATE_KEY}" \
    --summary-out="${SUBMIT_SUMMARY}" \
    --response-out="${SUBMIT_RESPONSE}"
else
  cat <<'EOF'
Skipping manifest submission because preview credentials were not provided.
Provide them via a JSON config (see docs/examples/sorafs_preview_publish.json) or set:
  - SORA_FS_PREVIEW_TORII_URL
  - SORA_FS_PREVIEW_AUTHORITY
  - SORA_FS_PREVIEW_PRIVATE_KEY
  - SORA_FS_PREVIEW_SUBMITTED_EPOCH
Artifacts remain in the SoraFS preview directory for manual submission.
EOF
fi
