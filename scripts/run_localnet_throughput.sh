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

PROFILE="debug"
ARTIFACT_DIR=""
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
      ARTIFACT_DIR="$2"
      shift 2
      ;;
    --keep-dirs)
      KEEP_DIRS=true
      shift
      ;;
    --target-blocks)
      TARGET_BLOCKS="$2"
      shift 2
      ;;
    --warmup-blocks)
      WARMUP_BLOCKS="$2"
      shift 2
      ;;
    --steady-blocks)
      STEADY_BLOCKS="$2"
      shift 2
      ;;
    --submit-batch)
      SUBMIT_BATCH="$2"
      shift 2
      ;;
    --parallelism)
      PARALLELISM="$2"
      shift 2
      ;;
    --queue-soft-limit)
      QUEUE_SOFT_LIMIT="$2"
      shift 2
      ;;
    --payload-bytes)
      PAYLOAD_BYTES="$2"
      shift 2
      ;;
    --rng-seed)
      RNG_SEED="$2"
      shift 2
      ;;
    --rbc-encodings)
      RBC_ENCODINGS="$2"
      shift 2
      ;;
    --slo-p95-ms)
      SLO_P95_MS="$2"
      shift 2
      ;;
    --slo-p99-ms)
      SLO_P99_MS="$2"
      shift 2
      ;;
    --slo-view-change-rate)
      SLO_VIEW_CHANGE_RATE="$2"
      shift 2
      ;;
    --slo-backpressure-rate)
      SLO_BACKPRESSURE_RATE="$2"
      shift 2
      ;;
    --slo-queue-sat-frac)
      SLO_QUEUE_SAT_FRAC="$2"
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

if [[ -z "$ARTIFACT_DIR" ]]; then
  ARTIFACT_DIR="$(pwd)/artifacts/localnet-throughput"
fi
mkdir -p "$ARTIFACT_DIR"

ENV_VARS=()
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

CMD=(cargo test -p integration_tests --test sumeragi_localnet_smoke permissioned_localnet_throughput_10k_tps -- --ignored --nocapture)
if [[ "$PROFILE" == "release" ]]; then
  CMD=(cargo test -p integration_tests --release --test sumeragi_localnet_smoke permissioned_localnet_throughput_10k_tps -- --ignored --nocapture)
fi

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
