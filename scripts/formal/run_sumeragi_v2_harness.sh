#!/usr/bin/env bash
set -euo pipefail

# Run a Cargo command against the workspace-excluded formal harness without
# creating a lockfile in the production workspace. The copied authoritative
# reducer keeps the verification package's source-link relationship intact.
if (($# == 0)); then
  echo "usage: $0 [--fetch|--unit|--fast-network|--model-replay|--chaos-100k|<command> [argument ...]]" >&2
  exit 2
fi

readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly PRODUCTION_CORE_DIR="${REPO_ROOT}/crates/iroha_core/src/sumeragi/v2_core"
readonly HARNESS_LOCK="${REPO_ROOT}/scripts/formal/sumeragi_v2_harness.lock"
readonly HARNESS_LOCK_SHA256="9c49a60551d9f66c8786f2497cb107fb3214fb3420c4f5c23ba3d24814b3f97e"

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

if [[ ! -f "$HARNESS_LOCK" || -L "$HARNESS_LOCK" \
  || "$(hash_file "$HARNESS_LOCK")" != "$HARNESS_LOCK_SHA256" ]]; then
  echo "pinned Sumeragi v2 harness lock is missing or has the wrong digest" >&2
  exit 1
fi
if [[ "$1" == "--fetch" ]]; then
  export CARGO_NET_OFFLINE=false
else
  export CARGO_NET_OFFLINE=true
fi
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
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(verus_only)'] }
EOF

cd "$verify_workspace"
cp -- "$HARNESS_LOCK" Cargo.lock
case "$1" in
  --fetch)
    if (($# != 1)); then
      echo "--fetch accepts no additional arguments" >&2
      exit 2
    fi
    # Populate a cold Cargo home from the checksum-pinned standalone lock.
    # All verification/test modes remain offline and consume this exact graph.
    cargo fetch --locked
    ;;
  --unit)
    if (($# != 1)); then
      echo "--unit accepts no additional arguments" >&2
      exit 2
    fi
    unit_test_list="$(
      cargo test --locked --offline -p iroha_sumeragi_core \
        --lib -- --list
    )"
    listed_unit_tests=()
    while IFS= read -r test_name; do
      [[ -n "$test_name" ]] && listed_unit_tests+=("$test_name")
    done < <(sed -n 's/: test$//p' <<<"$unit_test_list")
    if ((${#listed_unit_tests[@]} != 105)); then
      printf '%s\n' "${listed_unit_tests[@]}" >&2
      echo "expected exactly 105 Sumeragi v2 reducer unit tests" >&2
      exit 1
    fi
    unit_ignored_test_list="$(
      cargo test --locked --offline -p iroha_sumeragi_core \
        --lib -- --list --ignored
    )"
    listed_ignored_unit_tests=()
    while IFS= read -r test_name; do
      [[ -n "$test_name" ]] && listed_ignored_unit_tests+=("$test_name")
    done < <(sed -n 's/: test$//p' <<<"$unit_ignored_test_list")
    if ((${#listed_ignored_unit_tests[@]} != 0)); then
      printf '%s\n' "${listed_ignored_unit_tests[@]}" >&2
      echo "reducer unit gate requires all 105 tests to be runnable" >&2
      exit 1
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
      asymmetric_partition_stalls_without_dual_quorum_then_heals_and_applies
      leader_crash_after_proposal_broadcast_does_not_block_the_remaining_quorum
      leader_crash_with_a_locked_body_rotates_and_rebuilds_the_old_commit_quorum
      corrupted_chunks_and_withheld_commit_evidence_recover_by_bounded_retransmission
      crash_after_proposal_wal_before_signature_replays_exact_intent
      taira_divergent_views_converge_and_commit_within_one_rotation
      accelerated_chain_chaos_smoke_preserves_prefix
    )
    ignored_test="accelerated_100_000_block_chaos_preserves_chain_prefix"
    network_test_list="$(
      cargo test --locked --offline -p iroha_sumeragi_core \
        --test network_simulation -- --list
    )"
    listed_tests=()
    while IFS= read -r test_name; do
      [[ -n "$test_name" ]] && listed_tests+=("$test_name")
    done < <(sed -n 's/: test$//p' <<<"$network_test_list")
    if ((${#listed_tests[@]} != ${#required_tests[@]} + 1)); then
      printf '%s\n' "${listed_tests[@]}" >&2
      echo "expected exactly nine fast and one ignored Sumeragi v2 simulations" >&2
      exit 1
    fi
    for test_name in "${listed_tests[@]}"; do
      found=false
      for required_test in "${required_tests[@]}" "$ignored_test"; do
        if [[ "$test_name" == "$required_test" ]]; then
          found=true
          break
        fi
      done
      if [[ "$found" != true ]]; then
        echo "unexpected Sumeragi v2 simulation in inventory: ${test_name}" >&2
        exit 1
      fi
    done
    ignored_test_list="$(
      cargo test --locked --offline -p iroha_sumeragi_core \
        --test network_simulation -- --list --ignored
    )"
    listed_ignored_tests=()
    while IFS= read -r test_name; do
      [[ -n "$test_name" ]] && listed_ignored_tests+=("$test_name")
    done < <(sed -n 's/: test$//p' <<<"$ignored_test_list")
    if ((${#listed_ignored_tests[@]} != 1)) \
      || [[ "${listed_ignored_tests[0]:-}" != "$ignored_test" ]]; then
      printf '%s\n' "${listed_ignored_tests[@]}" >&2
      echo "expected only ${ignored_test} to be ignored" >&2
      exit 1
    fi
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
    readonly ignored_test="accelerated_100_000_block_chaos_preserves_chain_prefix"
    ignored_test_list="$(
      cargo test --locked --offline -p iroha_sumeragi_core \
        --test network_simulation -- --list --ignored
    )"
    ignored_count="$(grep -Fxc "${ignored_test}: test" <<<"${ignored_test_list}" || true)"
    if [[ "${ignored_count}" != 1 ]]; then
      echo "expected exactly one ignored chaos test named ${ignored_test}; found ${ignored_count}" >&2
      exit 1
    fi
    cargo test --locked --offline -p iroha_sumeragi_core \
      --test network_simulation \
      "$ignored_test" \
      -- --exact --ignored --nocapture
    ;;
  --model-replay)
    if (($# != 1)); then
      echo "--model-replay accepts no additional arguments" >&2
      exit 2
    fi
    required_replay_tests=(
      tlc_liveness_witness_replays_against_the_production_reducer
      identical_commit_envelope_stutters_before_lock_and_is_admitted_after_persistence
      malformed_and_unsafe_normalized_traces_fail_closed
      crash_replay_rejects_stale_completion_and_resumes_exact_intent
      unsafe_certificate_and_vote_equivocation_do_not_decide
      invalid_body_never_authorizes_prepare_or_decision
      overlapping_timeout_groups_are_rejected_transactionally
      timeout_equivocation_with_different_full_high_qcs_is_reported
    )
    model_replay_test_list="$(
      cargo test --locked --offline -p iroha_sumeragi_core \
        --test model_trace_replay -- --list
    )"
    listed_replay_tests=()
    while IFS= read -r test_name; do
      [[ -n "$test_name" ]] && listed_replay_tests+=("$test_name")
    done < <(sed -n 's/: test$//p' <<<"$model_replay_test_list")
    if ((${#listed_replay_tests[@]} != ${#required_replay_tests[@]})); then
      printf '%s\n' "${listed_replay_tests[@]}" >&2
      echo "expected exactly eight Sumeragi v2 model-replay tests" >&2
      exit 1
    fi
    for test_name in "${listed_replay_tests[@]}"; do
      found=false
      for required_test in "${required_replay_tests[@]}"; do
        if [[ "$test_name" == "$required_test" ]]; then
          found=true
          break
        fi
      done
      if [[ "$found" != true ]]; then
        echo "unexpected Sumeragi v2 model-replay test in inventory: ${test_name}" >&2
        exit 1
      fi
    done
    replay_ignored_test_list="$(
      cargo test --locked --offline -p iroha_sumeragi_core \
        --test model_trace_replay -- --list --ignored
    )"
    listed_ignored_replay_tests=()
    while IFS= read -r test_name; do
      [[ -n "$test_name" ]] && listed_ignored_replay_tests+=("$test_name")
    done < <(sed -n 's/: test$//p' <<<"$replay_ignored_test_list")
    if ((${#listed_ignored_replay_tests[@]} != 0)); then
      printf '%s\n' "${listed_ignored_replay_tests[@]}" >&2
      echo "model-replay gate requires all eight tests to be runnable" >&2
      exit 1
    fi
    cargo test --locked --offline -p iroha_sumeragi_core \
      --test model_trace_replay -- --test-threads=1
    ;;
  --*)
    echo "unknown harness mode: $1" >&2
    exit 2
    ;;
  *)
    if [[ "$1" == cargo ]]; then
      command_line=" $* "
      if [[ "$command_line" != *" --locked "* \
        || "$command_line" != *" --offline "* ]]; then
        echo "custom Cargo harness commands require --locked --offline" >&2
        exit 2
      fi
    fi
    "$@"
    ;;
esac
