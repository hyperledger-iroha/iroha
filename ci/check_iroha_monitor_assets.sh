#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT_DIR=$(cd "${SCRIPT_DIR}/.." && pwd)

EXPECTED_DIR="${ROOT_DIR}/docs/source/images/iroha_monitor_demo"
CAPTURE_SCRIPT="${ROOT_DIR}/scripts/iroha_monitor_demo.sh"
OUT_DIR=$(mktemp -d)
trap 'rm -rf "${OUT_DIR}"' EXIT

command -v python3 >/dev/null 2>&1 || { echo "python3 is required" >&2; exit 1; }
command -v cargo >/dev/null 2>&1 || { echo "cargo is required" >&2; exit 1; }

MONITOR_BIN="${MONITOR_BIN:-${ROOT_DIR}/target/debug/iroha_monitor}"

if [[ ! -x "${MONITOR_BIN}" ]]; then
    (cd "${ROOT_DIR}" && cargo build -p iroha_monitor --no-default-features)
fi

"${CAPTURE_SCRIPT}" \
    --output-dir "${OUT_DIR}" \
    --monitor-binary "${MONITOR_BIN}" \
    --duration "${IROHA_MONITOR_DEMO_DURATION:-6}" \
    --cols "${IROHA_MONITOR_DEMO_COLS:-120}" \
    --rows "${IROHA_MONITOR_DEMO_ROWS:-48}" \
    --interval "${IROHA_MONITOR_DEMO_INTERVAL:-500}" \
    --seed "${IROHA_MONITOR_DEMO_SEED:-iroha-monitor-demo}" \
    --peers "${IROHA_MONITOR_DEMO_PEERS:-3}" \
    --manifest "${OUT_DIR}/manifest.json"

FILES=(
    iroha_monitor_demo_overview.svg
    iroha_monitor_demo_overview.ans
    iroha_monitor_demo_pipeline.svg
    iroha_monitor_demo_pipeline.ans
    manifest.json
    checksums.json
)

for name in "${FILES[@]}"; do
    if [[ ! -f "${EXPECTED_DIR}/${name}" ]]; then
        echo "Expected committed asset missing: ${EXPECTED_DIR}/${name}" >&2
        exit 1
    fi
    if [[ ! -f "${OUT_DIR}/${name}" ]]; then
        echo "Capture did not produce ${name}" >&2
        exit 1
    fi
    if ! cmp -s "${EXPECTED_DIR}/${name}" "${OUT_DIR}/${name}"; then
        echo "iroha_monitor asset drift detected for ${name}" >&2
        diff -u "${EXPECTED_DIR}/${name}" "${OUT_DIR}/${name}" || true
        echo "Regenerate with scripts/iroha_monitor_demo.sh to refresh the committed snapshots." >&2
        exit 1
    fi
done

if ! grep -q "iroha_monitor_demo_overview.svg" "${ROOT_DIR}/docs/source/iroha_monitor.md"; then
    echo "docs/source/iroha_monitor.md must reference iroha_monitor_demo_overview.svg" >&2
    exit 1
fi
if ! grep -q "iroha_monitor_demo_pipeline.svg" "${ROOT_DIR}/docs/source/iroha_monitor.md"; then
    echo "docs/source/iroha_monitor.md must reference iroha_monitor_demo_pipeline.svg" >&2
    exit 1
fi

echo "[check] iroha_monitor assets match committed snapshots"
