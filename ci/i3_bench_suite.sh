#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

JSON_OUT="${JSON_OUT:-benchmarks/i3/latest.json}"
CSV_OUT="${CSV_OUT:-benchmarks/i3/latest.csv}"
MD_OUT="${MD_OUT:-benchmarks/i3/latest.md}"
THRESHOLDS="${THRESHOLDS:-benchmarks/i3/thresholds.json}"

mkdir -p "$(dirname "$JSON_OUT")"

args=(
  --iterations "${I3_BENCH_ITERATIONS:-64}"
  --sample-count "${I3_BENCH_SAMPLES:-5}"
  --json-out "$JSON_OUT"
  --csv-out "$CSV_OUT"
  --markdown-out "$MD_OUT"
  --threshold "$THRESHOLDS"
)

if [[ "${I3_BENCH_ALLOW_OVERWRITE:-}" != "" ]]; then
  args+=(--allow-overwrite)
fi

cargo xtask i3-bench-suite "${args[@]}"
