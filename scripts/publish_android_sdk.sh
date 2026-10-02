#!/usr/bin/env bash
# Canonical Kotlin Android publication entry point. JDK21, Android SDK, exact
# pinned native tools/source and cosign are required by the existing builders.
# Outputs must be new external directories; no dirty/source/quality waiver.
set -euo pipefail
usage() {
  cat <<'USAGE'
Usage: scripts/publish_android_sdk.sh --version <version> [options]

Test and publish core-jvm, client-android and kagemusha-wallet-android at the
same Maven version through kotlin/gradlew. A leading release-tag v is removed
once. Java consumers retain the canonical Kotlin JDK8 API checks.

Required: MOBILE_SDK_ANDROID_ARTIFACT_DIR (existing canonical external build
root), pinned native build inputs, JDK21, Android SDK and cosign.
Options:
  --repo-dir <path>    New external local Maven repository (default: build root/maven)
  --report-dir <path>  New external publication receipt directory
  --sbom-dir <path>    New external signed CycloneDX inventory directory
  --repo-url <https>   Optional remote Maven repository; local validated graph is retained
  --username <value>   Runtime-only remote username (or ANDROID_PUBLISH_REPO_USERNAME)
  --password <value>   Runtime-only remote password (prefer ANDROID_PUBLISH_REPO_PASSWORD)
  --dry-run           Print canonical tasks without running them or creating outputs
  --help              Show this help
--skip-sbom is refused. The remote repository uses the same three canonical
publications; credentials are passed to Gradle only through its environment,
never project arguments or receipt fields. Runtime environment inputs are
preferred to command-line credentials.
USAGE
}
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
VERSION=""; REPO_DIR=""; REPORT_DIR=""; SBOM_DIR=""; DRY_RUN=0
REMOTE_URL="${ANDROID_PUBLISH_REPO_URL:-}"
REMOTE_USERNAME="${ANDROID_PUBLISH_REPO_USERNAME:-}"
REMOTE_PASSWORD="${ANDROID_PUBLISH_REPO_PASSWORD:-}"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --version|--repo-dir|--report-dir|--sbom-dir|--repo-url|--username|--password)
      [[ $# -ge 2 && -n "$2" ]] || { usage >&2; exit 64; }
      case "$1" in
        --version) VERSION="$2" ;; --repo-dir) REPO_DIR="$2" ;;
        --report-dir) REPORT_DIR="$2" ;; --sbom-dir) SBOM_DIR="$2" ;;
        --repo-url) REMOTE_URL="$2" ;; --username) REMOTE_USERNAME="$2" ;; --password) REMOTE_PASSWORD="$2" ;;
      esac
      shift 2 ;;
    --dry-run) DRY_RUN=1; shift ;;
    --help|-h) usage; exit 0 ;;
    --skip-sbom)
      echo 'error: canonical publication requires signed SBOMs; --skip-sbom is refused' >&2
      exit 64 ;;
    *) echo 'error: unsupported Android publication argument' >&2; usage >&2; exit 64 ;;
  esac
done
[[ -n "$VERSION" ]] || { echo 'error: --version is required' >&2; exit 64; }
export ANDROID_PUBLISH_REPO_URL="$REMOTE_URL" ANDROID_PUBLISH_REPO_USERNAME="$REMOTE_USERNAME" ANDROID_PUBLISH_REPO_PASSWORD="$REMOTE_PASSWORD"
export ANDROID_PUBLISH_VERSION="${VERSION#v}" ANDROID_PUBLISH_DRY_RUN="$DRY_RUN"
[[ -z "$REPO_DIR" ]] || export ANDROID_PUBLISH_REPO_DIR="$REPO_DIR"
[[ -z "$REPORT_DIR" ]] || export ANDROID_PUBLISH_REPORT_DIR="$REPORT_DIR"
[[ -z "$SBOM_DIR" ]] || export MOBILE_SDK_SBOM_OUTPUT_DIR="$SBOM_DIR"
exec /bin/bash "$ROOT_DIR/scripts/android_publish_snapshot.sh"
