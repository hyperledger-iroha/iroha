#!/usr/bin/env bash
# Generates the DA threat-model report JSON via cargo xtask.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../.." && pwd)"
OUT_DIR="${1:-${REPO_ROOT}/artifacts/da}"

mkdir -p "${OUT_DIR}"
JSON_PATH="${OUT_DIR%/}/threat_model_report.json"

pushd "${REPO_ROOT}" >/dev/null
cargo xtask da-threat-model-report --out "${JSON_PATH}"
popd >/dev/null

echo "DA threat-model report written to ${JSON_PATH}"
