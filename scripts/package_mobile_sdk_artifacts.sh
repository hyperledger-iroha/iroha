#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/package_mobile_sdk_artifacts.sh [--root <repo-root>] [--version <version>] [--apple] [--android]

Packages built mobile SDK artifacts into dist/mobile-sdk:
  --apple    Package dist/NoritoBridge.xcframework and its artifact manifest.
  --android  Package Kotlin core/client/offline-wallet Android release outputs.

When neither --apple nor --android is passed, both platforms are packaged.
USAGE
}

ROOT_ARG=""
VERSION=""
PACKAGE_APPLE=0
PACKAGE_ANDROID=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --root)
      shift
      if [[ $# -eq 0 ]]; then
        echo "[mobile-sdk-package] ERROR: --root requires a value" >&2
        exit 64
      fi
      ROOT_ARG="$1"
      ;;
    --root=*)
      ROOT_ARG="${1#*=}"
      ;;
    --version)
      shift
      if [[ $# -eq 0 ]]; then
        echo "[mobile-sdk-package] ERROR: --version requires a value" >&2
        exit 64
      fi
      VERSION="$1"
      ;;
    --version=*)
      VERSION="${1#*=}"
      ;;
    --apple)
      PACKAGE_APPLE=1
      ;;
    --android)
      PACKAGE_ANDROID=1
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "[mobile-sdk-package] ERROR: unexpected argument: $1" >&2
      usage >&2
      exit 64
      ;;
  esac
  shift
done

if [[ -z "$ROOT_ARG" ]]; then
  ROOT_ARG="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fi
ROOT_DIR="$(cd "$ROOT_ARG" && pwd)"

if [[ "$PACKAGE_APPLE" == "0" && "$PACKAGE_ANDROID" == "0" ]]; then
  PACKAGE_APPLE=1
  PACKAGE_ANDROID=1
fi

if [[ -z "$VERSION" ]]; then
  VERSION_SOURCE="$ROOT_DIR/IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"
  if [[ -f "$VERSION_SOURCE" ]]; then
    if command -v rg >/dev/null 2>&1; then
      VERSION="$(rg -n "expectedVersion" "$VERSION_SOURCE" | head -n1 | sed -E 's/.*"([^"]+)".*/\1/')"
    else
      VERSION="$(grep -m1 "expectedVersion" "$VERSION_SOURCE" | sed -E 's/.*"([^"]+)".*/\1/')"
    fi
  fi
fi

if [[ -z "$VERSION" ]]; then
  VERSION="$(git -C "$ROOT_DIR" describe --tags --always --dirty 2>/dev/null || true)"
fi

if [[ -z "$VERSION" ]]; then
  echo "[mobile-sdk-package] ERROR: unable to determine artifact version" >&2
  exit 65
fi

VERSION="${VERSION//\//-}"
if [[ ! "$VERSION" =~ ^[A-Za-z0-9._+-]+$ ]]; then
  echo "[mobile-sdk-package] ERROR: version contains unsupported filename characters: $VERSION" >&2
  exit 65
fi

OUT_DIR="$ROOT_DIR/dist/mobile-sdk"
MODE_LABEL="all"
if [[ "$PACKAGE_APPLE" == "1" && "$PACKAGE_ANDROID" == "0" ]]; then
  MODE_LABEL="apple"
elif [[ "$PACKAGE_APPLE" == "0" && "$PACKAGE_ANDROID" == "1" ]]; then
  MODE_LABEL="android"
fi

CHECKSUMS="$OUT_DIR/SHA256SUMS-${MODE_LABEL}-${VERSION}.txt"
MANIFEST="$OUT_DIR/mobile-sdk-${MODE_LABEL}-${VERSION}.artifacts.json"
ARTIFACT_RECORDS=()

hash_file() {
  local path="$1"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$path" | awk '{print $1}'
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$path" | awk '{print $1}'
  else
    python3 - "$path" <<'PY'
import hashlib
import sys

hasher = hashlib.sha256()
with open(sys.argv[1], "rb") as handle:
    for chunk in iter(lambda: handle.read(1024 * 1024), b""):
        hasher.update(chunk)
print(hasher.hexdigest())
PY
  fi
}

require_file() {
  local path="$1"
  local label="$2"
  if [[ ! -f "$path" ]]; then
    echo "[mobile-sdk-package] ERROR: missing $label: $path" >&2
    exit 66
  fi
}

require_dir() {
  local path="$1"
  local label="$2"
  if [[ ! -d "$path" ]]; then
    echo "[mobile-sdk-package] ERROR: missing $label: $path" >&2
    exit 66
  fi
}

single_match() {
  local pattern="$1"
  local label="$2"
  local matches=()
  local match

  while IFS= read -r match; do
    matches+=("$match")
  done < <(compgen -G "$pattern" || true)

  if [[ "${#matches[@]}" -ne 1 ]]; then
    echo "[mobile-sdk-package] ERROR: expected exactly one $label for pattern $pattern, found ${#matches[@]}" >&2
    exit 66
  fi

  printf '%s' "${matches[0]}"
}

resolve_core_jar() {
  local stripped_version="${VERSION#v}"
  local candidate

  for candidate in \
    "$ROOT_DIR/kotlin/core-jvm/build/libs/core-jvm-${VERSION}.jar" \
    "$ROOT_DIR/kotlin/core-jvm/build/libs/core-jvm-${stripped_version}.jar"; do
    if [[ -f "$candidate" ]]; then
      printf '%s' "$candidate"
      return
    fi
  done

  single_match "$ROOT_DIR/kotlin/core-jvm/build/libs/core-jvm-*.jar" "core-jvm built jar"
}

record_artifact() {
  local path="$1"
  local kind="$2"
  local name rel sha bytes

  require_file "$path" "$kind artifact"
  name="$(basename "$path")"
  rel="${path#$ROOT_DIR/}"
  sha="$(hash_file "$path")"
  bytes="$(wc -c < "$path" | tr -d '[:space:]')"
  printf '%s  %s\n' "$sha" "$rel" >> "$CHECKSUMS"
  ARTIFACT_RECORDS+=("    {\"kind\":\"$kind\",\"name\":\"$name\",\"path\":\"$rel\",\"sha256\":\"$sha\",\"bytes\":$bytes}")
}

copy_android_artifact() {
  local src="$1"
  local dest="$2"
  local stage="$3"
  local stage_checksums="$4"
  local sha

  require_file "$src" "Android SDK package input"
  mkdir -p "$(dirname "$stage/$dest")"
  cp "$src" "$stage/$dest"
  sha="$(hash_file "$stage/$dest")"
  printf '%s  %s\n' "$sha" "$dest" >> "$stage_checksums"
}

write_manifest() {
  local index count
  count="${#ARTIFACT_RECORDS[@]}"
  if [[ "$count" -eq 0 ]]; then
    echo "[mobile-sdk-package] ERROR: no artifacts were packaged" >&2
    exit 70
  fi

  {
    printf '{\n'
    printf '  "version": "%s",\n' "$VERSION"
    printf '  "mode": "%s",\n' "$MODE_LABEL"
    printf '  "artifacts": [\n'
    for index in "${!ARTIFACT_RECORDS[@]}"; do
      if [[ "$index" -gt 0 ]]; then
        printf ',\n'
      fi
      printf '%s' "${ARTIFACT_RECORDS[$index]}"
    done
    printf '\n  ]\n'
    printf '}\n'
  } > "$MANIFEST"

  record_artifact "$MANIFEST" "manifest"
}

package_apple() {
  local xcframework="$ROOT_DIR/dist/NoritoBridge.xcframework"
  local bridge_manifest="$ROOT_DIR/dist/NoritoBridge.artifacts.json"
  local apple_zip="$OUT_DIR/NoritoBridge-${VERSION}.xcframework.zip"
  local versioned_manifest="$OUT_DIR/NoritoBridge-${VERSION}.artifacts.json"

  bash "$ROOT_DIR/scripts/check_mobile_sdk_artifacts.sh" --root "$ROOT_DIR" --apple-only
  require_dir "$xcframework" "NoritoBridge XCFramework"
  require_file "$bridge_manifest" "NoritoBridge artifact manifest"

  rm -f "$apple_zip" "$versioned_manifest"
  if command -v ditto >/dev/null 2>&1; then
    ditto -c -k --sequesterRsrc --keepParent "$xcframework" "$apple_zip"
  else
    (cd "$ROOT_DIR/dist" && zip -qr "$apple_zip" NoritoBridge.xcframework)
  fi
  cp "$bridge_manifest" "$versioned_manifest"

  record_artifact "$apple_zip" "apple-xcframework"
  record_artifact "$versioned_manifest" "apple-manifest"
}

package_android() {
  local stage="$OUT_DIR/iroha-mobile-sdk-android-${VERSION}"
  local stage_checksums="$stage/SHA256SUMS.txt"
  local android_zip="$OUT_DIR/iroha-mobile-sdk-android-${VERSION}.zip"
  local maven_repo="$ROOT_DIR/dist/mobile-sdk-maven"
  local core_jar
  local rel

  bash "$ROOT_DIR/scripts/check_mobile_sdk_artifacts.sh" --root "$ROOT_DIR" --android-only --require-built-android
  rm -rf "$stage" "$android_zip"
  mkdir -p "$stage"
  : > "$stage_checksums"

  core_jar="$(resolve_core_jar)"
  copy_android_artifact \
    "$core_jar" \
    "core-jvm/$(basename "$core_jar")" \
    "$stage" \
    "$stage_checksums"
  copy_android_artifact \
    "$ROOT_DIR/kotlin/client-android/build/outputs/aar/client-android-release.aar" \
    "client-android/client-android-release.aar" \
    "$stage" \
    "$stage_checksums"
  copy_android_artifact \
    "$ROOT_DIR/kotlin/offline-wallet-android/build/outputs/aar/offline-wallet-android-release.aar" \
    "offline-wallet-android/offline-wallet-android-release.aar" \
    "$stage" \
    "$stage_checksums"
  copy_android_artifact \
    "$ROOT_DIR/kotlin/client-android/src/main/jniLibs/arm64-v8a/libconnect_norito_bridge.so" \
    "native/arm64-v8a/libconnect_norito_bridge.so" \
    "$stage" \
    "$stage_checksums"
  copy_android_artifact \
    "$ROOT_DIR/kotlin/client-android/src/main/jniLibs/x86_64/libconnect_norito_bridge.so" \
    "native/x86_64/libconnect_norito_bridge.so" \
    "$stage" \
    "$stage_checksums"

  if [[ -d "$maven_repo" ]]; then
    while IFS= read -r rel; do
      rel="${rel#./}"
      copy_android_artifact "$maven_repo/$rel" "maven/$rel" "$stage" "$stage_checksums"
    done < <(cd "$maven_repo" && find . -type f | sort)
  fi

  (cd "$OUT_DIR" && zip -qr "$(basename "$android_zip")" "$(basename "$stage")")
  rm -rf "$stage"
  record_artifact "$android_zip" "android-sdk"
}

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"
: > "$CHECKSUMS"

if [[ "$PACKAGE_APPLE" == "1" ]]; then
  package_apple
fi

if [[ "$PACKAGE_ANDROID" == "1" ]]; then
  package_android
fi

write_manifest

echo "[mobile-sdk-package] wrote artifacts to $OUT_DIR"
echo "[mobile-sdk-package] wrote checksums to $CHECKSUMS"
echo "[mobile-sdk-package] wrote manifest to $MANIFEST"
