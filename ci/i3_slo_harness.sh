#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

OUT_DIR="${OUT_DIR:-artifacts/i3_slo/latest}"
BUDGETS="${BUDGETS:-benchmarks/i3/slo_budgets.json}"
THRESHOLDS="${THRESHOLDS:-benchmarks/i3/slo_thresholds.json}"

mkdir -p "$OUT_DIR"

args=(
  --iterations "${I3_SLO_ITERATIONS:-64}"
  --sample-count "${I3_SLO_SAMPLES:-5}"
  --out-dir "$OUT_DIR"
  --budget "$BUDGETS"
  --threshold "$THRESHOLDS"
)

if [[ "${I3_SLO_ALLOW_OVERWRITE:-}" != "" ]]; then
  args+=(--allow-overwrite)
fi

if [[ "${I3_SLO_FLAMEGRAPH_HINT:-}" != "" ]]; then
  args+=(--flamegraph-hint)
fi

cargo xtask i3-slo-harness "${args[@]}"
