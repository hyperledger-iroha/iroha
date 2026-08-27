#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"

MODE="check"
if [[ "$#" -gt 0 ]]; then
  case "$1" in
    --check)
      shift
      ;;
    --test)
      MODE="test"
      shift
      ;;
    --*)
      printf 'Unknown option: %s\n' "$1" >&2
      exit 2
      ;;
  esac
fi

if [[ "$#" -eq 0 ]]; then
  printf 'Usage: %s <rules-file> [<rules-file> ...]\n' "$0" >&2
  printf '       %s --test <rule-test-file> [<rule-test-file> ...]\n' "$0" >&2
  exit 2
fi

RULES_FILES=("$@")
RULES_PATHS=()
for rules_file in "${RULES_FILES[@]}"; do
  rules_path="${PROJECT_ROOT}/${rules_file}"
  if [[ ! -f "${rules_path}" ]]; then
    printf 'Rules file not found: %s\n' "${rules_path}" >&2
    exit 1
  fi
  RULES_PATHS+=("${rules_path}")
done

if command -v promtool >/dev/null 2>&1; then
  promtool "${MODE}" rules "${RULES_PATHS[@]}"
  exit 0
fi

if command -v docker >/dev/null 2>&1; then
  docker run --rm --entrypoint /bin/promtool \
    -v "${PROJECT_ROOT}:/workspace:ro" \
    --workdir /workspace \
    prom/prometheus "${MODE}" rules "${RULES_FILES[@]}"
  exit 0
fi

printf 'promtool not found. Install Prometheus or run with Docker (prom/prometheus image).\n' >&2
exit 1
