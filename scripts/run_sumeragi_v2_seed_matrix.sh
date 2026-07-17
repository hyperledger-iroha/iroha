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

workspace_source_manifest() {
  python3 scripts/compute_workspace_source_manifest.py --root "$repo_root"
}

require_source_manifest() {
  local checkpoint="$1"
  local observed
  if ! observed="$(workspace_source_manifest)"; then
    echo "failed to compute the workspace source manifest at ${checkpoint}" >&2
    return 1
  fi
  if [[ ! "$observed" =~ ^[0-9a-f]{64}$ ]]; then
    echo "workspace source manifest helper returned an invalid digest at ${checkpoint}" >&2
    return 1
  fi
  if [[ "$observed" != "$source_manifest_sha256" ]]; then
    echo "workspace sources changed during the seed matrix at ${checkpoint}: expected ${source_manifest_sha256}, observed ${observed}" >&2
    return 1
  fi
}

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    echo "sha256sum or shasum is required to attest seed-matrix evidence" >&2
    return 1
  fi
}

observed_source_manifest_sha256="$(workspace_source_manifest)"
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
  echo "seed-matrix source manifest does not match the parent release invocation: expected ${source_manifest_sha256}, observed ${observed_source_manifest_sha256}" >&2
  exit 1
fi
release_head_commit=""
release_head_tree=""
release_cargo_lock_sha256=""
if [[ "$profile" == "release" ]]; then
  release_head_commit="${IROHA_RELEASE_HEAD_COMMIT:-}"
  release_head_tree="${IROHA_RELEASE_HEAD_TREE:-}"
  release_cargo_lock_sha256="${IROHA_RELEASE_CARGO_LOCK_SHA256:-}"
  if [[ ! "$release_head_commit" =~ ^([0-9a-f]{40}|[0-9a-f]{64})$ \
    || ! "$release_head_tree" =~ ^([0-9a-f]{40}|[0-9a-f]{64})$ \
    || ! "$release_cargo_lock_sha256" =~ ^[0-9a-f]{64}$ ]]; then
    echo "release seed matrix requires exact parent HEAD, tree, and Cargo.lock identities" >&2
    exit 1
  fi
fi
readonly release_head_commit release_head_tree release_cargo_lock_sha256
readonly source_bound_root="${repo_root}/target/sumeragi-v2-release/${source_manifest_sha256}"
export IROHA_RELEASE_SOURCE_MANIFEST_SHA256="$source_manifest_sha256"
export CARGO_TARGET_DIR="${source_bound_root}/test-suite"
export IROHA_TEST_TARGET_DIR="${source_bound_root}/programs"
# A release/PR corridor must fail if the real four-peer network cannot start;
# sandbox skips are useful for ad-hoc developer runs but are not gate evidence.
export IROHA_TEST_REQUIRE_NETWORK=1
# A successful seed must represent one fresh network, not a later harness
# retry which hides a protocol-induced startup stall.
export IROHA_TEST_NETWORK_START_ATTEMPTS=1
# Explicit binary paths bypass the test-network freshness check. Clear them in
# this standalone runner as well as in the parent release gate, and force the
# re-entrant source-fingerprint build path for every scenario process.
unset TEST_NETWORK_BIN_IROHAD KAGAMI_BIN CARGO_BIN_EXE_iroha3d CARGO_BIN_EXE_kagami
unset TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL TEST_NETWORK_BIN_IROHA CARGO_BIN_EXE_iroha
unset TEST_NETWORK_IROHAD_FEATURES TEST_NETWORK_CARGO
export IROHA_TEST_SKIP_BUILD=0
export IROHA_TEST_ALLOW_REENTRANT_BUILD=1
# These are the bounded waits implemented by the integration harness itself.
# The launcher intentionally does not signal Cargo, rustc, or validator process
# groups. If an unbounded path escapes these internal deadlines, the invocation
# remains incomplete and cannot acquire the completion attestation below.
export IROHA_TEST_BUILD_TIMEOUT_MS=3600
export IROHA_TEST_PROCESS_TIMEOUT_MS=300
export IROHA_TEST_NETWORK_PERMIT_WAIT_TIMEOUT=300
readonly scenarios=(
  "authoritative_v2_genesis_commits_on_every_validator|authoritative_v2_genesis_commits_on_every_validator|normal"
  "authoritative_v2_finalizes_through_validator_restart|authoritative_v2_finalizes_through_validator_restart|normal"
  "taira_npos_leader_timeout_commits_within_rotation_bound|taira_npos_leader_timeout_commits_within_rotation_bound|normal"
  "real_network_divergent_prepare_qcs_converge_after_ordered_release|real_network_divergent_prepare_qcs_converge_after_ordered_release|normal"
)

# Keep command output after the process exits so a failed gate can be audited.
# The override names an evidence *root*, not one invocation directory. An
# atomic lock rejects concurrent writers to that root, while mktemp preserves
# every completed or partial invocation instead of erasing earlier evidence.
evidence_root="${SUMERAGI_V2_SEED_MATRIX_EVIDENCE_DIR:-${repo_root}/target/sumeragi-v2-seed-matrix/${profile}/${source_manifest_sha256}}"
mkdir -p -- "$evidence_root"
evidence_root="$(cd -- "$evidence_root" && pwd -P)"
readonly evidence_root
readonly evidence_lock="${evidence_root}/.seed-matrix.lock"
if ! mkdir -- "$evidence_lock"; then
  echo "another seed-matrix invocation owns ${evidence_lock}; refusing shared evidence" >&2
  exit 1
fi
cleanup_evidence_lock() {
  local status=$?
  rm -f -- "${evidence_lock}/owner"
  if ! rmdir -- "$evidence_lock"; then
    echo "failed to remove seed-matrix evidence lock ${evidence_lock}" >&2
    status=1
  fi
  trap - EXIT
  exit "$status"
}
trap cleanup_evidence_lock EXIT
printf 'pid=%s\nprofile=%s\nsource_manifest_sha256=%s\n' \
  "$$" "$profile" "$source_manifest_sha256" >"${evidence_lock}/owner"

evidence_dir="$(mktemp -d "${evidence_root}/invocation.XXXXXX")"
readonly evidence_dir
readonly run_log_dir="${evidence_dir}/runs"
readonly summary="${evidence_dir}/summary.tsv"
readonly inventory_log="${evidence_dir}/test-inventory.log"
readonly ignored_inventory_log="${evidence_dir}/ignored-test-inventory.log"
readonly invocation_attestation="${evidence_dir}/invocation.tsv"
readonly completion_attestation="${evidence_dir}/COMPLETED.tsv"
readonly expected_runs="$((seed_count * ${#scenarios[@]}))"
mkdir -p -- "$run_log_dir" "${evidence_dir}/localnets"
printf '%s\t%s\n' \
  schema_version 1 \
  profile "$profile" \
  source_manifest_sha256 "$source_manifest_sha256" \
  source_bound_root "$source_bound_root" \
  cargo_target_dir "$CARGO_TARGET_DIR" \
  iroha_test_target_dir "$IROHA_TEST_TARGET_DIR" \
  expected_runs "$expected_runs" \
  build_timeout_seconds 3600 \
  process_timeout_seconds 300 \
  network_permit_wait_timeout_seconds 300 \
  process_lifetime_enforcement internal_deadlines_no_outer_process_signal \
  completion_file "$(basename "$completion_attestation")" \
  >"$invocation_attestation"
printf '%s\n' $'profile\tsource_manifest_sha256\tscenario\tseed\tresult\tcargo_status\ttee_status\trun_log_sha256\toutput\tlocalnet\tcommand' >"$summary"
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
require_source_manifest "after test inventory" || exit 1
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
    require_source_manifest "before ${test_name} seed index ${seed_index}" || exit 1
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
    command="IROHA_RELEASE_SOURCE_MANIFEST_SHA256=${source_manifest_sha256} IROHA_TEST_REQUIRE_NETWORK=1 IROHA_TEST_NETWORK_START_ATTEMPTS=1 IROHA_TEST_SKIP_BUILD=0 IROHA_TEST_ALLOW_REENTRANT_BUILD=1 IROHA_TEST_BUILD_TIMEOUT_MS=3600 IROHA_TEST_PROCESS_TIMEOUT_MS=300 IROHA_TEST_NETWORK_PERMIT_WAIT_TIMEOUT=300 IROHA_TEST_NETWORK_BASE_SEED=${seed} TEST_NETWORK_TMP_DIR=${localnet_dir} IROHA_TEST_NETWORK_KEEP_DIRS=1 cargo test --locked -p integration_tests --test ${target} ${module}::${test_name} -- ${test_args[*]}"
    ((run_index += 1))

    set +e
    TEST_NETWORK_TMP_DIR="$localnet_dir" IROHA_TEST_NETWORK_KEEP_DIRS=1 \
      cargo test --locked -p integration_tests --test "${target}" \
      "${module}::${test_name}" -- "${test_args[@]}" \
      2>&1 | tee "$run_log"
    pipeline_status=("${PIPESTATUS[@]}")
    set -e
    run_log_sha256="$(sha256_file "$run_log")"
    if [[ ! "$run_log_sha256" =~ ^[0-9a-f]{64}$ ]]; then
      echo "failed to hash seed-matrix run log ${run_log}" >&2
      exit 1
    fi
    if ! require_source_manifest "after ${test_name} seed ${seed}"; then
      printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$profile" "$source_manifest_sha256" "$test_name" "$seed" \
        "source_changed" "${pipeline_status[0]}" "${pipeline_status[1]}" \
        "$run_log_sha256" "$run_output" "$localnet_output" "$command" \
        >>"$summary"
      exit 1
    fi
    if ((pipeline_status[0] != 0 || pipeline_status[1] != 0)); then
      printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$profile" "$source_manifest_sha256" "$test_name" "$seed" "command_failed" \
        "${pipeline_status[0]}" "${pipeline_status[1]}" "$run_log_sha256" \
        "$run_output" "$localnet_output" "$command" \
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
      printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$profile" "$source_manifest_sha256" "$test_name" "$seed" "invalid_output" \
        "${pipeline_status[0]}" "${pipeline_status[1]}" "$run_log_sha256" \
        "$run_output" "$localnet_output" "$command" \
        >>"$summary"
      echo "expected exactly one ${module}::${test_name} test to run and pass for seed ${seed}; refusing zero-test or ambiguous Cargo success; output: ${run_log}; localnet: ${localnet_dir}; summary: ${summary}" >&2
      exit 1
    fi
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
      "$profile" "$source_manifest_sha256" "$test_name" "$seed" "passed" \
      "${pipeline_status[0]}" "${pipeline_status[1]}" "$run_log_sha256" \
      "$run_output" "$localnet_output" "$command" \
      >>"$summary"
  done
done

if [[ "$run_index" != "$expected_runs" ]]; then
  echo "seed-matrix completed ${run_index} command runs, expected ${expected_runs}" >&2
  exit 1
fi
require_source_manifest "before completion attestation" || exit 1
summary_sha256="$(sha256_file "$summary")"
if [[ ! "$summary_sha256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "failed to hash the seed-matrix summary" >&2
  exit 1
fi
completion_tmp="${evidence_dir}/.COMPLETED.tsv.$$"
{
  printf '%s\t%s\n' \
    schema_version 1 \
    profile "$profile" \
    source_manifest_sha256 "$source_manifest_sha256"
  if [[ "$profile" == "release" ]]; then
    printf '%s\t%s\n' \
      head_commit "$release_head_commit" \
      head_tree "$release_head_tree" \
      cargo_lock_sha256 "$release_cargo_lock_sha256"
  fi
  printf '%s\t%s\n' \
    completed_runs "$run_index" \
    expected_runs "$expected_runs" \
    summary_sha256 "$summary_sha256"
} >"$completion_tmp"
mv -- "$completion_tmp" "$completion_attestation"
if ! require_source_manifest "after completion attestation"; then
  rm -f -- "$completion_attestation"
  exit 1
fi
if [[ -n "${IROHA_SEED_MATRIX_COMPLETION_PATH_FILE:-}" ]]; then
  completion_path_tmp="${IROHA_SEED_MATRIX_COMPLETION_PATH_FILE}.$$"
  printf '%s\n' "$completion_attestation" >"$completion_path_tmp"
  mv -- "$completion_path_tmp" "$IROHA_SEED_MATRIX_COMPLETION_PATH_FILE"
fi

echo "seed-matrix completed ${run_index} command runs; evidence: ${summary}; completion: ${completion_attestation}" >&2
