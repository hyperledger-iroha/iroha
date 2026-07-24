#!/usr/bin/env bash
# Static inventory guard for mandatory four-peer Sumeragi V2 multilane release gates.

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

readonly autoscale_file="integration_tests/tests/nexus/autoscale_localnet.rs"
readonly native_file="integration_tests/tests/native_amx_routing.rs"
readonly launcher="scripts/run_nexus_cross_dataspace_atomic_swap.sh"
readonly release_runner="scripts/run_sumeragi_v2_release_gates.sh"
readonly grouped_parity_harness="ci/run_native_amx_v2_grouped_sdk_parity.sh"
readonly grouped_fixture="fixtures/sumeragi_v2/native_amx_v2_grouped.json"
readonly autoscale_test="nexus_autoscale_four_peer_release_lifecycle_recreates_lane_and_rejects_stale_artifacts"
readonly autoscale_qualified_test="nexus::autoscale_localnet::${autoscale_test}"
readonly native_test="native_amx_rotating_validator_fault_soak_preserves_independent_participant_qcs"

require_nonignored_test() {
  local path="$1"
  local test_name="$2"
  local declaration_count declaration_line window_start
  declaration_count="$(
    grep -Ec -- "^(async )?fn ${test_name}\\(" "$path" || true
  )"
  if [[ "$declaration_count" != 1 ]]; then
    echo "expected one exact mandatory multilane test declaration ${test_name} in ${path}; found ${declaration_count}" >&2
    exit 1
  fi
  declaration_line="$(
    grep -En -- "^(async )?fn ${test_name}\\(" "$path" | cut -d: -f1
  )"
  window_start=$((declaration_line > 12 ? declaration_line - 12 : 1))
  if ! sed -n "${window_start},${declaration_line}p" "$path" \
    | grep -Eq '^#\[(tokio::)?test'; then
    echo "mandatory multilane declaration lacks a test attribute: ${test_name}" >&2
    exit 1
  fi
  if sed -n "${window_start},${declaration_line}p" "$path" \
    | grep -Eq '^#\[ignore([=(]|$)'; then
    echo "mandatory multilane release test must not be ignored: ${test_name}" >&2
    exit 1
  fi
}

require_exact_token() {
  local path="$1"
  local token="$2"
  if [[ "$(grep -Fxc -- "$token" "$path" || true)" != 1 ]]; then
    echo "required multilane release inventory token is missing or duplicated in ${path}: ${token}" >&2
    exit 1
  fi
}

require_nonignored_test "$autoscale_file" "$autoscale_test"
require_nonignored_test "$native_file" "$native_test"

require_exact_token \
  "$launcher" \
  "readonly AUTOSCALE_FOUR_PEER_RELEASE_TEST=\"${autoscale_qualified_test}\""
require_exact_token \
  "$launcher" \
  "readonly NATIVE_AMX_FAULT_SOAK_TEST=\"${native_test}\""
require_exact_token \
  "$release_runner" \
  "readonly multilane_autoscale_four_peer_release_test=\"${autoscale_qualified_test}\""
require_exact_token \
  "$release_runner" \
  "readonly multilane_native_amx_rotating_release_test=\"${native_test}\""
require_exact_token \
  "$release_runner" \
  "readonly native_amx_grouped_parity_harness=\"${grouped_parity_harness}\""
require_exact_token \
  "$release_runner" \
  "readonly expected_multilane_focus_test_count=113"
require_exact_token \
  "$release_runner" \
  "  readonly expected_corridor_leg_count=71"
require_exact_token \
  "$release_runner" \
  "    native_amx_grouped_fixture_sha256 \"\$native_amx_grouped_fixture_sha256\" \\"
require_exact_token \
  "$release_runner" \
  "    native_amx_grouped_negative_control_count 34 \\"

for grouped_surface in openapi python javascript swift kotlin java; do
  if [[ "$(grep -Ec -- "^    ${grouped_surface}$" "$release_runner" || true)" != 1 ]]; then
    echo "production release runner must inventory grouped Native AMX V2 ${grouped_surface} parity exactly once" >&2
    exit 1
  fi
  if ! grep -Fq -- "${grouped_surface})" "$grouped_parity_harness"; then
    echo "grouped Native AMX V2 parity harness lacks ${grouped_surface} execution" >&2
    exit 1
  fi
done

if [[ ! -f "$grouped_fixture" || -L "$grouped_fixture" ]]; then
  echo "Rust-owned grouped Native AMX V2 corpus must be a regular non-symlink fixture" >&2
  exit 1
fi
grouped_fixture_sha256="$(bash "$grouped_parity_harness" --fixture-sha256)"
grouped_suite_source_manifest_sha256="$(
  bash "$grouped_parity_harness" --suite-source-manifest-sha256
)"
receipt_suite_source_manifest_sha256="$(
  python3 -I -S - "$repo_root" <<'PY'
from pathlib import Path
import runpy
import sys

root = Path(sys.argv[1]).resolve(strict=True)
sys.path.insert(0, str(root))
symbols = runpy.run_path(str(root / "scripts/write_sumeragi_v2_release_receipt.py"))
print(symbols["_native_amx_grouped_suite_source_manifest"](root))
PY
)"
if [[ ! "$grouped_fixture_sha256" =~ ^[0-9a-f]{64}$ \
  || ! "$grouped_suite_source_manifest_sha256" =~ ^[0-9a-f]{64}$ \
  || "$receipt_suite_source_manifest_sha256" \
    != "$grouped_suite_source_manifest_sha256" ]]; then
  echo "grouped Native AMX V2 fixture/suite source binding is invalid" >&2
  exit 1
fi

if [[ "$(grep -Fxc -- '      --multilane-four-peer-release' "$release_runner" || true)" != 1 ]]; then
  echo "production release runner must invoke the mandatory four-peer launcher exactly once" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "    skipped_runs 0 \\" "$launcher" || true)" != 1 ]]; then
  echo "mandatory four-peer completion contract must record exactly zero skips" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "    env \"\${ENV_VARS[@]}\" IROHA_MULTILANE_RELEASE_MODE=1 \"\${CMD[@]}\" \\" "$launcher" || true)" != 1 ]]; then
  echo "mandatory four-peer launcher must set the exact release-mode environment" >&2
  exit 1
fi

echo "[multilane-release-inventory] mandatory four-peer gates and grouped SDK corpus parity are source-bound (fixture_sha256=${grouped_fixture_sha256}, suite_source_manifest_sha256=${grouped_suite_source_manifest_sha256})"
