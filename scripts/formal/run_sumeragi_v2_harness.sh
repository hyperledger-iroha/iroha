#!/usr/bin/env bash
set -euo pipefail

# Run a Cargo command against the workspace-excluded formal harness without
# creating a lockfile in the production workspace. The copied authoritative
# reducer keeps the verification package's source-link relationship intact.
if (($# == 0)); then
  echo "usage: $0 [--unit|--fast-network|--model-replay|--chaos-100k|<command> [argument ...]]" >&2
  exit 2
fi

readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly PRODUCTION_CORE_DIR="${REPO_ROOT}/crates/iroha_core/src/sumeragi/v2_core"
verify_workspace="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-harness.XXXXXX")"
cleanup_paths=("$verify_workspace")
if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
  CARGO_TARGET_DIR="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-harness-target.XXXXXX")"
  cleanup_paths+=("$CARGO_TARGET_DIR")
  export CARGO_TARGET_DIR
fi
cleanup() {
  rm -rf -- "${cleanup_paths[@]}"
}
trap cleanup EXIT

mkdir -p \
  "$verify_workspace/crates" \
  "$verify_workspace/crates/iroha_core/src/sumeragi"
cp -R \
  "$REPO_ROOT/crates/iroha_sumeragi_core" \
  "$verify_workspace/crates/iroha_sumeragi_core"
cp -R \
  "$PRODUCTION_CORE_DIR" \
  "$verify_workspace/crates/iroha_core/src/sumeragi/v2_core"
cat >"$verify_workspace/Cargo.toml" <<'EOF'
[workspace]
members = ["crates/iroha_sumeragi_core"]
resolver = "2"

[workspace.package]
edition = "2024"
rust-version = "1.92"
version = "2.0.0-rc.2.0"
authors = ["Iroha 2 contributors"]
description = "Pure Sumeragi v2 verification workspace"
repository = "https://github.com/hyperledger-iroha/iroha"
documentation = "https://docs.iroha.tech"
homepage = "https://iroha.tech"
license = "Apache-2.0"
keywords = ["blockchain", "consensus"]
categories = ["algorithms"]

[workspace.lints.rust]
unsafe_code = "deny"
EOF

cd "$verify_workspace"
cargo generate-lockfile
case "$1" in
  --unit)
    if (($# != 1)); then
      echo "--unit accepts no additional arguments" >&2
      exit 2
    fi
    cargo test --locked --offline -p iroha_sumeragi_core \
      --lib -- --test-threads=1
    ;;
  --fast-network)
    if (($# != 1)); then
      echo "--fast-network accepts no additional arguments" >&2
      exit 2
    fi
    required_tests=(
      lossy_offline_leader_simulations_commit_for_4_7_and_10_validators
      two_by_two_partition_cannot_advance_but_healing_retransmits_tc_and_commits
      leader_crash_after_proposal_broadcast_does_not_block_the_remaining_quorum
      corrupted_chunks_and_withheld_commit_evidence_recover_by_bounded_retransmission
      crash_after_proposal_wal_before_signature_replays_exact_intent
      taira_divergent_views_converge_and_commit_within_one_rotation
      accelerated_chain_chaos_smoke_preserves_prefix
    )
    network_test_list="$(
      cargo test --locked --offline -p iroha_sumeragi_core \
        --test network_simulation -- --list
    )"
    for test_name in "${required_tests[@]}"; do
      if ! grep -Fqx "${test_name}: test" <<<"$network_test_list"; then
        echo "required Sumeragi v2 simulation is missing: ${test_name}" >&2
        exit 1
      fi
    done
    for test_name in "${required_tests[@]}"; do
      cargo test --locked --offline -p iroha_sumeragi_core \
        --test network_simulation "$test_name" \
        -- --exact --test-threads=1
    done
    ;;
  --chaos-100k)
    if (($# != 1)); then
      echo "--chaos-100k accepts no additional arguments" >&2
      exit 2
    fi
    cargo test --locked --offline -p iroha_sumeragi_core \
      --test network_simulation \
      accelerated_100_000_block_chaos_preserves_chain_prefix \
      -- --exact --ignored --nocapture
    ;;
  --model-replay)
    if (($# != 1)); then
      echo "--model-replay accepts no additional arguments" >&2
      exit 2
    fi
    cargo test --locked --offline -p iroha_sumeragi_core \
      --test model_trace_replay -- --test-threads=1
    ;;
  --*)
    echo "unknown harness mode: $1" >&2
    exit 2
    ;;
  *)
    "$@"
    ;;
esac
