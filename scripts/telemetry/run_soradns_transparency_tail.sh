#!/usr/bin/env bash

set -euo pipefail

print_usage() {
  cat <<'USAGE'
Usage: run_soradns_transparency_tail.sh --log <path> [options]

Required arguments:
  --log PATH              Resolver transparency log to ingest (required).

Optional arguments:
  --output PATH           Structured output path (JSONL/TSV). Defaults to
                          artifacts/soradns/transparency.jsonl.
  --format jsonl|tsv      Output format (default: jsonl).
  --metrics-output PATH   Prometheus metrics output path. Defaults to
                          artifacts/soradns/transparency.prom. Use '-' to emit
                          metrics to stdout.
  --tailer-bin PATH       Path to prebuilt soradns_transparency_tail binary.
                          When omitted, the script executes `cargo run`.
  --push-url URL          Optional Prometheus Pushgateway base URL (e.g.
                          https://push.example.com:9091). When provided, the
                          script pushes the generated metrics.
  --push-job NAME         Pushgateway job label (default: soradns_transparency).
  --push-instance NAME    Pushgateway instance label (default: hostname -s).
  --help                  Show this message.
USAGE
}

LOG_PATH=""
OUTPUT_PATH=""
FORMAT="jsonl"
METRICS_PATH=""
TAILER_BIN=""
PUSH_URL=""
PUSH_JOB="soradns_transparency"
PUSH_INSTANCE="$(hostname -s 2>/dev/null || echo "soradns-resolver")"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --log)
      LOG_PATH=$2
      shift 2
      ;;
    --output)
      OUTPUT_PATH=$2
      shift 2
      ;;
    --format)
      FORMAT=$2
      shift 2
      ;;
    --metrics-output)
      METRICS_PATH=$2
      shift 2
      ;;
    --tailer-bin)
      TAILER_BIN=$2
      shift 2
      ;;
    --push-url)
      PUSH_URL=$2
      shift 2
      ;;
    --push-job)
      PUSH_JOB=$2
      shift 2
      ;;
    --push-instance)
      PUSH_INSTANCE=$2
      shift 2
      ;;
    --help|-h)
      print_usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      print_usage
      exit 1
      ;;
  esac
done

if [[ -z "${LOG_PATH}" ]]; then
  echo "Error: --log is required." >&2
  print_usage
  exit 1
fi

if [[ ! -f "${LOG_PATH}" ]]; then
  echo "Error: log file not found at ${LOG_PATH}" >&2
  exit 1
fi

if [[ -z "${OUTPUT_PATH}" ]]; then
  OUTPUT_PATH="artifacts/soradns/transparency.${FORMAT}"
fi

if [[ -z "${METRICS_PATH}" ]]; then
  METRICS_PATH="artifacts/soradns/transparency.prom"
fi

if [[ "${OUTPUT_PATH}" != "-" ]]; then
  mkdir -p "$(dirname "${OUTPUT_PATH}")"
fi

if [[ "${METRICS_PATH}" != "-" ]]; then
  mkdir -p "$(dirname "${METRICS_PATH}")"
fi

TAILER_ARGS=(
  --input "${LOG_PATH}"
  --format "${FORMAT}"
  --output "${OUTPUT_PATH}"
  --metrics-output "${METRICS_PATH}"
)

if [[ -n "${TAILER_BIN}" ]]; then
  if [[ ! -x "${TAILER_BIN}" ]]; then
    echo "Error: tailer binary '${TAILER_BIN}' is not executable." >&2
    exit 1
  fi
  "${TAILER_BIN}" "${TAILER_ARGS[@]}"
else
  cargo run --quiet -p soradns-resolver --bin soradns_transparency_tail -- \
    "${TAILER_ARGS[@]}"
fi

if [[ -n "${PUSH_URL}" && "${METRICS_PATH}" != "-" ]]; then
  if ! command -v curl >/dev/null 2>&1; then
    echo "Warning: curl not available; skipping Pushgateway upload." >&2
  else
    PUSH_ENDPOINT="${PUSH_URL%/}/metrics/job/${PUSH_JOB}/instance/${PUSH_INSTANCE}"
    curl --fail --silent --show-error \
      --data-binary "@${METRICS_PATH}" \
      "${PUSH_ENDPOINT}" || {
        echo "Warning: failed to push metrics to ${PUSH_ENDPOINT}" >&2
      }
  fi
fi

echo "SoraDNS transparency artefacts written to:"
echo "  Records : ${OUTPUT_PATH}"
echo "  Metrics : ${METRICS_PATH}"
if [[ -n "${PUSH_URL}" ]]; then
  echo "  Pushgateway: ${PUSH_URL} (job=${PUSH_JOB}, instance=${PUSH_INSTANCE})"
fi
