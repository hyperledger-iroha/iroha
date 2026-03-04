#!/usr/bin/env bash
set -euo pipefail

# Regenerates Python Norito fixtures by mirroring the canonical Android set.
# This keeps the Python, Swift, and Java SDKs aligned until the shared exporter
# (`scripts/export_norito_fixtures`) grows language-specific outputs.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_DIR="${PYTHON_FIXTURE_SOURCE:-${REPO_ROOT}/java/iroha_android/src/test/resources}"
TARGET_DIR="${PYTHON_FIXTURE_OUT:-${REPO_ROOT}/python/iroha_python/tests/fixtures}"
STATE_FILE="${PYTHON_FIXTURE_STATE_FILE:-${REPO_ROOT}/artifacts/python_fixture_regen_state.json}"
ROTATION_OWNER="${PYTHON_FIXTURE_ROTATION_OWNER:-unassigned}"
CADENCE_LABEL="${PYTHON_FIXTURE_CADENCE:-twice-weekly-tue-fri-0900utc}"

if [[ ! -d "${SOURCE_DIR}" ]]; then
  echo "[python-fixtures] source directory not found: ${SOURCE_DIR}" >&2
  exit 1
fi

mkdir -p "${TARGET_DIR}"

echo "[python-fixtures] syncing fixtures from ${SOURCE_DIR} to ${TARGET_DIR}"
rsync -a --delete --prune-empty-dirs \
  --include '*/' \
  --include '*.norito' \
  --include '*transaction_payload*.json' \
  --include '*transaction_fixtures*.json' \
  --include '*trigger_instructions.json' \
  --exclude '*' \
  "${SOURCE_DIR}/" "${TARGET_DIR}/"

changes=$(cd "${REPO_ROOT}" && git status --short -- "${TARGET_DIR#${REPO_ROOT}/}") || true
if [[ -z "${changes}" ]]; then
  echo "[python-fixtures] no changes detected"
else
  echo "[python-fixtures] updated files:" >&2
  echo "${changes}" >&2
fi

mkdir -p "$(dirname "${STATE_FILE}")"
timestamp="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
cat > "${STATE_FILE}" <<EOF
{
  "generated_at": "${timestamp}",
  "rotation_owner": "${ROTATION_OWNER}",
  "cadence": "${CADENCE_LABEL}",
  "source": "${SOURCE_DIR}",
  "target": "${TARGET_DIR}"
}
EOF
echo "[python-fixtures] wrote cadence state to ${STATE_FILE}"
echo "[python-fixtures] rotation owner: ${ROTATION_OWNER} | cadence: ${CADENCE_LABEL} | generated_at: ${timestamp}"

echo "[python-fixtures] done"
