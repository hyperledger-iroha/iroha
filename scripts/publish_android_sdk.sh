#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/publish_android_sdk.sh --version <semver> [options]

Build, test, and publish the Android SDK Maven artifacts. The script always
produces a local Maven repository tree (defaults to
artifacts/android/maven/<version>). Pass --repo-url to push the same artifacts
to a remote repository after the local copy succeeds.

Options:
  --version <semver>        SDK version recorded in the published POM (required)
  --repo-dir <path>         Local repository output (default: artifacts/android/maven/<version>)
  --repo-url <url>          Remote Maven repository URL (optional, published after local copy)
  --username <value>        Username for --repo-url (optional)
  --password <value>        Password/token for --repo-url (optional)
  --skip-sbom               Skip SBOM/provenance generation (tests still run)
  --dry-run                 Print the steps without executing them
  -h, --help                Show this message
USAGE
}

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GRADLE_WRAPPER="${ROOT_DIR}/examples/android/gradlew"
SBOM_SCRIPT="${ROOT_DIR}/scripts/android_sbom_provenance.sh"
ANDROID_TESTS="${ROOT_DIR}/java/iroha_android/run_tests.sh"
SAMPLES_SCRIPT="${ROOT_DIR}/scripts/check_android_samples.sh"

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
  REPO_DIR="${ROOT_DIR}/artifacts/android/maven/${VERSION}"
fi

abs_path() {
  python3 - "$1" <<'PY'
import os, sys
print(os.path.abspath(sys.argv[1]))
PY
}

REPO_DIR="$(abs_path "$REPO_DIR")"

log() {
  echo "[publish-android-sdk] $*"
}

execute() {
  local workdir="$1"
  shift
  local -a cmd=("$@")
  local printable
  printf -v printable '%q ' "${cmd[@]}"
  if [[ "$workdir" != "." ]]; then
    local dir_prefix
    printf -v dir_prefix '%q' "$workdir"
    printable="(cd ${dir_prefix} && ${printable})"
  fi
  if [[ "$DRY_RUN" -eq 1 ]]; then
    log "(dry-run) ${printable}"
    return 0
  fi
  log "${printable}"
  if [[ "$workdir" == "." ]]; then
    "${cmd[@]}"
  else
    (
      cd "$workdir"
      "${cmd[@]}"
    )
  fi
}

ensure_gradle_wrapper() {
  if [[ "$DRY_RUN" -eq 0 && ! -x "$GRADLE_WRAPPER" ]]; then
    echo "error: expected Gradle wrapper at ${GRADLE_WRAPPER}" >&2
    exit 1
  fi
}

ensure_gradle_wrapper

if [[ "$DRY_RUN" -eq 0 ]]; then
  rm -rf "$REPO_DIR"
  mkdir -p "$REPO_DIR"
else
  log "(dry-run) would prepare ${REPO_DIR}"
fi

run_quality_checks() {
  if [[ "$SKIP_SBOM" -eq 0 ]]; then
    execute "." "$SBOM_SCRIPT" "$VERSION"
  else
    log "Skipping SBOM/provenance generation; running tests only"
    execute "." "$ANDROID_TESTS"
    execute "." "$SAMPLES_SCRIPT"
  fi
}

run_quality_checks

if [[ "$DRY_RUN" -eq 1 ]]; then
  log "(dry-run) would publish via scripts/android_publish_snapshot.sh with version=${VERSION}"
  exit 0
fi

ANDROID_PUBLISH_VERSION="$VERSION" \
ANDROID_PUBLISH_REPO_DIR="$REPO_DIR" \
ANDROID_PUBLISH_REPO_URL="$REMOTE_URL" \
ANDROID_PUBLISH_REPO_USERNAME="$REPO_USERNAME" \
ANDROID_PUBLISH_REPO_PASSWORD="$REPO_PASSWORD" \
bash "${ROOT_DIR}/scripts/android_publish_snapshot.sh"

log "Local Maven repository ready at ${REPO_DIR}"
if [[ -n "$REMOTE_URL" ]]; then
  log "Remote repository published to ${REMOTE_URL}"
fi
