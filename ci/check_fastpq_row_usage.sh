#!/usr/bin/env bash
set -euo pipefail

# Verify that the reference FASTPQ row_usage snapshots do not regress.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

python3 "$REPO_ROOT/scripts/fastpq/check_row_usage.py" \
  --baseline "$REPO_ROOT/artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json" \
  --candidate "$REPO_ROOT/artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json" \
  --max-transfer-ratio-increase 0.0 \
  --max-total-rows-increase 0 \
  --summary-out "$REPO_ROOT/fastpq_row_usage_summary.json"

echo "[fastpq] row_usage snapshot comparison passed"
