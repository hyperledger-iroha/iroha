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
  --native-amx-fault-soak Run the ignored rotating-validator Native AMX fault soak
  --native-amx-iterations <N>
                          Native AMX soak iterations, 1..100 (default: 10)
  --target-dir <PATH>     Set CARGO_TARGET_DIR for the test run
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
KEEP_DIRS=false
SKIP_BUILD=true
NO_CAPTURE=false
TEST_THREADS="1"
EXTRA_ENV=()
TARGET_DIR=""
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
      if [[ "$RUN_SCOPE" == "native-amx" ]]; then
        echo "--all-nexus cannot be combined with --native-amx-fault-soak" >&2
        exit 2
      fi
      RUN_SCOPE="nexus"
      shift
      ;;
    --native-amx-fault-soak)
      if [[ "$RUN_SCOPE" == "nexus" ]]; then
        echo "--native-amx-fault-soak cannot be combined with --all-nexus" >&2
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

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
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

if [[ "$USE_CARGO_FAST" == true ]]; then
  CMD=("${cargo_runner[@]}" -- test)
else
  CMD=("${cargo_runner[@]}" test)
fi
if [[ "$PROFILE" == "release" ]]; then
  CMD+=("--release")
fi
CMD+=("--locked" "--offline")
if [[ "$RUN_SCOPE" == "native-amx" ]]; then
  CMD+=(
    "-p"
    "integration_tests"
    "--test"
    "native_amx_routing"
    "native_amx_rotating_validator_fault_soak_preserves_independent_participant_qcs"
  )
else
  CMD+=("-p" "integration_tests" "--test" "nexus_and_streaming")
  if [[ "$RUN_SCOPE" == "case" ]]; then
    CMD+=("nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing")
  else
    CMD+=("nexus::")
  fi
fi
CMD+=("--")
if [[ "$RUN_SCOPE" == "native-amx" ]]; then
  CMD+=("--ignored")
fi
CMD+=("--test-threads=${TEST_THREADS}")

if [[ "$NO_CAPTURE" == false ]]; then
  CMD+=("--nocapture")
fi

echo "Command: ${ENV_VARS[*]} ${CMD[*]}"
env "${ENV_VARS[@]}" "${CMD[@]}"
