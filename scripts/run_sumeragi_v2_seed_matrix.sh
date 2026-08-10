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
  if [[ "${IROHA_RELEASE_SEALED_WORKTREE:-0}" == 1 ]] \
    && ! python3 scripts/seal_workspace_source.py \
      --verify --root "$repo_root" --no-writable-paths; then
    echo "workspace source seal changed during the seed matrix at ${checkpoint}" >&2
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

source "${repo_root}/scripts/sumeragi_v2_release_process_policy.sh"

source "${repo_root}/scripts/sumeragi_v2_prebuilt_bundle.sh"

localnet_binary_attestation_valid() {
  sumeragi_v2_localnet_binary_attestation_valid \
    "$repo_root" "$source_manifest_sha256"
}

ensure_source_bound_localnet_binaries() {
  sumeragi_v2_ensure_source_bound_localnet_binaries \
    "$repo_root" "$source_manifest_sha256"
}

export_source_bound_localnet_binaries() {
  sumeragi_v2_export_source_bound_localnet_binaries \
    "$repo_root" "$source_manifest_sha256"
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
  seed_invocation_root="$(
    mktemp -d /private/tmp/iroha-sumeragi-v2-seed-matrix.XXXXXX
  )"
  mkdir -m 0700 -- \
    "$seed_invocation_root/target" \
    "$seed_invocation_root/artifacts"
  export CARGO_TARGET_DIR="$seed_invocation_root/target"
  export IROHA_RELEASE_ARTIFACT_ROOT="$seed_invocation_root/artifacts"
  export IROHA_RELEASE_CANCEL_REQUEST_PATH="$seed_invocation_root/cancel-request.json"
fi
require_external_cargo_target_dir "$repo_root"
require_external_release_artifact_root "$repo_root"
require_disjoint_release_roots "$repo_root"
release_gate_boundary "seed-matrix:entry" || exit $?
readonly source_bound_root="${IROHA_RELEASE_ARTIFACT_ROOT:?IROHA_RELEASE_ARTIFACT_ROOT is required}/sumeragi-v2-release/${source_manifest_sha256}"
export IROHA_RELEASE_SOURCE_MANIFEST_SHA256="$source_manifest_sha256"
require_source_manifest "release entry" || exit 1
# A release/PR corridor must fail if the real four-peer network cannot start;
# sandbox skips are useful for ad-hoc developer runs but are not gate evidence.
export IROHA_TEST_REQUIRE_NETWORK=1
# A successful seed must represent one fresh network, not a later harness
# retry which hides a protocol-induced startup stall.
export IROHA_TEST_NETWORK_START_ATTEMPTS=1
# Explicit binary paths bypass the test-network freshness check. Clear them in
# this standalone runner as well as in the parent release gate. All binaries
# are prebuilt and attested before Cargo starts a test process.
unset TEST_NETWORK_BIN_IROHAD KAGAMI_BIN CARGO_BIN_EXE_irohad CARGO_BIN_EXE_kagami
unset TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL TEST_NETWORK_BIN_IROHA CARGO_BIN_EXE_iroha
unset TEST_NETWORK_IROHAD_FEATURES TEST_NETWORK_CARGO
export IROHA_TEST_SKIP_BUILD=1
export IROHA_TEST_ALLOW_REENTRANT_BUILD=0
export IROHA_TEST_BUILD_PROFILE=release
export PROFILE=release
export CARGO_NET_OFFLINE=true
# These are the bounded waits implemented by the integration harness itself.
# The launcher intentionally does not signal Cargo, rustc, or validator process
# groups. If an unbounded path escapes these internal deadlines, the invocation
# remains incomplete and cannot acquire the completion attestation below.
export IROHA_TEST_BUILD_TIMEOUT_MS=3600
export IROHA_TEST_PROCESS_TIMEOUT_MS=300
export IROHA_TEST_NETWORK_PERMIT_WAIT_TIMEOUT=300
release_gate_boundary "seed-matrix:prebuilt-publication:before" || exit $?
ensure_source_bound_localnet_binaries
export_source_bound_localnet_binaries
release_gate_boundary "seed-matrix:prebuilt-publication:after" || exit $?
# This source-bound corridor is deliberately the fixed five-scenario,
# four-validator acceptance matrix. The independent nine-peer signed-observer
# pressure test is useful stress coverage, but is not one of these 32-by-5
# release obligations and must not change the attested run count.
readonly scenarios=(
  "authoritative_v2_genesis_commits_on_every_validator|authoritative_v2_genesis_commits_on_every_validator|normal"
  "authoritative_v2_finalizes_through_validator_restart|authoritative_v2_finalizes_through_validator_restart|normal"
  "taira_npos_leader_timeout_commits_within_rotation_bound|taira_npos_leader_timeout_commits_within_rotation_bound|normal"
  "real_network_same_subject_locked_reproposal_converges_after_ordered_quorum_release|real_network_same_subject_locked_reproposal_converges_after_ordered_quorum_release|normal"
  "real_network_distinct_subject_prepare_qcs_converge_after_causal_release|real_network_distinct_subject_prepare_qcs_converge_after_causal_release|normal"
)

# Keep command output after the process exits so a failed gate can be audited.
# The override names an evidence *root*, not one invocation directory. An
# atomic lock rejects concurrent writers to that root, while mktemp preserves
# every completed or partial invocation instead of erasing earlier evidence.
evidence_root="${SUMERAGI_V2_SEED_MATRIX_EVIDENCE_DIR:-${IROHA_RELEASE_ARTIFACT_ROOT}/sumeragi-v2-seed-matrix/${profile}/${source_manifest_sha256}}"
require_release_artifact_path "$evidence_root" || exit $?
mkdir -p -- "$evidence_root"
evidence_root="$(cd -- "$evidence_root" && pwd -P)"
require_release_artifact_directory "$evidence_root" || exit $?
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
readonly localnet_manifest_dir="${evidence_dir}/localnet-manifests"
readonly localnet_manifests="${evidence_dir}/localnet-manifests.tsv"
readonly summary="${evidence_dir}/summary.tsv"
readonly inventory_log="${evidence_dir}/test-inventory.log"
readonly ignored_inventory_log="${evidence_dir}/ignored-test-inventory.log"
readonly invocation_attestation="${evidence_dir}/invocation.tsv"
readonly completion_attestation="${evidence_dir}/COMPLETED.tsv"
readonly expected_runs="$((seed_count * ${#scenarios[@]}))"
mkdir -p -- "$run_log_dir" "$localnet_manifest_dir" "${evidence_dir}/localnets"
printf '%s\n' $'run_index\tlocalnet\tmanifest\tmanifest_sha256' >"$localnet_manifests"
printf '%s\t%s\n' \
  schema_version 1 \
  profile "$profile" \
  source_manifest_sha256 "$source_manifest_sha256" \
  source_bound_root "$source_bound_root" \
  cargo_target_dir "$CARGO_TARGET_DIR" \
  iroha_test_target_dir "$IROHA_TEST_TARGET_DIR" \
  prebuilt_manifest_sha256 "$IROHA_RELEASE_PREBUILT_MANIFEST_SHA256" \
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
release_gate_boundary "seed-matrix:inventory-harness:before" || exit $?
set +e
run_cargo test --locked --offline -p integration_tests --test "${target}" -- --list \
  >"$inventory_log" 2>&1
inventory_status=$?
run_cargo test --locked --offline -p integration_tests --test "${target}" -- --list --ignored \
  >"$ignored_inventory_log" 2>&1
ignored_inventory_status=$?
set -e
release_gate_boundary "seed-matrix:inventory-harness:after" || exit $?
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
    # Record a canonical replay command. The placeholder keeps the receipt
    # independent of the invocation's incidental absolute archive path; the
    # adjacent `localnet` field binds it to the exact retained directory.
    command="CARGO_TARGET_DIR=${CARGO_TARGET_DIR} IROHA_TEST_TARGET_DIR=${IROHA_TEST_TARGET_DIR} IROHA_RELEASE_SOURCE_MANIFEST_SHA256=${source_manifest_sha256} IROHA_RELEASE_PREBUILT_MANIFEST_SHA256=${IROHA_RELEASE_PREBUILT_MANIFEST_SHA256} TEST_NETWORK_BIN_IROHAD=${TEST_NETWORK_BIN_IROHAD} TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL=${TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL} TEST_NETWORK_BIN_IROHA=${TEST_NETWORK_BIN_IROHA} KAGAMI_BIN=${KAGAMI_BIN} CARGO_NET_OFFLINE=true IROHA_TEST_REQUIRE_NETWORK=1 IROHA_TEST_NETWORK_START_ATTEMPTS=1 IROHA_TEST_SKIP_BUILD=1 IROHA_TEST_ALLOW_REENTRANT_BUILD=0 IROHA_TEST_BUILD_PROFILE=release PROFILE=release IROHA_TEST_BUILD_TIMEOUT_MS=3600 IROHA_TEST_PROCESS_TIMEOUT_MS=300 IROHA_TEST_NETWORK_PERMIT_WAIT_TIMEOUT=300 IROHA_TEST_NETWORK_BASE_SEED=${seed} TEST_NETWORK_TMP_DIR=\${SEED_MATRIX_EVIDENCE_DIRECTORY}/${localnet_output} IROHA_TEST_NETWORK_KEEP_DIRS=1 cargo test --locked --offline -p integration_tests --test ${target} ${module}::${test_name} -- ${test_args[*]}"
    ((run_index += 1))

    release_gate_boundary "seed-matrix:test-harness-${run_index}:before" || exit $?
    set +e
    TEST_NETWORK_TMP_DIR="$localnet_dir" IROHA_TEST_NETWORK_KEEP_DIRS=1 \
      run_cargo test --locked --offline -p integration_tests --test "${target}" \
      "${module}::${test_name}" -- "${test_args[@]}" \
      2>&1 | tee "$run_log"
    pipeline_status=("${PIPESTATUS[@]}")
    set -e
    release_gate_boundary "seed-matrix:test-harness-${run_index}:after" || exit $?
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
    expected_seed_line="test ${module}::${test_name} ... ${test_name}: deterministic network seed = ${seed}"
    expected_seed_line_count="$(grep -Fxc -- "$expected_seed_line" "$run_log" || true)"
    if [[ "$running_total" != 1 || "$running_one" != 1 \
      || "$result_total" != 1 || "$passing_one" != 1 \
      || "$expected_seed_line_count" != 1 ]]; then
      printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$profile" "$source_manifest_sha256" "$test_name" "$seed" "invalid_output" \
        "${pipeline_status[0]}" "${pipeline_status[1]}" "$run_log_sha256" \
        "$run_output" "$localnet_output" "$command" \
        >>"$summary"
      echo "expected exactly one ${module}::${test_name} test to run and pass with deterministic seed ${seed}; refusing zero-test, wrong-seed, or ambiguous Cargo success; output: ${run_log}; localnet: ${localnet_dir}; summary: ${summary}" >&2
      exit 1
    fi
    manifest_index="$((run_index - 1))"
    printf -v manifest_output 'localnet-manifests/run-%03d.tsv' "$manifest_index"
    manifest_path="${evidence_dir}/${manifest_output}"
    if ! python3 scripts/sumeragi_v2_localnet_manifest.py \
      --root "$localnet_dir" --output "$manifest_path"; then
      printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$profile" "$source_manifest_sha256" "$test_name" "$seed" "invalid_localnet" \
        "${pipeline_status[0]}" "${pipeline_status[1]}" "$run_log_sha256" \
        "$run_output" "$localnet_output" "$command" \
        >>"$summary"
      echo "retained localnet for ${module}::${test_name} seed ${seed} is unsafe or unstable; localnet: ${localnet_dir}; summary: ${summary}" >&2
      exit 1
    fi
    manifest_sha256="$(sha256_file "$manifest_path")"
    if [[ ! "$manifest_sha256" =~ ^[0-9a-f]{64}$ ]]; then
      echo "failed to hash retained-localnet manifest ${manifest_path}" >&2
      exit 1
    fi
    printf '%s\t%s\t%s\t%s\n' \
      "$manifest_index" "$localnet_output" "$manifest_output" "$manifest_sha256" \
      >>"$localnet_manifests"
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
localnet_manifest_count="$(($(wc -l <"$localnet_manifests") - 1))"
if [[ "$localnet_manifest_count" != "$expected_runs" ]]; then
  echo "seed-matrix retained ${localnet_manifest_count} localnet manifests, expected ${expected_runs}" >&2
  exit 1
fi
require_source_manifest "before completion attestation" || exit 1
if ! localnet_binary_attestation_valid; then
  echo "source-bound localnet binary bundle changed before seed-matrix completion" >&2
  exit 1
fi
summary_sha256="$(sha256_file "$summary")"
if [[ ! "$summary_sha256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "failed to hash the seed-matrix summary" >&2
  exit 1
fi
localnet_manifests_sha256="$(sha256_file "$localnet_manifests")"
if [[ ! "$localnet_manifests_sha256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "failed to hash the retained-localnet manifest index" >&2
  exit 1
fi
if ! require_source_manifest "immediately before completion publication"; then
  exit 1
fi
if ! localnet_binary_attestation_valid; then
  echo "source-bound localnet binary bundle changed while publishing seed-matrix completion" >&2
  exit 1
fi
completion_body="$(
  printf '%s\t%s\n' \
    schema_version 2 \
    profile "$profile" \
    source_manifest_sha256 "$source_manifest_sha256" \
    prebuilt_manifest_sha256 "$IROHA_RELEASE_PREBUILT_MANIFEST_SHA256"
  if [[ "$profile" == "release" ]]; then
    printf '%s\t%s\n' \
      head_commit "$release_head_commit" \
      head_tree "$release_head_tree" \
      cargo_lock_sha256 "$release_cargo_lock_sha256"
  fi
  printf '%s\t%s\n' \
    completed_runs "$run_index" \
    expected_runs "$expected_runs" \
    summary_sha256 "$summary_sha256" \
    localnet_manifest_count "$localnet_manifest_count" \
    localnet_manifests_path "$(basename "$localnet_manifests")" \
    localnet_manifests_sha256 "$localnet_manifests_sha256"
  while IFS=$'\t' read -r manifest_index _ manifest_path manifest_sha256; do
    [[ "$manifest_index" == "run_index" ]] && continue
    printf 'localnet_manifest_%03d_path\t%s\n' "$manifest_index" "$manifest_path"
    printf 'localnet_manifest_%03d_sha256\t%s\n' "$manifest_index" "$manifest_sha256"
  done <"$localnet_manifests"
)"
marker_publish_args=(
  --output "$completion_attestation"
  --maximum-bytes 131072
)
if [[ -n "${IROHA_SEED_MATRIX_COMPLETION_PATH_FILE:-}" ]]; then
  completion_pointer_parent="${IROHA_SEED_MATRIX_COMPLETION_PATH_FILE%/*}"
  require_release_artifact_path "$completion_pointer_parent" || exit $?
  require_release_artifact_directory "$completion_pointer_parent" || exit $?
  marker_publish_args+=(
    --pointer "$IROHA_SEED_MATRIX_COMPLETION_PATH_FILE"
  )
fi
release_gate_boundary "seed-matrix:completion-publication:before" || exit $?
printf '%s\n' "$completion_body" |
  python3 -I -S "${repo_root}/scripts/publish_release_marker.py" \
    "${marker_publish_args[@]}"
publication_status=0
release_gate_boundary "seed-matrix:completion-publication:after" \
  || publication_status=$?
if ((publication_status != 0)); then
  rm -f -- "$completion_attestation"
  if [[ -n "${IROHA_SEED_MATRIX_COMPLETION_PATH_FILE:-}" ]]; then
    rm -f -- "$IROHA_SEED_MATRIX_COMPLETION_PATH_FILE"
  fi
  exit "$publication_status"
fi

echo "seed-matrix completed ${run_index} command runs; evidence: ${summary}; completion: ${completion_attestation}" >&2
