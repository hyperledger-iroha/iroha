#!/usr/bin/env bash
set -euo pipefail

RULES_FILE="${1:-docs/source/references/prometheus.rules.sumeragi_vrf.yml}"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
RULES_PATH="${PROJECT_ROOT}/${RULES_FILE}"

if [[ ! -f "${RULES_PATH}" ]]; then
  printf 'Rules file not found: %s\n' "${RULES_PATH}" >&2
  exit 1
fi

if command -v promtool >/dev/null 2>&1; then
  promtool check rules "${RULES_PATH}"
  exit 0
fi

if command -v docker >/dev/null 2>&1; then
  docker run --rm -v "${PROJECT_ROOT}:/workspace" prom/prometheus promtool check rules \
    "${RULES_FILE}"
  exit 0
fi

printf 'promtool not found. Install Prometheus or run with Docker (prom/prometheus image).\n' >&2
exit 1
