#!/usr/bin/env bash
# Run the override-event validator and emit JSON/Markdown summaries.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
OUT_DIR="${OUT_DIR:-${ROOT}/artifacts/android/telemetry}"
JSON_OUT="${OUT_DIR}/override_events_summary_${STAMP}.json"
MD_OUT="${OUT_DIR}/override_events_summary_${STAMP}.md"
EVENTS="${EVENTS_PATH:-${ROOT}/docs/source/sdk/android/readiness/override_logs/override_events.ndjson}"
LEDGER="${LEDGER_PATH:-${ROOT}/docs/source/sdk/android/telemetry_override_log.md}"

mkdir -p "${OUT_DIR}"

if [[ -n "${AND7_BUNDLE_MANIFEST:-}" ]]; then
  python3 "${ROOT}/scripts/telemetry/validate_override_events.py" \
    --events "${EVENTS}" \
    --ledger "${LEDGER}" \
    --json-out "${JSON_OUT}" \
    --markdown-out "${MD_OUT}" \
    --bundle-manifest "${AND7_BUNDLE_MANIFEST}"
else
  python3 "${ROOT}/scripts/telemetry/validate_override_events.py" \
    --events "${EVENTS}" \
    --ledger "${LEDGER}" \
    --json-out "${JSON_OUT}" \
    --markdown-out "${MD_OUT}"
fi

echo "Override event validation complete."
echo "JSON summary: ${JSON_OUT}"
echo "Markdown summary: ${MD_OUT}"
