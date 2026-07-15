#!/usr/bin/env bash
set -euo pipefail

readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
readonly TAIRA_SOAK_TEST="taira_public_localnet::taira_profile_24h_packet_impairment_and_restart_soak"

usage() {
  cat <<'USAGE'
Usage: scripts/run_taira_v2_24h_soak.sh [--help]

Run the ignored four-validator Taira-profile Sumeragi v2 production soak.
The acceptance profile is fixed at 24 hours, 10% deterministic inbound and
outbound packet loss, 5 TPS, and process/membership churn every five minutes.
Profile overrides are intentionally unsupported so every successful run is
comparable release evidence.
USAGE
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
# checkout state. A content-addressed target also prevents a stale candidate
# from a different source tree from satisfying binary discovery.
source_manifest_sha256="$(python3 scripts/compute_workspace_source_manifest.py)"
readonly source_manifest_sha256
if [[ ! "$source_manifest_sha256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "workspace source manifest helper returned an invalid digest" >&2
  exit 1
fi
readonly source_bound_root="${REPO_ROOT}/target/sumeragi-v2-release/${source_manifest_sha256}"
readonly evidence_path="${source_bound_root}/evidence/taira_v2_24h_soak.json"
unset TEST_NETWORK_BIN_IROHAD KAGAMI_BIN CARGO_BIN_EXE_iroha3d CARGO_BIN_EXE_kagami
unset TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL TEST_NETWORK_BIN_IROHA CARGO_BIN_EXE_iroha
unset TEST_NETWORK_IROHAD_FEATURES TEST_NETWORK_CARGO PROFILE IROHA_TEST_BUILD_PROFILE
export IROHA_TEST_SKIP_BUILD=0
export IROHA_TEST_ALLOW_REENTRANT_BUILD=1
export IROHA_TEST_BUILD_TIMEOUT_MS=3600
export CARGO_TARGET_DIR="${source_bound_root}/test-suite"
export IROHA_TEST_TARGET_DIR="${source_bound_root}/programs"
export IROHA_RELEASE_SOURCE_MANIFEST_SHA256="$source_manifest_sha256"
export IROHA_TAIRA_EVIDENCE_PATH="$evidence_path"
rm -f -- "$evidence_path"

# Cargo's test filter succeeds when it selects zero tests. First require the
# exact ignored test to exist, then validate the executed libtest summary too so
# an inventory/execution race cannot become zero-test release evidence.
ignored_inventory="$(
  cargo test --locked -p integration_tests --test consensus_and_da -- --list --ignored
)"
inventory_count="$(grep -Fxc "${TAIRA_SOAK_TEST}: test" <<<"$ignored_inventory" || true)"
if [[ "$inventory_count" != 1 ]]; then
  echo "expected exactly one ignored Taira soak named ${TAIRA_SOAK_TEST}; found ${inventory_count}" >&2
  exit 1
fi

run_log="$(mktemp "${TMPDIR:-/tmp}/taira-v2-production-soak.XXXXXX")"
trap 'rm -f -- "$run_log"' EXIT

set +e
cargo test --locked -p integration_tests --test consensus_and_da \
  "$TAIRA_SOAK_TEST" -- \
  --exact --ignored --nocapture --test-threads=1 \
  2>&1 | tee "$run_log"
pipeline_status=("${PIPESTATUS[@]}")
set -e
if ((pipeline_status[0] != 0 || pipeline_status[1] != 0)); then
  echo "Taira production soak command failed (cargo=${pipeline_status[0]}, tee=${pipeline_status[1]})" >&2
  exit 1
fi

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

if [[ ! -s "$evidence_path" ]]; then
  echo "Taira soak passed without writing durable release evidence at ${evidence_path}" >&2
  exit 1
fi
python3 scripts/check_taira_v2_soak_evidence.py \
  "$evidence_path" \
  --source-manifest "$source_manifest_sha256" \
  --build-root "$source_bound_root" \
  --repo-root "$REPO_ROOT"

final_source_manifest_sha256="$(python3 scripts/compute_workspace_source_manifest.py)"
if [[ "$final_source_manifest_sha256" != "$source_manifest_sha256" ]]; then
  echo "workspace sources changed during the Taira production soak" >&2
  exit 1
fi

echo "Taira v2 production soak passed with exactly one test; durable evidence=${evidence_path}" >&2
