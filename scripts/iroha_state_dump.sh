#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${IROHA_STATE_DUMP_OUT:-${REPO_ROOT}/artifacts/state_dumps}"
FORMAT="${IROHA_STATE_DUMP_FORMAT:-norito}"
CONFIG_PATH="${IROHA_STATE_DUMP_CONFIG:-${REPO_ROOT}/defaults/peer/config.json}"
BINARY_PATH="${IROHA_STATE_DUMP_BINARY:-${REPO_ROOT}/target/release/irohad}"

if [[ ! -x "${BINARY_PATH}" ]]; then
  echo "[state-dump] irohad binary not found at ${BINARY_PATH}" >&2
  echo "[state-dump] build it first or override IROHA_STATE_DUMP_BINARY." >&2
  exit 1
fi

mkdir -p "${OUTPUT_DIR}"
TIMESTAMP="$(date -u +"%Y-%m-%dT%H-%M-%SZ")"
OUT_FILE="${OUTPUT_DIR}/state_dump_${TIMESTAMP}.${FORMAT}" 

set -x
"${BINARY_PATH}" \
  --config "${CONFIG_PATH}" \
  state dump \
  --format "${FORMAT}" \
  --output "${OUT_FILE}"
set +x

echo "[state-dump] state snapshot written to ${OUT_FILE}"
