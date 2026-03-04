#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACTS_DIR="${ROOT_DIR}/artifacts/norito_bench"
CRITERION_OUT="${ROOT_DIR}/target/criterion"

mkdir -p "${ARTIFACTS_DIR}"

echo "[norito-bench] running enum_packed_bench with reduced sample size…" >&2
cargo bench -p norito enum_packed_bench -- --sample-size 10 --warm-up-time 1

SUMMARY_FILE="${ARTIFACTS_DIR}/enum_packed_summary.txt"

if [[ -d "${CRITERION_OUT}/enum_packed_bench" ]]; then
  REPORT_JSON="${CRITERION_OUT}/enum_packed_bench/new/benchmark.json"
  HTML_REPORT="${CRITERION_OUT}/enum_packed_bench/report/index.html"

  {
    echo "enum_packed_bench summary"
    echo "generated_at=$(TZ=UTC date -u +"%Y-%m-%dT%H:%M:%SZ")"
    if [[ -f "${REPORT_JSON}" ]]; then
      echo "benchmark_json=${REPORT_JSON}"
    fi
    if [[ -f "${HTML_REPORT}" ]]; then
      echo "html_report=${HTML_REPORT}"
    fi
  } > "${SUMMARY_FILE}"
else
  echo "[norito-bench] WARN: Criterion output directory not found; writing empty summary" >&2
  echo "enum_packed_bench summary" > "${SUMMARY_FILE}"
fi

echo "[norito-bench] summary written to ${SUMMARY_FILE}" >&2
