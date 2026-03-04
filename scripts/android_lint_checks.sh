#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT_DIR="${ROOT_DIR}/examples/android"
GRADLE_WRAPPER="${PROJECT_DIR}/gradlew"
ARTIFACT_ROOT="${ROOT_DIR}/artifacts/android/lint"

VERSION="dev"
LABEL=""
OUT_DIR=""
DRY_RUN=0

usage() {
  cat <<'USAGE'
Usage: scripts/android_lint_checks.sh [options]

Run the Android lint, style, and dependency guard tasks and archive the reports
under artifacts/android/lint/<label>. Intended for AND6 release gating.

Options:
  --version <semver>   Version label recorded in the summary (default: dev)
  --label <value>      Override the archive directory label
  --out-dir <path>     Override the archive directory path
  --dry-run            Print the steps without executing Gradle/tasks
  -h, --help           Show this message
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="${2:-}"
      shift 2
      ;;
    --label)
      LABEL="${2:-}"
      shift 2
      ;;
    --out-dir)
      OUT_DIR="${2:-}"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ ! -x "${GRADLE_WRAPPER}" ]]; then
  echo "Gradle wrapper not found at ${GRADLE_WRAPPER}" >&2
  exit 1
fi

timestamp="$(date -u +"%Y%m%dT%H%M%SZ")"
if [[ -z "${LABEL}" ]]; then
  LABEL="${VERSION}-${timestamp}"
fi
if [[ -z "${OUT_DIR}" ]]; then
  OUT_DIR="${ARTIFACT_ROOT}/${LABEL}"
fi

mkdir -p "${OUT_DIR}"

run_gradle() {
  local -a args=(
    "lintRelease"
    "ktlintCheck"
    "detekt"
    "dependencyGuardBaseline"
    ":operator-console:lintRelease"
    ":retail-wallet:lintRelease"
  )
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    printf '[android-lint] (dry-run) %q %q\n' "${GRADLE_WRAPPER}" "${args[*]}"
    return 0
  fi
  (cd "${PROJECT_DIR}" && "${GRADLE_WRAPPER}" "${args[@]}")
}

copy_tree() {
  local src="$1"
  local dest="$2"
  if [[ -d "${src}" ]]; then
    rsync -a --delete "${src}/" "${dest}/"
  fi
}

collect_reports() {
  local report_dest="${OUT_DIR}/reports"
  mkdir -p "${report_dest}"
  while IFS= read -r rel; do
    [[ -z "${rel}" ]] && continue
    local abs="${PROJECT_DIR}/${rel#.}"
    local target="${report_dest}/${rel#.}"
    mkdir -p "${target}"
    rsync -a "${abs}/" "${target}/"
  done < <(cd "${PROJECT_DIR}" && find . -type d -path "*/build/reports")
  if [[ -d "${PROJECT_DIR}/build/dependencyGuard" ]]; then
    mkdir -p "${OUT_DIR}/dependencyGuard"
    rsync -a "${PROJECT_DIR}/build/dependencyGuard/" "${OUT_DIR}/dependencyGuard/"
  fi
}

write_summary() {
  local summary="${OUT_DIR}/summary.json"
  python3 - "$summary" <<PY
import datetime as dt
import json
import os
import sys

summary_path = sys.argv[1]
data = {
    "version": "${VERSION}",
    "label": "${LABEL}",
    "generated_at": dt.datetime.utcnow().replace(microsecond=0).isoformat() + "Z",
    "project_dir": "${PROJECT_DIR}",
    "tasks": [
        "lintRelease",
        "ktlintCheck",
        "detekt",
        "dependencyGuardBaseline",
        ":operator-console:lintRelease",
        ":retail-wallet:lintRelease",
    ],
}
os.makedirs(os.path.dirname(summary_path), exist_ok=True)
with open(summary_path, "w", encoding="utf-8") as handle:
    json.dump(data, handle, indent=2)
    handle.write("\\n")
PY
}

run_gradle
collect_reports
write_summary

ln -sfn "${OUT_DIR}" "${ARTIFACT_ROOT}/latest"

echo "[android-lint] reports archived at ${OUT_DIR}"
