#!/usr/bin/env bash
set -euo pipefail

readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly TAIRA_SOAK_TEST="taira_public_localnet::taira_profile_24h_packet_impairment_and_restart_soak"

usage() {
  cat <<'USAGE'
Usage: scripts/run_taira_v2_24h_soak.sh [--help]

Run the ignored four-validator Taira-profile Sumeragi v2 production soak.
The acceptance profile is fixed at 24 hours, 10% deterministic inbound and
outbound packet loss, 5 TPS, and process/membership churn every five minutes.
Profile overrides are intentionally unsupported so every successful run is
comparable release evidence. The runner uses release-profile binaries and
offline Cargo resolution; fetch the locked dependencies before launching it.
USAGE
}

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

source "${REPO_ROOT}/scripts/sumeragi_v2_release_process_policy.sh"

source "${REPO_ROOT}/scripts/sumeragi_v2_prebuilt_bundle.sh"

localnet_binary_attestation_valid() {
  sumeragi_v2_localnet_binary_attestation_valid \
    "$REPO_ROOT" "$source_manifest_sha256"
}

ensure_source_bound_localnet_binaries() {
  sumeragi_v2_ensure_source_bound_localnet_binaries \
    "$REPO_ROOT" "$source_manifest_sha256"
}

export_source_bound_localnet_binaries() {
  sumeragi_v2_export_source_bound_localnet_binaries \
    "$REPO_ROOT" "$source_manifest_sha256"
}

if (($#)); then
  if (($# == 1)) && [[ "$1" == "--help" || "$1" == "-h" ]]; then
    usage
    exit 0
  fi
  echo "the Taira production-soak profile is fixed; profile overrides are not supported" >&2
  usage >&2
  exit 2
fi

# Keep this complete profile synchronized with the production-release corridor.
# Assign every value explicitly so inherited developer settings cannot weaken the
# run or translate an unavailable network into a successful skip.
export IROHA_TEST_REQUIRE_NETWORK=1
export IROHA_TAIRA_SIM_DURATION_SECS=86400
export IROHA_TAIRA_SIM_SEED=taira-public-sim
export IROHA_TAIRA_LOAD_TPS=5
export IROHA_TAIRA_PACKET_LOSS_PERCENT=10
export IROHA_TAIRA_CHURN_INTERVAL_SECS=300
export IROHA_TAIRA_MAX_HEIGHT_SKEW=2
export IROHA_TAIRA_MAX_HEIGHT_SKEW_GRACE_SECS=30
export IROHA_TAIRA_MAX_TRANSIENT_HEIGHT_SKEW=32
export IROHA_TAIRA_STALL_TIMEOUT_SECS=300
export IROHA_TAIRA_MAX_VIEW_CHANGE_RATE=0.2
export IROHA_TAIRA_MAX_LAGGED_CYCLE_RATIO=0.35
export IROHA_TAIRA_MIN_COMMITTED_TPS_RATIO=0.6
export IROHA_TAIRA_KEEP_LOCALNET=1

cd "$REPO_ROOT"

# Bind every compiled and retained artifact to the complete tracked/untracked
# checkout state plus the ignored workspace Cargo.lock. The helper also rejects
# unresolved entries and active Git operations. A content-addressed target
# prevents a stale candidate from a different source tree from satisfying
# binary discovery.
observed_source_manifest_sha256="$(
  python3 scripts/compute_workspace_source_manifest.py --root "$REPO_ROOT"
)"
if [[ ! "$observed_source_manifest_sha256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "workspace source manifest helper returned an invalid digest" >&2
  exit 1
fi
source_manifest_sha256="${IROHA_RELEASE_SOURCE_MANIFEST_SHA256:-$observed_source_manifest_sha256}"
if [[ ! "$source_manifest_sha256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "IROHA_RELEASE_SOURCE_MANIFEST_SHA256 must be a lowercase SHA-256 digest" >&2
  exit 1
fi
readonly source_manifest_sha256
if [[ "$observed_source_manifest_sha256" != "$source_manifest_sha256" ]]; then
  echo "Taira soak source manifest does not match the parent release invocation: expected ${source_manifest_sha256}, observed ${observed_source_manifest_sha256}" >&2
  exit 1
fi
readonly head_commit="${IROHA_RELEASE_HEAD_COMMIT:-}"
readonly head_tree="${IROHA_RELEASE_HEAD_TREE:-}"
readonly cargo_lock_sha256="${IROHA_RELEASE_CARGO_LOCK_SHA256:-}"
if [[ ! "$head_commit" =~ ^([0-9a-f]{40}|[0-9a-f]{64})$ \
  || ! "$head_tree" =~ ^([0-9a-f]{40}|[0-9a-f]{64})$ \
  || ! "$cargo_lock_sha256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "Taira release soak requires exact parent HEAD, tree, and Cargo.lock identities" >&2
  exit 1
fi
if [[ -z "${CARGO_TARGET_DIR:-}" \
  && -z "${IROHA_RELEASE_ARTIFACT_ROOT:-}" \
  && -z "${IROHA_RELEASE_CANCEL_REQUEST_PATH:-}" ]]; then
  taira_invocation_root="$(
    mktemp -d /private/tmp/iroha-sumeragi-v2-taira.XXXXXX
  )"
  mkdir -m 0700 -- \
    "$taira_invocation_root/target" \
    "$taira_invocation_root/artifacts"
  export CARGO_TARGET_DIR="$taira_invocation_root/target"
  export IROHA_RELEASE_ARTIFACT_ROOT="$taira_invocation_root/artifacts"
  export IROHA_RELEASE_CANCEL_REQUEST_PATH="$taira_invocation_root/cancel-request.json"
elif [[ -z "${CARGO_TARGET_DIR:-}" \
  || -z "${IROHA_RELEASE_ARTIFACT_ROOT:-}" \
  || -z "${IROHA_RELEASE_CANCEL_REQUEST_PATH:-}" ]]; then
  echo "Taira requires CARGO_TARGET_DIR, IROHA_RELEASE_ARTIFACT_ROOT, and IROHA_RELEASE_CANCEL_REQUEST_PATH together" >&2
  exit 2
fi
require_external_cargo_target_dir "$REPO_ROOT"
require_external_release_artifact_root "$REPO_ROOT"
require_disjoint_release_roots "$REPO_ROOT"
release_gate_boundary "taira:entry" || exit $?
readonly source_bound_root="${IROHA_RELEASE_ARTIFACT_ROOT}/sumeragi-v2-release/${source_manifest_sha256}"
readonly evidence_root="${source_bound_root}/evidence/taira-v2-24h"
unset TEST_NETWORK_BIN_IROHAD KAGAMI_BIN CARGO_BIN_EXE_iroha3d CARGO_BIN_EXE_kagami
unset TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL TEST_NETWORK_BIN_IROHA CARGO_BIN_EXE_iroha
unset TEST_NETWORK_IROHAD_FEATURES TEST_NETWORK_CARGO
export IROHA_TEST_SKIP_BUILD=1
export IROHA_TEST_ALLOW_REENTRANT_BUILD=0
export IROHA_TEST_BUILD_TIMEOUT_MS=3600
export IROHA_TEST_BUILD_PROFILE=release
export PROFILE=release
export RUST_LOG=info
export CARGO_NET_OFFLINE=true
export IROHA_RELEASE_SOURCE_MANIFEST_SHA256="$source_manifest_sha256"
release_gate_boundary "taira:before-prebuilt-bundle" || exit $?
ensure_source_bound_localnet_binaries
export_source_bound_localnet_binaries
release_gate_boundary "taira:after-prebuilt-bundle" || exit $?

# A source digest intentionally selects one build/evidence root. Serialize the
# complete 24-hour run so two release jobs cannot overwrite retained evidence.
# An abnormally interrupted run leaves the lock behind and must be inspected
# explicitly instead of being mistaken for a safe retry.
mkdir -p -- "$evidence_root"
readonly soak_lock_path="${evidence_root}/.taira_v2_24h_soak.lock"
if ! mkdir -- "$soak_lock_path"; then
  echo "another Taira production soak owns ${soak_lock_path}; refusing shared release evidence" >&2
  exit 1
fi
invocation_dir="$(mktemp -d "${evidence_root}/invocation.XXXXXX")"
readonly invocation_dir
readonly evidence_path="${invocation_dir}/taira_v2_24h_soak.json"
readonly partial_evidence_path="${invocation_dir}/.taira_v2_24h_soak.partial.json"
readonly completion_attestation="${invocation_dir}/COMPLETED.tsv"
readonly run_log="${invocation_dir}/taira-v2-24h.log"
export IROHA_TAIRA_EVIDENCE_PATH="$partial_evidence_path"
cleanup() {
  local status=$?
  rm -f -- "$partial_evidence_path"
  if [[ ! -f "$completion_attestation" ]]; then
    rm -f -- "$evidence_path"
  fi
  rm -f -- "${soak_lock_path}/owner"
  if ! rmdir -- "$soak_lock_path"; then
    echo "failed to remove Taira production-soak lock ${soak_lock_path}" >&2
    status=1
  fi
  trap - EXIT
  exit "$status"
}
trap cleanup EXIT
printf 'pid=%s\nsource_manifest_sha256=%s\n' \
  "$$" "$source_manifest_sha256" >"${soak_lock_path}/owner"
rm -f -- "$partial_evidence_path" "$completion_attestation"

# Cargo's test filter succeeds when it selects zero tests. First require the
# exact ignored test to exist, then validate the executed libtest summary too so
# an inventory/execution race cannot become zero-test release evidence.
ignored_inventory="$(
  run_cargo test --locked --offline --release -p integration_tests \
    --test consensus_and_da -- --list --ignored
)"
inventory_count="$(grep -Fxc "${TAIRA_SOAK_TEST}: test" <<<"$ignored_inventory" || true)"
if [[ "$inventory_count" != 1 ]]; then
  echo "expected exactly one ignored Taira soak named ${TAIRA_SOAK_TEST}; found ${inventory_count}" >&2
  exit 1
fi

release_gate_boundary "taira:before-soak" || exit $?
set +e
run_cargo test --locked --offline --release -p integration_tests --test consensus_and_da \
  "$TAIRA_SOAK_TEST" -- \
  --exact --ignored --nocapture --test-threads=1 \
  2>&1 | tee "$run_log"
pipeline_status=("${PIPESTATUS[@]}")
set -e
if ((pipeline_status[0] != 0 || pipeline_status[1] != 0)); then
  echo "Taira production soak command failed (cargo=${pipeline_status[0]}, tee=${pipeline_status[1]})" >&2
  exit 1
fi
release_gate_boundary "taira:after-soak-natural-completion" || exit $?

running_total="$(grep -Ec '^running [0-9]+ tests?$' "$run_log" || true)"
running_one="$(grep -Fxc 'running 1 test' "$run_log" || true)"
result_total="$(grep -Ec '^test result:' "$run_log" || true)"
passing_one="$(
  grep -Ec '^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in .+$' \
    "$run_log" || true
)"
if [[ "$running_total" != 1 || "$running_one" != 1 \
  || "$result_total" != 1 || "$passing_one" != 1 ]] \
  || ! grep -Fq "test ${TAIRA_SOAK_TEST} " "$run_log"; then
  echo "expected exactly one Taira soak test to run and pass; refusing zero-test or ambiguous Cargo success" >&2
  exit 1
fi

if [[ ! -s "$partial_evidence_path" ]]; then
  echo "Taira soak passed without writing provisional release evidence at ${partial_evidence_path}" >&2
  exit 1
fi
python3 scripts/check_taira_v2_soak_evidence.py \
  "$partial_evidence_path" \
  --source-manifest "$source_manifest_sha256" \
  --build-root "$source_bound_root" \
  --repo-root "$REPO_ROOT"

final_source_manifest_sha256="$(
  python3 scripts/compute_workspace_source_manifest.py --root "$REPO_ROOT"
)"
if [[ "$final_source_manifest_sha256" != "$source_manifest_sha256" ]]; then
  echo "workspace sources changed during the Taira production soak" >&2
  exit 1
fi

if [[ -n "${IROHA_RELEASE_EXPECTED_IDENTITY_PATH:-}" ]]; then
  expected_identity="$(<"$IROHA_RELEASE_EXPECTED_IDENTITY_PATH")"
  observed_identity="$(
    python3 scripts/compute_workspace_source_manifest.py \
      --root "$REPO_ROOT" --release-identity-json
  )"
  if [[ "$observed_identity" != "$expected_identity" ]]; then
    echo "release source identity changed during the Taira production soak" >&2
    exit 1
  fi
fi

if ! localnet_binary_attestation_valid; then
  echo "source-bound localnet binary bundle changed before Taira completion" >&2
  exit 1
fi
release_gate_boundary "taira:before-evidence-publication" || exit $?
mv -- "$partial_evidence_path" "$evidence_path"
evidence_sha256="$(sha256_file "$evidence_path")"
log_sha256="$(sha256_file "$run_log")"

post_completion_manifest="$(
  python3 scripts/compute_workspace_source_manifest.py --root "$REPO_ROOT"
)"
if [[ "$post_completion_manifest" != "$source_manifest_sha256" ]]; then
  echo "workspace sources changed while publishing Taira completion evidence" >&2
  exit 1
fi
if [[ -n "${IROHA_RELEASE_EXPECTED_IDENTITY_PATH:-}" ]]; then
  post_completion_identity="$(
    python3 scripts/compute_workspace_source_manifest.py \
      --root "$REPO_ROOT" --release-identity-json
  )"
  if [[ "$post_completion_identity" != "$expected_identity" ]]; then
    echo "release identity changed while publishing Taira completion evidence" >&2
    exit 1
  fi
fi
if ! localnet_binary_attestation_valid; then
  echo "source-bound localnet binary bundle changed while publishing Taira completion" >&2
  exit 1
fi

completion_body="$(
  printf '%s\t%s\n' \
    schema_version 1 \
    head_commit "$head_commit" \
    head_tree "$head_tree" \
    source_manifest_sha256 "$source_manifest_sha256" \
    cargo_lock_sha256 "$cargo_lock_sha256" \
    prebuilt_manifest_sha256 "$IROHA_RELEASE_PREBUILT_MANIFEST_SHA256" \
    evidence_sha256 "$evidence_sha256" \
    log_sha256 "$log_sha256"
)"
marker_publish_args=(
  --output "$completion_attestation"
  --maximum-bytes 4096
)
if [[ -n "${IROHA_TAIRA_COMPLETION_PATH_FILE:-}" ]]; then
  marker_publish_args+=(
    --pointer "$IROHA_TAIRA_COMPLETION_PATH_FILE"
  )
fi
printf '%s\n' "$completion_body" |
  python3 -I -S "${REPO_ROOT}/scripts/publish_release_marker.py" \
    "${marker_publish_args[@]}"
release_gate_boundary "taira:after-evidence-publication" || exit $?

echo "Taira v2 production soak passed with exactly one test; retained evidence=${evidence_path}; completion=${completion_attestation}" >&2
