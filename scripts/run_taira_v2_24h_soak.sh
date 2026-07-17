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
readonly source_bound_root="${REPO_ROOT}/target/sumeragi-v2-release/${source_manifest_sha256}"
readonly evidence_root="${source_bound_root}/evidence/taira-v2-24h"
unset TEST_NETWORK_BIN_IROHAD KAGAMI_BIN CARGO_BIN_EXE_iroha3d CARGO_BIN_EXE_kagami
unset TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL TEST_NETWORK_BIN_IROHA CARGO_BIN_EXE_iroha
unset TEST_NETWORK_IROHAD_FEATURES TEST_NETWORK_CARGO
export IROHA_TEST_SKIP_BUILD=0
export IROHA_TEST_ALLOW_REENTRANT_BUILD=1
export IROHA_TEST_BUILD_TIMEOUT_MS=3600
export IROHA_TEST_BUILD_PROFILE=release
export PROFILE=release
export RUST_LOG=info
export CARGO_NET_OFFLINE=true
export CARGO_TARGET_DIR="${source_bound_root}/test-suite"
export IROHA_TEST_TARGET_DIR="${source_bound_root}/programs"
export IROHA_RELEASE_SOURCE_MANIFEST_SHA256="$source_manifest_sha256"

# A source digest intentionally selects one build/evidence root. Serialize the
# complete 24-hour run so two release jobs cannot overwrite that root's binary
# cache or retained evidence. A hard-killed run leaves the lock behind and must
# be inspected explicitly instead of being mistaken for a safe retry.
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
export IROHA_TAIRA_EVIDENCE_PATH="$partial_evidence_path"
run_log=""
cleanup() {
  local status=$?
  if [[ -n "$run_log" ]]; then
    rm -f -- "$run_log"
  fi
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
  cargo test --locked --offline --release -p integration_tests \
    --test consensus_and_da -- --list --ignored
)"
inventory_count="$(grep -Fxc "${TAIRA_SOAK_TEST}: test" <<<"$ignored_inventory" || true)"
if [[ "$inventory_count" != 1 ]]; then
  echo "expected exactly one ignored Taira soak named ${TAIRA_SOAK_TEST}; found ${inventory_count}" >&2
  exit 1
fi

run_log="$(mktemp "${TMPDIR:-/tmp}/taira-v2-production-soak.XXXXXX")"

set +e
cargo test --locked --offline --release -p integration_tests --test consensus_and_da \
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

mv -- "$partial_evidence_path" "$evidence_path"
if command -v sha256sum >/dev/null 2>&1; then
  evidence_sha256="$(sha256sum "$evidence_path" | awk '{print $1}')"
else
  evidence_sha256="$(shasum -a 256 "$evidence_path" | awk '{print $1}')"
fi
completion_tmp="${invocation_dir}/.COMPLETED.tsv.$$"
printf '%s\t%s\n' \
  schema_version 1 \
  head_commit "$head_commit" \
  head_tree "$head_tree" \
  source_manifest_sha256 "$source_manifest_sha256" \
  cargo_lock_sha256 "$cargo_lock_sha256" \
  evidence_sha256 "$evidence_sha256" \
  >"$completion_tmp"
mv -- "$completion_tmp" "$completion_attestation"

post_completion_manifest="$(
  python3 scripts/compute_workspace_source_manifest.py --root "$REPO_ROOT"
)"
if [[ "$post_completion_manifest" != "$source_manifest_sha256" ]]; then
  rm -f -- "$completion_attestation" "$evidence_path"
  echo "workspace sources changed while publishing Taira completion evidence" >&2
  exit 1
fi
if [[ -n "${IROHA_RELEASE_EXPECTED_IDENTITY_PATH:-}" ]]; then
  post_completion_identity="$(
    python3 scripts/compute_workspace_source_manifest.py \
      --root "$REPO_ROOT" --release-identity-json
  )"
  if [[ "$post_completion_identity" != "$expected_identity" ]]; then
    rm -f -- "$completion_attestation" "$evidence_path"
    echo "release identity changed while publishing Taira completion evidence" >&2
    exit 1
  fi
fi

if [[ -n "${IROHA_TAIRA_COMPLETION_PATH_FILE:-}" ]]; then
  completion_path_tmp="${IROHA_TAIRA_COMPLETION_PATH_FILE}.$$"
  printf '%s\n' "$completion_attestation" >"$completion_path_tmp"
  mv -- "$completion_path_tmp" "$IROHA_TAIRA_COMPLETION_PATH_FILE"
fi

echo "Taira v2 production soak passed with exactly one test; retained evidence=${evidence_path}; completion=${completion_attestation}" >&2
