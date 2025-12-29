#!/usr/bin/env bash
set -euo pipefail

# Collects Stage7-1 alert evidence (queue headroom, zero-fill, CPU fallback)
# from Prometheus and writes the payloads to .prom files that can be dropped
# directly into a rollout bundle.

usage() {
  cat <<'EOF'
Usage: scripts/fastpq/capture_alert_evidence.sh --device-class <label> [options]

Options:
  --device-class LABEL   Device class label (e.g., apple-m4-max, xeon-rtx-sm80). Required.
  --prom-url URL        Prometheus base URL (defaults to $PROMETHEUS_URL).
  --out DIR             Directory to write evidence files to. Defaults to
                        artifacts/fastpq_rollouts/evidence/<timestamp>_<label>.
  --timestamp VALUE     Override the ISO8601 timestamp recorded in the files.
  -h, --help            Show this help text.

The script writes three files:
  metrics_headroom.prom   -> fastpq_metal_queue_depth{metric="limit|max_in_flight",…}
  metrics_zero_fill.prom  -> fastpq_zero_fill_duration_ms{…}
  metrics_cpu_fallback.prom -> fastpq_execution_mode_total{requested="gpu",backend="cpu",…}
Each file includes the promQL query, generation timestamp, and the raw JSON result
returned by Prometheus.
EOF
}

PROM_URL="${PROMETHEUS_URL:-}"
DEVICE_CLASS=""
OUT_DIR=""
CUSTOM_TS=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --device-class)
      shift
      [[ $# -gt 0 ]] || { echo "--device-class requires a value" >&2; exit 1; }
      DEVICE_CLASS="$1"
      ;;
    --prom-url)
      shift
      [[ $# -gt 0 ]] || { echo "--prom-url requires a value" >&2; exit 1; }
      PROM_URL="$1"
      ;;
    --out)
      shift
      [[ $# -gt 0 ]] || { echo "--out requires a path" >&2; exit 1; }
      OUT_DIR="$1"
      ;;
    --timestamp)
      shift
      [[ $# -gt 0 ]] || { echo "--timestamp requires a value" >&2; exit 1; }
      CUSTOM_TS="$1"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
  shift || true
done

if [[ -z "$DEVICE_CLASS" ]]; then
  echo "--device-class is required" >&2
  usage >&2
  exit 1
fi

if [[ -z "$PROM_URL" ]]; then
  echo "Set --prom-url or PROMETHEUS_URL before running this script." >&2
  exit 1
fi

if [[ -z "$CUSTOM_TS" ]]; then
  CUSTOM_TS="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
fi

ESCAPED_CLASS="${DEVICE_CLASS//\"/\\\"}"

if [[ -z "$OUT_DIR" ]]; then
  SAFE_LABEL="${DEVICE_CLASS//[^a-zA-Z0-9._-]/_}"
  OUT_DIR="artifacts/fastpq_rollouts/evidence/${CUSTOM_TS}_${SAFE_LABEL}"
fi

mkdir -p "$OUT_DIR"

query_and_write() {
  local name="$1"
  local query="$2"
  local target="${OUT_DIR}/${name}.prom"

  curl -fsS -G "${PROM_URL%/}/api/v1/query" \
    --data-urlencode "query=${query}" \
    > "${target}.tmp"

  {
    printf '# query: %s\n' "$query"
    printf '# generated_at: %s\n' "$CUSTOM_TS"
    cat "${target}.tmp"
  } > "$target"
  rm -f "${target}.tmp"
  echo "[fastpq] wrote ${target}"
}

queue_query="fastpq_metal_queue_depth{metric=~\"limit|max_in_flight\",device_class=\"${ESCAPED_CLASS}\"}"
zero_fill_query="fastpq_zero_fill_duration_ms{device_class=\"${ESCAPED_CLASS}\"}"
cpu_fallback_query="fastpq_execution_mode_total{requested=\"gpu\",backend=\"cpu\",device_class=\"${ESCAPED_CLASS}\"}"

query_and_write "metrics_headroom" "$queue_query"
query_and_write "metrics_zero_fill" "$zero_fill_query"
query_and_write "metrics_cpu_fallback" "$cpu_fallback_query"

echo "[fastpq] evidence directory: $OUT_DIR"
