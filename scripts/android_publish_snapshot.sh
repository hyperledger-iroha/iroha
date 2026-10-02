#!/usr/bin/env bash
# Publish the canonical three Kotlin SDK modules and retain exact runtime/POM,
# signed SBOM and native provenance hashes. This is not release qualification.
# Prerequisites: clean source, existing canonical external artifact root, JDK21,
# Android SDK, existing pinned Rust/NDK/source-seal inputs, host JNI and cosign.
# Existing output generations are never overwritten. No source/provenance waiver.
set -euo pipefail
if [[ "${1:-}" == --help || "${1:-}" == -h ]]; then
  cat <<'USAGE'
Usage: scripts/android_publish_snapshot.sh
Required: ANDROID_PUBLISH_VERSION, MOBILE_SDK_ANDROID_ARTIFACT_DIR.
Optional new external outputs: ANDROID_PUBLISH_REPO_DIR,
ANDROID_PUBLISH_REPORT_DIR, MOBILE_SDK_SBOM_OUTPUT_DIR.
ANDROID_PUBLISH_DRY_RUN=1 prints canonical tasks without changes.
ANDROID_PUBLISH_SIGN=1 additionally signs published artifacts with cosign.
Source/NDK/native provenance and signed SBOMs remain mandatory.
Tests and lint are optional development diagnostics, outside publication.
Optional ANDROID_PUBLISH_REPO_URL selects the same three remote publications.
ANDROID_PUBLISH_REPO_USERNAME/PASSWORD are a runtime-only complete pair.
Skipped SBOM/sample quality policies are refused.
USAGE
  exit 0
fi
[[ $# == 0 ]] || { echo 'error: snapshot publisher takes environment inputs only' >&2; exit 64; }
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
VERSION="${ANDROID_PUBLISH_VERSION:?ANDROID_PUBLISH_VERSION is required}"
ARTIFACT_DIR="${MOBILE_SDK_ANDROID_ARTIFACT_DIR:?MOBILE_SDK_ANDROID_ARTIFACT_DIR is required}"
REPO_DIR="${ANDROID_PUBLISH_REPO_DIR:-$ARTIFACT_DIR/maven}"
REPORT_DIR="${ANDROID_PUBLISH_REPORT_DIR:-$ARTIFACT_DIR/publication-$VERSION}"
SBOM_DIR="${MOBILE_SDK_SBOM_OUTPUT_DIR:-$ARTIFACT_DIR/sbom-$VERSION}"
DRY_RUN="${ANDROID_PUBLISH_DRY_RUN:-0}"
SIGN_ARTIFACTS="${ANDROID_PUBLISH_SIGN:-0}"
[[ "$VERSION" =~ ^[A-Za-z0-9][A-Za-z0-9._+-]*$ && "$DRY_RUN" =~ ^[01]$ && "$SIGN_ARTIFACTS" =~ ^[01]$ ]] || {
  echo 'error: invalid canonical publication version or policy' >&2; exit 64;
}
[[ -z "${ANDROID_PUBLISH_SKIP_SAMPLE:-}" ]] || {
  echo 'error: retired quality publication waiver is not admitted' >&2; exit 64;
}
REMOTE_URL="${ANDROID_PUBLISH_REPO_URL:-}"
REMOTE_USERNAME="${ANDROID_PUBLISH_REPO_USERNAME:-}"
REMOTE_PASSWORD="${ANDROID_PUBLISH_REPO_PASSWORD:-}"
PYTHON="${MOBILE_SDK_PYTHON_BINARY:-python3}"
GRADLE="$ROOT_DIR/kotlin/gradlew"
OWNER="$ROOT_DIR/scripts/mobile_sdk_android_artifacts.py"
PUBLICATION_OWNER="$ROOT_DIR/scripts/mobile_sdk_android_publication.py"
[[ -x "$GRADLE" ]] || { echo 'error: canonical Kotlin Gradle wrapper is missing' >&2; exit 1; }
"$PYTHON" -I -S -B "$OWNER" --root "$ROOT_DIR" --artifact-dir "$ARTIFACT_DIR" --print-build-root >/dev/null
"$PYTHON" -I -S -B "$PUBLICATION_OWNER" validate \
  --root "$ROOT_DIR" --artifact-dir "$ARTIFACT_DIR" --repo "$REPO_DIR" \
  --report "$REPORT_DIR" --sbom "$SBOM_DIR" --version "$VERSION" \
  --remote-url "$REMOTE_URL"
# Environment-only credentials keep Gradle project arguments/receipts public.
[[ -n "$REMOTE_URL" || -z "$REMOTE_USERNAME$REMOTE_PASSWORD" ]] || {
  echo 'error: remote credentials require an explicit remote repository' >&2; exit 64;
}
[[ (-z "$REMOTE_USERNAME" && -z "$REMOTE_PASSWORD") || (-n "$REMOTE_USERNAME" && -n "$REMOTE_PASSWORD") ]] || {
  echo 'error: remote credentials must be a complete pair' >&2; exit 64;
}
export IROHA_SDK_MAVEN_URL="$REMOTE_URL"
export IROHA_SDK_MAVEN_USERNAME="$REMOTE_USERNAME" IROHA_SDK_MAVEN_PASSWORD="$REMOTE_PASSWORD"
if [[ -z "$REMOTE_URL" ]]; then unset IROHA_SDK_MAVEN_URL IROHA_SDK_MAVEN_USERNAME IROHA_SDK_MAVEN_PASSWORD; fi
if [[ -n "$REMOTE_URL" && -z "$REMOTE_USERNAME" ]]; then unset IROHA_SDK_MAVEN_USERNAME IROHA_SDK_MAVEN_PASSWORD; fi
TASKS=(
  :core-jvm:publishReleasePublicationToMobileSdkRepository
  :client-android:publishReleasePublicationToMobileSdkRepository
  :kagemusha-wallet-android:publishReleasePublicationToMobileSdkRepository
)
ARGS=(-p "$ROOT_DIR/kotlin" --no-daemon --no-configuration-cache
  --project-cache-dir "$ARTIFACT_DIR/publish-project-cache"
  "-PirohaSdkVersion=$VERSION" "-PirohaSdkRepoDir=$REPO_DIR")
REMOTE_TASKS=(:core-jvm:publishReleasePublicationToRemoteSdkRepository
  :client-android:publishReleasePublicationToRemoteSdkRepository
  :kagemusha-wallet-android:publishReleasePublicationToRemoteSdkRepository)
if [[ "$DRY_RUN" == 1 ]]; then
  printf '[android-publish] dry-run: signed canonical SBOMs, then '
  printf '%q ' "$GRADLE" "${ARGS[@]}" "${TASKS[@]}"
  if [[ -n "$REMOTE_URL" ]]; then
    printf '\n[android-publish] dry-run: after local admission, '
    printf '%q ' "$GRADLE" "${ARGS[@]}" "${REMOTE_TASKS[@]}"
  fi
  printf '\n[android-publish] dry-run: validate complete built/Maven graph and native outputs; retain receipt\n'
  exit 0
fi
SOURCE_COMMIT="$(git -C "$ROOT_DIR" rev-parse HEAD)"
assert_source() {
  [[ "$(git -C "$ROOT_DIR" rev-parse HEAD)" == "$SOURCE_COMMIT" && -z "$(git -C "$ROOT_DIR" status --porcelain --untracked-files=all)" ]] || {
    echo 'error: canonical publication refuses dirty or changed release source' >&2; exit 1;
  }
}
assert_source
bash "$ROOT_DIR/scripts/check_mobile_sdk_artifacts.sh" --root "$ROOT_DIR" --android-only
MOBILE_SDK_SBOM_OUTPUT_DIR="$SBOM_DIR" bash "$ROOT_DIR/scripts/android_sbom_provenance.sh" "$VERSION"
assert_source
# Claim the generation exclusively before Gradle may write it. Failure leaves
# diagnostic outputs for the operator; no later invocation overwrites them.
mkdir -m 700 "$REPO_DIR"
"$GRADLE" "${ARGS[@]}" "${TASKS[@]}"
assert_source
"$PYTHON" -I -S -B "$OWNER" --root "$ROOT_DIR" --artifact-dir "$ARTIFACT_DIR" \
  --version "$VERSION" --maven-repo "$REPO_DIR" > /dev/null
MOBILE_SDK_MAVEN_VERSION="$VERSION" bash "$ROOT_DIR/scripts/check_mobile_sdk_artifacts.sh" \
  --root "$ROOT_DIR" --android-only --require-built-android
assert_source
if [[ -n "$REMOTE_URL" ]]; then
  assert_source
  "$GRADLE" "${ARGS[@]}" "${REMOTE_TASKS[@]}"
  assert_source
fi
"$PYTHON" -I -S -B "$PUBLICATION_OWNER" receipt \
  --root "$ROOT_DIR" --artifact-dir "$ARTIFACT_DIR" --repo "$REPO_DIR" \
  --report "$REPORT_DIR" --sbom "$SBOM_DIR" --version "$VERSION" \
  --source-commit "$SOURCE_COMMIT" --remote-url "$REMOTE_URL"
if [[ "$SIGN_ARTIFACTS" == 1 ]]; then
  COSIGN_BIN="${ANDROID_PUBLISH_COSIGN_BIN:-${COSIGN:-cosign}}"
  command -v "$COSIGN_BIN" >/dev/null || { echo 'error: artifact cosign is missing' >&2; exit 1; }
  mkdir -m 700 "$REPORT_DIR/signatures"
  while IFS= read -r artifact; do
    "$COSIGN_BIN" sign-blob --yes --bundle "$REPORT_DIR/signatures/$(basename "$artifact").sigstore" "$artifact"
  done < <("$PYTHON" -I -S -B "$OWNER" --root "$ROOT_DIR" --artifact-dir "$ARTIFACT_DIR" \
    --version "$VERSION" --maven-repo "$REPO_DIR")
fi
assert_source
printf '[android-publish] complete canonical Maven graph: %s\n[android-publish] structural receipt: %s\n' "$REPO_DIR" "$REPORT_DIR"
