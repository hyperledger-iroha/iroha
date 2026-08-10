#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: run_localnet_throughput.sh [OPTIONS]

Runs the ignored 7-peer localnet throughput regression with deterministic inputs
and captures artifacts when enabled.

Options:
  --release                     Run with --release (default: debug)
  --artifact-dir <DIR>          Output directory for artifacts (default: ./artifacts/localnet-throughput)
  --target-dir <DIR>            Set CARGO_TARGET_DIR for cargo and binary reuse
  --fast                        Run cargo via scripts/cargo_fast.sh when available
  --fast-zero-debug             With --fast, set CARGO_PROFILE_{DEV,TEST}_DEBUG=0
  --fast-no-incremental         With --fast, set CARGO_INCREMENTAL=0
  --no-skip-build               Do not auto-set IROHA_TEST_SKIP_BUILD when binaries already exist
  --keep-dirs                   Preserve test network tempdirs (IROHA_TEST_NETWORK_KEEP_DIRS=1)
  --target-blocks <N>           Total blocks (warmup + steady)
  --warmup-blocks <N>           Warmup blocks
  --steady-blocks <N>           Steady-state blocks
  --submit-batch <N>            Batch size per submit loop
  --parallelism <N>             Submit parallelism
  --queue-soft-limit <N>        Submit queue soft limit
  --payload-bytes <N>           Log payload size (bytes)
  --rng-seed <N>                RNG seed for payloads
  --rbc-encodings <MODE>        plain, rs16, or both (default: both)
  --slo-p95-ms <N>              Commit p95 SLO (ms)
  --slo-p99-ms <N>              Commit p99 SLO (ms)
  --slo-view-change-rate <N>    View-change rate SLO (per sec)
  --slo-backpressure-rate <N>   Backpressure deferral rate SLO (per sec)
  --slo-queue-sat-frac <N>      Queue saturation fraction SLO (0..1)
  --env <KEY=VALUE>             Extra environment variable (repeatable)
  -h, --help                    Show this help
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
ARTIFACT_DIR=""
TARGET_DIR=""
USE_CARGO_FAST=false
FAST_ZERO_DEBUG=false
FAST_NO_INCREMENTAL=false
AUTO_SKIP_BUILD=true
KEEP_DIRS=false
TARGET_BLOCKS=""
WARMUP_BLOCKS=""
STEADY_BLOCKS=""
SUBMIT_BATCH=""
PARALLELISM=""
QUEUE_SOFT_LIMIT=""
PAYLOAD_BYTES=""
RNG_SEED=""
RBC_ENCODINGS="both"
SLO_P95_MS=""
SLO_P99_MS=""
SLO_VIEW_CHANGE_RATE=""
SLO_BACKPRESSURE_RATE=""
SLO_QUEUE_SAT_FRAC=""
EXTRA_ENV=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release)
      PROFILE="release"
      shift
      ;;
    --artifact-dir)
      require_option_value "--artifact-dir" "${2-}"
      ARTIFACT_DIR="$2"
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
    --no-skip-build)
      AUTO_SKIP_BUILD=false
      shift
      ;;
    --keep-dirs)
      KEEP_DIRS=true
      shift
      ;;
    --target-blocks)
      require_option_value "--target-blocks" "${2-}"
      TARGET_BLOCKS="$2"
      shift 2
      ;;
    --warmup-blocks)
      require_option_value "--warmup-blocks" "${2-}"
      WARMUP_BLOCKS="$2"
      shift 2
      ;;
    --steady-blocks)
      require_option_value "--steady-blocks" "${2-}"
      STEADY_BLOCKS="$2"
      shift 2
      ;;
    --submit-batch)
      require_option_value "--submit-batch" "${2-}"
      SUBMIT_BATCH="$2"
      shift 2
      ;;
    --parallelism)
      require_option_value "--parallelism" "${2-}"
      PARALLELISM="$2"
      shift 2
      ;;
    --queue-soft-limit)
      require_option_value "--queue-soft-limit" "${2-}"
      QUEUE_SOFT_LIMIT="$2"
      shift 2
      ;;
    --payload-bytes)
      require_option_value "--payload-bytes" "${2-}"
      PAYLOAD_BYTES="$2"
      shift 2
      ;;
    --rng-seed)
      require_option_value "--rng-seed" "${2-}"
      RNG_SEED="$2"
      shift 2
      ;;
    --rbc-encodings)
      require_option_value "--rbc-encodings" "${2-}"
      RBC_ENCODINGS="$2"
      shift 2
      ;;
    --slo-p95-ms)
      require_option_value "--slo-p95-ms" "${2-}"
      SLO_P95_MS="$2"
      shift 2
      ;;
    --slo-p99-ms)
      require_option_value "--slo-p99-ms" "${2-}"
      SLO_P99_MS="$2"
      shift 2
      ;;
    --slo-view-change-rate)
      require_option_value "--slo-view-change-rate" "${2-}"
      SLO_VIEW_CHANGE_RATE="$2"
      shift 2
      ;;
    --slo-backpressure-rate)
      require_option_value "--slo-backpressure-rate" "${2-}"
      SLO_BACKPRESSURE_RATE="$2"
      shift 2
      ;;
    --slo-queue-sat-frac)
      require_option_value "--slo-queue-sat-frac" "${2-}"
      SLO_QUEUE_SAT_FRAC="$2"
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

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"

resolve_dir() {
  local path="$1"
  local candidate
  if [[ "${path}" = /* ]]; then
    candidate="${path}"
  else
    candidate="${repo_root}/${path}"
  fi
  mkdir -p "${candidate}"
  (
    cd "${candidate}"
    pwd
  )
}

bin_name() {
  local raw="$1"
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*)
      printf '%s.exe\n' "${raw}"
      ;;
    *)
      printf '%s\n' "${raw}"
      ;;
  esac
}

resolve_existing_binary() {
  local root="$1"
  local bin
  bin="$(bin_name "$2")"
  local candidate
  for candidate in \
    "${root}/debug/${bin}" \
    "${root}/release/${bin}"
  do
    if [[ -f "${candidate}" ]]; then
      printf '%s\n' "${candidate}"
      return 0
    fi
  done
  return 1
}

export_if_unset() {
  local name="$1"
  local value="$2"
  if [[ -z "${!name+x}" ]]; then
    export "${name}=${value}"
  fi
}

cargo_runner=(cargo)
if [[ "${USE_CARGO_FAST}" == true ]]; then
  cargo_fast_script="${repo_root}/scripts/cargo_fast.sh"
  if [[ ! -x "${cargo_fast_script}" ]]; then
    echo "scripts/cargo_fast.sh is not available or not executable" >&2
    exit 2
  fi
  cargo_runner=("${cargo_fast_script}")
  if [[ "${FAST_ZERO_DEBUG}" == true ]]; then
    cargo_runner+=("--zero-debug")
  fi
  if [[ "${FAST_NO_INCREMENTAL}" == true ]]; then
    cargo_runner+=("--no-incremental")
  fi
  echo "[localnet-throughput] using scripts/cargo_fast.sh for cargo commands"
elif [[ "${FAST_ZERO_DEBUG}" == true || "${FAST_NO_INCREMENTAL}" == true ]]; then
  echo "--fast-zero-debug and --fast-no-incremental require --fast" >&2
  exit 2
fi

if [[ -n "${TARGET_DIR}" ]]; then
  export CARGO_TARGET_DIR="$(resolve_dir "${TARGET_DIR}")"
fi
target_root="$(resolve_dir "${CARGO_TARGET_DIR:-target}")"
export CARGO_TARGET_DIR="${target_root}"
export_if_unset IROHA_TEST_TARGET_DIR "${target_root}"

if [[ -z "$ARTIFACT_DIR" ]]; then
  ARTIFACT_DIR="$(pwd)/artifacts/localnet-throughput"
fi
mkdir -p "$ARTIFACT_DIR"

ENV_VARS=()
if [[ -z "${IROHA_TEST_NETWORK_PERMIT_DIR+x}" ]]; then
  ENV_VARS+=("IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d)")
fi
if [[ "$KEEP_DIRS" == true ]]; then
  ENV_VARS+=("IROHA_TEST_NETWORK_KEEP_DIRS=1")
fi
if [[ -n "$TARGET_BLOCKS" ]]; then
  ENV_VARS+=("IROHA_THROUGHPUT_TARGET_BLOCKS=$TARGET_BLOCKS")
fi
if [[ -n "$WARMUP_BLOCKS" ]]; then
  ENV_VARS+=("IROHA_THROUGHPUT_WARMUP_BLOCKS=$WARMUP_BLOCKS")
fi
if [[ -n "$STEADY_BLOCKS" ]]; then
  ENV_VARS+=("IROHA_THROUGHPUT_STEADY_BLOCKS=$STEADY_BLOCKS")
fi
if [[ -n "$SUBMIT_BATCH" ]]; then
  ENV_VARS+=("IROHA_THROUGHPUT_SUBMIT_BATCH=$SUBMIT_BATCH")
fi
if [[ -n "$PARALLELISM" ]]; then
  ENV_VARS+=("IROHA_THROUGHPUT_PARALLELISM=$PARALLELISM")
fi
if [[ -n "$QUEUE_SOFT_LIMIT" ]]; then
  ENV_VARS+=("IROHA_THROUGHPUT_QUEUE_SOFT_LIMIT=$QUEUE_SOFT_LIMIT")
fi
if [[ -n "$PAYLOAD_BYTES" ]]; then
  ENV_VARS+=("IROHA_THROUGHPUT_PAYLOAD_BYTES=$PAYLOAD_BYTES")
fi
if [[ -n "$RNG_SEED" ]]; then
  ENV_VARS+=("IROHA_THROUGHPUT_RNG_SEED=$RNG_SEED")
fi
if [[ -n "$SLO_P95_MS" ]]; then
  ENV_VARS+=("IROHA_THROUGHPUT_SLO_P95_MS=$SLO_P95_MS")
fi
if [[ -n "$SLO_P99_MS" ]]; then
  ENV_VARS+=("IROHA_THROUGHPUT_SLO_P99_MS=$SLO_P99_MS")
fi
if [[ -n "$SLO_VIEW_CHANGE_RATE" ]]; then
  ENV_VARS+=("IROHA_THROUGHPUT_SLO_VIEW_CHANGE_RATE=$SLO_VIEW_CHANGE_RATE")
fi
if [[ -n "$SLO_BACKPRESSURE_RATE" ]]; then
  ENV_VARS+=("IROHA_THROUGHPUT_SLO_BACKPRESSURE_RATE=$SLO_BACKPRESSURE_RATE")
fi
if [[ -n "$SLO_QUEUE_SAT_FRAC" ]]; then
  ENV_VARS+=("IROHA_THROUGHPUT_SLO_QUEUE_SAT_FRAC=$SLO_QUEUE_SAT_FRAC")
fi

for extra in ${EXTRA_ENV[@]+"${EXTRA_ENV[@]}"}; do
  ENV_VARS+=("$extra")
done

if irohad_bin="$(resolve_existing_binary "${target_root}" "iroha3d")"; then
  export_if_unset TEST_NETWORK_BIN_IROHAD "${irohad_bin}"
fi
if iroha_bin="$(resolve_existing_binary "${target_root}" "iroha")"; then
  export_if_unset TEST_NETWORK_BIN_IROHA "${iroha_bin}"
fi
if [[ "${AUTO_SKIP_BUILD}" == true ]] \
  && [[ -n "${TEST_NETWORK_BIN_IROHAD:-}" ]] \
  && [[ -z "${IROHA_TEST_SKIP_BUILD+x}" ]]; then
  export IROHA_TEST_SKIP_BUILD=1
fi

CMD=("${cargo_runner[@]}" -- test -p integration_tests)
if [[ "$PROFILE" == "release" ]]; then
  CMD+=("--release")
fi
CMD+=(
  --test consensus_and_da
  sumeragi_localnet_smoke::permissioned_localnet_throughput_10k_tps
  -- --ignored --exact --nocapture
)

run_one() {
  local encoding="$1"
  local artifact_dir="$2"
  local -a run_env=(
    "IROHA_THROUGHPUT_ARTIFACT_DIR=$artifact_dir"
    "IROHA_THROUGHPUT_RBC_ENCODING=$encoding"
  )
  if [[ "$encoding" == "rs16" ]]; then
    run_env+=(
      "IROHA_THROUGHPUT_RBC_DATA_SHARDS=4"
      "IROHA_THROUGHPUT_RBC_PARITY_SHARDS=2"
    )
  fi
  for extra in ${ENV_VARS[@]+"${ENV_VARS[@]}"}; do
    run_env+=("$extra")
  done
  echo "Artifacts: $artifact_dir"
  echo "Encoding: $encoding"
  echo "Target dir: ${CARGO_TARGET_DIR}"
  echo "Command: ${run_env[*]} ${CMD[*]}"
  env "${run_env[@]}" "${CMD[@]}"
}

case "$RBC_ENCODINGS" in
  both)
    run_one plain "$ARTIFACT_DIR/plain"
    run_one rs16 "$ARTIFACT_DIR/rs16"
    ;;
  plain|rs16)
    run_one "$RBC_ENCODINGS" "$ARTIFACT_DIR/$RBC_ENCODINGS"
    ;;
  *)
    echo "Unsupported --rbc-encodings value: $RBC_ENCODINGS" >&2
    exit 2
    ;;
esac
