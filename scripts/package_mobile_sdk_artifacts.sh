#!/usr/bin/env bash
set -euo pipefail
umask 077
PATH=/usr/bin:/bin
export PATH
unset \
  DYLD_INSERT_LIBRARIES \
  DYLD_LIBRARY_PATH \
  LD_LIBRARY_PATH \
  LD_PRELOAD \
  SDKROOT \
  PYTHONHOME \
  PYTHONPATH

resolve_trusted_python312() {
  local candidate canonical
  local override="${MOBILE_SDK_PYTHON_BINARY:-}"
  local candidates=()

  if [[ -n "$override" ]]; then
    if [[ "$override" != /* || ! -f "$override" || -L "$override" || ! -x "$override" ]]; then
      echo "[mobile-sdk-package] ERROR: MOBILE_SDK_PYTHON_BINARY must be an absolute canonical regular executable" >&2
      return 1
    fi
    candidates=("$override")
  else
    candidates=(
      /opt/homebrew/opt/python@3.12/bin/python3.12
      /opt/homebrew/bin/python3.12
      /usr/local/opt/python@3.12/bin/python3.12
      /usr/local/bin/python3.12
      /usr/bin/python3.12
      /usr/bin/python3
    )
  fi

  for candidate in "${candidates[@]}"; do
    [[ -f "$candidate" && -x "$candidate" ]] || continue
    if ! canonical="$(
      env -i \
        HOME=/tmp \
        PATH=/usr/bin:/bin \
        TMPDIR=/tmp \
        LANG=C.UTF-8 \
        LC_ALL=C.UTF-8 \
        "$candidate" -I -S -c '
import os
import pathlib
import stat
import sys

if sys.version_info[:2] != (3, 12) or not sys.flags.isolated:
    raise SystemExit(1)
if "SDKROOT" in os.environ:
    raise SystemExit(1)
resolved = pathlib.Path(sys.executable).resolve(strict=True)
metadata = resolved.stat()
if not stat.S_ISREG(metadata.st_mode) or not os.access(resolved, os.X_OK):
    raise SystemExit(1)
print(resolved)
'
    )"; then
      continue
    fi
    if [[ "$canonical" != /* || ! -f "$canonical" || -L "$canonical" || ! -x "$canonical" ]]; then
      continue
    fi
    if [[ -n "$override" && "$canonical" != "$override" ]]; then
      echo "[mobile-sdk-package] ERROR: MOBILE_SDK_PYTHON_BINARY must already name its canonical executable" >&2
      return 1
    fi
    printf '%s\n' "$canonical"
    return 0
  done

  if [[ -n "$override" ]]; then
    echo "[mobile-sdk-package] ERROR: MOBILE_SDK_PYTHON_BINARY must be an isolated Python 3.12 executable" >&2
  else
    echo "[mobile-sdk-package] ERROR: a trusted absolute Python 3.12 executable is required" >&2
  fi
  return 1
}

PYTHON_BINARY="$(resolve_trusted_python312)" || exit 1

run_isolated_python() {
  env -i \
    HOME=/tmp \
    PATH=/usr/bin:/bin \
    TMPDIR=/tmp \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    "$PYTHON_BINARY" -I -S "$@"
}

usage() {
  cat <<'USAGE'
Usage:
  scripts/package_mobile_sdk_artifacts.sh [--root <repo-root>] [--version <version>] [--apple] [--android]

Packages built mobile SDK artifacts into dist/mobile-sdk by default:
  --apple    Package NoritoBridge.xcframework and its artifact manifest.
  --android  Package Kotlin core/client/offline-wallet Android release outputs,
             generated native bridge bytes, and their embedded provenance.

MOBILE_SDK_APPLE_ARTIFACT_DIR may select an external Apple artifact directory.
MOBILE_SDK_ANDROID_ARTIFACT_DIR is required for Android release packaging and
must identify the canonical external Gradle/artifact root.
MOBILE_SDK_PACKAGE_OUT_DIR may select a dedicated external package directory
whose final path component contains "mobile-sdk".
MOBILE_SDK_PYTHON_BINARY may select an absolute, already-canonical regular
Python 3.12 executable when the fixed Homebrew/system locators are unavailable.

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
ROOT_DIR="$(cd "$ROOT_ARG" && pwd -P)"
APPLE_ARTIFACT_DIR="${MOBILE_SDK_APPLE_ARTIFACT_DIR:-$ROOT_DIR/dist}"
if [[ "$APPLE_ARTIFACT_DIR" != /* ]]; then
  APPLE_ARTIFACT_DIR="$ROOT_DIR/$APPLE_ARTIFACT_DIR"
fi

if [[ "$PACKAGE_APPLE" == "0" && "$PACKAGE_ANDROID" == "0" ]]; then
  PACKAGE_APPLE=1
  PACKAGE_ANDROID=1
fi

ANDROID_ARTIFACT_DIR="${MOBILE_SDK_ANDROID_ARTIFACT_DIR:-}"
ANDROID_KOTLIN_BUILD_ROOT=""
ANDROID_MAVEN_REPO_DIR=""
if [[ "$PACKAGE_ANDROID" == "1" ]]; then
  if [[ -z "$ANDROID_ARTIFACT_DIR" || "$ANDROID_ARTIFACT_DIR" != /* \
    || ! -d "$ANDROID_ARTIFACT_DIR" || -L "$ANDROID_ARTIFACT_DIR" ]]; then
    echo "[mobile-sdk-package] ERROR: Android release packaging requires an absolute non-symbolic MOBILE_SDK_ANDROID_ARTIFACT_DIR" >&2
    exit 66
  fi
  canonical_android_artifact_dir="$(cd "$ANDROID_ARTIFACT_DIR" && pwd -P)"
  if [[ "$canonical_android_artifact_dir" != "$ANDROID_ARTIFACT_DIR" ]]; then
    echo "[mobile-sdk-package] ERROR: MOBILE_SDK_ANDROID_ARTIFACT_DIR must be canonical" >&2
    exit 66
  fi
  case "$ANDROID_ARTIFACT_DIR/" in
    "$ROOT_DIR/"*)
      echo "[mobile-sdk-package] ERROR: MOBILE_SDK_ANDROID_ARTIFACT_DIR must be outside the Iroha source tree" >&2
      exit 66
      ;;
  esac
  ANDROID_KOTLIN_BUILD_ROOT="$ANDROID_ARTIFACT_DIR/gradle-build/iroha_kotlin_sdk"
  ANDROID_MAVEN_REPO_DIR="${MOBILE_SDK_ANDROID_MAVEN_REPO_DIR:-$ANDROID_ARTIFACT_DIR/maven}"
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

OUT_DIR="${MOBILE_SDK_PACKAGE_OUT_DIR:-$ROOT_DIR/dist/mobile-sdk}"
if [[ "$OUT_DIR" != /* ]]; then
  OUT_DIR="$ROOT_DIR/$OUT_DIR"
fi
case "$OUT_DIR/" in
  *"/../"*|*"/./"*|*"//"*)
    echo "[mobile-sdk-package] ERROR: package output path must be canonical: $OUT_DIR" >&2
    exit 65
    ;;
esac
OUT_BASENAME="${OUT_DIR##*/}"
case "$OUT_BASENAME" in
  *mobile-sdk*) ;;
  *)
    echo "[mobile-sdk-package] ERROR: package output must be a dedicated mobile-sdk directory: $OUT_DIR" >&2
    exit 65
    ;;
esac
OUT_PARENT="${OUT_DIR%/*}"
mkdir -p "$OUT_PARENT"
OUT_PARENT="$(cd "$OUT_PARENT" && pwd -P)"
OUT_DIR="$OUT_PARENT/$OUT_BASENAME"
if [[ -L "$OUT_DIR" ]]; then
  echo "[mobile-sdk-package] ERROR: package output must not be a symbolic link: $OUT_DIR" >&2
  exit 65
fi
case "$OUT_DIR" in
  /|"$ROOT_DIR"|"$ROOT_DIR/dist"|"$APPLE_ARTIFACT_DIR")
    echo "[mobile-sdk-package] ERROR: refusing broad package output path: $OUT_DIR" >&2
    exit 65
    ;;
esac
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
    run_isolated_python - "$path" <<'PY'
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
    "$ANDROID_KOTLIN_BUILD_ROOT/core-jvm/libs/core-jvm-${VERSION}.jar" \
    "$ANDROID_KOTLIN_BUILD_ROOT/core-jvm/libs/core-jvm-${stripped_version}.jar"; do
    if [[ -f "$candidate" ]]; then
      printf '%s' "$candidate"
      return
    fi
  done

  single_match "$ANDROID_KOTLIN_BUILD_ROOT/core-jvm/libs/core-jvm-*.jar" "core-jvm built jar"
}

resolve_android_native_mode() {
  local aar="$1"
  run_isolated_python - "$aar" <<'PY'
import json
import sys
import zipfile

entry = "assets/iroha/native-build-provenance-v1.json"
with zipfile.ZipFile(sys.argv[1]) as archive:
    manifest = json.loads(archive.read(entry))
production = manifest.get("privacy_production_enabled")
if type(production) is not bool:
    raise SystemExit("native provenance privacy_production_enabled is not boolean")
print("production" if production else "default")
PY
}

record_artifact() {
  local path="$1"
  local kind="$2"
  local name rel sha bytes

  require_file "$path" "$kind artifact"
  name="$(basename "$path")"
  if [[ "$path" == "$OUT_DIR/"* ]]; then
    rel="${path#"$OUT_DIR/"}"
  else
    rel="${path#"$ROOT_DIR/"}"
  fi
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
  local artifact_root
  local xcframework
  local bridge_manifest
  local apple_zip="$OUT_DIR/NoritoBridge-${VERSION}.xcframework.zip"
  local versioned_manifest="$OUT_DIR/NoritoBridge-${VERSION}.artifacts.json"

  require_dir "$APPLE_ARTIFACT_DIR" "Apple artifact directory"
  if [[ -L "$APPLE_ARTIFACT_DIR" ]]; then
    echo "[mobile-sdk-package] ERROR: Apple artifact directory must not be a symbolic link: $APPLE_ARTIFACT_DIR" >&2
    exit 66
  fi
  artifact_root="$(cd "$APPLE_ARTIFACT_DIR" && pwd -P)"
  if [[ "$artifact_root" != "$APPLE_ARTIFACT_DIR" ]]; then
    echo "[mobile-sdk-package] ERROR: Apple artifact directory must be canonical: $APPLE_ARTIFACT_DIR" >&2
    exit 66
  fi
  xcframework="$artifact_root/NoritoBridge.xcframework"
  bridge_manifest="$artifact_root/NoritoBridge.artifacts.json"

  MOBILE_SDK_APPLE_ARTIFACT_DIR="$artifact_root" \
    bash "$ROOT_DIR/scripts/check_mobile_sdk_artifacts.sh" --root "$ROOT_DIR" --apple-only
  require_dir "$xcframework" "NoritoBridge XCFramework"
  require_file "$bridge_manifest" "NoritoBridge artifact manifest"

  rm -f "$apple_zip" "$versioned_manifest"
  if command -v ditto >/dev/null 2>&1; then
    ditto -c -k --sequesterRsrc --keepParent "$xcframework" "$apple_zip"
  else
    (cd "$artifact_root" && zip -qr "$apple_zip" NoritoBridge.xcframework)
  fi
  cp "$bridge_manifest" "$versioned_manifest"

  record_artifact "$apple_zip" "apple-xcframework"
  record_artifact "$versioned_manifest" "apple-manifest"
}

package_android() {
  local stage="$OUT_DIR/iroha-mobile-sdk-android-${VERSION}"
  local stage_checksums="$stage/SHA256SUMS.txt"
  local android_zip="$OUT_DIR/iroha-mobile-sdk-android-${VERSION}.zip"
  local maven_repo="$ANDROID_MAVEN_REPO_DIR"
  local client_build_root="$ANDROID_KOTLIN_BUILD_ROOT/client-android"
  local client_aar="$client_build_root/outputs/aar/client-android-release.aar"
  local core_jar
  local native_mode
  local generated_native_root
  local generated_native_provenance
  local rel

  MOBILE_SDK_ANDROID_ARTIFACT_DIR="$ANDROID_ARTIFACT_DIR" \
    bash "$ROOT_DIR/scripts/check_mobile_sdk_artifacts.sh" --root "$ROOT_DIR" --android-only --require-built-android
  native_mode="$(resolve_android_native_mode "$client_aar")"
  generated_native_root="$client_build_root/generated/jniLibs/$native_mode"
  generated_native_provenance="$client_build_root/generated/nativeProvenance/$native_mode/iroha/native-build-provenance-v1.json"
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
    "$client_aar" \
    "client-android/client-android-release.aar" \
    "$stage" \
    "$stage_checksums"
  copy_android_artifact \
    "$generated_native_root/arm64-v8a/libconnect_norito_bridge.so" \
    "native/arm64-v8a/libconnect_norito_bridge.so" \
    "$stage" \
    "$stage_checksums"
  copy_android_artifact \
    "$generated_native_root/x86_64/libconnect_norito_bridge.so" \
    "native/x86_64/libconnect_norito_bridge.so" \
    "$stage" \
    "$stage_checksums"
  copy_android_artifact \
    "$generated_native_provenance" \
    "native/native-build-provenance-v1.json" \
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
