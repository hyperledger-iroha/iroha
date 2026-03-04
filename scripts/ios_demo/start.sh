#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

CONFIG_PATH="${ROOT_DIR}/examples/ios/NoritoDemoXcode/Configs/SampleAccounts.json"
ARTIFACTS_DIR="${ROOT_DIR}/artifacts"
PROFILE="full"
EXIT_AFTER_READY=false

usage() {
    cat <<'USAGE'
Usage: scripts/ios_demo/start.sh [options]

Options:
  --config PATH             Accounts configuration JSON (default: examples/ios/NoritoDemoXcode/Configs/SampleAccounts.json)
  --artifacts DIR           Directory for generated artefacts (default: ./artifacts)
  --telemetry-profile NAME  Telemetry profile passed to the demo node (default: full)
  --exit-after-ready        Exit once provisioning finishes (useful for CI)
  --help                    Show this message
USAGE
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --config)
            CONFIG_PATH="$2"
            shift 2
            ;;
        --artifacts|--artifacts-dir)
            ARTIFACTS_DIR="$2"
            shift 2
            ;;
        --telemetry-profile|--profile)
            PROFILE="$2"
            shift 2
            ;;
        --exit-after-ready)
            EXIT_AFTER_READY=true
            shift
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

if [[ "${CONFIG_PATH}" != /* ]]; then
    CONFIG_PATH="${ROOT_DIR}/${CONFIG_PATH}"
fi
if [[ "${ARTIFACTS_DIR}" != /* ]]; then
    ARTIFACTS_DIR="${ROOT_DIR}/${ARTIFACTS_DIR}"
fi

if [[ ! -f "${CONFIG_PATH}" ]]; then
    echo "Config file not found: ${CONFIG_PATH}" >&2
    exit 1
fi

STATE_FILE="${ARTIFACTS_DIR}/ios_demo_state.json"
RUN_LOG="${ARTIFACTS_DIR}/ios_demo_runner.log"
TORII_LOG="${ARTIFACTS_DIR}/torii.log"
METRICS_FILE="${ARTIFACTS_DIR}/metrics.prom"
JWT_FILE="${ARTIFACTS_DIR}/torii.jwt"

command -v python3 >/dev/null 2>&1 || {
    echo "python3 is required to run this script" >&2
    exit 1
}

mkdir -p "${ARTIFACTS_DIR}"
rm -f "${STATE_FILE}" "${TORII_LOG}" "${METRICS_FILE}" "${JWT_FILE}" "${RUN_LOG}"

declare -a CMD
CMD=(cargo run -p iroha_test_network --example ios_demo -- --config "${CONFIG_PATH}" --state "${STATE_FILE}" --telemetry-profile "${PROFILE}")
if [[ "${EXIT_AFTER_READY}" == true ]]; then
    CMD+=(--exit-after-ready)
fi

(
    cd "${ROOT_DIR}"
    "${CMD[@]}"
) >"${RUN_LOG}" 2>&1 &
PID=$!

cleanup() {
    if [[ -n "${PID:-}" ]] && kill -0 "${PID}" 2>/dev/null; then
        kill "${PID}" >/dev/null 2>&1 || true
        wait "${PID}" >/dev/null 2>&1 || true
    fi
}

trap 'cleanup; exit 130' INT
trap 'cleanup; exit 143' TERM

for _ in {1..60}; do
    if [[ -f "${STATE_FILE}" ]]; then
        break
    fi
    if ! kill -0 "${PID}" 2>/dev/null; then
        echo "[ios-demo] Demo process exited early. See ${RUN_LOG} for details." >&2
        cleanup
        exit 1
    fi
    sleep 1
done

if [[ ! -f "${STATE_FILE}" ]]; then
    echo "[ios-demo] Timed out waiting for state file ${STATE_FILE}" >&2
    cleanup
    exit 1
fi

IFS=$'\n' read -r TORII_URL METRICS_URL STDOUT_LOG <<OUTPUT
$(python3 - <<'PY'
import json, sys
with open(sys.argv[1], 'r', encoding='utf-8') as fh:
    data = json.load(fh)
print(data.get('torii_url', ''))
print(data.get('metrics_url', ''))
print(data.get('stdout_log', '') or '')
PY
"${STATE_FILE}")
OUTPUT

if [[ -n "${STDOUT_LOG}" && -f "${STDOUT_LOG}" ]]; then
    cp "${STDOUT_LOG}" "${TORII_LOG}"
else
    echo "[ios-demo] warning: stdout log not found (expected ${STDOUT_LOG})" >&2
fi

if command -v curl >/dev/null 2>&1 && [[ -n "${METRICS_URL}" ]]; then
    for _ in {1..10}; do
        if curl -sf "${METRICS_URL}" -o "${METRICS_FILE}"; then
            break
        fi
        sleep 1
    done
    if [[ ! -s "${METRICS_FILE}" ]]; then
        echo "[ios-demo] warning: failed to download metrics from ${METRICS_URL}" >&2
    fi
else
    echo "[ios-demo] warning: metrics collection skipped (curl missing or metrics URL unavailable)" >&2
fi

python3 - <<'PY'
import json, sys
from pathlib import Path
state_path, out_path = sys.argv[1:3]
with open(state_path, 'r', encoding='utf-8') as fh:
    data = json.load(fh)
accounts = []
for entry in data.get('accounts', []):
    record = {
        'account_id': entry.get('account_id'),
        'public_key': entry.get('public_key')
    }
    if entry.get('private_key'):
        record['private_key'] = entry['private_key']
    accounts.append(record)
Path(out_path).write_text(json.dumps({'accounts': accounts}, indent=2) + '\n', encoding='utf-8')
PY
"${STATE_FILE}" "${JWT_FILE}"

echo "[ios-demo] Torii URL: ${TORII_URL}"
[[ -s "${METRICS_FILE}" ]] && echo "[ios-demo] Metrics: ${METRICS_FILE}" || echo "[ios-demo] Metrics: unavailable"
echo "[ios-demo] Torii log: ${TORII_LOG}"
echo "[ios-demo] Credentials: ${JWT_FILE}"
echo "[ios-demo] Demo running (PID ${PID}). Press Ctrl+C to stop."

wait "${PID}"
STATUS=$?
cleanup
exit "${STATUS}"
