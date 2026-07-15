#!/usr/bin/env bash
# Execute the source-bound Sumeragi v2 PR or production-release corridor.

set -euo pipefail

profile="${1:---pr}"
if [[ $# -gt 1 ]] || [[ "$profile" != "--pr" && "$profile" != "--release" ]]; then
  echo "usage: $0 [--pr|--release]" >&2
  exit 2
fi

readonly repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
# Every real-network leg in this parent shell must fail rather than translate a
# socket/sandbox denial into a successful developer skip.
export IROHA_TEST_REQUIRE_NETWORK=1
unset TEST_NETWORK_BIN_IROHAD KAGAMI_BIN CARGO_BIN_EXE_iroha3d CARGO_BIN_EXE_kagami
unset TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL TEST_NETWORK_BIN_IROHA CARGO_BIN_EXE_iroha
unset TEST_NETWORK_IROHAD_FEATURES TEST_NETWORK_CARGO
unset IROHA_TEST_SKIP_BUILD IROHA_TEST_ALLOW_REENTRANT_BUILD
unset IROHA_TEST_TARGET_DIR IROHA_TEST_BUILD_PROFILE IROHA_TEST_BUILD_TIMEOUT_MS PROFILE
unset TLAPM_BIN TLAPM_STDLIB TLA2TOOLS_JAR

release_source_manifest_sha256=""
if [[ "$profile" == "--release" ]]; then
  release_source_manifest_sha256="$(python3 scripts/compute_workspace_source_manifest.py)"
  if [[ ! "$release_source_manifest_sha256" =~ ^[0-9a-f]{64}$ ]]; then
    echo "workspace source manifest helper returned an invalid digest" >&2
    exit 1
  fi
  readonly release_source_bound_root="${repo_root}/target/sumeragi-v2-release/${release_source_manifest_sha256}"
  export CARGO_TARGET_DIR="${release_source_bound_root}/test-suite"
  export IROHA_TEST_TARGET_DIR="${release_source_bound_root}/programs"
  export IROHA_TEST_SKIP_BUILD=0
  export IROHA_TEST_ALLOW_REENTRANT_BUILD=1
  export IROHA_TEST_BUILD_TIMEOUT_MS=3600
fi

bash scripts/run_sumeragi_v2_seed_matrix.sh "$profile"

if [[ "$profile" == "--pr" ]]; then
  python3 scripts/formal/check_sumeragi_v2_proof_ledger.py
  bash scripts/formal/run_sumeragi_v2_harness.sh --unit
  bash scripts/formal/run_sumeragi_v2_harness.sh --fast-network
  bash scripts/formal/run_sumeragi_v2_harness.sh --model-replay
  echo "Sumeragi v2 PR gate passed: 4 seeds × 4 scenarios (16 runs), reducer invariants, adversarial simulations, and trace replay" >&2
  exit 0
fi

bash ci/check_sumeragi_formal.sh
bash scripts/formal/run_sumeragi_v2_harness.sh --chaos-100k
bash scripts/run_taira_v2_24h_soak.sh

final_release_source_manifest_sha256="$(python3 scripts/compute_workspace_source_manifest.py)"
if [[ "$final_release_source_manifest_sha256" != "$release_source_manifest_sha256" ]]; then
  echo "workspace sources changed during the production release corridor" >&2
  exit 1
fi
# Revalidate the deductive evidence after every long-running gate so a TLA+
# edit during chaos or soak execution cannot inherit stale TLAPS success.
python3 scripts/formal/check_sumeragi_v2_proof_ledger.py \
  --release \
  --evidence target/formal/sumeragi_v2/proof_evidence.json

echo "Sumeragi v2 production release gates passed, including 100,000 heights and the 24-hour Taira soak" >&2
