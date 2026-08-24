#!/usr/bin/env bash
set -euo pipefail

readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$REPO_ROOT"

bash ci/check_sumeragi_v2_multilane_release_inventory.sh
source "${REPO_ROOT}/scripts/sumeragi_v2_release_process_policy.sh"
source "${REPO_ROOT}/scripts/sumeragi_v2_prebuilt_bundle.sh"

workspace_source_manifest() {
  python3 -I -S scripts/compute_workspace_source_manifest.py --root "$REPO_ROOT"
}

observed_source_manifest_sha256="$(workspace_source_manifest)"
if [[ ! "$observed_source_manifest_sha256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "workspace source manifest helper returned an invalid digest" >&2
  exit 1
fi
source_manifest_sha256="${IROHA_RELEASE_SOURCE_MANIFEST_SHA256:-$observed_source_manifest_sha256}"
if [[ ! "$source_manifest_sha256" =~ ^[0-9a-f]{64}$ \
  || "$source_manifest_sha256" != "$observed_source_manifest_sha256" ]]; then
  echo "Nexus cross-lane source manifest disagrees with the current workspace" >&2
  exit 1
fi
readonly source_manifest_sha256
export IROHA_RELEASE_SOURCE_MANIFEST_SHA256="$source_manifest_sha256"

require_source_manifest() {
  local checkpoint="$1"
  local observed
  observed="$(workspace_source_manifest)" || return 1
  if [[ "$observed" != "$source_manifest_sha256" ]]; then
    echo "workspace sources changed during Nexus cross-lane proof at ${checkpoint}" >&2
    return 1
  fi
}

release_root_input_count=0
for release_root_name in \
  CARGO_TARGET_DIR \
  IROHA_RELEASE_ARTIFACT_ROOT \
  IROHA_RELEASE_CANCEL_REQUEST_PATH; do
  if [[ -n "${!release_root_name:-}" ]]; then
    release_root_input_count=$((release_root_input_count + 1))
  fi
done
if ((release_root_input_count != 0 && release_root_input_count != 3)); then
  echo "CARGO_TARGET_DIR, IROHA_RELEASE_ARTIFACT_ROOT, and IROHA_RELEASE_CANCEL_REQUEST_PATH must be supplied all-or-none" >&2
  exit 2
fi
if ((release_root_input_count == 0)); then
  nexus_invocation_root="$(
    mktemp -d /private/tmp/iroha-nexus-cross-lane-pr.XXXXXX
  )"
  mkdir -m 0700 -- \
    "$nexus_invocation_root/target" \
    "$nexus_invocation_root/artifacts"
  export CARGO_TARGET_DIR="$nexus_invocation_root/target"
  export IROHA_RELEASE_ARTIFACT_ROOT="$nexus_invocation_root/artifacts"
  export IROHA_RELEASE_CANCEL_REQUEST_PATH="$nexus_invocation_root/cancel-request.json"
fi
require_external_cargo_target_dir "$REPO_ROOT"
require_external_release_artifact_root "$REPO_ROOT"
require_disjoint_release_roots "$REPO_ROOT"
release_gate_boundary "nexus-cross-lane-pr:entry" || exit $?

unset TEST_NETWORK_BIN_IROHAD KAGAMI_BIN CARGO_BIN_EXE_iroha3d CARGO_BIN_EXE_kagami
unset TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL TEST_NETWORK_BIN_IROHA CARGO_BIN_EXE_iroha
unset TEST_NETWORK_IROHAD_FEATURES TEST_NETWORK_CARGO
export NORITO_SKIP_BINDINGS_SYNC=1
export CARGO_NET_OFFLINE=true
export IROHA_TEST_REQUIRE_NETWORK=1
export IROHA_TEST_NETWORK_START_ATTEMPTS=1
export IROHA_TEST_SKIP_BUILD=1
export IROHA_TEST_BUILD_PROFILE=release
export PROFILE=release

release_gate_boundary "nexus-cross-lane-pr:prebuilt:before" || exit $?
sumeragi_v2_ensure_source_bound_localnet_binaries \
  "$REPO_ROOT" "$source_manifest_sha256"
sumeragi_v2_export_source_bound_localnet_binaries \
  "$REPO_ROOT" "$source_manifest_sha256"
release_gate_boundary "nexus-cross-lane-pr:prebuilt:after" || exit $?

require_source_manifest "before harness" || exit 1
release_gate_boundary "nexus-cross-lane-pr:harness:before" || exit $?
run_cargo test --locked --offline -p iroha --lib \
  batch_verification_ -- --nocapture
run_cargo test --locked --offline -p iroha --lib \
  get_sumeragi_status_wire_rejects_ -- --nocapture
run_cargo test --locked --offline -p iroha --lib \
  get_cross_lane_transfer_proofs_ -- --nocapture
IROHA_FAIL_ON_SANDBOX_SKIP=1 \
  run_cargo test --locked --offline -p integration_tests \
    --test sumeragi_localnet_smoke \
    sumeragi_status_json_endpoint_decodes_to_wire_end_to_end \
    -- --nocapture --test-threads=1
release_gate_boundary "nexus-cross-lane-pr:harness:after" || exit $?
require_source_manifest "after harness" || exit 1

echo "[nexus] cross-lane proof filters passed"
