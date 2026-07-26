#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: deploy/android/publish_ga.sh --version <semver> [options]

Wrapper used during the GA go/no-go window. It delegates to
scripts/publish_android_sdk.sh, copies the resulting Maven repo + SBOM bundle
into artifacts/releases/android/<version>/, and prepares a README summarising
the evidence for compliance.

Options:
  --version <semver>        Target SDK version (required)
  --repo-dir <path>         Local Maven repo (default: artifacts/releases/android/<version>/maven)
  --repo-url <url>          Remote Maven repository to publish after the local copy
  --username <value>        Username/token for --repo-url (optional)
  --password <value>        Password for --repo-url (optional)
  --skip-sbom               Skip SBOM/provenance generation (tests still run)
  --dry-run                 Print the derived commands without executing them
  -h, --help                Show this message
USAGE
}

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PUBLISH_SCRIPT="${ROOT_DIR}/scripts/publish_android_sdk.sh"
RELEASE_ROOT="${ROOT_DIR}/artifacts/releases/android"

VERSION=""
REPO_DIR=""
REMOTE_URL=""
REPO_USERNAME=""
REPO_PASSWORD=""
SKIP_SBOM=0
DRY_RUN=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="${2:-}"
      shift 2
      ;;
    --repo-dir)
      REPO_DIR="${2:-}"
      shift 2
      ;;
    --repo-url)
      REMOTE_URL="${2:-}"
      shift 2
      ;;
    --username)
      REPO_USERNAME="${2:-}"
      shift 2
      ;;
    --password)
      REPO_PASSWORD="${2:-}"
      shift 2
      ;;
    --skip-sbom)
      SKIP_SBOM=1
      shift
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

if [[ -z "$VERSION" ]]; then
  echo "error: --version is required" >&2
  usage >&2
  exit 1
fi

if [[ -z "$REPO_DIR" ]]; then
  REPO_DIR="${RELEASE_ROOT}/${VERSION}/maven"
fi

abs_path() {
  python3 - "$1" <<'PY'
import os, sys
print(os.path.abspath(sys.argv[1]))
PY
}

REPO_DIR="$(abs_path "$REPO_DIR")"
RELEASE_DIR="$(dirname "$REPO_DIR")"

log() {
  echo "[publish-android-ga] $*"
}

cmd=( "$PUBLISH_SCRIPT" "--version" "$VERSION" "--repo-dir" "$REPO_DIR" )
if [[ -n "$REMOTE_URL" ]]; then
  cmd+=("--repo-url" "$REMOTE_URL")
fi
if [[ -n "$REPO_USERNAME" ]]; then
  cmd+=("--username" "$REPO_USERNAME")
fi
if [[ -n "$REPO_PASSWORD" ]]; then
  cmd+=("--password" "$REPO_PASSWORD")
fi
if [[ "$SKIP_SBOM" -eq 1 ]]; then
  cmd+=("--skip-sbom")
fi
if [[ "$DRY_RUN" -eq 1 ]]; then
  cmd+=("--dry-run")
fi

log "Executing ${cmd[*]}"
"${cmd[@]}"

if [[ "$DRY_RUN" -eq 1 ]]; then
  log "Dry-run mode; skipping release bundle assembly"
  exit 0
fi

SBOM_SRC="${ROOT_DIR}/artifacts/android/sbom/${VERSION}"
SBOM_DEST="${RELEASE_ROOT}/${VERSION}/sbom"

mkdir -p "$RELEASE_DIR"

if [[ -d "$SBOM_SRC" ]]; then
  rm -rf "$SBOM_DEST"
  mkdir -p "$(dirname "$SBOM_DEST")"
  cp -R "$SBOM_SRC" "$SBOM_DEST"
  log "Copied SBOM bundle to ${SBOM_DEST}"
else
  log "warning: SBOM bundle ${SBOM_SRC} not found"
fi

README_PATH="${RELEASE_ROOT}/${VERSION}/README.txt"
{
  echo "Android SDK release bundle"
  echo "Version: ${VERSION}"
  echo "Generated: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  echo "Local Maven repo: ${REPO_DIR}"
  if [[ -n "$REMOTE_URL" ]]; then
    echo "Remote Maven repo: ${REMOTE_URL}"
  fi
  if [[ -d "$SBOM_DEST" ]]; then
    echo "SBOM bundle: ${SBOM_DEST}"
  fi
} > "$README_PATH"

log "Release bundle staged under ${RELEASE_ROOT}/${VERSION}"
