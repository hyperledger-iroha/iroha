#!/usr/bin/env bash
# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

REPO_ROOT=$(git rev-parse --show-toplevel)
SWIFT_DIR="$REPO_ROOT/IrohaSwift"
METRICS_PATH="${TORII_MOCK_HARNESS_METRICS_PATH:-${REPO_ROOT}/mock-harness-metrics-swift.prom}"
SCENARIO="${TORII_MOCK_HARNESS_SCENARIO:-submit}"
SDK="swift"

start_ns=$(date +%s%N)
pushd "$SWIFT_DIR" >/dev/null
swift test --filter ToriiClientIntegrationTests
popd >/dev/null
end_ns=$(date +%s%N)

duration_ms=$(( (end_ns - start_ns) / 1000000 ))

fixture_source="$REPO_ROOT/fixtures/norito_rpc/schema_hashes.json"
if [[ ! -f "$fixture_source" ]]; then
  echo "Expected Norito RPC fixtures at $fixture_source" >&2
  exit 1
fi

if command -v shasum >/dev/null 2>&1; then
  fixture_version=$(shasum -a 256 "$fixture_source" | awk '{print $1}')
elif command -v sha256sum >/dev/null 2>&1; then
  fixture_version=$(sha256sum "$fixture_source" | awk '{print $1}')
else
  fixture_version=$(python3 - <<'PY' "$fixture_source"
import hashlib, pathlib, sys
path = pathlib.Path(sys.argv[1])
sys.stdout.write(hashlib.sha256(path.read_bytes()).hexdigest())
PY
  )
fi

cat >"$METRICS_PATH" <<EOF
# HELP torii_mock_harness_duration_ms Execution time for mock harness scenarios.
# TYPE torii_mock_harness_duration_ms gauge
torii_mock_harness_duration_ms{sdk="$SDK",scenario="$SCENARIO"} $duration_ms
# HELP torii_mock_harness_retry_total Number of retries triggered during mock harness smoke tests.
# TYPE torii_mock_harness_retry_total counter
torii_mock_harness_retry_total{sdk="$SDK",scenario="$SCENARIO"} ${TORII_MOCK_HARNESS_RETRY_TOTAL:-0}
# HELP torii_mock_harness_fixture_version Fixture bundle hash used for the run.
# TYPE torii_mock_harness_fixture_version gauge
torii_mock_harness_fixture_version{sdk="$SDK",scenario="$SCENARIO",fixture_version="$fixture_version"} 1
EOF

echo "[swift/mock_harness_smoke] Scenario '$SCENARIO' completed in ${duration_ms}ms. Metrics: $METRICS_PATH"
