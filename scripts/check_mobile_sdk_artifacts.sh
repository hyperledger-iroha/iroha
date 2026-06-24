#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/check_mobile_sdk_artifacts.sh [--root <repo-root>] [--require-built-android]

Checks that the Iroha mobile SDK packaging surface is ready for wallet
integration:
  - SwiftPM package manifest and NoritoBridge binary target exist.
  - NoritoBridge.xcframework contains iOS device, iOS simulator, and macOS slices.
  - NoritoBridge.artifacts.json records per-slice SHA-256 hashes.
  - Kotlin/Android SDK modules are included and publishable.

By default Android build outputs are not required. Pass --require-built-android
or set MOBILE_SDK_REQUIRE_ANDROID_OUTPUTS=1 to require jar/aar outputs too.
USAGE
}

ROOT_ARG=""
REQUIRE_ANDROID_OUTPUTS="${MOBILE_SDK_REQUIRE_ANDROID_OUTPUTS:-0}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --root)
      shift
      if [[ $# -eq 0 ]]; then
        echo "[mobile-sdk-artifacts] ERROR: --root requires a value" >&2
        exit 64
      fi
      ROOT_ARG="$1"
      ;;
    --root=*)
      ROOT_ARG="${1#*=}"
      ;;
    --require-built-android)
      REQUIRE_ANDROID_OUTPUTS=1
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      if [[ -z "$ROOT_ARG" ]]; then
        ROOT_ARG="$1"
      else
        echo "[mobile-sdk-artifacts] ERROR: unexpected argument: $1" >&2
        usage >&2
        exit 64
      fi
      ;;
  esac
  shift
done

if [[ -z "$ROOT_ARG" ]]; then
  ROOT_ARG="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fi

if [[ ! -d "$ROOT_ARG" ]]; then
  echo "[mobile-sdk-artifacts] ERROR: repo root does not exist: $ROOT_ARG" >&2
  exit 66
fi

ROOT_DIR="$(cd "$ROOT_ARG" && pwd)"
FAILURES=0

relpath() {
  local path="$1"
  case "$path" in
    "$ROOT_DIR"/*) printf '%s' "${path#$ROOT_DIR/}" ;;
    *) printf '%s' "$path" ;;
  esac
}

fail() {
  printf '[mobile-sdk-artifacts] ERROR: %s\n' "$*" >&2
  FAILURES=1
}

require_file() {
  local path="$1"
  local label="$2"
  if [[ ! -f "$path" ]]; then
    fail "missing $label: $(relpath "$path")"
  fi
}

require_dir() {
  local path="$1"
  local label="$2"
  if [[ ! -d "$path" ]]; then
    fail "missing $label: $(relpath "$path")"
  fi
}

require_literal() {
  local path="$1"
  local literal="$2"
  local label="$3"
  if [[ ! -f "$path" ]]; then
    fail "cannot inspect missing $label file: $(relpath "$path")"
    return
  fi
  if ! grep -Fq -- "$literal" "$path"; then
    fail "$label not found in $(relpath "$path")"
  fi
}

require_regex() {
  local path="$1"
  local pattern="$2"
  local label="$3"
  if [[ ! -f "$path" ]]; then
    fail "cannot inspect missing $label file: $(relpath "$path")"
    return
  fi
  if ! grep -Eq -- "$pattern" "$path"; then
    fail "$label not found in $(relpath "$path")"
  fi
}

require_glob() {
  local pattern="$1"
  local label="$2"
  local matches=()
  shopt -s nullglob
  matches=($pattern)
  shopt -u nullglob
  if [[ ${#matches[@]} -eq 0 ]]; then
    fail "missing $label: $pattern"
  fi
}

plist_contains() {
  local plist="$1"
  local needle="$2"
  if [[ ! -f "$plist" ]]; then
    return 1
  fi
  if grep -Fq -- "$needle" "$plist"; then
    return 0
  fi
  if command -v plutil >/dev/null 2>&1 && plutil -p "$plist" 2>/dev/null | grep -Fq -- "$needle"; then
    return 0
  fi
  return 1
}

require_plist_slice() {
  local plist="$1"
  local slice="$2"
  if ! plist_contains "$plist" "$slice"; then
    fail "Info.plist does not list XCFramework slice $slice"
  fi
}

check_swift_package() {
  local package="$ROOT_DIR/IrohaSwift/Package.swift"

  require_file "$package" "Swift package manifest"
  require_dir "$ROOT_DIR/IrohaSwift/Sources/IrohaSwift" "IrohaSwift sources"
  require_literal "$package" 'name: "IrohaSwift"' "IrohaSwift package name"
  require_literal "$package" '.binaryTarget(' "NoritoBridge binary target declaration"
  require_literal "$package" 'name: "NoritoBridge"' "NoritoBridge binary target name"
  require_literal "$package" '.iOS(.v15)' "IrohaSwift iOS platform floor"
  require_literal "$package" 'path: bridgeRelativePath' "NoritoBridge local artifact path"
}

check_xcframework() {
  local xcframework="$ROOT_DIR/dist/NoritoBridge.xcframework"
  local info="$xcframework/Info.plist"
  local manifest="$ROOT_DIR/dist/NoritoBridge.artifacts.json"
  local slices=(ios-arm64 ios-arm64_x86_64-simulator macos-arm64)
  local slice

  require_dir "$xcframework" "NoritoBridge XCFramework"
  require_file "$info" "NoritoBridge XCFramework metadata"

  for slice in "${slices[@]}"; do
    local slice_dir="$xcframework/$slice"
    local headers_dir="$slice_dir/Headers"
    require_plist_slice "$info" "$slice"
    require_dir "$slice_dir" "XCFramework slice directory"
    if [[ -d "$slice_dir" ]]; then
      require_file "$slice_dir/libNoritoBridge.a" "XCFramework slice binary"
      require_dir "$headers_dir" "XCFramework slice headers"
      if [[ -d "$headers_dir" ]]; then
        require_file "$headers_dir/NoritoBridge.h" "XCFramework slice header"
        require_file "$headers_dir/connect_norito_bridge.h" "XCFramework bridge C header"
        require_file "$headers_dir/module.modulemap" "XCFramework module map"
      fi
    fi
  done

  require_file "$manifest" "NoritoBridge artifact manifest"
  if [[ -f "$manifest" ]]; then
    require_regex "$manifest" '"version"[[:space:]]*:[[:space:]]*"[^"]+"' "NoritoBridge artifact version"
    for slice in "${slices[@]}"; do
      require_regex "$manifest" "\"$slice\"[[:space:]]*:[[:space:]]*\"[[:xdigit:]]{64}\"" "NoritoBridge artifact manifest hash for $slice"
    done
  fi
}

check_gradle_publication() {
  local module="$1"
  local artifact_id="$2"
  local build_file="$ROOT_DIR/kotlin/$module/build.gradle.kts"

  require_file "$build_file" "$module Gradle build file"
  require_regex "$build_file" 'maven-publish' "$module maven-publish plugin"
  require_regex "$build_file" 'group[[:space:]]*=[[:space:]]*"org\.hyperledger\.iroha\.sdk"' "$module Maven group"
  require_regex "$build_file" 'version[[:space:]]*=[[:space:]]*"[^"]+"' "$module Maven version"
  require_regex "$build_file" 'create<MavenPublication>\("release"\)' "$module release publication"
  require_regex "$build_file" "artifactId[[:space:]]*=[[:space:]]*\"$artifact_id\"" "$module artifact id"
}

check_android_package() {
  local settings="$ROOT_DIR/kotlin/settings.gradle.kts"

  require_file "$settings" "Kotlin settings manifest"
  require_literal "$settings" 'include(":core-jvm")' "core-jvm module include"
  require_literal "$settings" 'include(":client-android")' "client-android module include"
  require_literal "$settings" 'include(":offline-wallet-android")' "offline-wallet-android module include"

  check_gradle_publication "core-jvm" "core-jvm"
  check_gradle_publication "client-android" "client-android"
  check_gradle_publication "offline-wallet-android" "offline-wallet-android"

  require_file "$ROOT_DIR/kotlin/client-android/src/main/AndroidManifest.xml" "client-android AndroidManifest"
  require_file "$ROOT_DIR/kotlin/offline-wallet-android/src/main/AndroidManifest.xml" "offline-wallet-android AndroidManifest"

  if [[ "$REQUIRE_ANDROID_OUTPUTS" == "1" ]]; then
    require_glob "$ROOT_DIR/kotlin/core-jvm/build/libs/core-jvm-*.jar" "core-jvm built jar"
    require_glob "$ROOT_DIR/kotlin/client-android/build/outputs/aar/client-android-release.aar" "client-android release aar"
    require_glob "$ROOT_DIR/kotlin/offline-wallet-android/build/outputs/aar/offline-wallet-android-release.aar" "offline-wallet-android release aar"
  fi
}

check_swift_package
check_xcframework
check_android_package

if [[ "$FAILURES" -ne 0 ]]; then
  echo "[mobile-sdk-artifacts] validation failed for $ROOT_DIR" >&2
  exit 1
fi

echo "[mobile-sdk-artifacts] validation passed for $ROOT_DIR"
