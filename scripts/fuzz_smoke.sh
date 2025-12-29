#!/usr/bin/env bash
set -euo pipefail

# Run short libFuzzer sessions for Norito JSON fuzz targets.
# Intended for CI smoke: limited iterations to catch regressions quickly.

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
FUZZ_DIR="$ROOT_DIR/crates/norito/fuzz"
RUNS="${RUNS:-4000}"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found; skipping fuzz smoke" >&2
  exit 0
fi
if ! command -v cargo-fuzz >/dev/null 2>&1; then
  echo "cargo-fuzz not installed; skipping fuzz smoke (install: cargo install cargo-fuzz)" >&2
  exit 0
fi

pushd "$FUZZ_DIR" >/dev/null

targets=(
  json_parse_string
  json_parse_string_ref
  json_skip_value
  json_from_json_equiv
)

for t in "${targets[@]}"; do
  echo "[fuzz-smoke] running $t for $RUNS runs"
  # Use UBSAN/ASAN defaults; libFuzzer will stop on crash. Limit to quick run count.
  cargo fuzz run "$t" -- -runs="$RUNS" -rss_limit_mb=3072 -max_total_time=30 || {
    echo "[fuzz-smoke] target $t failed" >&2
    exit 1
  }
done

popd >/dev/null
echo "[fuzz-smoke] all targets passed"

# Run IVM fuzz smoke if available.
IVM_FUZZ_DIR="$ROOT_DIR/crates/ivm/fuzz"
if [ -d "$IVM_FUZZ_DIR" ]; then
  pushd "$IVM_FUZZ_DIR" >/dev/null
  ivm_targets=(
    tlv_validate
    kotodama_lower
  )
  for t in "${ivm_targets[@]}"; do
    echo "[fuzz-smoke] running ivm::$t for $RUNS runs"
    cargo fuzz run "$t" -- -runs="$RUNS" -rss_limit_mb=3072 -max_total_time=30 || {
      echo "[fuzz-smoke] ivm target $t failed" >&2
      exit 1
    }
  done
  popd >/dev/null
fi
