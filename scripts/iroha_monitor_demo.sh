#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT_DIR=$(cd "${SCRIPT_DIR}/.." && pwd)
export LANG=C.UTF-8
export LC_ALL=C.UTF-8
export TERM=${TERM:-xterm-256color}
export PYTHONHASHSEED=0

usage() {
    cat <<'EOF'
Usage: scripts/iroha_monitor_demo.sh [options]

Launch `iroha_monitor --spawn-lite`, capture the overview/pipeline frames, and
store the artefacts under docs/source/images/iroha_monitor_demo/.

Options:
  --output-dir DIR      Directory for SVG/ANS outputs
                        (default: docs/source/images/iroha_monitor_demo)
  --monitor-binary BIN  Path to iroha_monitor binary (default: target/debug/iroha_monitor)
  --duration SEC        Seconds to keep the monitor running per capture (default: 4)
  --cols N              Terminal column count for captures (default: 120)
  --rows N              Terminal row count for captures (default: 48)
  --interval MS         Refresh interval passed to the monitor (default: 500)
  --seed VALUE          Seed forwarded to the capture script for determinism
                        (default: iroha-monitor-demo)
  --headless-max-frames N
                        Cap the headless fallback when running in dumb terminals
                        (default: 24)
  --manifest PATH       Optional manifest path (default: <output-dir>/manifest.json)
  --checksums PATH      Optional checksum manifest (default: <output-dir>/checksums.json)
  --allow-fallback      Permit baked-in fallback frames instead of failing (default: enabled)
  --no-fallback         Fail when captures are empty/missing markers
  --monitor-arg ARG     Extra argument to pass to iroha_monitor (repeatable)
  --peers N             Peer count for spawn-lite (default: 3)
  --help                Show this message

Prerequisites:
  * Build iroha_monitor beforehand (`cargo build -p iroha_monitor --no-default-features`)
  * python3 must be available on PATH
EOF
}

OUTPUT_DIR="${ROOT_DIR}/docs/source/images/iroha_monitor_demo"
MONITOR_BIN="${ROOT_DIR}/target/debug/iroha_monitor"
DURATION=4
COLS=120
ROWS=48
INTERVAL=500
SEED="iroha-monitor-demo"
PEERS=3
MANIFEST_PATH=""
HEADLESS_FRAMES=24
ALLOW_FALLBACK=1
MONITOR_ARGS=()
CHECKSUMS_PATH=""
export ROOT_DIR OUTPUT_DIR

while [[ $# -gt 0 ]]; do
    case "$1" in
        --output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --monitor-binary)
            MONITOR_BIN="$2"
            shift 2
            ;;
        --duration)
            DURATION="$2"
            shift 2
            ;;
        --cols)
            COLS="$2"
            shift 2
            ;;
        --rows)
            ROWS="$2"
            shift 2
            ;;
        --interval)
            INTERVAL="$2"
            shift 2
            ;;
        --seed)
            SEED="$2"
            shift 2
            ;;
        --manifest)
            MANIFEST_PATH="$2"
            shift 2
            ;;
        --headless-max-frames)
            HEADLESS_FRAMES="$2"
            shift 2
            ;;
        --checksums)
            CHECKSUMS_PATH="$2"
            shift 2
            ;;
        --allow-fallback)
            ALLOW_FALLBACK=1
            shift
            ;;
        --no-fallback)
            ALLOW_FALLBACK=0
            shift
            ;;
        --monitor-arg)
            MONITOR_ARGS+=("$2")
            shift 2
            ;;
        --peers)
            PEERS="$2"
            shift 2
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if [[ -z "${MANIFEST_PATH}" ]]; then
    MANIFEST_PATH="${OUTPUT_DIR}/manifest.json"
fi
if [[ -z "${CHECKSUMS_PATH}" ]]; then
    CHECKSUMS_PATH="${OUTPUT_DIR}/checksums.json"
fi

command -v python3 >/dev/null 2>&1 || { echo "python3 is required" >&2; exit 1; }

CAPTURE_SCRIPT="${SCRIPT_DIR}/run_iroha_monitor_demo.py"

if [[ ! -f "${CAPTURE_SCRIPT}" ]]; then
    echo "Missing capture helper: ${CAPTURE_SCRIPT}" >&2
    exit 1
fi

mkdir -p "${OUTPUT_DIR}"

echo "[demo] Capturing monitor frames (spawn-lite mode)"
PY_ARGS=(
    "${CAPTURE_SCRIPT}"
    --binary "${MONITOR_BIN}"
    --mode spawn-lite
    --peers "${PEERS}"
    --interval "${INTERVAL}"
    --duration "${DURATION}"
    --cols "${COLS}"
    --rows "${ROWS}"
    --output-dir "${OUTPUT_DIR}"
    --seed "${SEED}"
    --headless-max-frames "${HEADLESS_FRAMES}"
    --manifest "${MANIFEST_PATH}"
    --checksums "${CHECKSUMS_PATH}"
)

if [[ "${ALLOW_FALLBACK}" -eq 1 ]]; then
    PY_ARGS+=(--allow-fallback)
fi

if [[ ${#MONITOR_ARGS[@]} -gt 0 ]]; then
    for arg in "${MONITOR_ARGS[@]}"; do
        PY_ARGS+=(--monitor-arg "$arg")
    done
fi

python3 "${PY_ARGS[@]}"

python3 "${SCRIPT_DIR}/check_iroha_monitor_screenshots.py" \
    --dir "${OUTPUT_DIR}" \
    --record "${CHECKSUMS_PATH}" \
    --update

for expected in \
    "${OUTPUT_DIR}/iroha_monitor_demo_overview.svg" \
    "${OUTPUT_DIR}/iroha_monitor_demo_overview.ans" \
    "${OUTPUT_DIR}/iroha_monitor_demo_pipeline.svg" \
    "${OUTPUT_DIR}/iroha_monitor_demo_pipeline.ans" \
    "${MANIFEST_PATH}" \
    "${CHECKSUMS_PATH}"; do
    if [[ ! -s "${expected}" ]]; then
        echo "Expected artefact missing or empty: ${expected}" >&2
        exit 1
    fi
done

echo "[demo] Artefacts stored in ${OUTPUT_DIR}"
