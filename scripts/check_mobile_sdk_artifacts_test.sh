#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECK_SCRIPT="$SCRIPT_DIR/check_mobile_sdk_artifacts.sh"
TMP_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

fail() {
  printf '[mobile-sdk-artifacts-test] ERROR: %s\n' "$*" >&2
  exit 1
}

make_gradle_file() {
  local path="$1"
  local artifact_id="$2"
  mkdir -p "$(dirname "$path")"
  cat >"$path" <<GRADLE
plugins {
    \`maven-publish\`
}

group = "org.hyperledger.iroha.sdk"
version = "0.1-SNAPSHOT"

publishing {
    publications {
        create<MavenPublication>("release") {
            artifactId = "$artifact_id"
        }
    }
}
GRADLE
}

make_fixture() {
  local root="$1"
  local hash_a="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
  local hash_b="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
  local hash_c="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
  local slice

  mkdir -p "$root/IrohaSwift/Sources/IrohaSwift"
  cat >"$root/IrohaSwift/Package.swift" <<'SWIFT'
// swift-tools-version:5.9
import PackageDescription

let bridgeRelativePath = "../dist/NoritoBridge.xcframework"

let package = Package(
    name: "IrohaSwift",
    platforms: [
        .iOS(.v15)
    ],
    targets: [
        .binaryTarget(
            name: "NoritoBridge",
            path: bridgeRelativePath
        )
    ]
)
SWIFT

  mkdir -p "$root/dist/NoritoBridge.xcframework"
  cat >"$root/dist/NoritoBridge.xcframework/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
  <key>AvailableLibraries</key>
  <array>
    <dict><key>LibraryIdentifier</key><string>ios-arm64</string></dict>
    <dict><key>LibraryIdentifier</key><string>ios-arm64_x86_64-simulator</string></dict>
    <dict><key>LibraryIdentifier</key><string>macos-arm64</string></dict>
  </array>
</dict>
</plist>
PLIST

  for slice in ios-arm64 ios-arm64_x86_64-simulator macos-arm64; do
    mkdir -p "$root/dist/NoritoBridge.xcframework/$slice/Headers"
    printf 'fake static library for %s\n' "$slice" >"$root/dist/NoritoBridge.xcframework/$slice/libNoritoBridge.a"
    printf 'void norito_%s(void);\n' "$slice" >"$root/dist/NoritoBridge.xcframework/$slice/Headers/NoritoBridge.h"
    printf 'void connect_%s(void);\n' "$slice" >"$root/dist/NoritoBridge.xcframework/$slice/Headers/connect_norito_bridge.h"
    printf 'module NoritoBridge {}\n' >"$root/dist/NoritoBridge.xcframework/$slice/Headers/module.modulemap"
  done

  cat >"$root/dist/NoritoBridge.artifacts.json" <<JSON
{
  "version": "1.0.0",
  "hashes": {
    "ios-arm64": "$hash_a",
    "ios-arm64_x86_64-simulator": "$hash_b",
    "macos-arm64": "$hash_c"
  }
}
JSON

  mkdir -p "$root/kotlin/client-android/src/main" "$root/kotlin/offline-wallet-android/src/main"
  cat >"$root/kotlin/settings.gradle.kts" <<'SETTINGS'
rootProject.name = "iroha_kotlin_sdk"
include(":core-jvm")
include(":client-android")
include(":offline-wallet-android")
SETTINGS
  make_gradle_file "$root/kotlin/core-jvm/build.gradle.kts" "core-jvm"
  make_gradle_file "$root/kotlin/client-android/build.gradle.kts" "client-android"
  make_gradle_file "$root/kotlin/offline-wallet-android/build.gradle.kts" "offline-wallet-android"
  printf '<manifest />\n' >"$root/kotlin/client-android/src/main/AndroidManifest.xml"
  printf '<manifest />\n' >"$root/kotlin/offline-wallet-android/src/main/AndroidManifest.xml"
}

run_expect_pass() {
  local root="$1"
  shift
  local output
  if ! output="$(bash "$CHECK_SCRIPT" "$root" "$@" 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "expected validation to pass for $root"
  fi
}

run_expect_fail() {
  local root="$1"
  local expected="$2"
  shift 2
  local output
  if output="$(bash "$CHECK_SCRIPT" "$root" "$@" 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "expected validation to fail for $root"
  fi
  case "$output" in
    *"$expected"*) ;;
    *)
      printf '%s\n' "$output" >&2
      fail "expected failure containing: $expected"
      ;;
  esac
}

fixture="$TMP_DIR/valid"
make_fixture "$fixture"
run_expect_pass "$fixture"

missing_ios="$TMP_DIR/missing-ios"
make_fixture "$missing_ios"
rm -rf "$missing_ios/dist/NoritoBridge.xcframework/ios-arm64"
run_expect_fail "$missing_ios" "missing XCFramework slice directory"

missing_header="$TMP_DIR/missing-header"
make_fixture "$missing_header"
rm -f "$missing_header/dist/NoritoBridge.xcframework/ios-arm64_x86_64-simulator/Headers/NoritoBridge.h"
run_expect_fail "$missing_header" "missing XCFramework slice header"

missing_hash="$TMP_DIR/missing-hash"
make_fixture "$missing_hash"
cat >"$missing_hash/dist/NoritoBridge.artifacts.json" <<'JSON'
{
  "version": "1.0.0",
  "hashes": {
    "ios-arm64": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "macos-arm64": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
  }
}
JSON
run_expect_fail "$missing_hash" "NoritoBridge artifact manifest hash for ios-arm64_x86_64-simulator"

missing_android_publication="$TMP_DIR/missing-android-publication"
make_fixture "$missing_android_publication"
cat >"$missing_android_publication/kotlin/client-android/build.gradle.kts" <<'GRADLE'
plugins {
}

group = "org.hyperledger.iroha.sdk"
version = "0.1-SNAPSHOT"
GRADLE
run_expect_fail "$missing_android_publication" "client-android maven-publish plugin"

missing_android_outputs="$TMP_DIR/missing-android-outputs"
make_fixture "$missing_android_outputs"
run_expect_fail "$missing_android_outputs" "missing core-jvm built jar" --require-built-android

with_android_outputs="$TMP_DIR/with-android-outputs"
make_fixture "$with_android_outputs"
mkdir -p \
  "$with_android_outputs/kotlin/core-jvm/build/libs" \
  "$with_android_outputs/kotlin/client-android/build/outputs/aar" \
  "$with_android_outputs/kotlin/offline-wallet-android/build/outputs/aar"
printf 'jar\n' >"$with_android_outputs/kotlin/core-jvm/build/libs/core-jvm-0.1-SNAPSHOT.jar"
printf 'aar\n' >"$with_android_outputs/kotlin/client-android/build/outputs/aar/client-android-release.aar"
printf 'aar\n' >"$with_android_outputs/kotlin/offline-wallet-android/build/outputs/aar/offline-wallet-android-release.aar"
run_expect_pass "$with_android_outputs" --require-built-android

echo "[mobile-sdk-artifacts-test] all checks passed"
