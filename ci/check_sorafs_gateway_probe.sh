#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE_DIR="${REPO_ROOT}/fixtures/sorafs_gateway/probe_demo"
ARTIFACT_DIR="${REPO_ROOT}/artifacts/sorafs_gateway_probe/ci"
LOG_PATH="${ARTIFACT_DIR}/drill_log.md"
PD_PAYLOAD="${ARTIFACT_DIR}/pagerduty.json"

if [[ ! -f "${FIXTURE_DIR}/demo.gar.jws" ]]; then
  echo "error: gateway probe fixtures missing at ${FIXTURE_DIR}" >&2
  exit 1
fi

mkdir -p "${ARTIFACT_DIR}"

GAR_KEY_HEX="$(<"${FIXTURE_DIR}/gar_pub.hex")"

# Run the drill in "training" mode (PagerDuty dry-run) against the demo headers.
"${REPO_ROOT}/scripts/telemetry/run_sorafs_gateway_probe.sh" \
  --workspace "${REPO_ROOT}" \
  --scenario "gateway-probe-ci" \
  --drill-log "${LOG_PATH}" \
  --notes "CI demo probe via fixtures" \
  --artifact-dir "${ARTIFACT_DIR}" \
  --pagerduty-routing-key "ci-demo-routing-key" \
  --pagerduty-output "${PD_PAYLOAD}" \
  --pagerduty-detail environment=ci \
  --pagerduty-detail playbook=docs/source/sorafs_gateway_tls_automation.md \
  --pagerduty-dry-run \
  -- \
  --headers-file "${FIXTURE_DIR}/headers_demo.txt" \
  --host "demo.gw.sora.id" \
  --gar "${FIXTURE_DIR}/demo.gar.jws" \
  --gar-key "demo-gar=${GAR_KEY_HEX}"
