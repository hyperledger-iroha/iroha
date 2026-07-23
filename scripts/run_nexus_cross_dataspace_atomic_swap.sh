#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: run_nexus_cross_dataspace_atomic_swap.sh [OPTIONS]

Runs the Nexus cross-dataspace atomic swap localnet proof test:
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing

Options:
  --release               Run tests with --release
  --all-nexus             Run the full Nexus integration subset (nexus:: filter)
  --cross-dataspace-fault-soak
                          Run the ignored two-hour 12-peer rotating-validator soak
  --cross-dataspace-seed <SEED>
                          Soak seed (nexus-cross-dataspace-v1-seed-00..09)
  --cross-dataspace-soak-duration-secs <SECONDS>
                          Fault-soak duration; must be exactly 7200
  --native-amx-fault-soak Run the ignored rotating-validator Native AMX fault soak
  --native-amx-iterations <N>
                          Native AMX soak iterations, 1..100 (default: 10)
  --target-dir <PATH>     Set CARGO_TARGET_DIR for the test run
  --evidence-dir <PATH>   Persist exact per-run logs and completion accounting
  --fast                  Run cargo via scripts/cargo_fast.sh when available
  --fast-zero-debug       With --fast, set CARGO_PROFILE_{DEV,TEST}_DEBUG=0
  --fast-no-incremental   With --fast, set CARGO_INCREMENTAL=0
  --keep-dirs             Preserve temp network directories (IROHA_TEST_NETWORK_KEEP_DIRS=1)
  --no-skip-build         Do not set IROHA_TEST_SKIP_BUILD=1
  --capture               Do not pass --nocapture to cargo test
  --test-threads <N>      Set --test-threads (default: 1)
  --env <KEY=VALUE>       Extra environment variable (repeatable)
  -h, --help              Show this help
EOF
}

require_option_value() {
  local flag="$1"
  local value="${2-}"
  if [[ -z "$value" ]] || [[ "$value" == --* ]]; then
    echo "Missing value for ${flag}" >&2
    exit 2
  fi
}

PROFILE="debug"
RUN_SCOPE="case"
NATIVE_AMX_ITERATIONS=""
readonly NATIVE_AMX_FAULT_SOAK_TEST="native_amx_rotating_validator_fault_soak_preserves_independent_participant_qcs"
readonly CROSS_DATASPACE_CASE_TEST="nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing"
readonly CROSS_DATASPACE_FAULT_SOAK_TEST="nexus::cross_dataspace_localnet::cross_dataspace_two_hour_fault_soak_preserves_multilane_application"
readonly CROSS_DATASPACE_SEED_PREFIX="nexus-cross-dataspace-v1-seed-"
readonly CROSS_DATASPACE_SEED_COUNT=10
readonly CROSS_DATASPACE_FAULT_SOAK_DURATION_SECS=7200
CROSS_DATASPACE_SEED=""
CROSS_DATASPACE_FAULT_SOAK_DURATION=""
KEEP_DIRS=false
SKIP_BUILD=true
NO_CAPTURE=false
TEST_THREADS="1"
EXTRA_ENV=()
TARGET_DIR=""
EVIDENCE_DIR=""
USE_CARGO_FAST=false
FAST_ZERO_DEBUG=false
FAST_NO_INCREMENTAL=false
PERMIT_DIR_OVERRIDE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release)
      PROFILE="release"
      shift
      ;;
    --all-nexus)
      if [[ "$RUN_SCOPE" != "case" ]]; then
        echo "--all-nexus cannot be combined with another run scope" >&2
        exit 2
      fi
      RUN_SCOPE="nexus"
      shift
      ;;
    --cross-dataspace-fault-soak)
      if [[ "$RUN_SCOPE" != "case" ]]; then
        echo "--cross-dataspace-fault-soak cannot be combined with another run scope" >&2
        exit 2
      fi
      RUN_SCOPE="cross-fault-soak"
      shift
      ;;
    --cross-dataspace-seed)
      require_option_value "--cross-dataspace-seed" "${2-}"
      CROSS_DATASPACE_SEED="$2"
      shift 2
      ;;
    --cross-dataspace-soak-duration-secs)
      require_option_value "--cross-dataspace-soak-duration-secs" "${2-}"
      CROSS_DATASPACE_FAULT_SOAK_DURATION="$2"
      shift 2
      ;;
    --native-amx-fault-soak)
      if [[ "$RUN_SCOPE" != "case" ]]; then
        echo "--native-amx-fault-soak cannot be combined with another run scope" >&2
        exit 2
      fi
      RUN_SCOPE="native-amx"
      shift
      ;;
    --native-amx-iterations)
      require_option_value "--native-amx-iterations" "${2-}"
      NATIVE_AMX_ITERATIONS="$2"
      shift 2
      ;;
    --target-dir)
      require_option_value "--target-dir" "${2-}"
      TARGET_DIR="$2"
      shift 2
      ;;
    --evidence-dir)
      require_option_value "--evidence-dir" "${2-}"
      EVIDENCE_DIR="$2"
      shift 2
      ;;
    --fast)
      USE_CARGO_FAST=true
      shift
      ;;
    --fast-zero-debug)
      FAST_ZERO_DEBUG=true
      shift
      ;;
    --fast-no-incremental)
      FAST_NO_INCREMENTAL=true
      shift
      ;;
    --keep-dirs)
      KEEP_DIRS=true
      shift
      ;;
    --no-skip-build)
      SKIP_BUILD=false
      shift
      ;;
    --capture)
      NO_CAPTURE=true
      shift
      ;;
    --test-threads)
      require_option_value "--test-threads" "${2-}"
      TEST_THREADS="$2"
      shift 2
      ;;
    --env)
      require_option_value "--env" "${2-}"
      EXTRA_ENV+=("$2")
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ ! "$TEST_THREADS" =~ ^[0-9]+$ ]] || [[ "$TEST_THREADS" -lt 1 ]]; then
  echo "Invalid --test-threads value: $TEST_THREADS (expected positive integer)" >&2
  exit 2
fi
if [[ -n "$NATIVE_AMX_ITERATIONS" ]]; then
  if [[ "$RUN_SCOPE" != "native-amx" ]]; then
    echo "--native-amx-iterations requires --native-amx-fault-soak" >&2
    exit 2
  fi
  if [[ ! "$NATIVE_AMX_ITERATIONS" =~ ^[0-9]+$ ]] \
    || [[ "$NATIVE_AMX_ITERATIONS" -lt 1 ]] \
    || [[ "$NATIVE_AMX_ITERATIONS" -gt 100 ]]; then
    echo "Invalid --native-amx-iterations value: $NATIVE_AMX_ITERATIONS (expected 1..100)" >&2
    exit 2
  fi
fi
if [[ -n "$CROSS_DATASPACE_SEED" && "$RUN_SCOPE" != "cross-fault-soak" ]]; then
  echo "--cross-dataspace-seed requires --cross-dataspace-fault-soak" >&2
  exit 2
fi
if [[ -n "$CROSS_DATASPACE_FAULT_SOAK_DURATION" \
  && "$RUN_SCOPE" != "cross-fault-soak" ]]; then
  echo "--cross-dataspace-soak-duration-secs requires --cross-dataspace-fault-soak" >&2
  exit 2
fi
if [[ "$RUN_SCOPE" == "cross-fault-soak" ]]; then
  CROSS_DATASPACE_SEED="${CROSS_DATASPACE_SEED:-${CROSS_DATASPACE_SEED_PREFIX}00}"
  CROSS_DATASPACE_FAULT_SOAK_DURATION="${CROSS_DATASPACE_FAULT_SOAK_DURATION:-$CROSS_DATASPACE_FAULT_SOAK_DURATION_SECS}"
  if [[ ! "$CROSS_DATASPACE_SEED" =~ ^nexus-cross-dataspace-v1-seed-0[0-9]$ ]]; then
    echo "Invalid --cross-dataspace-seed value: ${CROSS_DATASPACE_SEED}" >&2
    exit 2
  fi
  if [[ "$CROSS_DATASPACE_FAULT_SOAK_DURATION" != \
    "$CROSS_DATASPACE_FAULT_SOAK_DURATION_SECS" ]]; then
    echo "Invalid --cross-dataspace-soak-duration-secs value: ${CROSS_DATASPACE_FAULT_SOAK_DURATION} (must be exactly ${CROSS_DATASPACE_FAULT_SOAK_DURATION_SECS})" >&2
    exit 2
  fi
fi
for extra in ${EXTRA_ENV[@]+"${EXTRA_ENV[@]}"}; do
  case "${extra%%=*}" in
    IROHA_TEST_NETWORK_BASE_SEED|IROHA_NEXUS_CROSS_REQUIRE_SEED|IROHA_NEXUS_CROSS_FAULT_SOAK_DURATION_SECS)
      echo "--env may not override reserved cross-dataspace evidence control ${extra%%=*}" >&2
      exit 2
      ;;
  esac
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
cd "$repo_root"
release_head_commit="${IROHA_RELEASE_HEAD_COMMIT:-}"
release_head_tree="${IROHA_RELEASE_HEAD_TREE:-}"
release_source_manifest_sha256="${IROHA_RELEASE_SOURCE_MANIFEST_SHA256:-}"
release_cargo_lock_sha256="${IROHA_RELEASE_CARGO_LOCK_SHA256:-}"
if [[ "$PROFILE" == "release" \
  && "$RUN_SCOPE" != "nexus" \
  && "$RUN_SCOPE" != "native-amx" ]]; then
  if [[ ! "$release_head_commit" =~ ^([0-9a-f]{40}|[0-9a-f]{64})$ \
    || ! "$release_head_tree" =~ ^([0-9a-f]{40}|[0-9a-f]{64})$ \
    || ! "$release_source_manifest_sha256" =~ ^[0-9a-f]{64}$ \
    || ! "$release_cargo_lock_sha256" =~ ^[0-9a-f]{64}$ ]]; then
    echo "--release cross-dataspace evidence requires exact parent release identity exports" >&2
    exit 2
  fi
fi
readonly release_head_commit release_head_tree
readonly release_source_manifest_sha256 release_cargo_lock_sha256
cargo_runner=(cargo)
if [[ "$USE_CARGO_FAST" == true ]]; then
  cargo_fast_script="${repo_root}/scripts/cargo_fast.sh"
  if [[ ! -x "${cargo_fast_script}" ]]; then
    echo "scripts/cargo_fast.sh is not available or not executable" >&2
    exit 2
  fi
  cargo_runner=("${cargo_fast_script}")
  if [[ "$FAST_ZERO_DEBUG" == true ]]; then
    cargo_runner+=("--zero-debug")
  fi
  if [[ "$FAST_NO_INCREMENTAL" == true ]]; then
    cargo_runner+=("--no-incremental")
  fi
  echo "[nexus-cross-swap] using scripts/cargo_fast.sh for cargo commands"
elif [[ "$FAST_ZERO_DEBUG" == true || "$FAST_NO_INCREMENTAL" == true ]]; then
  echo "--fast-zero-debug and --fast-no-incremental require --fast" >&2
  exit 2
fi

if [[ -z "${IROHA_TEST_NETWORK_PERMIT_DIR+x}" ]]; then
  PERMIT_DIR_OVERRIDE="$(mktemp -d)"
fi

ENV_VARS=("NORITO_SKIP_BINDINGS_SYNC=1")
if [[ "$KEEP_DIRS" == true ]]; then
  ENV_VARS+=("IROHA_TEST_NETWORK_KEEP_DIRS=1")
fi
if [[ "$SKIP_BUILD" == true ]]; then
  ENV_VARS+=("IROHA_TEST_SKIP_BUILD=1")
fi
if [[ -n "$TARGET_DIR" ]]; then
  ENV_VARS+=("CARGO_TARGET_DIR=${TARGET_DIR}")
fi
if [[ -n "$PERMIT_DIR_OVERRIDE" ]]; then
  ENV_VARS+=("IROHA_TEST_NETWORK_PERMIT_DIR=${PERMIT_DIR_OVERRIDE}")
fi
if [[ -n "$NATIVE_AMX_ITERATIONS" ]]; then
  ENV_VARS+=("IROHA_NATIVE_AMX_SOAK_ITERATIONS=${NATIVE_AMX_ITERATIONS}")
fi
for extra in ${EXTRA_ENV[@]+"${EXTRA_ENV[@]}"}; do
  ENV_VARS+=("$extra")
done
# This proof launcher must never translate a sandbox-denied localnet into a
# successful test. Append the requirement after caller-supplied values so an
# `--env` override cannot weaken it.
ENV_VARS+=("IROHA_TEST_REQUIRE_NETWORK=1")
ENV_VARS+=("IROHA_TEST_NETWORK_START_ATTEMPTS=1")

wait_for_cargo_idle() {
  while true; do
    local snapshot active
    snapshot="$(ps -axo pid,etime,command)"
    printf '%s\n' "$snapshot" >&2
    active="$(
      awk -v self_pid="$$" '
        NR > 1 && $1 != self_pid && $0 !~ /<defunct>/ &&
        ($0 ~ /(^|[[:space:]\/])cargo([[:space:]]|$)/ ||
         $0 ~ /(^|[[:space:]\/])rustc([[:space:]]|$)/) {
          print
        }
      ' <<<"$snapshot"
    )"
    if [[ -z "$active" ]]; then
      return
    fi
    echo "[nexus-cross-swap] waiting for active Cargo/rustc processes:" >&2
    printf '%s\n' "$active" >&2
    sleep 5
  done
}

sha256_file() {
  local path="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -- "$path" | awk '{print $1}'
  else
    shasum -a 256 -- "$path" | awk '{print $1}'
  fi
}

validate_exact_test_log() {
  local test_name="$1"
  local log_path="$2"
  if [[ "$(grep -Fxc -- "test ${test_name} ... ok" "$log_path" || true)" != 1 ]]; then
    echo "${test_name} did not report one exact passing test result" >&2
    return 1
  fi
  if [[ "$(grep -Fxc -- "running 1 test" "$log_path" || true)" != 1 ]]; then
    echo "${test_name} did not report exactly one scheduled test" >&2
    return 1
  fi
  if [[ "$(
    grep -Ec \
      '^test result: ok[.] 1 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in .+$' \
      "$log_path" || true
  )" != 1 ]]; then
    echo "${test_name} did not report one unambiguous one-test summary" >&2
    return 1
  fi
}

publish_completion_path() {
  local completion_path="$1"
  local pointer_path="${IROHA_NEXUS_CROSS_COMPLETION_PATH_FILE:-}"
  [[ -n "$pointer_path" ]] || return 0
  if [[ -L "$pointer_path" ]]; then
    echo "completion path pointer must not be a symlink: ${pointer_path}" >&2
    return 1
  fi
  mkdir -p -- "$(dirname "$pointer_path")"
  local pointer_tmp="${pointer_path}.tmp.$$"
  printf '%s\n' "$completion_path" >"$pointer_tmp"
  mv -- "$pointer_tmp" "$pointer_path"
}

if [[ "$USE_CARGO_FAST" == true ]]; then
  CARGO_TEST_CMD=("${cargo_runner[@]}" -- test)
else
  CARGO_TEST_CMD=("${cargo_runner[@]}" test)
fi
if [[ "$PROFILE" == "release" ]]; then
  CARGO_TEST_CMD+=("--release")
fi
CARGO_TEST_CMD+=("--locked" "--offline")

TEST_ARGS=("--test-threads=${TEST_THREADS}")
if [[ "$NO_CAPTURE" == false ]]; then
  TEST_ARGS+=("--nocapture")
fi

if [[ "$RUN_SCOPE" == "nexus" ]]; then
  CMD=(
    "${CARGO_TEST_CMD[@]}"
    -p integration_tests
    --test nexus_and_streaming
    "nexus::"
    --
    "${TEST_ARGS[@]}"
  )
  wait_for_cargo_idle
  echo "Command: ${ENV_VARS[*]} ${CMD[*]}"
  env "${ENV_VARS[@]}" "${CMD[@]}"
  exit
fi

if [[ "$RUN_SCOPE" == "native-amx" ]]; then
  LIST_CMD=(
    "${CARGO_TEST_CMD[@]}"
    -p integration_tests
    --test native_amx_routing
    --
    --list
    --ignored
  )
  wait_for_cargo_idle
  native_amx_ignored_test_list="$(env "${ENV_VARS[@]}" "${LIST_CMD[@]}")"
  if ! grep -Fqx -- "${NATIVE_AMX_FAULT_SOAK_TEST}: test" \
    <<<"$native_amx_ignored_test_list"; then
    echo "missing required ignored Native AMX fault-soak test: ${NATIVE_AMX_FAULT_SOAK_TEST}" >&2
    exit 1
  fi
  CMD=(
    "${CARGO_TEST_CMD[@]}"
    -p integration_tests
    --test native_amx_routing
    "$NATIVE_AMX_FAULT_SOAK_TEST"
    --
    --exact
    --ignored
    "${TEST_ARGS[@]}"
  )
  NATIVE_AMX_RUN_LOG="$(mktemp "${TMPDIR:-/tmp}/native-amx-fault-soak.XXXXXX.log")"
  cleanup_native_amx_run_log() {
    local status=$?
    rm -f -- "$NATIVE_AMX_RUN_LOG"
    return "$status"
  }
  trap cleanup_native_amx_run_log EXIT
  wait_for_cargo_idle
  echo "Command: ${ENV_VARS[*]} ${CMD[*]}"
  set +e
  env "${ENV_VARS[@]}" "${CMD[@]}" 2>&1 | tee "$NATIVE_AMX_RUN_LOG"
  native_amx_pipeline_status=("${PIPESTATUS[@]}")
  set -e
  if ((native_amx_pipeline_status[0] != 0 || native_amx_pipeline_status[1] != 0)); then
    echo "Native AMX fault soak failed (cargo=${native_amx_pipeline_status[0]}, tee=${native_amx_pipeline_status[1]})" >&2
    exit 1
  fi
  validate_exact_test_log "$NATIVE_AMX_FAULT_SOAK_TEST" "$NATIVE_AMX_RUN_LOG"
  exit
fi

if [[ -z "$EVIDENCE_DIR" ]]; then
  if [[ -n "$TARGET_DIR" ]]; then
    EVIDENCE_DIR="${TARGET_DIR%/}/nexus-cross-dataspace"
  else
    EVIDENCE_DIR="${repo_root}/target/nexus-cross-dataspace"
  fi
fi
if [[ -L "$EVIDENCE_DIR" ]]; then
  echo "evidence directory must not be a symlink: ${EVIDENCE_DIR}" >&2
  exit 1
fi
mkdir -p -- "$EVIDENCE_DIR"

LIST_CMD=(
  "${CARGO_TEST_CMD[@]}"
  -p integration_tests
  --test nexus_and_streaming
  --
  --list
)
if [[ "$RUN_SCOPE" == "cross-fault-soak" ]]; then
  LIST_CMD+=("--ignored")
fi
wait_for_cargo_idle
cross_test_list="$(env "${ENV_VARS[@]}" "${LIST_CMD[@]}")"
if [[ "$RUN_SCOPE" == "case" ]]; then
  required_cross_test="$CROSS_DATASPACE_CASE_TEST"
else
  required_cross_test="$CROSS_DATASPACE_FAULT_SOAK_TEST"
fi
if ! grep -Fqx -- "${required_cross_test}: test" <<<"$cross_test_list"; then
  echo "missing required cross-dataspace test: ${required_cross_test}" >&2
  exit 1
fi

if [[ "$RUN_SCOPE" == "case" ]]; then
  evidence_run_dir="$(mktemp -d "${EVIDENCE_DIR%/}/seed-matrix.XXXXXX")"
  runs_path="${evidence_run_dir}/runs.tsv"
  printf '%s\n' $'ordinal\tseed\tstatus\tprocess_retries\tlog_sha256\tlog' >"$runs_path"
  passed_runs=0
  for ((ordinal = 0; ordinal < CROSS_DATASPACE_SEED_COUNT; ordinal += 1)); do
    printf -v seed '%s%02d' "$CROSS_DATASPACE_SEED_PREFIX" "$ordinal"
    printf -v run_log '%s/seed-%02d.log' "$evidence_run_dir" "$ordinal"
    RUN_ENV=(
      "${ENV_VARS[@]}"
      "IROHA_TEST_NETWORK_BASE_SEED=${seed}"
      "IROHA_NEXUS_CROSS_REQUIRE_SEED=1"
    )
    CMD=(
      "${CARGO_TEST_CMD[@]}"
      -p integration_tests
      --test nexus_and_streaming
      "$CROSS_DATASPACE_CASE_TEST"
      --
      --exact
      "${TEST_ARGS[@]}"
    )
    wait_for_cargo_idle
    echo "Command: ${RUN_ENV[*]} ${CMD[*]}"
    set +e
    env "${RUN_ENV[@]}" "${CMD[@]}" 2>&1 | tee "$run_log"
    run_pipeline_status=("${PIPESTATUS[@]}")
    set -e
    if ((run_pipeline_status[0] != 0 || run_pipeline_status[1] != 0)); then
      echo "cross-dataspace seed ${seed} failed (cargo=${run_pipeline_status[0]}, tee=${run_pipeline_status[1]}); no retry is permitted" >&2
      exit 1
    fi
    validate_exact_test_log "$CROSS_DATASPACE_CASE_TEST" "$run_log"
    printf '%s\t%s\tpassed\t0\t%s\t%s\n' \
      "$ordinal" "$seed" "$(sha256_file "$run_log")" "$(basename "$run_log")" \
      >>"$runs_path"
    ((passed_runs += 1))
  done
  if ((passed_runs != CROSS_DATASPACE_SEED_COUNT)); then
    echo "cross-dataspace seed matrix passed ${passed_runs}/${CROSS_DATASPACE_SEED_COUNT}" >&2
    exit 1
  fi
  completion_path="${evidence_run_dir}/COMPLETED.tsv"
  completion_tmp="${evidence_run_dir}/.COMPLETED.tsv.$$"
  printf '%s\t%s\n' \
    schema_version 1 \
    mode deterministic-seed-matrix \
    head_commit "$release_head_commit" \
    head_tree "$release_head_tree" \
    source_manifest_sha256 "$release_source_manifest_sha256" \
    cargo_lock_sha256 "$release_cargo_lock_sha256" \
    expected_runs "$CROSS_DATASPACE_SEED_COUNT" \
    passed_runs "$passed_runs" \
    failed_runs 0 \
    process_retry_runs 0 \
    runs_sha256 "$(sha256_file "$runs_path")" \
    >"$completion_tmp"
  mv -- "$completion_tmp" "$completion_path"
  publish_completion_path "$completion_path"
  echo "[nexus-cross-swap] strict ${passed_runs}/${CROSS_DATASPACE_SEED_COUNT} seed matrix passed; completion=${completion_path}"
  exit
fi

evidence_run_dir="$(mktemp -d "${EVIDENCE_DIR%/}/fault-soak.XXXXXX")"
run_log="${evidence_run_dir}/fault-soak.log"
RUN_ENV=(
  "${ENV_VARS[@]}"
  "IROHA_TEST_NETWORK_BASE_SEED=${CROSS_DATASPACE_SEED}"
  "IROHA_NEXUS_CROSS_REQUIRE_SEED=1"
  "IROHA_NEXUS_CROSS_FAULT_SOAK_DURATION_SECS=${CROSS_DATASPACE_FAULT_SOAK_DURATION}"
)
CMD=(
  "${CARGO_TEST_CMD[@]}"
  -p integration_tests
  --test nexus_and_streaming
  "$CROSS_DATASPACE_FAULT_SOAK_TEST"
  --
  --exact
  --ignored
  "${TEST_ARGS[@]}"
)
wait_for_cargo_idle
echo "Command: ${RUN_ENV[*]} ${CMD[*]}"
set +e
env "${RUN_ENV[@]}" "${CMD[@]}" 2>&1 | tee "$run_log"
fault_soak_pipeline_status=("${PIPESTATUS[@]}")
set -e
if ((fault_soak_pipeline_status[0] != 0 || fault_soak_pipeline_status[1] != 0)); then
  echo "cross-dataspace fault soak failed (cargo=${fault_soak_pipeline_status[0]}, tee=${fault_soak_pipeline_status[1]}); no retry is permitted" >&2
  exit 1
fi
validate_exact_test_log "$CROSS_DATASPACE_FAULT_SOAK_TEST" "$run_log"
completion_path="${evidence_run_dir}/COMPLETED.tsv"
completion_tmp="${evidence_run_dir}/.COMPLETED.tsv.$$"
printf '%s\t%s\n' \
  schema_version 1 \
  mode two-hour-fault-soak \
  head_commit "$release_head_commit" \
  head_tree "$release_head_tree" \
  source_manifest_sha256 "$release_source_manifest_sha256" \
  cargo_lock_sha256 "$release_cargo_lock_sha256" \
  seed "$CROSS_DATASPACE_SEED" \
  duration_seconds "$CROSS_DATASPACE_FAULT_SOAK_DURATION" \
  expected_runs 1 \
  passed_runs 1 \
  failed_runs 0 \
  process_retry_runs 0 \
  log_sha256 "$(sha256_file "$run_log")" \
  >"$completion_tmp"
mv -- "$completion_tmp" "$completion_path"
publish_completion_path "$completion_path"
echo "[nexus-cross-swap] exact ${CROSS_DATASPACE_FAULT_SOAK_DURATION}s fault soak passed; completion=${completion_path}"
