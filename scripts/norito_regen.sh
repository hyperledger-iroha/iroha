#!/usr/bin/env bash
set -euo pipefail

# Regenerate Norito JSON golden fixtures deterministically.
#
# Usage:
#   scripts/norito_regen.sh
#
# Environment variables:
#   NORITO_GOLDEN_DIR         Override the directory containing the *.in.json fixtures.
#   NORITO_REGEN_STATE_FILE   Path to the JSON state file recording the latest regeneration metadata.
#   NORITO_REGEN_ROTATION_OWNER  Owner responsible for reviewing updates.
#   NORITO_REGEN_CADENCE      Cadence label (e.g., weekly-wed-1700utc).

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GOLDEN_DIR="${NORITO_GOLDEN_DIR:-${REPO_ROOT}/crates/norito/tests/data/json_golden}"
STATE_FILE="${NORITO_REGEN_STATE_FILE:-${REPO_ROOT}/artifacts/norito_regen_state.json}"
ROTATION_OWNER="${NORITO_REGEN_ROTATION_OWNER:-unassigned}"
CADENCE_LABEL="${NORITO_REGEN_CADENCE:-manual}" 

if [[ ! -d "${GOLDEN_DIR}" ]]; then
  echo "[norito-regenerate] golden directory not found: ${GOLDEN_DIR}" >&2
  exit 1
fi

echo "[norito-regenerate] regenerating goldens in ${GOLDEN_DIR}"
cargo run --manifest-path "${REPO_ROOT}/crates/norito/Cargo.toml" \
  --bin norito_regen_goldens -- "${GOLDEN_DIR}"

relative_dir="${GOLDEN_DIR#${REPO_ROOT}/}"
changes=$(cd "${REPO_ROOT}" && git status --short -- ${relative_dir}) || true
if [[ -z "${changes}" ]]; then
  echo "[norito-regenerate] no changes detected"
else
  echo "[norito-regenerate] updated files:" >&2
  echo "${changes}" >&2
fi

mkdir -p "$(dirname "${STATE_FILE}")"
timestamp="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
cat > "${STATE_FILE}" <<EOF
{
  "generated_at": "${timestamp}",
  "rotation_owner": "${ROTATION_OWNER}",
  "cadence": "${CADENCE_LABEL}",
  "golden_dir": "${GOLDEN_DIR}"
}
EOF
echo "[norito-regenerate] wrote cadence state to ${STATE_FILE}"
echo "[norito-regenerate] rotation owner: ${ROTATION_OWNER} | cadence: ${CADENCE_LABEL} | generated_at: ${timestamp}"
echo "[norito-regenerate] done"
