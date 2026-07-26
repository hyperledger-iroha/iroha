#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NX18_ARTIFACT_DIR="${NEXUS_LANE_SMOKE_EVIDENCE_DIR:-${REPO_ROOT}/artifacts/nx18}"
if [[ -n "${NEXUS_LANE_SMOKE_EVIDENCE_DIR:-}" ]]; then
  if [[ "${NX18_ARTIFACT_DIR}" != /* ]]; then
    echo "NEXUS_LANE_SMOKE_EVIDENCE_DIR must be absolute" >&2
    exit 1
  fi
  if [[ -e "${NX18_ARTIFACT_DIR}" || -L "${NX18_ARTIFACT_DIR}" ]]; then
    echo "refusing existing Nexus lane evidence directory: ${NX18_ARTIFACT_DIR}" >&2
    exit 1
  fi
  mkdir -m 0700 -- "${NX18_ARTIFACT_DIR}"
else
  mkdir -p "${NX18_ARTIFACT_DIR}"
fi

cd "$REPO_ROOT"

python3 scripts/nexus_lane_smoke.py \
  --lifecycle-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --telemetry-file fixtures/nexus/lanes/telemetry_alias_migrated.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --min-block-height 500 \
  --max-finality-lag 2 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10 \
  --require-alias-migration core:payments

python3 scripts/telemetry/check_slot_duration.py \
  fixtures/nexus/lanes/metrics_ready.prom \
  --min-samples 10 \
  --json-out "${NX18_ARTIFACT_DIR}/slot_summary.json" \
  --quiet

python3 scripts/telemetry/nx18_acceptance.py \
  fixtures/nexus/lanes/metrics_ready.prom \
  --json-out "${NX18_ARTIFACT_DIR}/nx18_acceptance.json" \
  --quiet

python3 scripts/telemetry/bundle_slot_artifacts.py \
  --metrics fixtures/nexus/lanes/metrics_ready.prom \
  --summary "${NX18_ARTIFACT_DIR}/slot_summary.json" \
  --out-dir "${NX18_ARTIFACT_DIR}" \
  --metadata source=fixtures/nexus/lanes/metrics_ready.prom

echo "[nexus] lane smoke fixtures validated"
