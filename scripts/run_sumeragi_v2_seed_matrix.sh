#!/usr/bin/env bash
# Run the four-validator Sumeragi v2 release scenarios over a fixed seed corpus.

set -euo pipefail

seed_count=4
profile="pr"
if [[ "${1:-}" == "--release" ]]; then
  seed_count=32
  profile="release"
  shift
elif [[ "${1:-}" == "--pr" ]]; then
  shift
fi

if [[ $# -ne 0 ]]; then
  echo "usage: $0 [--pr|--release]" >&2
  exit 2
fi

readonly repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
readonly target="sumeragi_v2_runner_isolated"
readonly module="sumeragi_v2_runner"
# A release/PR corridor must fail if the real four-peer network cannot start;
# sandbox skips are useful for ad-hoc developer runs but are not gate evidence.
export IROHA_TEST_REQUIRE_NETWORK=1
# A successful seed must represent one fresh network, not a later harness
# retry which hides a protocol-induced startup stall.
export IROHA_TEST_NETWORK_START_ATTEMPTS=1
readonly scenarios=(
  "authoritative_v2_genesis_commits_on_every_validator|authoritative_v2_genesis_commits_on_every_validator|normal"
  "authoritative_v2_finalizes_through_validator_restart|authoritative_v2_finalizes_through_validator_restart|normal"
  "taira_npos_leader_timeout_commits_within_rotation_bound|taira_npos_leader_timeout_commits_within_rotation_bound|normal"
  "real_network_divergent_prepare_qcs_converge_after_ordered_release|real_network_divergent_prepare_qcs_converge_after_ordered_release|normal"
)

# Keep command output after the process exits so a failed gate can be audited.
# Callers may isolate evidence (for example, mocked launcher-contract tests) by
# overriding this path; the default remains an untracked target directory.
evidence_dir="${SUMERAGI_V2_SEED_MATRIX_EVIDENCE_DIR:-${repo_root}/target/sumeragi-v2-seed-matrix/${profile}}"
mkdir -p -- "$evidence_dir/runs" "$evidence_dir/localnets"
evidence_dir="$(cd -- "$evidence_dir" && pwd -P)"
readonly evidence_dir
readonly run_log_dir="${evidence_dir}/runs"
readonly summary="${evidence_dir}/summary.tsv"
readonly inventory_log="${evidence_dir}/test-inventory.log"
readonly ignored_inventory_log="${evidence_dir}/ignored-test-inventory.log"
printf '%s\n' $'profile\tscenario\tseed\tresult\tcargo_status\ttee_status\toutput\tlocalnet\tcommand' >"$summary"
echo "seed-matrix command evidence: ${summary}" >&2

# `cargo test <filter>` exits successfully when the filter matches no tests.
# Pin the inventory first so a rename, cfg exclusion, or accidental `#[ignore]`
# cannot turn the real-network corridor into zero-test success.
set +e
cargo test --locked -p integration_tests --test "${target}" -- --list \
  >"$inventory_log" 2>&1
inventory_status=$?
cargo test --locked -p integration_tests --test "${target}" -- --list --ignored \
  >"$ignored_inventory_log" 2>&1
ignored_inventory_status=$?
set -e
if ((inventory_status != 0 || ignored_inventory_status != 0)); then
  echo "seed-matrix inventory failed (inventory=${inventory_status}, ignored=${ignored_inventory_status}); output: ${inventory_log}, ${ignored_inventory_log}" >&2
  exit 1
fi
test_inventory="$(<"$inventory_log")"
ignored_test_inventory="$(<"$ignored_inventory_log")"
for scenario_spec in "${scenarios[@]}"; do
  IFS='|' read -r test_name _ ignored <<<"${scenario_spec}"
  full_test_name="${module}::${test_name}"
  inventory_count="$(grep -Fxc "${full_test_name}: test" <<<"${test_inventory}" || true)"
  ignored_count="$(grep -Fxc "${full_test_name}: test" <<<"${ignored_test_inventory}" || true)"
  if [[ "${inventory_count}" != 1 ]]; then
    echo "expected exactly one release test named ${full_test_name}; found ${inventory_count}" >&2
    exit 1
  fi
  if [[ "${ignored}" == "ignored" && "${ignored_count}" != 1 ]] \
    || [[ "${ignored}" != "ignored" && "${ignored_count}" != 0 ]]; then
    echo "release test ignore state disagrees with the seed-matrix declaration: ${full_test_name}" >&2
    exit 1
  fi
done

run_index=0

for scenario_spec in "${scenarios[@]}"; do
  IFS='|' read -r test_name base_seed ignored <<<"${scenario_spec}"
  for ((seed_index = 0; seed_index < seed_count; seed_index++)); do
    seed="${base_seed}"
    if ((seed_index > 0)); then
      printf -v suffix '%02d' "${seed_index}"
      seed="${base_seed}:seed:${suffix}"
    fi
    export IROHA_TEST_NETWORK_BASE_SEED="${seed}"
    echo "running ${test_name} with seed ${seed} ($((seed_index + 1))/${seed_count})" >&2
    test_args=(--exact --nocapture --test-threads=1)
    if [[ "${ignored}" == "ignored" ]]; then
      test_args+=(--ignored)
    fi
    printf -v run_log '%s/run-%03d.log' "$run_log_dir" "$run_index"
    run_output="runs/run-$(printf '%03d' "$run_index").log"
    localnet_output="localnets/run-$(printf '%03d' "$run_index")"
    localnet_dir="${evidence_dir}/${localnet_output}"
    rm -rf -- "$localnet_dir"
    mkdir -p -- "$localnet_dir"
    command="IROHA_TEST_NETWORK_BASE_SEED=${seed} TEST_NETWORK_TMP_DIR=${localnet_dir} IROHA_TEST_NETWORK_KEEP_DIRS=1 cargo test --locked -p integration_tests --test ${target} ${module}::${test_name} -- ${test_args[*]}"
    ((run_index += 1))

    set +e
    TEST_NETWORK_TMP_DIR="$localnet_dir" IROHA_TEST_NETWORK_KEEP_DIRS=1 \
      cargo test --locked -p integration_tests --test "${target}" \
      "${module}::${test_name}" -- "${test_args[@]}" \
      2>&1 | tee "$run_log"
    pipeline_status=("${PIPESTATUS[@]}")
    set -e
    if ((pipeline_status[0] != 0 || pipeline_status[1] != 0)); then
      printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$profile" "$test_name" "$seed" "command_failed" \
        "${pipeline_status[0]}" "${pipeline_status[1]}" "$run_output" \
        "$localnet_output" "$command" \
        >>"$summary"
      echo "seed-matrix test command failed for ${module}::${test_name} with seed ${seed} (cargo=${pipeline_status[0]}, tee=${pipeline_status[1]}); output: ${run_log}; localnet: ${localnet_dir}; summary: ${summary}" >&2
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
      || ! grep -Fq "test ${module}::${test_name} " "$run_log"; then
      printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$profile" "$test_name" "$seed" "invalid_output" \
        "${pipeline_status[0]}" "${pipeline_status[1]}" "$run_output" \
        "$localnet_output" "$command" \
        >>"$summary"
      echo "expected exactly one ${module}::${test_name} test to run and pass for seed ${seed}; refusing zero-test or ambiguous Cargo success; output: ${run_log}; localnet: ${localnet_dir}; summary: ${summary}" >&2
      exit 1
    fi
    rm -rf -- "$localnet_dir"
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
      "$profile" "$test_name" "$seed" "passed" \
      "${pipeline_status[0]}" "${pipeline_status[1]}" "$run_output" "-" "$command" \
      >>"$summary"
  done
done

echo "seed-matrix completed ${run_index} command runs; evidence: ${summary}" >&2
