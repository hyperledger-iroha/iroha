#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

mkdir -p "$REPO_ROOT/artifacts/sns"

python3 "$REPO_ROOT/scripts/run_sns_annex_jobs.py" \
  --base-dir "$REPO_ROOT" \
  --check-only

python3 "$REPO_ROOT/scripts/check_sns_annex_schedule.py" \
  --jobs docs/source/sns/regulatory/annex_jobs.json \
  --regulatory-root docs/source/sns/regulatory \
  --report-root docs/source/sns/reports \
  --json-out "$REPO_ROOT/artifacts/sns/annex_schedule_summary.json"

python3 "$REPO_ROOT/scripts/check_sns_annex_locales.py" \
  --jobs docs/source/sns/regulatory/annex_jobs.json \
  --report-root docs/source/sns/reports \
  --locales en,ar,es,fr,pt,ru,ur \
  --json-out "$REPO_ROOT/artifacts/sns/annex_locale_summary.json"

python3 "$REPO_ROOT/scripts/check_sns_annex_integrity.py" \
  --jobs docs/source/sns/regulatory/annex_jobs.json \
  --report-root docs/source/sns/reports \
  --json-out "$REPO_ROOT/artifacts/sns/annex_integrity_summary.json"

echo "[sns] annex automation verified"
