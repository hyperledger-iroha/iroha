#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

mkdir -p "$REPO_ROOT/artifacts/sns"

python3 "$REPO_ROOT/scripts/run_sns_annex_jobs.py" \
  --base-dir "$REPO_ROOT" \
  --check-only

python3 "$REPO_ROOT/scripts/check_sns_annex_schedule.py" \
  --jobs specs/sns/regulatory/annex_jobs.json \
  --regulatory-root specs/sns/regulatory \
  --report-root specs/sns/reports \
  --json-out "$REPO_ROOT/artifacts/sns/annex_schedule_summary.json"

python3 "$REPO_ROOT/scripts/check_sns_annex_integrity.py" \
  --jobs specs/sns/regulatory/annex_jobs.json \
  --report-root specs/sns/reports \
  --json-out "$REPO_ROOT/artifacts/sns/annex_integrity_summary.json"

echo "[sns] annex automation verified"
