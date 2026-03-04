#!/usr/bin/env bash
set -euo pipefail

SCENARIO=""
OUTPUT_DIR=""
EXECUTE=0
NOTE=""

usage() {
    cat <<'USAGE'
Usage: chaos_injector.sh --scenario <id> [--output <dir>] [--execute] [--note <text>]

Records a SNNet-15F1 chaos run using the xtask harness. Default mode is dry-run; pass --execute to run inject steps.
Supported scenarios:
  - prefix-withdrawal
  - trustless-verifier-failure
  - resolver-brownout
USAGE
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --scenario)
            SCENARIO=$2
            shift 2
            ;;
        --output)
            OUTPUT_DIR=$2
            shift 2
            ;;
        --execute)
            EXECUTE=1
            shift 1
            ;;
        --note)
            NOTE=$2
            shift 2
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            usage
            exit 1
            ;;
    esac
done

if [[ -z "${SCENARIO}" ]]; then
    echo "--scenario is required" >&2
    usage
    exit 1
fi

case "${SCENARIO}" in
        prefix-withdrawal)
            SELECTED="prefix-withdrawal"
            ;;
        trustless-verifier-failure)
            SELECTED="trustless-verifier-failure"
            ;;
        resolver-brownout)
            SELECTED="resolver-brownout"
            ;;
        *)
            echo "Unsupported scenario '${SCENARIO}'" >&2
            echo "Supported scenarios:"
  - prefix-withdrawal
  - trustless-verifier-failure
  - resolver-brownout
            exit 1
            ;;
    esac

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_PATH="${SCRIPT_DIR}/chaos_scenarios.json"
if [[ -z "${OUTPUT_DIR}" ]]; then
    OUTPUT_DIR="artifacts/soranet/gateway_chaos"
fi

mkdir -p "${OUTPUT_DIR}"
CMD=(cargo xtask soranet-gateway-chaos --pop soranet-pop-m0 --config "${CONFIG_PATH}" --out "${OUTPUT_DIR}" --scenario "${SCENARIO}")
if [[ ${EXECUTE} -eq 1 ]]; then
    CMD+=(--execute)
fi
if [[ -n "${NOTE}" ]]; then
    CMD+=(--note "${NOTE}")
fi

echo "Running ${SCENARIO} (execute=${EXECUTE}) -> ${OUTPUT_DIR}"
"${CMD[@]}"
echo "Finished. Inspect chaos_report.json/md under ${OUTPUT_DIR}/run_* for evidence."
