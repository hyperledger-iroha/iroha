#!/usr/bin/env bash
set -euo pipefail

# Run short libFuzzer sessions for Norito JSON and IVM boundary fuzz targets.
#
# Local use is optional by default: a missing fuzzing prerequisite prints a
# diagnostic and skips the run. CI and release gates MUST pass `--strict`,
# which makes every missing or mismatched pinned prerequisite a hard failure.

PINNED_NIGHTLY="nightly-2025-05-08"
PINNED_CARGO_FUZZ_VERSION="0.13.2"

usage() {
  cat <<'USAGE'
Usage: scripts/fuzz_smoke.sh [--strict] [--numeric-v1-only]

  --strict           Fail when cargo, rustup, the pinned nightly, or the
                     pinned cargo-fuzz version is unavailable.
  --numeric-v1-only  Run only the IVM Numeric V1 fuzz target.

Environment:
  RUNS                    libFuzzer run count (default: 4000)
  FUZZ_MAX_TOTAL_TIME     per-target time limit in seconds (default: 30)
  FUZZ_RSS_LIMIT_MB       per-target RSS limit in MiB (default: 3072)
  FUZZ_CODEGEN_UNITS      sanitizer-build codegen units (default: 16)
USAGE
}

STRICT=0
NUMERIC_V1_ONLY=0
while (($# > 0)); do
  case "$1" in
    --strict)
      STRICT=1
      ;;
    --numeric-v1-only)
      NUMERIC_V1_ONLY=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown fuzz-smoke argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
FUZZ_DIR="$ROOT_DIR/crates/norito/fuzz"
RUNS="${RUNS:-4000}"
MAX_TOTAL_TIME="${FUZZ_MAX_TOTAL_TIME:-30}"
RSS_LIMIT_MB="${FUZZ_RSS_LIMIT_MB:-3072}"
CODEGEN_UNITS="${FUZZ_CODEGEN_UNITS:-16}"

for numeric_setting in \
  "RUNS=$RUNS" \
  "FUZZ_MAX_TOTAL_TIME=$MAX_TOTAL_TIME" \
  "FUZZ_RSS_LIMIT_MB=$RSS_LIMIT_MB" \
  "FUZZ_CODEGEN_UNITS=$CODEGEN_UNITS"
do
  value="${numeric_setting#*=}"
  if [[ ! "$value" =~ ^[1-9][0-9]*$ ]]; then
    echo "${numeric_setting%%=*} must be a positive decimal integer" >&2
    exit 2
  fi
done

skip_or_fail() {
  local message="$1"
  if ((STRICT)); then
    echo "required fuzz prerequisite unavailable: $message" >&2
    exit 1
  fi
  echo "$message; skipping fuzz smoke" >&2
  exit 0
}

if ! command -v cargo >/dev/null 2>&1; then
  skip_or_fail "cargo not found"
fi
if ! command -v cargo-fuzz >/dev/null 2>&1; then
  skip_or_fail \
    "cargo-fuzz $PINNED_CARGO_FUZZ_VERSION not installed (install: cargo install cargo-fuzz --version $PINNED_CARGO_FUZZ_VERSION --locked)"
fi
actual_cargo_fuzz_version="$(cargo-fuzz --version 2>/dev/null || true)"
if [[ "$actual_cargo_fuzz_version" != "cargo-fuzz $PINNED_CARGO_FUZZ_VERSION" ]]; then
  skip_or_fail \
    "expected cargo-fuzz $PINNED_CARGO_FUZZ_VERSION, found ${actual_cargo_fuzz_version:-an unreadable version}"
fi
if ! command -v rustup >/dev/null 2>&1; then
  skip_or_fail "rustup not found"
fi
if ! rustup toolchain list \
  | awk '{print $1}' \
  | grep -Eq "^${PINNED_NIGHTLY}(-[^[:space:]]+)?$"
then
  skip_or_fail \
    "$PINNED_NIGHTLY not installed (install: rustup toolchain install $PINNED_NIGHTLY --profile minimal)"
fi

# cargo-fuzz defaults optimized builds to one codegen unit. Splitting the very
# large IVM graph avoids excessive clean-build latency and peak LLVM memory;
# sanitizer coverage and runtime semantics are unchanged.
fuzz_run=(
  cargo "+$PINNED_NIGHTLY" fuzz run
  --codegen-units "$CODEGEN_UNITS"
)
fuzzer_args=(
  "-runs=$RUNS"
  "-rss_limit_mb=$RSS_LIMIT_MB"
  "-max_total_time=$MAX_TOTAL_TIME"
)

if ((!NUMERIC_V1_ONLY)); then
  pushd "$FUZZ_DIR" >/dev/null

  targets=(
    json_parse_string
    json_parse_string_ref
    json_skip_value
    json_from_json_equiv
  )

  for t in "${targets[@]}"; do
    echo "[fuzz-smoke] running $t for $RUNS runs"
    # Use UBSAN/ASAN defaults; libFuzzer stops immediately on a crash.
    "${fuzz_run[@]}" --fuzz-dir "$FUZZ_DIR" "$t" -- "${fuzzer_args[@]}" || {
      echo "[fuzz-smoke] target $t failed" >&2
      exit 1
    }
  done

  popd >/dev/null
  echo "[fuzz-smoke] Norito targets passed"
fi

# Run IVM fuzz smoke if available.
IVM_FUZZ_DIR="$ROOT_DIR/crates/ivm/fuzz"
if [ ! -d "$IVM_FUZZ_DIR" ]; then
  skip_or_fail "IVM fuzz directory not found at $IVM_FUZZ_DIR"
fi
pushd "$IVM_FUZZ_DIR" >/dev/null
if ((NUMERIC_V1_ONLY)); then
  ivm_targets=(numeric_v1)
else
  ivm_targets=(
    tlv_validate
    kotodama_lower
    numeric_v1
  )
fi
for t in "${ivm_targets[@]}"; do
  echo "[fuzz-smoke] running ivm::$t for $RUNS runs"
  "${fuzz_run[@]}" --fuzz-dir "$IVM_FUZZ_DIR" "$t" -- "${fuzzer_args[@]}" || {
    echo "[fuzz-smoke] ivm target $t failed" >&2
    exit 1
  }
done
popd >/dev/null

echo "[fuzz-smoke] all targets passed"
