#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NX18_ARTIFACT_DIR="${REPO_ROOT}/artifacts/nx18"
mkdir -p "${NX18_ARTIFACT_DIR}"

python3 "$REPO_ROOT/scripts/nexus_lane_smoke.py" \
  --status-file "$REPO_ROOT/fixtures/nexus/lanes/status_ready.json" \
  --metrics-file "$REPO_ROOT/fixtures/nexus/lanes/metrics_ready.prom" \
  --telemetry-file "$REPO_ROOT/fixtures/nexus/lanes/telemetry_alias_migrated.ndjson" \
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

python3 "$REPO_ROOT/scripts/telemetry/check_slot_duration.py" \
  "$REPO_ROOT/fixtures/nexus/lanes/metrics_ready.prom" \
  --min-samples 10 \
  --json-out "${NX18_ARTIFACT_DIR}/slot_summary.json" \
  --quiet

python3 "$REPO_ROOT/scripts/telemetry/nx18_acceptance.py" \
  "$REPO_ROOT/fixtures/nexus/lanes/metrics_ready.prom" \
  --json-out "${NX18_ARTIFACT_DIR}/nx18_acceptance.json" \
  --quiet

python3 "$REPO_ROOT/scripts/telemetry/bundle_slot_artifacts.py" \
  --metrics "$REPO_ROOT/fixtures/nexus/lanes/metrics_ready.prom" \
  --summary "${NX18_ARTIFACT_DIR}/slot_summary.json" \
  --out-dir "${NX18_ARTIFACT_DIR}" \
  --metadata source=fixtures/nexus/lanes/metrics_ready.prom

echo "[nexus] lane smoke fixtures validated"
