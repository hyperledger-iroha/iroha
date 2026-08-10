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
  echo "Nexus cross-dataspace source manifest disagrees with the current workspace" >&2
  exit 1
fi
readonly source_manifest_sha256
export IROHA_RELEASE_SOURCE_MANIFEST_SHA256="$source_manifest_sha256"

require_source_manifest() {
  local checkpoint="$1"
  local observed
  observed="$(workspace_source_manifest)" || return 1
  if [[ "$observed" != "$source_manifest_sha256" ]]; then
    echo "workspace sources changed during Nexus cross-dataspace proof at ${checkpoint}" >&2
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
    mktemp -d /private/tmp/iroha-nexus-cross-dataspace-pr.XXXXXX
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
release_gate_boundary "nexus-cross-dataspace-pr:entry" || exit $?

# Build the four localnet programs once outside every Cargo test process, then
# require the launcher and test-network harness to reuse only that attested
# source-bound bundle.
unset TEST_NETWORK_BIN_IROHAD KAGAMI_BIN CARGO_BIN_EXE_irohad CARGO_BIN_EXE_kagami
unset TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL TEST_NETWORK_BIN_IROHA CARGO_BIN_EXE_iroha
unset TEST_NETWORK_IROHAD_FEATURES TEST_NETWORK_CARGO
export IROHA_TEST_REQUIRE_NETWORK=1
export IROHA_TEST_NETWORK_START_ATTEMPTS=1
export IROHA_TEST_SKIP_BUILD=1
export IROHA_TEST_ALLOW_REENTRANT_BUILD=0
export IROHA_TEST_BUILD_PROFILE=release
export PROFILE=release
export CARGO_NET_OFFLINE=true

release_gate_boundary "nexus-cross-dataspace-pr:prebuilt:before" || exit $?
sumeragi_v2_ensure_source_bound_localnet_binaries \
  "$REPO_ROOT" "$source_manifest_sha256"
sumeragi_v2_export_source_bound_localnet_binaries \
  "$REPO_ROOT" "$source_manifest_sha256"
release_gate_boundary "nexus-cross-dataspace-pr:prebuilt:after" || exit $?

evidence_dir="${NEXUS_CROSS_DATASPACE_EVIDENCE_DIR:-${IROHA_RELEASE_ARTIFACT_ROOT}/nexus-cross-dataspace}"
require_release_artifact_path "$evidence_dir" || exit $?

cross_dataspace_args=(
  --capture
  --evidence-dir "$evidence_dir"
)
require_source_manifest "before harness" || exit 1
release_gate_boundary "nexus-cross-dataspace-pr:harness:before" || exit $?
bash scripts/run_nexus_cross_dataspace_atomic_swap.sh "${cross_dataspace_args[@]}"
release_gate_boundary "nexus-cross-dataspace-pr:harness:after" || exit $?
require_source_manifest "after harness" || exit 1

echo "[nexus] strict 10/10 cross-dataspace localnet seed matrix passed"
