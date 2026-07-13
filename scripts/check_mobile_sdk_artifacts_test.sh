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

command -v zip >/dev/null 2>&1 || fail "zip command is required"

test_build_source_seal() {
  local root="$TMP_DIR/source-seal"
  mkdir -p "$root/scripts" "$root/crates/connect_norito_bridge"
  cp "$SCRIPT_DIR/build_norito_xcframework.sh" \
    "$root/scripts/build_norito_xcframework.sh"
  printf '[workspace]\nmembers = []\n' >"$root/Cargo.toml"
  printf '[package]\nname = "fixture"\nversion = "0.1.0"\n' \
    >"$root/crates/connect_norito_bridge/Cargo.toml"
  git -C "$root" init -q
  git -C "$root" add .
  git -C "$root" -c user.name=test -c user.email=test@example.invalid \
    commit -qm source-seal-fixture

  NORITO_BRIDGE_SOURCE_SEAL_TEST_ONLY=1 \
    bash "$root/scripts/build_norito_xcframework.sh"

  local output
  if output="$(NORITO_BRIDGE_SOURCE_SEAL_TEST_ONLY=1 \
      NORITO_BRIDGE_SOURCE_SEAL_TEST_MUTATE=Cargo.toml \
      bash "$root/scripts/build_norito_xcframework.sh" 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "expected source seal to reject an in-build source mutation"
  fi
  case "$output" in
    *"refusing mixed-source Apple slices"*) ;;
    *)
      printf '%s\n' "$output" >&2
      fail "source seal mutation failure was not explicit"
      ;;
  esac
}

test_build_source_seal

make_aar() {
  local archive="$1"
  shift
  local stage
  local entry

  stage="$(mktemp -d "$TMP_DIR/aar.XXXXXX")"
  for entry in "$@"; do
    mkdir -p "$stage/$(dirname "$entry")"
    printf 'fixture\n' >"$stage/$entry"
  done
  (cd "$stage" && zip -qr "$archive" .)
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
    printf 'uint32_t connect_norito_bridge_abi_version(void);\n' >"$root/dist/NoritoBridge.xcframework/$slice/Headers/connect_norito_bridge.h"
    printf 'module NoritoBridge {}\n' >"$root/dist/NoritoBridge.xcframework/$slice/Headers/module.modulemap"
  done

  hash_a="$(shasum -a 256 "$root/dist/NoritoBridge.xcframework/ios-arm64/libNoritoBridge.a" | awk '{print $1}')"
  hash_b="$(shasum -a 256 "$root/dist/NoritoBridge.xcframework/ios-arm64_x86_64-simulator/libNoritoBridge.a" | awk '{print $1}')"
  hash_c="$(shasum -a 256 "$root/dist/NoritoBridge.xcframework/macos-arm64/libNoritoBridge.a" | awk '{print $1}')"
  local header_hash
  header_hash="$(shasum -a 256 "$root/dist/NoritoBridge.xcframework/ios-arm64/Headers/connect_norito_bridge.h" | awk '{print $1}')"

  cat >"$root/dist/NoritoBridge.artifacts.json" <<JSON
{
  "version": "1.0.0",
  "native_bridge_abi_version": 19,
  "privacy_production_enabled": false,
  "source_commit": "0000000000000000000000000000000000000000",
  "source_tree_dirty": false,
  "source_fingerprint_sha256": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
  "bridge_header_sha256": "$header_hash",
  "required_symbols": [
    "connect_norito_bridge_abi_version",
    "connect_norito_detached_transaction_scaffold_inspect_v1",
    "connect_norito_detached_transaction_scaffold_finalize_ed25519_v1",
    "connect_norito_canonical_json_blake3_v1",
    "connect_norito_kagemusha_recursive_spend_capabilities_v3",
    "connect_norito_kagemusha_topup_finality_verify_v2",
    "connect_norito_kagemusha_topup_shield_build_unsigned_v2",
    "connect_norito_kagemusha_recursive_spend_artifact_begin_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_write_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_finalize_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_cancel_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_install_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v3",
    "connect_norito_kagemusha_recursive_spend_init_v2",
    "connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v2",
    "connect_norito_kagemusha_recursive_spend_topup_finalize_request_v2",
    "connect_norito_kagemusha_recursive_spend_topup_v2",
    "connect_norito_kagemusha_recursive_spend_append_v2",
    "connect_norito_kagemusha_recursive_spend_verify_v2",
    "connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v2",
    "connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v2",
    "connect_norito_kagemusha_recursive_spend_redeem_v2",
    "connect_norito_kagemusha_receiver_key_reference_v2",
    "connect_norito_kagemusha_recipient_output_derive_v2",
    "connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2",
    "connect_norito_kagemusha_recipient_payment_request_create_v2",
    "connect_norito_kagemusha_recipient_payment_request_verify_v2",
    "connect_norito_kagemusha_request_authorization_signing_bytes_v2",
    "connect_norito_kagemusha_request_authorization_create_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_payload_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_create_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_verify_v2",
    "connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v2",
    "connect_norito_kagemusha_recursive_spend_peer_payment_validate_v2",
    "connect_norito_kagemusha_recursive_spend_bundle_summary_v2",
    "connect_norito_kagemusha_recursive_spend_build_split_intent_v2"
  ],
  "hashes": {
    "ios-arm64": "$hash_a",
    "ios-arm64_x86_64-simulator": "$hash_b",
    "macos-arm64": "$hash_c"
  }
}
JSON

  mkdir -p "$root/kotlin/client-android/src/main"
  cat >"$root/kotlin/settings.gradle.kts" <<'SETTINGS'
rootProject.name = "iroha_kotlin_sdk"
include(":core-jvm")
include(":client-android")
SETTINGS
  make_gradle_file "$root/kotlin/core-jvm/build.gradle.kts" "core-jvm"
  make_gradle_file "$root/kotlin/client-android/build.gradle.kts" "client-android"
  printf '<manifest />\n' >"$root/kotlin/client-android/src/main/AndroidManifest.xml"
}

run_expect_pass() {
  local root="$1"
  shift
  local output
  if ! output="$(MOBILE_SDK_SKIP_BINARY_INSPECTION=1 bash "$CHECK_SCRIPT" "$root" "$@" 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "expected validation to pass for $root"
  fi
}

run_expect_fail() {
  local root="$1"
  local expected="$2"
  shift 2
  local output
  if output="$(MOBILE_SDK_SKIP_BINARY_INSPECTION=1 bash "$CHECK_SCRIPT" "$root" "$@" 2>&1)"; then
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

make_apple_inspection_tools() {
  local tools="$1"
  mkdir -p "$tools"
  cat >"$tools/lipo" <<'SH'
#!/usr/bin/env bash
case "${*: -1}" in
  *ios-arm64_x86_64-simulator*) printf 'arm64 x86_64\n' ;;
  *) printf 'arm64\n' ;;
esac
SH
  cat >"$tools/nm" <<'SH'
#!/usr/bin/env bash
binary="${*: -1}"
root="${binary%%/dist/*}"
python3 - "$root/dist/NoritoBridge.artifacts.json" <<'PY'
import json
import os
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    manifest = json.load(handle)
for symbol in manifest["required_symbols"]:
    print("_" + symbol)
if os.environ.get("MOBILE_SDK_TEST_EXTRA_KAGEMUSHA") == "1":
    print("_connect_norito_kagemusha_unexpected_v2")
PY
SH
  chmod +x "$tools/lipo" "$tools/nm"
}

run_expect_binary_fail() {
  local root="$1"
  local expected="$2"
  local tools="$3"
  local output
  if output="$(PATH="$tools:$PATH" MOBILE_SDK_SKIP_BINARY_INSPECTION=0 \
      MOBILE_SDK_TEST_EXTRA_KAGEMUSHA=1 bash "$CHECK_SCRIPT" "$root" --apple-only 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "expected strict binary validation to fail for $root"
  fi
  case "$output" in
    *"$expected"*) ;;
    *)
      printf '%s\n' "$output" >&2
      fail "expected strict binary failure containing: $expected"
      ;;
  esac
}

fixture="$TMP_DIR/valid"
make_fixture "$fixture"
run_expect_pass "$fixture"

wrong_bridge_abi="$TMP_DIR/wrong-bridge-abi"
make_fixture "$wrong_bridge_abi"
sed -i.bak 's/"native_bridge_abi_version": 19/"native_bridge_abi_version": 18/' \
  "$wrong_bridge_abi/dist/NoritoBridge.artifacts.json"
rm -f "$wrong_bridge_abi/dist/NoritoBridge.artifacts.json.bak"
run_expect_fail "$wrong_bridge_abi" "exact first-release NoritoBridge ABI 19"

enabled_privacy="$TMP_DIR/enabled-privacy"
make_fixture "$enabled_privacy"
sed -i.bak 's/"privacy_production_enabled": false/"privacy_production_enabled": true/' \
  "$enabled_privacy/dist/NoritoBridge.artifacts.json"
rm -f "$enabled_privacy/dist/NoritoBridge.artifacts.json.bak"
touch "$enabled_privacy/dist/NoritoBridge.xcframework/.privacy-production-enabled"
run_expect_pass "$enabled_privacy"

enabled_without_marker="$TMP_DIR/enabled-without-marker"
make_fixture "$enabled_without_marker"
sed -i.bak 's/"privacy_production_enabled": false/"privacy_production_enabled": true/' \
  "$enabled_without_marker/dist/NoritoBridge.artifacts.json"
rm -f "$enabled_without_marker/dist/NoritoBridge.artifacts.json.bak"
run_expect_fail "$enabled_without_marker" "missing privacy-production-enabled XCFramework marker"

default_with_marker="$TMP_DIR/default-with-marker"
make_fixture "$default_with_marker"
touch "$default_with_marker/dist/NoritoBridge.xcframework/.privacy-production-enabled"
run_expect_fail "$default_with_marker" "default privacy artifact must not carry the privacy-production-enabled XCFramework marker"

invalid_privacy_state="$TMP_DIR/invalid-privacy-state"
make_fixture "$invalid_privacy_state"
sed -i.bak 's/"privacy_production_enabled": false/"privacy_production_enabled": "false"/' \
  "$invalid_privacy_state/dist/NoritoBridge.artifacts.json"
rm -f "$invalid_privacy_state/dist/NoritoBridge.artifacts.json.bak"
run_expect_fail "$invalid_privacy_state" "must contain exactly one boolean privacy_production_enabled field"

dirty_source_manifest="$TMP_DIR/dirty-source-manifest"
make_fixture "$dirty_source_manifest"
sed -i.bak 's/"source_tree_dirty": false/"source_tree_dirty": true/' \
  "$dirty_source_manifest/dist/NoritoBridge.artifacts.json"
rm -f "$dirty_source_manifest/dist/NoritoBridge.artifacts.json.bak"
run_expect_fail "$dirty_source_manifest" "release artifact must be built from a clean source tree"

duplicate_mixed_privacy_state="$TMP_DIR/duplicate-mixed-privacy-state"
make_fixture "$duplicate_mixed_privacy_state"
sed -i.bak '/"privacy_production_enabled": false/a\
  "privacy_production_enabled": "false",' \
  "$duplicate_mixed_privacy_state/dist/NoritoBridge.artifacts.json"
rm -f "$duplicate_mixed_privacy_state/dist/NoritoBridge.artifacts.json.bak"
run_expect_fail "$duplicate_mixed_privacy_state" "must contain exactly one boolean privacy_production_enabled field"

apple_only_without_android="$TMP_DIR/apple-only-without-android"
make_fixture "$apple_only_without_android"
rm -rf "$apple_only_without_android/kotlin"
run_expect_pass "$apple_only_without_android" --apple-only

android_only_without_apple="$TMP_DIR/android-only-without-apple"
make_fixture "$android_only_without_apple"
rm -rf "$android_only_without_apple/IrohaSwift" "$android_only_without_apple/dist"
run_expect_pass "$android_only_without_apple" --android-only

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
  "privacy_production_enabled": false,
  "hashes": {
    "ios-arm64": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "macos-arm64": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
  }
}
JSON
run_expect_fail "$missing_hash" "NoritoBridge artifact manifest hash for ios-arm64_x86_64-simulator"

hash_mismatch="$TMP_DIR/hash-mismatch"
make_fixture "$hash_mismatch"
printf 'tampered\n' >>"$hash_mismatch/dist/NoritoBridge.xcframework/ios-arm64/libNoritoBridge.a"
run_expect_fail "$hash_mismatch" "NoritoBridge artifact hash mismatch for ios-arm64"

header_mismatch="$TMP_DIR/header-mismatch"
make_fixture "$header_mismatch"
printf 'void unexpected(void);\n' >>"$header_mismatch/dist/NoritoBridge.xcframework/ios-arm64_x86_64-simulator/Headers/connect_norito_bridge.h"
run_expect_fail "$header_mismatch" "NoritoBridge bridge header differs in ios-arm64_x86_64-simulator"

extra_binary_symbol="$TMP_DIR/extra-binary-symbol"
make_fixture "$extra_binary_symbol"
inspection_tools="$TMP_DIR/inspection-tools"
make_apple_inspection_tools "$inspection_tools"
run_expect_binary_fail \
  "$extra_binary_symbol" \
  "Kagemusha export inventory is not exact" \
  "$inspection_tools"

symbol_inventory_mismatch="$TMP_DIR/symbol-inventory-mismatch"
make_fixture "$symbol_inventory_mismatch"
sed -i.bak \
  's/connect_norito_canonical_json_blake3_v1/unexpected_symbol/' \
  "$symbol_inventory_mismatch/dist/NoritoBridge.artifacts.json"
rm -f "$symbol_inventory_mismatch/dist/NoritoBridge.artifacts.json.bak"
run_expect_fail "$symbol_inventory_mismatch" "required symbol inventory is missing or non-canonical"

extra_manifest_symbol="$TMP_DIR/extra-manifest-symbol"
make_fixture "$extra_manifest_symbol"
sed -i.bak '/"connect_norito_kagemusha_recursive_spend_init_v2",/i\
    "connect_norito_kagemusha_unexpected_v2",' \
  "$extra_manifest_symbol/dist/NoritoBridge.artifacts.json"
rm -f "$extra_manifest_symbol/dist/NoritoBridge.artifacts.json.bak"
run_expect_fail "$extra_manifest_symbol" "required symbol inventory is missing or non-canonical"

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

missing_client_android_aar="$TMP_DIR/missing-client-android-aar"
make_fixture "$missing_client_android_aar"
mkdir -p "$missing_client_android_aar/kotlin/core-jvm/build/libs"
printf 'jar\n' >"$missing_client_android_aar/kotlin/core-jvm/build/libs/core-jvm-0.1-SNAPSHOT.jar"
run_expect_fail "$missing_client_android_aar" "missing client-android release aar" --require-built-android

missing_client_native_source="$TMP_DIR/missing-client-native-source"
make_fixture "$missing_client_native_source"
mkdir -p \
  "$missing_client_native_source/kotlin/core-jvm/build/libs" \
  "$missing_client_native_source/kotlin/client-android/build/outputs/aar"
printf 'jar\n' >"$missing_client_native_source/kotlin/core-jvm/build/libs/core-jvm-0.1-SNAPSHOT.jar"
make_aar \
  "$missing_client_native_source/kotlin/client-android/build/outputs/aar/client-android-release.aar" \
  "AndroidManifest.xml" \
  "classes.jar" \
  "jni/arm64-v8a/libconnect_norito_bridge.so" \
  "jni/x86_64/libconnect_norito_bridge.so"
run_expect_fail "$missing_client_native_source" "missing client-android arm64-v8a native bridge library" --require-built-android

missing_client_native_aar_entry="$TMP_DIR/missing-client-native-aar-entry"
make_fixture "$missing_client_native_aar_entry"
mkdir -p \
  "$missing_client_native_aar_entry/kotlin/core-jvm/build/libs" \
  "$missing_client_native_aar_entry/kotlin/client-android/build/outputs/aar" \
  "$missing_client_native_aar_entry/kotlin/client-android/src/main/jniLibs/arm64-v8a" \
  "$missing_client_native_aar_entry/kotlin/client-android/src/main/jniLibs/x86_64"
printf 'jar\n' >"$missing_client_native_aar_entry/kotlin/core-jvm/build/libs/core-jvm-0.1-SNAPSHOT.jar"
printf 'so\n' >"$missing_client_native_aar_entry/kotlin/client-android/src/main/jniLibs/arm64-v8a/libconnect_norito_bridge.so"
printf 'so\n' >"$missing_client_native_aar_entry/kotlin/client-android/src/main/jniLibs/x86_64/libconnect_norito_bridge.so"
make_aar \
  "$missing_client_native_aar_entry/kotlin/client-android/build/outputs/aar/client-android-release.aar" \
  "AndroidManifest.xml" \
  "classes.jar" \
  "jni/arm64-v8a/libconnect_norito_bridge.so"
run_expect_fail "$missing_client_native_aar_entry" "client-android release aar missing ZIP entry jni/x86_64/libconnect_norito_bridge.so" --require-built-android

with_android_outputs="$TMP_DIR/with-android-outputs"
make_fixture "$with_android_outputs"
mkdir -p \
  "$with_android_outputs/kotlin/core-jvm/build/libs" \
  "$with_android_outputs/kotlin/client-android/build/outputs/aar" \
  "$with_android_outputs/kotlin/client-android/src/main/jniLibs/arm64-v8a" \
  "$with_android_outputs/kotlin/client-android/src/main/jniLibs/x86_64"
printf 'jar\n' >"$with_android_outputs/kotlin/core-jvm/build/libs/core-jvm-0.1-SNAPSHOT.jar"
printf 'so\n' >"$with_android_outputs/kotlin/client-android/src/main/jniLibs/arm64-v8a/libconnect_norito_bridge.so"
printf 'so\n' >"$with_android_outputs/kotlin/client-android/src/main/jniLibs/x86_64/libconnect_norito_bridge.so"
make_aar \
  "$with_android_outputs/kotlin/client-android/build/outputs/aar/client-android-release.aar" \
  "AndroidManifest.xml" \
  "classes.jar" \
  "jni/arm64-v8a/libconnect_norito_bridge.so" \
  "jni/x86_64/libconnect_norito_bridge.so"
run_expect_pass "$with_android_outputs" --require-built-android
rm -rf "$with_android_outputs/IrohaSwift" "$with_android_outputs/dist"
run_expect_pass "$with_android_outputs" --android-only --require-built-android

echo "[mobile-sdk-artifacts-test] all checks passed"
