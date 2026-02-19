#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: run_nexus_cross_dataspace_atomic_swap.sh [OPTIONS]

Runs the Nexus cross-dataspace atomic swap localnet proof test:
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing

Options:
  --release               Run tests with --release
  --all-nexus             Run the full Nexus integration subset (mod nexus::)
  --keep-dirs             Preserve temp network directories (IROHA_TEST_NETWORK_KEEP_DIRS=1)
  --no-skip-build         Do not set IROHA_TEST_SKIP_BUILD=1
  --capture               Do not pass --nocapture to cargo test
  --test-threads <N>      Set --test-threads (default: 1)
  --env <KEY=VALUE>       Extra environment variable (repeatable)
  -h, --help              Show this help
EOF
}

PROFILE="debug"
RUN_SCOPE="case"
KEEP_DIRS=false
SKIP_BUILD=true
NO_CAPTURE=false
TEST_THREADS="1"
EXTRA_ENV=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release)
      PROFILE="release"
      shift
      ;;
    --all-nexus)
      RUN_SCOPE="nexus"
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
      TEST_THREADS="$2"
      shift 2
      ;;
    --env)
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

ENV_VARS=("NORITO_SKIP_BINDINGS_SYNC=1")
if [[ "$KEEP_DIRS" == true ]]; then
  ENV_VARS+=("IROHA_TEST_NETWORK_KEEP_DIRS=1")
fi
if [[ "$SKIP_BUILD" == true ]]; then
  ENV_VARS+=("IROHA_TEST_SKIP_BUILD=1")
fi
for extra in ${EXTRA_ENV[@]+"${EXTRA_ENV[@]}"}; do
  ENV_VARS+=("$extra")
done

if [[ "$RUN_SCOPE" == "case" ]]; then
  CMD=(cargo test -p integration_tests --test mod nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing -- --test-threads="$TEST_THREADS")
else
  CMD=(cargo test -p integration_tests --test mod nexus:: -- --test-threads="$TEST_THREADS")
fi

if [[ "$PROFILE" == "release" ]]; then
  CMD=(cargo test -p integration_tests --release "${CMD[@]:4}")
fi

if [[ "$NO_CAPTURE" == false ]]; then
  CMD+=("--nocapture")
fi

echo "Command: ${ENV_VARS[*]} ${CMD[*]}"
env "${ENV_VARS[@]}" "${CMD[@]}"
