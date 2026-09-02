#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/check_mobile_sdk_artifacts.sh [--root <repo-root>] [--apple-only|--android-only] [--require-built-android] [--allow-dirty-source]

Validate the sole first-release mobile SDK surface:
  - exact Kagemusha V1 C/header exports;
  - source-complete Swift, Kotlin, and mirrored Java V1 codecs/transports;
  - source-authenticated NoritoBridge XCFramework manifest and slices; and
  - optional built Android jars/AARs with both qualified native ABIs.
USAGE
}

ROOT_ARG=""
CHECK_APPLE=1
CHECK_ANDROID=1
REQUIRE_ANDROID_OUTPUTS="${MOBILE_SDK_REQUIRE_ANDROID_OUTPUTS:-0}"
ALLOW_DIRTY_SOURCE="${MOBILE_SDK_ALLOW_DIRTY_SOURCE:-0}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --root)
      shift
      [[ $# -gt 0 ]] || { echo "[mobile-sdk-artifacts] --root requires a value" >&2; exit 64; }
      ROOT_ARG="$1"
      ;;
    --root=*) ROOT_ARG="${1#*=}" ;;
    --apple-only) CHECK_APPLE=1; CHECK_ANDROID=0 ;;
    --android-only) CHECK_APPLE=0; CHECK_ANDROID=1 ;;
    --require-built-android) REQUIRE_ANDROID_OUTPUTS=1 ;;
    --allow-dirty-source) ALLOW_DIRTY_SOURCE=1 ;;
    --help|-h) usage; exit 0 ;;
    *)
      if [[ -z "$ROOT_ARG" ]]; then
        ROOT_ARG="$1"
      else
        echo "[mobile-sdk-artifacts] unexpected argument: $1" >&2
        usage >&2
        exit 64
      fi
      ;;
  esac
  shift
done

[[ "$REQUIRE_ANDROID_OUTPUTS" == "0" || "$REQUIRE_ANDROID_OUTPUTS" == "1" ]] || {
  echo "[mobile-sdk-artifacts] MOBILE_SDK_REQUIRE_ANDROID_OUTPUTS must be 0 or 1" >&2
  exit 64
}
[[ "$ALLOW_DIRTY_SOURCE" == "0" || "$ALLOW_DIRTY_SOURCE" == "1" ]] || {
  echo "[mobile-sdk-artifacts] MOBILE_SDK_ALLOW_DIRTY_SOURCE must be 0 or 1" >&2
  exit 64
}

if [[ -z "$ROOT_ARG" ]]; then
  ROOT_ARG="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fi
[[ -d "$ROOT_ARG" ]] || { echo "[mobile-sdk-artifacts] repository root is missing: $ROOT_ARG" >&2; exit 66; }
ROOT_DIR="$(cd "$ROOT_ARG" && pwd -P)"
PYTHON_BIN="${MOBILE_SDK_PYTHON_BINARY:-$(command -v python3 || true)}"
[[ -n "$PYTHON_BIN" && -x "$PYTHON_BIN" ]] || {
  echo "[mobile-sdk-artifacts] Python 3 is required" >&2
  exit 69
}

FAILURES=0
fail() {
  echo "[mobile-sdk-artifacts] ERROR: $*" >&2
  FAILURES=1
}

require_file() {
  local path="$1"
  local label="$2"
  [[ -f "$path" && ! -L "$path" ]] || fail "missing $label: $path"
}

require_literal() {
  local path="$1"
  local literal="$2"
  local label="$3"
  if [[ ! -f "$path" ]] || ! grep -Fq -- "$literal" "$path"; then
    fail "$label is not exact in $path"
  fi
}

KAGEMUSHA_C_SYMBOLS=(
  connect_norito_kagemusha_v1_payment_request_validate
  connect_norito_kagemusha_v1_payment_validate
  connect_norito_kagemusha_v1_acknowledgement_validate
  connect_norito_kagemusha_v1_mint_credit_validate
  connect_norito_kagemusha_v1_redemption_voucher_validate
  connect_norito_kagemusha_v1_payment_request_text_validate
  connect_norito_kagemusha_v1_payment_text_validate
  connect_norito_kagemusha_v1_acknowledgement_text_validate
  connect_norito_kagemusha_v1_mint_credit_text_validate
  connect_norito_kagemusha_v1_redemption_voucher_text_validate
  connect_norito_kagemusha_device_capabilities_v1
  connect_norito_kagemusha_device_execute_v1
)

check_source_contract() {
  local gate="$ROOT_DIR/ci/check_connect_norito_bridge_header.sh"
  require_file "$gate" "NoritoBridge header parity gate"
  if [[ -f "$gate" ]] && ! bash "$gate"; then
    fail "NoritoBridge C/Rust export parity failed"
  fi

  require_file "$ROOT_DIR/fixtures/offline/kagemusha_v1.json" "shared Kagemusha V1 fixture"
  require_file "$ROOT_DIR/IrohaSwift/Sources/IrohaSwift/KagemushaWireV1.swift" "Swift Kagemusha V1 codec"
  require_file "$ROOT_DIR/IrohaSwift/Sources/IrohaSwift/KagemushaDeviceLifecycleBridgeV1.swift" "Swift hardware lifecycle bridge"
  require_file "$ROOT_DIR/kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaWireV1.kt" "Kotlin Kagemusha V1 codec"
  require_file "$ROOT_DIR/kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaDeviceLifecycleBridgeV1.kt" "Kotlin hardware lifecycle bridge"
  require_file "$ROOT_DIR/java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaWireV1.java" "Java Kagemusha V1 codec"
}

check_binary_symbols() {
  local binary="$1"
  local label="$2"
  local nm_mode="$3"
  local symbols
  if ! command -v nm >/dev/null 2>&1; then
    fail "nm is required to inspect $label"
    return
  fi
  if [[ "$nm_mode" == "apple" ]]; then
    symbols="$(nm -gUj "$binary" 2>/dev/null || true)"
  else
    symbols="$(nm -D --defined-only "$binary" 2>/dev/null | awk '{print $NF}' || true)"
  fi
  [[ -n "$symbols" ]] || { fail "$label has no inspectable exported symbols"; return; }
  local symbol
  for symbol in "${KAGEMUSHA_C_SYMBOLS[@]}"; do
    if ! grep -Eq "^_?${symbol}$" <<<"$symbols"; then
      fail "$label is missing $symbol"
    fi
  done
}

check_apple() {
  local artifact_root="${MOBILE_SDK_APPLE_ARTIFACT_DIR:-$ROOT_DIR/dist}"
  [[ "$artifact_root" == /* ]] || artifact_root="$ROOT_DIR/$artifact_root"
  local xcframework="$artifact_root/NoritoBridge.xcframework"
  local manifest="$xcframework/NoritoBridge.artifacts.json"
  local manifest_link="$artifact_root/NoritoBridge.artifacts.json"
  local loader="$ROOT_DIR/IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"
  if [[ "${MOBILE_SDK_STAGED_BUILD_VALIDATION:-0}" == "1" ]]; then
    loader="${MOBILE_SDK_PROSPECTIVE_SWIFT_LOADER_PATH:-}"
  fi

  require_file "$ROOT_DIR/IrohaSwift/Package.swift" "Swift package manifest"
  require_literal "$ROOT_DIR/IrohaSwift/Package.swift" 'name: "NoritoBridge"' "Swift binary target"
  require_file "$loader" "Swift native bridge hash owner"
  require_file "$manifest" "embedded NoritoBridge manifest"
  [[ -L "$manifest_link" ]] || fail "public NoritoBridge manifest must be a relative symlink"

  if [[ -f "$manifest" && -f "$loader" ]]; then
    local validation=(
      "$PYTHON_BIN" "$ROOT_DIR/scripts/validate_norito_bridge_xcframework.py"
      --root "$ROOT_DIR"
      --xcframework "$xcframework"
      --manifest "$manifest"
      --manifest-link "$manifest_link"
      --expected-link-target "NoritoBridge.xcframework/NoritoBridge.artifacts.json"
      --swift-loader "$loader"
    )
    if ! "${validation[@]}"; then
      fail "strict NoritoBridge XCFramework validation failed"
    fi
  fi

  if [[ "$(uname -s)" == "Darwin" && -d "$xcframework" ]]; then
    local slice
    for slice in ios-arm64 ios-arm64_x86_64-simulator macos-arm64_x86_64; do
      local binary="$xcframework/$slice/libNoritoBridge.a"
      require_file "$binary" "NoritoBridge $slice library"
      [[ -f "$binary" ]] && check_binary_symbols "$binary" "NoritoBridge $slice" apple
    done
  fi
}

check_android() {
  local settings="$ROOT_DIR/kotlin/settings.gradle.kts"
  require_file "$settings" "Kotlin settings"
  require_literal "$settings" 'include(":core-jvm")' "Kotlin core-jvm module"
  require_literal "$settings" 'include(":client-android")' "Kotlin client-android module"
  require_file "$ROOT_DIR/kotlin/core-jvm/build.gradle.kts" "Kotlin core-jvm build"
  require_file "$ROOT_DIR/kotlin/client-android/build.gradle.kts" "Kotlin client-android build"

  if [[ "$REQUIRE_ANDROID_OUTPUTS" != "1" ]]; then
    return 0
  fi
  local jar
  jar="$(find "$ROOT_DIR/kotlin/core-jvm/build/libs" -maxdepth 1 -type f -name 'core-jvm-*.jar' -print -quit 2>/dev/null || true)"
  [[ -n "$jar" ]] || fail "core-jvm built jar is missing"
  local aar="$ROOT_DIR/kotlin/client-android/build/outputs/aar/client-android-release.aar"
  require_file "$aar" "client-android release AAR"
  [[ -f "$aar" ]] || return
  command -v unzip >/dev/null 2>&1 || { fail "unzip is required for Android artifact validation"; return; }
  local entry
  for entry in \
    AndroidManifest.xml \
    classes.jar \
    assets/iroha/native-build-provenance-v1.json \
    jni/arm64-v8a/libconnect_norito_bridge.so \
    jni/x86_64/libconnect_norito_bridge.so; do
    if ! unzip -Z1 "$aar" | grep -Fxq -- "$entry"; then
      fail "client-android release AAR is missing $entry"
    fi
  done

  local tmp
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/iroha-mobile-sdk.XXXXXX")"
  trap 'rm -rf "$tmp"' RETURN
  local abi
  for abi in arm64-v8a x86_64; do
    local archive_entry="jni/$abi/libconnect_norito_bridge.so"
    if unzip -p "$aar" "$archive_entry" >"$tmp/$abi.so"; then
      check_binary_symbols "$tmp/$abi.so" "client-android $abi bridge" elf
    else
      fail "unable to extract client-android $abi bridge"
    fi
  done
}

check_source_contract
if [[ "$CHECK_APPLE" == "1" ]]; then
  check_apple
fi
if [[ "$CHECK_ANDROID" == "1" ]]; then
  check_android
fi

if [[ "$FAILURES" -ne 0 ]]; then
  echo "[mobile-sdk-artifacts] validation failed for $ROOT_DIR" >&2
  exit 1
fi
echo "[mobile-sdk-artifacts] validation passed for $ROOT_DIR"
