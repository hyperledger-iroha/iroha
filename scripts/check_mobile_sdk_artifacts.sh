#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/check_mobile_sdk_artifacts.sh [--root <repo-root>] [--apple-only|--android-only] [--require-built-android]

Checks that the Iroha mobile SDK packaging surface is ready for wallet
integration:
  - SwiftPM package manifest and NoritoBridge binary target exist.
  - NoritoBridge.xcframework contains iOS device, iOS simulator, and macOS slices.
  - NoritoBridge.artifacts.json records per-slice SHA-256 hashes and the
    privacy-production feature state, which must match the XCFramework marker.
  - Every manifest hash matches the actual slice, all headers are identical,
    and the manifest ABI/source fingerprint matches the checked-out bridge.
  - Apple archives contain their declared architectures and the complete
    Kagemusha recursive-spend symbol surface.
  - Kotlin/Android SDK modules are included and publishable.

By default Android build outputs are not required. Pass --require-built-android
or set MOBILE_SDK_REQUIRE_ANDROID_OUTPUTS=1 to require jar/aar outputs too.
By default both Apple and Android packaging surfaces are checked. Pass
--apple-only or --android-only when platform artifact builds run in separate CI
jobs.
USAGE
}

ROOT_ARG=""
REQUIRE_ANDROID_OUTPUTS="${MOBILE_SDK_REQUIRE_ANDROID_OUTPUTS:-0}"
CHECK_APPLE=1
CHECK_ANDROID=1

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
    --apple-only)
      CHECK_APPLE=1
      CHECK_ANDROID=0
      ;;
    --android-only)
      CHECK_APPLE=0
      CHECK_ANDROID=1
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
  while IFS= read -r match; do
    matches+=("$match")
  done < <(compgen -G "$pattern" || true)
  if [[ ${#matches[@]} -eq 0 ]]; then
    fail "missing $label: $pattern"
  fi
}

require_zip_entry() {
  local archive="$1"
  local entry="$2"
  local label="$3"
  local entries

  if [[ ! -f "$archive" ]]; then
    fail "cannot inspect missing $label: $(relpath "$archive")"
    return
  fi
  if ! command -v unzip >/dev/null 2>&1; then
    fail "unzip is required to inspect $label"
    return
  fi
  if ! entries="$(unzip -Z1 "$archive" 2>/dev/null)"; then
    fail "$label is not a readable ZIP/AAR archive: $(relpath "$archive")"
    return
  fi
  if ! grep -Fxq -- "$entry" <<<"$entries"; then
    fail "$label missing ZIP entry $entry in $(relpath "$archive")"
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

hash_file() {
  local path="$1"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$path" | awk '{print $1}'
  else
    sha256sum "$path" | awk '{print $1}'
  fi
}

manifest_json_value() {
  local manifest="$1"
  local key="$2"
  python3 - "$manifest" "$key" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    value = json.load(handle)
for component in sys.argv[2].split("."):
    value = value[component]
if isinstance(value, bool):
    print("true" if value else "false")
else:
    print(value)
PY
}

bridge_source_fingerprint() {
  python3 - "$ROOT_DIR" <<'PY'
import hashlib
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
source_roots = [root / "crates/connect_norito_bridge", root / "IrohaSwift/Sources/IrohaSwift"]
paths = [
    path.relative_to(root).as_posix()
    for source_root in source_roots
    for path in source_root.rglob("*")
    if path.is_file() and not path.is_symlink()
]
digest = hashlib.sha256()
for relative in sorted(paths):
    path = root / relative
    if not path.is_file():
        continue
    digest.update(relative.encode("utf-8"))
    digest.update(b"\0")
    digest.update(path.read_bytes())
    digest.update(b"\0")
print(digest.hexdigest())
PY
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
  local privacy_marker="$xcframework/.privacy-production-enabled"
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
    local privacy_keys=()
    local privacy_declarations=()
    local privacy_key
    local privacy_declaration
    local privacy_value
    while IFS= read -r privacy_key; do
      privacy_keys+=("$privacy_key")
    done < <(
      grep -Eo '"privacy_production_enabled"[[:space:]]*:' "$manifest" || true
    )
    while IFS= read -r privacy_declaration; do
      privacy_declarations+=("$privacy_declaration")
    done < <(
      grep -Eo '"privacy_production_enabled"[[:space:]]*:[[:space:]]*(true|false)' \
        "$manifest" || true
    )
    if [[ ${#privacy_keys[@]} -ne 1 || ${#privacy_declarations[@]} -ne 1 ]]; then
      fail "NoritoBridge artifact manifest must contain exactly one boolean privacy_production_enabled field"
    else
      privacy_value="${privacy_declarations[0]##*:}"
      privacy_value="${privacy_value//[[:space:]]/}"
      if [[ "$privacy_value" == "true" ]]; then
        require_file "$privacy_marker" "privacy-production-enabled XCFramework marker"
      elif [[ -e "$privacy_marker" ]]; then
        fail "default privacy artifact must not carry the privacy-production-enabled XCFramework marker"
      fi
    fi
    require_regex "$manifest" '"version"[[:space:]]*:[[:space:]]*"[^"]+"' "NoritoBridge artifact version"
    for slice in "${slices[@]}"; do
      require_regex "$manifest" "\"$slice\"[[:space:]]*:[[:space:]]*\"[[:xdigit:]]{64}\"" "NoritoBridge artifact manifest hash for $slice"
      if [[ -f "$xcframework/$slice/libNoritoBridge.a" ]]; then
        local expected_hash actual_hash
        expected_hash="$(manifest_json_value "$manifest" "hashes.$slice" 2>/dev/null || true)"
        actual_hash="$(hash_file "$xcframework/$slice/libNoritoBridge.a")"
        if [[ "$expected_hash" != "$actual_hash" ]]; then
          fail "NoritoBridge artifact hash mismatch for $slice"
        fi
      fi
    done

    require_regex "$manifest" '"native_bridge_abi_version"[[:space:]]*:[[:space:]]*18([[:space:]]*[,}])' "exact first-release NoritoBridge ABI 18"
    require_regex "$manifest" '"source_commit"[[:space:]]*:[[:space:]]*"[[:xdigit:]]{40}"' "NoritoBridge source commit"
    require_regex "$manifest" '"source_tree_dirty"[[:space:]]*:[[:space:]]*(true|false)' "NoritoBridge source dirty state"
    require_regex "$manifest" '"source_fingerprint_sha256"[[:space:]]*:[[:space:]]*"[[:xdigit:]]{64}"' "NoritoBridge source fingerprint"
    require_regex "$manifest" '"bridge_header_sha256"[[:space:]]*:[[:space:]]*"[[:xdigit:]]{64}"' "NoritoBridge header hash"
    local required_symbols=(
      connect_norito_bridge_abi_version
      connect_norito_kagemusha_recursive_spend_capabilities_v1
      connect_norito_kagemusha_topup_finality_verify_v2
      connect_norito_kagemusha_topup_shield_build_unsigned_v2
      connect_norito_kagemusha_recursive_spend_artifact_begin_v3
      connect_norito_kagemusha_recursive_spend_artifact_write_v3
      connect_norito_kagemusha_recursive_spend_artifact_finalize_v3
      connect_norito_kagemusha_recursive_spend_artifact_cancel_v3
      connect_norito_kagemusha_recursive_spend_artifact_set_install_v3
      connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v3
      connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v3
      connect_norito_kagemusha_recursive_spend_init_v2
      connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v2
      connect_norito_kagemusha_recursive_spend_topup_finalize_request_v2
      connect_norito_kagemusha_recursive_spend_topup_v2
      connect_norito_kagemusha_recursive_spend_append_v2
      connect_norito_kagemusha_recursive_spend_redeem_change_v2
      connect_norito_kagemusha_recursive_spend_verify_v2
      connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v2
      connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v2
      connect_norito_kagemusha_recursive_spend_redeem_v2
      connect_norito_kagemusha_receiver_key_reference_v2
      connect_norito_kagemusha_recipient_output_derive_v2
      connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2
      connect_norito_kagemusha_recipient_payment_request_create_v2
      connect_norito_kagemusha_recipient_payment_request_verify_v2
      connect_norito_kagemusha_request_authorization_signing_bytes_v2
      connect_norito_kagemusha_request_authorization_create_v2
      connect_norito_kagemusha_receiver_acknowledgement_payload_v2
      connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2
      connect_norito_kagemusha_receiver_acknowledgement_create_v2
      connect_norito_kagemusha_receiver_acknowledgement_verify_v2
      connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v2
      connect_norito_kagemusha_recursive_spend_peer_payment_validate_v2
      connect_norito_kagemusha_recursive_spend_bundle_summary_v2
      connect_norito_kagemusha_recursive_spend_build_split_intent_v2
      connect_norito_kagemusha_recursive_spend_build_redemption_intent_v2
    )
    if ! python3 - "$manifest" "${required_symbols[@]}" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
expected = sys.argv[2:]
actual = payload.get("required_symbols")
raise SystemExit(0 if actual == expected else 1)
PY
    then
      fail "NoritoBridge artifact required symbol inventory is missing or non-canonical"
    fi

    local canonical_header="$xcframework/ios-arm64/Headers/connect_norito_bridge.h"
    if [[ -f "$canonical_header" ]]; then
      local manifest_header_hash actual_header_hash
      manifest_header_hash="$(manifest_json_value "$manifest" bridge_header_sha256 2>/dev/null || true)"
      actual_header_hash="$(hash_file "$canonical_header")"
      if [[ "$manifest_header_hash" != "$actual_header_hash" ]]; then
        fail "NoritoBridge artifact header hash mismatch"
      fi
      for slice in "${slices[@]}"; do
        local slice_header="$xcframework/$slice/Headers/connect_norito_bridge.h"
        if [[ -f "$slice_header" && "$(hash_file "$slice_header")" != "$actual_header_hash" ]]; then
          fail "NoritoBridge bridge header differs in $slice"
        fi
      done
    fi

    local bridge_source="$ROOT_DIR/crates/connect_norito_bridge/src/lib.rs"
    if [[ -f "$bridge_source" && -d "$ROOT_DIR/.git" ]]; then
      local source_abi manifest_abi source_commit manifest_commit source_dirty manifest_dirty source_fingerprint manifest_fingerprint
      source_abi="$(sed -nE 's/.*CONNECT_NORITO_BRIDGE_ABI_VERSION:[[:space:]]*u32[[:space:]]*=[[:space:]]*([0-9]+).*/\1/p' "$bridge_source" | head -n1)"
      manifest_abi="$(manifest_json_value "$manifest" native_bridge_abi_version 2>/dev/null || true)"
      if [[ "$source_abi" != "18" || "$manifest_abi" != "18" ]]; then
        fail "NoritoBridge artifact and bridge source must both use exact first-release ABI 18"
      fi
      source_commit="$(git -C "$ROOT_DIR" rev-parse HEAD)"
      manifest_commit="$(manifest_json_value "$manifest" source_commit 2>/dev/null || true)"
      if [[ "$manifest_commit" != "$source_commit" ]]; then
        fail "NoritoBridge artifact source commit does not match checkout"
      fi
      source_dirty=false
      if [[ -n "$(git -C "$ROOT_DIR" status --porcelain -- crates/connect_norito_bridge IrohaSwift/Sources/IrohaSwift)" ]]; then
        source_dirty=true
      fi
      manifest_dirty="$(manifest_json_value "$manifest" source_tree_dirty 2>/dev/null || true)"
      if [[ "$manifest_dirty" != "$source_dirty" ]]; then
        fail "NoritoBridge artifact source dirty state does not match checkout"
      fi
      source_fingerprint="$(bridge_source_fingerprint)"
      manifest_fingerprint="$(manifest_json_value "$manifest" source_fingerprint_sha256 2>/dev/null || true)"
      if [[ "$manifest_fingerprint" != "$source_fingerprint" ]]; then
        fail "NoritoBridge artifact source fingerprint does not match checkout"
      fi
    fi

    if [[ "${MOBILE_SDK_SKIP_BINARY_INSPECTION:-0}" != "1" ]]; then
      local index symbol actual_arches
      for index in "${!slices[@]}"; do
        slice="${slices[$index]}"
        local binary="$xcframework/$slice/libNoritoBridge.a"
        [[ -f "$binary" ]] || continue
        if ! command -v lipo >/dev/null 2>&1; then
          fail "lipo is required for strict Apple artifact validation"
          break
        fi
        actual_arches="$(lipo -archs "$binary" 2>/dev/null || true)"
        case "$slice" in
          ios-arm64|macos-arm64)
            if [[ "$actual_arches" != "arm64" ]]; then
              fail "NoritoBridge $slice architectures must be arm64 (found ${actual_arches:-unreadable})"
            fi
            ;;
          ios-arm64_x86_64-simulator)
            if [[ " $actual_arches " != *" arm64 "* \
              || " $actual_arches " != *" x86_64 "* \
              || "$(wc -w <<<"$actual_arches" | tr -d '[:space:]')" != "2" ]]; then
              fail "NoritoBridge $slice architectures must be arm64 and x86_64 (found ${actual_arches:-unreadable})"
            fi
            ;;
        esac
        if ! command -v nm >/dev/null 2>&1; then
          fail "nm is required for strict Apple artifact validation"
          break
        fi
        local symbols
        symbols="$(nm -gj "$binary" 2>/dev/null || true)"
        for symbol in "${required_symbols[@]}"; do
          if ! grep -Eq "^_?${symbol}$" <<<"$symbols"; then
            fail "NoritoBridge $slice is missing required symbol $symbol"
          fi
        done
      done
    fi
  fi
}

check_gradle_publication() {
  local module="$1"
  local artifact_id="$2"
  local build_file="$ROOT_DIR/kotlin/$module/build.gradle.kts"

  require_file "$build_file" "$module Gradle build file"
  require_regex "$build_file" 'maven-publish' "$module maven-publish plugin"
  require_regex "$build_file" 'group[[:space:]]*=[[:space:]]*"org\.hyperledger\.iroha\.sdk"' "$module Maven group"
  require_regex "$build_file" 'version[[:space:]]*=[[:space:]]*("[^"]+"|providers\.gradleProperty\("irohaSdkVersion"\))' "$module Maven version"
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
    local client_aar="$ROOT_DIR/kotlin/client-android/build/outputs/aar/client-android-release.aar"
    local offline_aar="$ROOT_DIR/kotlin/offline-wallet-android/build/outputs/aar/offline-wallet-android-release.aar"
    local abi

    require_glob "$ROOT_DIR/kotlin/core-jvm/build/libs/core-jvm-*.jar" "core-jvm built jar"
    require_glob "$client_aar" "client-android release aar"
    require_glob "$offline_aar" "offline-wallet-android release aar"

    require_zip_entry "$client_aar" "AndroidManifest.xml" "client-android release aar"
    require_zip_entry "$client_aar" "classes.jar" "client-android release aar"
    require_zip_entry "$offline_aar" "AndroidManifest.xml" "offline-wallet-android release aar"
    require_zip_entry "$offline_aar" "classes.jar" "offline-wallet-android release aar"

    for abi in arm64-v8a x86_64; do
      require_file "$ROOT_DIR/kotlin/client-android/src/main/jniLibs/$abi/libconnect_norito_bridge.so" "client-android $abi native bridge library"
      require_zip_entry "$client_aar" "jni/$abi/libconnect_norito_bridge.so" "client-android release aar"
    done
  fi
}

if [[ "$CHECK_APPLE" == "1" ]]; then
  check_swift_package
  check_xcframework
fi

if [[ "$CHECK_ANDROID" == "1" ]]; then
  check_android_package
fi

if [[ "$FAILURES" -ne 0 ]]; then
  echo "[mobile-sdk-artifacts] validation failed for $ROOT_DIR" >&2
  exit 1
fi

echo "[mobile-sdk-artifacts] validation passed for $ROOT_DIR"
