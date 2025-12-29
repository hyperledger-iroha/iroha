#!/usr/bin/env bash
# Runs the Android sample app builds to ensure the AND5 scaffolds keep compiling.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SAMPLES_DIR="$ROOT_DIR/examples/android"
SCREENSHOT_ROOT="$ROOT_DIR/docs/source/sdk/android/samples/screenshots"
LOCALIZATION_HELPER="$ROOT_DIR/scripts/android_sample_localization.py"
REQUIRED_SAMPLE_LOCALES=("en" "ja" "he")

if [[ ! -x "$SAMPLES_DIR/gradlew" ]]; then
  echo "Android sample Gradle helper not found under $SAMPLES_DIR" >&2
  exit 1
fi

( cd "$SAMPLES_DIR" && ./gradlew \
    :android-sdk:jar \
    :operator-console:assembleDebug \
    :operator-console:generateSampleManifest \
    :retail-wallet:assembleDebug \
    :retail-wallet:generateSampleManifest )

echo "==> Building minimal samples-android app (module-local AAR)"
ANDROID_GRADLE="${ROOT_DIR}/examples/android/gradlew"
if [[ -x "$ANDROID_GRADLE" ]]; then
  GRADLE_BIN="$ANDROID_GRADLE"
else
  GRADLE_BIN="${GRADLE:-gradle}"
fi
if ! "$GRADLE_BIN" -p "$ROOT_DIR/java/iroha_android" :samples-android:assembleDebug; then
  echo "Failed to build java/iroha_android/samples-android" >&2
  exit 1
fi

annotate_manifest_localization() {
  local sample="$1"
  local manifest_path="$2"

  if [[ ! -f "$manifest_path" ]]; then
    echo "warning: manifest for $sample not found at $manifest_path" >&2
    return
  fi

  if [[ ! -f "$LOCALIZATION_HELPER" ]]; then
    echo "warning: missing helper $LOCALIZATION_HELPER; skipping localization metadata" >&2
    return
  fi

  python3 "$LOCALIZATION_HELPER" \
    --manifest "$manifest_path" \
    --sample "$sample" \
    --screenshots-root "$SCREENSHOT_ROOT" \
    --locales "${REQUIRED_SAMPLE_LOCALES[@]}" \
    --repo-root "$ROOT_DIR"
}

for module in operator-console retail-wallet; do
  manifest="$SAMPLES_DIR/$module/build/sample-manifest/sample_manifest.json"
  if [[ ! -s "$manifest" ]]; then
    echo "Missing manifest for $module at $manifest" >&2
    exit 1
  fi
  annotate_manifest_localization "$module" "$manifest"
  echo "=== $module sample_manifest.json ==="
  cat "$manifest"
done
