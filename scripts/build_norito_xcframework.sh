#!/usr/bin/env bash
set -euo pipefail

# Build a NoritoBridge.xcframework from the Rust connect_norito_bridge crate.
# - Produces a static-library XCFramework for every Apple slice so Xcode links it
#   without trying to embed/sign a framework inside simulator app bundles.
# - Bridge packaging skips the broader Norito bindings sync gate because unrelated
#   Kotlin/Java parity drift should not block rebuilding the Swift bridge artifact.
# - Requires: rustup + cargo, xcodebuild, lipo.
#
# Usage:
#   scripts/build_norito_xcframework.sh
#   scripts/build_norito_xcframework.sh --bridge-version 1.0.0
#   scripts/build_norito_xcframework.sh --privacy-production-enabled
#   scripts/build_norito_xcframework.sh --privacy-production-enabled --allow-dirty-source
#
# Outputs into ./dist/NoritoBridge.xcframework

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)
CRATE_DIR="$ROOT_DIR/crates/connect_norito_bridge"
INC_DIR="$CRATE_DIR/include"
OUT_DIR="${NORITO_BRIDGE_OUT_DIR:-$ROOT_DIR/dist}"
BUILD_DIR="${NORITO_BRIDGE_BUILD_DIR:-$ROOT_DIR/build/norito_bridge}"
STAGE_DIR="$BUILD_DIR/stage"
PUBLISH_ROOT=""
BUILD_PUBLISH_LOCK="$ROOT_DIR/build/.NoritoBridge.build-publish.lock"
BUILD_PUBLISH_LOCK_HELD=0
BUILD_PUBLISH_LOCK_TOKEN="$$.$RANDOM.$RANDOM"

LIB_CRATE_NAME="connect_norito_bridge"
FRAMEWORK_NAME="NoritoBridge"
STATIC_LIB_NAME="libNoritoBridge.a"
FRAMEWORK_BUNDLE_ID="${FRAMEWORK_BUNDLE_ID:-org.hyperledger.iroha.NoritoBridge}"

cleanup_build_state() {
  local status=$?
  local owner_token=""
  trap - EXIT HUP INT TERM
  set +e
  if [[ -n "$PUBLISH_ROOT" && -d "$PUBLISH_ROOT" \
    && "${PUBLISH_ROOT##*/}" == .NoritoBridge.publish.* ]]; then
    rm -rf -- "$PUBLISH_ROOT"
  fi
  if [[ "$BUILD_PUBLISH_LOCK_HELD" == "1" \
    && -d "$BUILD_PUBLISH_LOCK" \
    && ! -L "$BUILD_PUBLISH_LOCK" \
    && "${BUILD_PUBLISH_LOCK##*/}" == .NoritoBridge.build-publish.lock ]]; then
    if [[ -f "$BUILD_PUBLISH_LOCK/owner" && ! -L "$BUILD_PUBLISH_LOCK/owner" ]]; then
      owner_token=$(sed -n '3p' "$BUILD_PUBLISH_LOCK/owner")
    fi
    if [[ -z "$owner_token" || "$owner_token" == "$BUILD_PUBLISH_LOCK_TOKEN" ]]; then
      rm -f -- \
        "$BUILD_PUBLISH_LOCK/owner" \
        "$BUILD_PUBLISH_LOCK/owner.tmp.$BUILD_PUBLISH_LOCK_TOKEN"
      if ! rmdir -- "$BUILD_PUBLISH_LOCK"; then
        echo "[!] Unable to remove NoritoBridge build lock: $BUILD_PUBLISH_LOCK" >&2
      fi
    else
      echo "[!] NoritoBridge build lock ownership changed; refusing to remove it" >&2
    fi
  fi
  exit "$status"
}
trap cleanup_build_state EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

process_start_identity() {
  local pid="$1"
  ps -p "$pid" -o lstart= 2>/dev/null \
    | sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//'
}

lock_age_seconds() {
  local lock_path="$1"
  local modified=""
  local now=""
  if modified=$(stat -f %m "$lock_path" 2>/dev/null); then
    :
  elif modified=$(stat -c %Y "$lock_path" 2>/dev/null); then
    :
  else
    return 1
  fi
  now=$(date +%s)
  printf '%s\n' "$((now - modified))"
}

lock_owner_is_live() {
  local owner_file="$BUILD_PUBLISH_LOCK/owner"
  local owner_pid=""
  local owner_start=""
  local current_start=""
  if [[ ! -f "$owner_file" || -L "$owner_file" ]]; then
    return 1
  fi
  owner_pid=$(sed -n '1p' "$owner_file")
  owner_start=$(sed -n '2p' "$owner_file")
  if [[ ! "$owner_pid" =~ ^[0-9]+$ || -z "$owner_start" ]]; then
    return 1
  fi
  current_start=$(process_start_identity "$owner_pid")
  [[ -n "$current_start" && "$current_start" == "$owner_start" ]]
}

acquire_build_publish_lock() {
  local attempt=0
  local lock_age=""
  local owner_tmp=""
  local process_start=""
  local stale_lock=""

  mkdir -p "$(dirname "$BUILD_PUBLISH_LOCK")"
  while [[ "$attempt" -lt 3 ]]; do
    attempt=$((attempt + 1))
    if mkdir "$BUILD_PUBLISH_LOCK" 2>/dev/null; then
      BUILD_PUBLISH_LOCK_HELD=1
      process_start=$(process_start_identity "$$")
      if [[ -z "$process_start" ]]; then
        echo "[-] Unable to identify the NoritoBridge build lock owner" >&2
        exit 1
      fi
      owner_tmp="$BUILD_PUBLISH_LOCK/owner.tmp.$BUILD_PUBLISH_LOCK_TOKEN"
      printf '%s\n%s\n%s\n' \
        "$$" "$process_start" "$BUILD_PUBLISH_LOCK_TOKEN" > "$owner_tmp"
      mv "$owner_tmp" "$BUILD_PUBLISH_LOCK/owner"
      return
    fi

    if [[ -L "$BUILD_PUBLISH_LOCK" || ! -d "$BUILD_PUBLISH_LOCK" ]]; then
      echo "[-] Refusing unexpected NoritoBridge build lock path: $BUILD_PUBLISH_LOCK" >&2
      exit 1
    fi
    if lock_owner_is_live; then
      echo "[-] Another NoritoBridge builder holds $BUILD_PUBLISH_LOCK" >&2
      exit 1
    fi
    lock_age=$(lock_age_seconds "$BUILD_PUBLISH_LOCK" || true)
    if [[ -z "$lock_age" || "$lock_age" -lt 30 ]]; then
      echo "[-] NoritoBridge build lock has no live authenticated owner but is too new to reclaim: $BUILD_PUBLISH_LOCK" >&2
      exit 1
    fi

    stale_lock="$(dirname "$BUILD_PUBLISH_LOCK")/.NoritoBridge.build-publish.stale.$$.$RANDOM"
    if mv "$BUILD_PUBLISH_LOCK" "$stale_lock" 2>/dev/null; then
      rm -rf -- "$stale_lock"
      echo "[+] Reclaimed stale NoritoBridge build lock" >&2
    fi
  done
  echo "[-] Unable to acquire NoritoBridge build lock: $BUILD_PUBLISH_LOCK" >&2
  exit 1
}

: "${IPHONEOS_DEPLOYMENT_TARGET:=15.0}"
: "${IPHONESIMULATOR_DEPLOYMENT_TARGET:=15.0}"
: "${MACOSX_DEPLOYMENT_TARGET:=12.0}"
export IPHONESIMULATOR_DEPLOYMENT_TARGET
export MACOSX_DEPLOYMENT_TARGET

BRIDGE_VERSION=""
PRIVACY_PRODUCTION_ENABLED=0
ALLOW_DIRTY_SOURCE=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --bridge-version)
      shift
      BRIDGE_VERSION="${1:-}"
      if [[ -z "$BRIDGE_VERSION" ]]; then
        echo "[-] --bridge-version requires a value" >&2
        exit 1
      fi
      ;;
    --bridge-version=*)
      BRIDGE_VERSION="${1#*=}"
      ;;
    --privacy-production-enabled)
      PRIVACY_PRODUCTION_ENABLED=1
      ;;
    --allow-dirty-source)
      ALLOW_DIRTY_SOURCE=1
      ;;
    *)
      echo "[-] Unknown argument: $1" >&2
      echo "    Usage: $0 [--bridge-version <version>] [--privacy-production-enabled] [--allow-dirty-source]" >&2
      exit 1
      ;;
  esac
  shift
done

if [[ "$PRIVACY_PRODUCTION_ENABLED" == "1" && "${NORITO_BRIDGE_SKIP_CARGO_BUILDS:-0}" == "1" ]]; then
  echo "[-] --privacy-production-enabled cannot be combined with NORITO_BRIDGE_SKIP_CARGO_BUILDS=1" >&2
  exit 1
fi

CARGO_FEATURE_ARGS=()
if [[ "$PRIVACY_PRODUCTION_ENABLED" == "1" ]]; then
  CARGO_FEATURE_ARGS+=(--features privacy-production-enabled)
  CARGO_FEATURE_PROFILE="privacy-production-enabled"
  echo "[+] Enabling the audited privacy production bridge feature for every Apple slice" >&2
else
  CARGO_FEATURE_PROFILE="privacy-production-disabled"
  echo "[+] Privacy proof dispatch remains fail-closed (default bridge build)" >&2
fi
# Keep feature variants in disjoint Cargo targets. In particular, the default
# skip-build fast path must never package libraries left by an enabled build.
CARGO_BUILD_DIR_BASE="$BUILD_DIR/cargo-ios${IPHONEOS_DEPLOYMENT_TARGET//./_}-sim${IPHONESIMULATOR_DEPLOYMENT_TARGET//./_}-${CARGO_FEATURE_PROFILE}"

bridge_source_fingerprint() {
  python3 "$ROOT_DIR/scripts/norito_bridge_source_seal.py" \
    fingerprint --root "$ROOT_DIR"
}

bridge_source_status() {
  python3 "$ROOT_DIR/scripts/norito_bridge_source_seal.py" \
    status --root "$ROOT_DIR"
}

SOURCE_COMMIT_START=$(git -C "$ROOT_DIR" rev-parse HEAD)
SOURCE_STATUS_START=$(bridge_source_status)
SOURCE_FINGERPRINT_START=$(bridge_source_fingerprint)
if [[ -n "$SOURCE_STATUS_START" && "$ALLOW_DIRTY_SOURCE" != "1" ]]; then
  echo "[-] NoritoBridge production artifacts require a clean dependency-closure source tree" >&2
  echo "    Commit the bridge inputs or pass --allow-dirty-source for a fingerprint-bound local integration artifact." >&2
  exit 1
fi

assert_bridge_source_seal() {
  local phase="$1"
  local current_commit current_status current_fingerprint
  if ! current_commit=$(git -C "$ROOT_DIR" rev-parse HEAD) \
      || ! current_status=$(bridge_source_status) \
      || ! current_fingerprint=$(bridge_source_fingerprint); then
    echo "[-] NoritoBridge source became unreadable during $phase; refusing mixed-source Apple slices" >&2
    exit 1
  fi
  if [[ "$current_commit" != "$SOURCE_COMMIT_START" \
      || "$current_status" != "$SOURCE_STATUS_START" \
      || "$current_fingerprint" != "$SOURCE_FINGERPRINT_START" ]]; then
    echo "[-] NoritoBridge source changed during $phase; refusing mixed-source Apple slices" >&2
    exit 1
  fi
}

if [[ "${NORITO_BRIDGE_SOURCE_SEAL_TEST_ONLY:-0}" == "1" ]]; then
  if [[ -n "${NORITO_BRIDGE_SOURCE_SEAL_TEST_MUTATE:-}" ]]; then
    printf '\nsource-seal-negative-test\n' >> \
      "$ROOT_DIR/$NORITO_BRIDGE_SOURCE_SEAL_TEST_MUTATE"
  fi
  assert_bridge_source_seal "source-seal self-test"
  exit 0
fi

echo "[+] Using iOS deployment target (device): $IPHONEOS_DEPLOYMENT_TARGET" >&2
echo "[+] Using iOS deployment target (simulator): $IPHONESIMULATOR_DEPLOYMENT_TARGET" >&2

acquire_build_publish_lock
if [[ "${NORITO_BRIDGE_PRESERVE_CARGO_TARGETS:-0}" == "1" || "${NORITO_BRIDGE_SKIP_CARGO_BUILDS:-0}" == "1" ]]; then
  rm -rf "$STAGE_DIR"
else
  rm -rf "$CARGO_BUILD_DIR_BASE" "$STAGE_DIR"
fi
mkdir -p "$STAGE_DIR" "$OUT_DIR"
PUBLISH_ROOT="$(mktemp -d "$OUT_DIR/.NoritoBridge.publish.XXXXXX")"
PUBLISH_XCFRAMEWORK="$PUBLISH_ROOT/${FRAMEWORK_NAME}.xcframework"
PUBLISH_MANIFEST="$PUBLISH_XCFRAMEWORK/${FRAMEWORK_NAME}.artifacts.json"
PUBLISH_MANIFEST_LINK="$PUBLISH_ROOT/${FRAMEWORK_NAME}.artifacts.json"
FINAL_XCFRAMEWORK="$OUT_DIR/${FRAMEWORK_NAME}.xcframework"
FINAL_MANIFEST="$OUT_DIR/${FRAMEWORK_NAME}.artifacts.json"
CANONICAL_MANIFEST_RELATIVE_TARGET="${FRAMEWORK_NAME}.xcframework/${FRAMEWORK_NAME}.artifacts.json"

DEVICE_TRIPLE="aarch64-apple-ios"
SIM_ARM_TRIPLE="aarch64-apple-ios-sim"
SIM_X64_TRIPLE="x86_64-apple-ios"
MACOS_TRIPLE="aarch64-apple-darwin"
CARGO_BUILD_DIR_DEVICE="$CARGO_BUILD_DIR_BASE/$DEVICE_TRIPLE"
CARGO_BUILD_DIR_SIM_ARM="$CARGO_BUILD_DIR_BASE/$SIM_ARM_TRIPLE"
CARGO_BUILD_DIR_SIM_X64="$CARGO_BUILD_DIR_BASE/$SIM_X64_TRIPLE"
CARGO_BUILD_DIR_MACOS="$CARGO_BUILD_DIR_BASE/$MACOS_TRIPLE"

stage_cargo_library() {
  local cargo_target_dir="$1"
  local target_triple="$2"
  local label="$3"
  local source_library="$cargo_target_dir/$target_triple/release/lib${LIB_CRATE_NAME}.a"
  local staged_library="$STAGE_DIR/cargo-libraries/$target_triple/lib${LIB_CRATE_NAME}.a"
  if [[ ! -f "$source_library" ]]; then
    echo "[-] Missing $label static library after Cargo build: $source_library" >&2
    exit 1
  fi
  mkdir -p "$(dirname "$staged_library")"
  cp "$source_library" "$staged_library"
  if [[ "${NORITO_BRIDGE_PRESERVE_CARGO_TARGETS:-0}" != "1" ]]; then
    echo "[+] Reclaiming generated $label Cargo intermediates after staging its library" >&2
    rm -rf "$cargo_target_dir"
  fi
  printf '%s\n' "$staged_library"
}

if [[ "${NORITO_BRIDGE_SKIP_CARGO_BUILDS:-0}" == "1" ]]; then
  echo "[+] Skipping Rust static library builds; using existing target artifacts" >&2
  LIB_DEV="$CARGO_BUILD_DIR_DEVICE/$DEVICE_TRIPLE/release/lib${LIB_CRATE_NAME}.a"
  LIB_SIM_ARM="$CARGO_BUILD_DIR_SIM_ARM/$SIM_ARM_TRIPLE/release/lib${LIB_CRATE_NAME}.a"
  LIB_SIM_X64="$CARGO_BUILD_DIR_SIM_X64/$SIM_X64_TRIPLE/release/lib${LIB_CRATE_NAME}.a"
  LIB_MAC="$CARGO_BUILD_DIR_MACOS/$MACOS_TRIPLE/release/lib${LIB_CRATE_NAME}.a"
else
  echo "[+] Building Rust static libraries (release)" >&2
  echo "    Targets: $DEVICE_TRIPLE, $SIM_ARM_TRIPLE, $SIM_X64_TRIPLE, $MACOS_TRIPLE" >&2

  echo "    (Make sure you have installed targets via: rustup target add $DEVICE_TRIPLE $SIM_ARM_TRIPLE $SIM_X64_TRIPLE $MACOS_TRIPLE)" >&2

  # Rust uses IPHONEOS_DEPLOYMENT_TARGET for both iOS device and simulator targets,
  # while cc-based dependencies also honor IPHONESIMULATOR_DEPLOYMENT_TARGET.
  env IPHONEOS_DEPLOYMENT_TARGET="$IPHONEOS_DEPLOYMENT_TARGET" \
    NORITO_SKIP_BINDINGS_SYNC=1 \
    CARGO_TARGET_DIR="$CARGO_BUILD_DIR_DEVICE" \
    cargo build --locked -p "$LIB_CRATE_NAME" --lib --release --target "$DEVICE_TRIPLE" \
      "${CARGO_FEATURE_ARGS[@]+"${CARGO_FEATURE_ARGS[@]}"}"
  assert_bridge_source_seal "the iOS device build"
  LIB_DEV=$(stage_cargo_library "$CARGO_BUILD_DIR_DEVICE" "$DEVICE_TRIPLE" "iOS device")
  env IPHONEOS_DEPLOYMENT_TARGET="$IPHONESIMULATOR_DEPLOYMENT_TARGET" \
    IPHONESIMULATOR_DEPLOYMENT_TARGET="$IPHONESIMULATOR_DEPLOYMENT_TARGET" \
    NORITO_SKIP_BINDINGS_SYNC=1 \
    CARGO_TARGET_DIR="$CARGO_BUILD_DIR_SIM_ARM" \
    cargo build --locked -p "$LIB_CRATE_NAME" --lib --release --target "$SIM_ARM_TRIPLE" \
      "${CARGO_FEATURE_ARGS[@]+"${CARGO_FEATURE_ARGS[@]}"}"
  assert_bridge_source_seal "the arm64 simulator build"
  LIB_SIM_ARM=$(stage_cargo_library "$CARGO_BUILD_DIR_SIM_ARM" "$SIM_ARM_TRIPLE" "arm64 simulator")
  env IPHONEOS_DEPLOYMENT_TARGET="$IPHONESIMULATOR_DEPLOYMENT_TARGET" \
    IPHONESIMULATOR_DEPLOYMENT_TARGET="$IPHONESIMULATOR_DEPLOYMENT_TARGET" \
    NORITO_SKIP_BINDINGS_SYNC=1 \
    CARGO_TARGET_DIR="$CARGO_BUILD_DIR_SIM_X64" \
    cargo build --locked -p "$LIB_CRATE_NAME" --lib --release --target "$SIM_X64_TRIPLE" \
      "${CARGO_FEATURE_ARGS[@]+"${CARGO_FEATURE_ARGS[@]}"}"
  assert_bridge_source_seal "the x86_64 simulator build"
  LIB_SIM_X64=$(stage_cargo_library "$CARGO_BUILD_DIR_SIM_X64" "$SIM_X64_TRIPLE" "x86_64 simulator")
  env MACOSX_DEPLOYMENT_TARGET="$MACOSX_DEPLOYMENT_TARGET" \
    NORITO_SKIP_BINDINGS_SYNC=1 \
    CARGO_TARGET_DIR="$CARGO_BUILD_DIR_MACOS" \
    cargo build --locked -p "$LIB_CRATE_NAME" --lib --release --target "$MACOS_TRIPLE" \
      "${CARGO_FEATURE_ARGS[@]+"${CARGO_FEATURE_ARGS[@]}"}"
  assert_bridge_source_seal "the macOS build"
  LIB_MAC=$(stage_cargo_library "$CARGO_BUILD_DIR_MACOS" "$MACOS_TRIPLE" "macOS")
fi

assert_bridge_source_seal "Apple slice staging"

if [[ ! -f "$LIB_DEV" || ! -f "$LIB_SIM_ARM" || ! -f "$LIB_SIM_X64" || ! -f "$LIB_MAC" ]]; then
  echo "[-] Missing built libraries. Did the cargo builds succeed?" >&2
  exit 1
fi

if [[ -z "${BRIDGE_VERSION}" ]]; then
  VERSION_SOURCE="$ROOT_DIR/IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"
  if command -v rg >/dev/null 2>&1; then
    BRIDGE_VERSION=$(rg -n "expectedVersion" "$VERSION_SOURCE" | head -n1 | sed -E 's/.*"([^"]+)".*/\1/')
  else
    BRIDGE_VERSION=$(grep -m1 "expectedVersion" "$VERSION_SOURCE" | sed -E 's/.*"([^"]+)".*/\1/')
  fi
fi
if [[ -z "${BRIDGE_VERSION}" ]]; then
  echo "[-] Unable to determine NoritoBridge version for artifact manifest" >&2
  exit 1
fi
BRIDGE_BUNDLE_VERSION="${BRIDGE_VERSION%%-*}"
if [[ -z "$BRIDGE_BUNDLE_VERSION" ]]; then
  BRIDGE_BUNDLE_VERSION="1"
fi

echo "[+] Creating simulator universal static library" >&2
SIM_UNI="$STAGE_DIR/${FRAMEWORK_NAME}-sim-universal.a"
lipo -create -output "$SIM_UNI" "$LIB_SIM_ARM" "$LIB_SIM_X64"

echo "[+] Staging XCFramework slices" >&2
HEADERS_DEV="$STAGE_DIR/device-headers"
HEADERS_SIM="$STAGE_DIR/simulator-headers"
HEADERS_MAC="$STAGE_DIR/macos-headers"
LIB_DEV_STAGED="$STAGE_DIR/device/${STATIC_LIB_NAME}"
LIB_SIM_STAGED="$STAGE_DIR/simulator/${STATIC_LIB_NAME}"
LIB_MAC_STAGED="$STAGE_DIR/macos/${STATIC_LIB_NAME}"

mkdir -p "$HEADERS_DEV" "$HEADERS_SIM" "$HEADERS_MAC" "$(dirname "$LIB_DEV_STAGED")" "$(dirname "$LIB_SIM_STAGED")" "$(dirname "$LIB_MAC_STAGED")"

# Stage iOS static libraries. The normal production path moves the already
# staged raw libraries and drops the two simulator inputs after `lipo`, keeping
# only one copy of each final slice while xcodebuild packages the XCFramework.
if [[ "${NORITO_BRIDGE_PRESERVE_CARGO_TARGETS:-0}" != "1" \
    && "${NORITO_BRIDGE_SKIP_CARGO_BUILDS:-0}" != "1" ]]; then
  mv "$LIB_DEV" "$LIB_DEV_STAGED"
  mv "$SIM_UNI" "$LIB_SIM_STAGED"
  mv "$LIB_MAC" "$LIB_MAC_STAGED"
  rm -f "$LIB_SIM_ARM" "$LIB_SIM_X64"
else
  cp "$LIB_DEV" "$LIB_DEV_STAGED"
  cp "$SIM_UNI" "$LIB_SIM_STAGED"
  cp "$LIB_MAC" "$LIB_MAC_STAGED"
fi

# Copy headers for static-library slices. xcodebuild copies this directory as the
# slice's Headers bundle, so the modulemap lives at Headers/module.modulemap.
cp "$INC_DIR/connect_norito_bridge.h" "$HEADERS_DEV/connect_norito_bridge.h"
cp "$INC_DIR/NoritoBridge.h" "$HEADERS_DEV/NoritoBridge.h"
cp "$CRATE_DIR/module.modulemap.template" "$HEADERS_DEV/module.modulemap"
cp "$INC_DIR/connect_norito_bridge.h" "$HEADERS_SIM/connect_norito_bridge.h"
cp "$INC_DIR/NoritoBridge.h" "$HEADERS_SIM/NoritoBridge.h"
cp "$CRATE_DIR/module.modulemap.template" "$HEADERS_SIM/module.modulemap"
cp "$INC_DIR/connect_norito_bridge.h" "$HEADERS_MAC/connect_norito_bridge.h"
cp "$INC_DIR/NoritoBridge.h" "$HEADERS_MAC/NoritoBridge.h"
cp "$CRATE_DIR/module.modulemap.template" "$HEADERS_MAC/module.modulemap"

write_static_xcframework_info_plist() {
  local plist="$PUBLISH_XCFRAMEWORK/Info.plist"

  mkdir -p "$(dirname "$plist")"
  cat > "$plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>AvailableLibraries</key>
  <array>
    <dict>
      <key>HeadersPath</key>
      <string>Headers</string>
      <key>LibraryIdentifier</key>
      <string>ios-arm64</string>
      <key>LibraryPath</key>
      <string>${STATIC_LIB_NAME}</string>
      <key>SupportedArchitectures</key>
      <array>
        <string>arm64</string>
      </array>
      <key>SupportedPlatform</key>
      <string>ios</string>
    </dict>
    <dict>
      <key>HeadersPath</key>
      <string>Headers</string>
      <key>LibraryIdentifier</key>
      <string>ios-arm64_x86_64-simulator</string>
      <key>LibraryPath</key>
      <string>${STATIC_LIB_NAME}</string>
      <key>SupportedArchitectures</key>
      <array>
        <string>arm64</string>
        <string>x86_64</string>
      </array>
      <key>SupportedPlatform</key>
      <string>ios</string>
      <key>SupportedPlatformVariant</key>
      <string>simulator</string>
    </dict>
    <dict>
      <key>HeadersPath</key>
      <string>Headers</string>
      <key>LibraryIdentifier</key>
      <string>macos-arm64</string>
      <key>LibraryPath</key>
      <string>${STATIC_LIB_NAME}</string>
      <key>SupportedArchitectures</key>
      <array>
        <string>arm64</string>
      </array>
      <key>SupportedPlatform</key>
      <string>macos</string>
    </dict>
  </array>
  <key>CFBundlePackageType</key>
  <string>XFWK</string>
  <key>XCFrameworkFormatVersion</key>
  <string>1.0</string>
</dict>
</plist>
EOF
}

echo "[+] Creating XCFramework" >&2
if ! xcodebuild -create-xcframework \
  -library "$LIB_DEV_STAGED" -headers "$HEADERS_DEV" \
  -library "$LIB_SIM_STAGED" -headers "$HEADERS_SIM" \
  -library "$LIB_MAC_STAGED" -headers "$HEADERS_MAC" \
  -output "$PUBLISH_XCFRAMEWORK"; then
  echo "[!] xcodebuild reported a non-zero exit; rebuilding the fallback from an empty candidate" >&2
  rm -rf -- "$PUBLISH_XCFRAMEWORK"
  copy_static_xcframework_slice() {
    local identifier="$1"
    local source_lib="$2"
    local source_headers="$3"
    local slice_dir="$PUBLISH_XCFRAMEWORK/$identifier"

    mkdir -p "$slice_dir/Headers"
    cp "$source_lib" "$slice_dir/${STATIC_LIB_NAME}"
    cp "$source_headers/NoritoBridge.h" "$slice_dir/Headers/NoritoBridge.h"
    cp "$source_headers/connect_norito_bridge.h" "$slice_dir/Headers/connect_norito_bridge.h"
    cp "$source_headers/module.modulemap" "$slice_dir/Headers/module.modulemap"
  }

  rm -rf "$PUBLISH_XCFRAMEWORK/ios-arm64-simulator"
  copy_static_xcframework_slice "ios-arm64" "$LIB_DEV_STAGED" "$HEADERS_DEV"
  copy_static_xcframework_slice "ios-arm64_x86_64-simulator" "$LIB_SIM_STAGED" "$HEADERS_SIM"
  copy_static_xcframework_slice "macos-arm64" "$LIB_MAC_STAGED" "$HEADERS_MAC"
  write_static_xcframework_info_plist

  REQUIRED_OUTPUTS=(
    "$PUBLISH_XCFRAMEWORK/Info.plist"
    "$PUBLISH_XCFRAMEWORK/ios-arm64/${STATIC_LIB_NAME}"
    "$PUBLISH_XCFRAMEWORK/ios-arm64/Headers/NoritoBridge.h"
    "$PUBLISH_XCFRAMEWORK/ios-arm64/Headers/connect_norito_bridge.h"
    "$PUBLISH_XCFRAMEWORK/ios-arm64_x86_64-simulator/${STATIC_LIB_NAME}"
    "$PUBLISH_XCFRAMEWORK/ios-arm64_x86_64-simulator/Headers/NoritoBridge.h"
    "$PUBLISH_XCFRAMEWORK/ios-arm64_x86_64-simulator/Headers/connect_norito_bridge.h"
    "$PUBLISH_XCFRAMEWORK/macos-arm64/${STATIC_LIB_NAME}"
    "$PUBLISH_XCFRAMEWORK/macos-arm64/Headers/NoritoBridge.h"
    "$PUBLISH_XCFRAMEWORK/macos-arm64/Headers/connect_norito_bridge.h"
  )
  for output in "${REQUIRED_OUTPUTS[@]}"; do
    if [[ ! -f "$output" ]]; then
      echo "[-] Missing XCFramework output after xcodebuild failure: $output" >&2
      exit 1
    fi
  done
fi

assert_bridge_source_seal "XCFramework packaging"

echo "[+] XCFramework staged: $PUBLISH_XCFRAMEWORK" >&2
if [[ "$PRIVACY_PRODUCTION_ENABLED" == "1" ]]; then
  touch "$PUBLISH_XCFRAMEWORK/.privacy-production-enabled"
fi

IOS_BIN="$PUBLISH_XCFRAMEWORK/ios-arm64/${STATIC_LIB_NAME}"
SIM_BIN="$PUBLISH_XCFRAMEWORK/ios-arm64_x86_64-simulator/${STATIC_LIB_NAME}"
MAC_BIN="$PUBLISH_XCFRAMEWORK/macos-arm64/${STATIC_LIB_NAME}"
if [[ ! -f "$IOS_BIN" || ! -f "$SIM_BIN" || ! -f "$MAC_BIN" ]]; then
  echo "[-] Missing XCFramework binaries needed to emit NoritoBridge.artifacts.json" >&2
  exit 1
fi

IOS_HASH=$(shasum -a 256 "$IOS_BIN" | awk '{print $1}')
SIM_HASH=$(shasum -a 256 "$SIM_BIN" | awk '{print $1}')
MAC_HASH=$(shasum -a 256 "$MAC_BIN" | awk '{print $1}')
HEADER_HASH=$(shasum -a 256 "$INC_DIR/connect_norito_bridge.h" | awk '{print $1}')
BRIDGE_ABI_VERSION=$(sed -nE \
  's/.*CONNECT_NORITO_BRIDGE_ABI_VERSION:[[:space:]]*u32[[:space:]]*=[[:space:]]*([0-9]+).*/\1/p' \
  "$CRATE_DIR/src/lib.rs" | head -n1)
if [[ -z "$BRIDGE_ABI_VERSION" ]]; then
  echo "[-] Unable to determine native bridge ABI version" >&2
  exit 1
fi
if [[ "$BRIDGE_ABI_VERSION" != "21" ]]; then
  echo "[-] First-release NoritoBridge artifacts require exact native bridge ABI 21 (found $BRIDGE_ABI_VERSION)" >&2
  exit 1
fi
SOURCE_COMMIT="$SOURCE_COMMIT_START"
SOURCE_TREE_DIRTY=false
if [[ -n "$SOURCE_STATUS_START" ]]; then
  SOURCE_TREE_DIRTY=true
fi
SOURCE_FINGERPRINT="$SOURCE_FINGERPRINT_START"
PRIVACY_PRODUCTION_JSON=false
CARGO_FEATURES_JSON='[]'
if [[ "$PRIVACY_PRODUCTION_ENABLED" == "1" ]]; then
  PRIVACY_PRODUCTION_JSON=true
  CARGO_FEATURES_JSON='["privacy-production-enabled"]'
fi

cat > "$PUBLISH_MANIFEST" <<EOF
{
  "version": "$BRIDGE_VERSION",
  "native_bridge_abi_version": $BRIDGE_ABI_VERSION,
  "privacy_production_enabled": $PRIVACY_PRODUCTION_JSON,
  "cargo_features": $CARGO_FEATURES_JSON,
  "source_commit": "$SOURCE_COMMIT",
  "source_tree_dirty": $SOURCE_TREE_DIRTY,
  "source_fingerprint_sha256": "$SOURCE_FINGERPRINT",
  "bridge_header_sha256": "$HEADER_HASH",
  "required_symbols": [
    "connect_norito_bridge_abi_version",
    "connect_norito_free",
    "connect_norito_encode_transfer_signed_transaction",
    "connect_norito_encode_transfer_instruction_box",
    "connect_norito_detached_transaction_scaffold_inspect_v1",
    "connect_norito_detached_transaction_scaffold_finalize_ed25519_v1",
    "connect_norito_canonical_json_blake3_v1",
    "connect_norito_encode_account_onboarding_plan_body_v1",
    "connect_norito_alias_instruction_round_trip_v1",
    "connect_norito_validation_fee_current_policy_proof_request_v1",
    "connect_norito_validation_fee_current_policy_proof_verify_v1",
    "connect_norito_kagemusha_recursive_spend_capabilities_v4",
    "connect_norito_kagemusha_topup_finality_verify_v4",
    "connect_norito_kagemusha_topup_shield_build_unsigned_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_begin_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_write_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_finalize_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_cancel_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_set_install_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v4",
    "connect_norito_kagemusha_recursive_spend_installed_manifest_sha256_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v4",
    "connect_norito_kagemusha_output_membership_frontier_build_v4",
    "connect_norito_kagemusha_output_membership_paths_derive_v4",
    "connect_norito_kagemusha_recursive_spend_branch_validate_v4",
    "connect_norito_kagemusha_recursive_spend_topup_provenance_build_v4",
    "connect_norito_kagemusha_recursive_spend_topup_provenance_validate_v4",
    "connect_norito_kagemusha_recursive_spend_init_v4",
    "connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v4",
    "connect_norito_kagemusha_recursive_spend_topup_finalize_request_v4",
    "connect_norito_kagemusha_recursive_spend_topup_v4",
    "connect_norito_kagemusha_recursive_spend_append_v4",
    "connect_norito_kagemusha_recursive_spend_verify_v4",
    "connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v4",
    "connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v4",
    "connect_norito_kagemusha_recursive_spend_redeem_v4",
    "connect_norito_kagemusha_recursive_spend_redemption_change_prepare_v4",
    "connect_norito_kagemusha_secret_free_buffer",
    "connect_norito_kagemusha_receiver_key_reference_v2",
    "connect_norito_kagemusha_recipient_output_derive_v2",
    "connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2",
    "connect_norito_kagemusha_recipient_payment_request_create_v2",
    "connect_norito_kagemusha_recipient_payment_request_verify_v2",
    "connect_norito_kagemusha_recipient_lineage_query_create_v2",
    "connect_norito_kagemusha_recipient_registration_lineage_verify_v1",
    "connect_norito_kagemusha_recipient_registration_lineage_verify_v2",
    "connect_norito_kagemusha_recipient_receive_offer_create_v2",
    "connect_norito_kagemusha_recipient_receive_offer_project_v2",
    "connect_norito_kagemusha_recipient_receive_offer_verify_v2",
    "connect_norito_kagemusha_request_authorization_signing_bytes_v2",
    "connect_norito_kagemusha_request_authorization_create_v2",
    "connect_norito_kagemusha_request_authorization_finalize_hardware_v2",
    "connect_norito_kagemusha_request_authorization_finalize_ios_app_attest_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_payload_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_create_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_verify_v2",
    "connect_norito_kagemusha_recursive_spend_peer_split_change_prepare_v4",
    "connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v4",
    "connect_norito_kagemusha_recursive_spend_peer_payment_validate_v4",
    "connect_norito_kagemusha_recursive_spend_bundle_summary_v4"
  ],
  "kagemusha_mobile_artifact_roles": [
    {
      "role": "native_bridge",
      "purpose": "typed Norito codecs and privacy proof execution",
      "circuit_id": null,
      "abi": $BRIDGE_ABI_VERSION,
      "artifact_type": "xcframework",
      "delivery": "bridge_embedded",
      "required_by": ["topup", "peer_send", "peer_receive", "redemption"]
    },
    {
      "role": "transfer_proving_key",
      "purpose": "prove exact confidential top-up and offline split transitions",
      "circuit_id": "confidential-transfer-v2",
      "abi": $BRIDGE_ABI_VERSION,
      "artifact_type": "halo2_ipa_proving_key",
      "delivery": "bridge_embedded",
      "production_ready": $PRIVACY_PRODUCTION_JSON,
      "required_by": ["topup", "peer_send"]
    },
    {
      "role": "transfer_verifier_record",
      "purpose": "verify top-up and offline split evidence at an active height",
      "circuit_id": "confidential-transfer-v2",
      "abi": $BRIDGE_ABI_VERSION,
      "artifact_type": "norito_verifying_key_record",
      "delivery": "torii_readiness_snapshot",
      "required_by": ["topup", "peer_send", "peer_receive"]
    },
    {
      "role": "unshield_proving_key",
      "purpose": "prove full or partial offline-to-online redemption",
      "circuit_id": "confidential-unshield-v3",
      "abi": $BRIDGE_ABI_VERSION,
      "artifact_type": "halo2_ipa_proving_key",
      "delivery": "bridge_embedded",
      "production_ready": $PRIVACY_PRODUCTION_JSON,
      "required_by": ["redemption"]
    },
    {
      "role": "unshield_verifier_record",
      "purpose": "verify proof-bound public credit and optional offline change",
      "circuit_id": "confidential-unshield-v3",
      "abi": $BRIDGE_ABI_VERSION,
      "artifact_type": "norito_verifying_key_record",
      "delivery": "torii_readiness_snapshot",
      "required_by": ["redemption"]
    },
    {
      "role": "step_eq_params_ipa",
      "purpose": "step_eq_params_ipa",
      "file_name": "step-eq.params-ipa.krv4",
      "circuit_id": "kagemusha-recursive-spend-step-eq-compact-layout-v5",
      "abi": $BRIDGE_ABI_VERSION,
      "artifact_type": "KagemushaRecursiveSpendPastaCycleArtifactsV4",
      "delivery": "content_addressed_external",
      "required_by": ["topup", "peer_send", "peer_receive", "redemption"]
    },
    {
      "role": "step_eq_proving_key",
      "purpose": "step_eq_proving_key",
      "file_name": "step-eq.proving-key.krv4",
      "circuit_id": "kagemusha-recursive-spend-step-eq-compact-layout-v5",
      "abi": $BRIDGE_ABI_VERSION,
      "artifact_type": "KagemushaRecursiveSpendPastaCycleArtifactsV4",
      "delivery": "content_addressed_external",
      "required_by": ["topup", "peer_send", "redemption"]
    },
    {
      "role": "step_eq_verifying_key",
      "purpose": "step_eq_verifying_key",
      "file_name": "step-eq.verifying-key.krv4",
      "circuit_id": "kagemusha-recursive-spend-step-eq-compact-layout-v5",
      "abi": $BRIDGE_ABI_VERSION,
      "artifact_type": "KagemushaRecursiveSpendPastaCycleArtifactsV4",
      "delivery": "content_addressed_external",
      "required_by": ["topup", "peer_send", "peer_receive", "redemption"]
    },
    {
      "role": "step_eq_bootstrap_witness",
      "purpose": "step_eq_bootstrap_witness",
      "file_name": "step-eq.bootstrap-witness.krv4",
      "circuit_id": "kagemusha-recursive-spend-step-eq-compact-layout-v5",
      "abi": $BRIDGE_ABI_VERSION,
      "artifact_type": "KagemushaRecursiveSpendPastaCycleArtifactsV4",
      "delivery": "content_addressed_external",
      "required_by": ["topup", "peer_send", "peer_receive", "redemption"]
    },
    {
      "role": "step_ep_params_ipa",
      "purpose": "step_ep_params_ipa",
      "file_name": "step-ep.params-ipa.krv4",
      "circuit_id": "kagemusha-recursive-spend-step-ep-compact-lineage-v5",
      "abi": $BRIDGE_ABI_VERSION,
      "artifact_type": "KagemushaRecursiveSpendPastaCycleArtifactsV4",
      "delivery": "content_addressed_external",
      "required_by": ["topup", "peer_send", "peer_receive", "redemption"]
    },
    {
      "role": "step_ep_proving_key",
      "purpose": "step_ep_proving_key",
      "file_name": "step-ep.proving-key.krv4",
      "circuit_id": "kagemusha-recursive-spend-step-ep-compact-lineage-v5",
      "abi": $BRIDGE_ABI_VERSION,
      "artifact_type": "KagemushaRecursiveSpendPastaCycleArtifactsV4",
      "delivery": "content_addressed_external",
      "required_by": ["topup", "peer_send", "redemption"]
    },
    {
      "role": "step_ep_verifying_key",
      "purpose": "step_ep_verifying_key",
      "file_name": "step-ep.verifying-key.krv4",
      "circuit_id": "kagemusha-recursive-spend-step-ep-compact-lineage-v5",
      "abi": $BRIDGE_ABI_VERSION,
      "artifact_type": "KagemushaRecursiveSpendPastaCycleArtifactsV4",
      "delivery": "content_addressed_external",
      "required_by": ["topup", "peer_send", "peer_receive", "redemption"]
    },
    {
      "role": "step_ep_bootstrap_witness",
      "purpose": "step_ep_bootstrap_witness",
      "file_name": "step-ep.bootstrap-witness.krv4",
      "circuit_id": "kagemusha-recursive-spend-step-ep-compact-lineage-v5",
      "abi": $BRIDGE_ABI_VERSION,
      "artifact_type": "KagemushaRecursiveSpendPastaCycleArtifactsV4",
      "delivery": "content_addressed_external",
      "required_by": ["topup", "peer_send", "peer_receive", "redemption"]
    },
    {
      "role": "topup_finality_roster",
      "purpose": "topup_finality_roster",
      "circuit_id": "kagemusha-topup-finality-qc-merkle-v2",
      "abi": $BRIDGE_ABI_VERSION,
      "artifact_type": "iroha_data_model::offline::model::KagemushaTopUpFinalityRosterArtifactV2",
      "delivery": "content_addressed_external",
      "required_by": ["topup"]
    }
  ],
  "hashes": {
    "ios-arm64": "$IOS_HASH",
    "ios-arm64_x86_64-simulator": "$SIM_HASH",
    "macos-arm64": "$MAC_HASH"
  }
}
EOF
echo "[+] Wrote staged artifact manifest: $PUBLISH_MANIFEST" >&2
ln -s "$CANONICAL_MANIFEST_RELATIVE_TARGET" "$PUBLISH_MANIFEST_LINK"

python3 - \
  "$PUBLISH_XCFRAMEWORK" "$PUBLISH_MANIFEST" \
  "$PUBLISH_MANIFEST_LINK" "$CANONICAL_MANIFEST_RELATIVE_TARGET" <<'PY'
import hashlib
import json
import os
from pathlib import Path
import plistlib
import stat
import sys


def object_without_duplicates(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON member: {key}")
        result[key] = value
    return result


xcframework = Path(sys.argv[1])
manifest_path = Path(sys.argv[2])
manifest_link = Path(sys.argv[3])
manifest_link_target = sys.argv[4]
expected_slices = {
    "ios-arm64": {
        "architectures": ["arm64"],
        "platform": "ios",
        "variant": None,
    },
    "ios-arm64_x86_64-simulator": {
        "architectures": ["arm64", "x86_64"],
        "platform": "ios",
        "variant": "simulator",
    },
    "macos-arm64": {
        "architectures": ["arm64"],
        "platform": "macos",
        "variant": None,
    },
}
library_name = "libNoritoBridge.a"

if xcframework.is_symlink() or not xcframework.is_dir():
    raise SystemExit("staged NoritoBridge XCFramework root is not a regular directory")
for root, directories, files in os.walk(xcframework, followlinks=False):
    for name in [*directories, *files]:
        path = Path(root) / name
        if stat.S_ISLNK(path.lstat().st_mode):
            raise SystemExit(f"staged NoritoBridge contains a symlink: {path}")

with manifest_path.open("r", encoding="utf-8") as handle:
    manifest = json.load(handle, object_pairs_hook=object_without_duplicates)
if manifest.get("native_bridge_abi_version") != 21:
    raise SystemExit("staged NoritoBridge manifest does not bind exact ABI 21")
hashes = manifest.get("hashes")
if not isinstance(hashes, dict) or set(hashes) != set(expected_slices):
    raise SystemExit("staged NoritoBridge manifest has a non-canonical slice inventory")
if not manifest_link.is_symlink() or os.readlink(manifest_link) != manifest_link_target:
    raise SystemExit("staged NoritoBridge public manifest link is not canonical")

info_path = xcframework / "Info.plist"
with info_path.open("rb") as handle:
    info = plistlib.load(handle)
libraries = info.get("AvailableLibraries")
if (
    not isinstance(libraries, list)
    or len(libraries) != len(expected_slices)
    or any(not isinstance(library, dict) for library in libraries)
):
    raise SystemExit("staged NoritoBridge Info.plist has a non-canonical slice list")
metadata = {}
for library in libraries:
    identifier = library.get("LibraryIdentifier")
    if identifier in metadata:
        raise SystemExit(f"duplicate staged NoritoBridge slice: {identifier!r}")
    metadata[identifier] = library
if set(metadata) != set(expected_slices):
    raise SystemExit("staged NoritoBridge Info.plist has a non-canonical slice inventory")
if info.get("CFBundlePackageType") != "XFWK":
    raise SystemExit("staged NoritoBridge Info.plist has the wrong package type")
if info.get("XCFrameworkFormatVersion") != "1.0":
    raise SystemExit("staged NoritoBridge Info.plist has the wrong format version")

for identifier, expected in expected_slices.items():
    library = metadata[identifier]
    if library.get("LibraryPath") != library_name:
        raise SystemExit(
            f"staged NoritoBridge slice {identifier} has a non-canonical LibraryPath"
        )
    if "BinaryPath" in library and library["BinaryPath"] != library_name:
        raise SystemExit(
            f"staged NoritoBridge slice {identifier} has a conflicting BinaryPath"
        )
    if library.get("HeadersPath") != "Headers":
        raise SystemExit(
            f"staged NoritoBridge slice {identifier} has a non-canonical HeadersPath"
        )
    if library.get("SupportedArchitectures") != expected["architectures"]:
        raise SystemExit(
            f"staged NoritoBridge slice {identifier} has non-canonical architectures"
        )
    if library.get("SupportedPlatform") != expected["platform"]:
        raise SystemExit(
            f"staged NoritoBridge slice {identifier} has a non-canonical platform"
        )
    if expected["variant"] is None:
        if "SupportedPlatformVariant" in library:
            raise SystemExit(
                f"staged NoritoBridge slice {identifier} has an unexpected variant"
            )
    elif library.get("SupportedPlatformVariant") != expected["variant"]:
        raise SystemExit(
            f"staged NoritoBridge slice {identifier} has a non-canonical variant"
        )

    binary = xcframework / identifier / library_name
    headers = xcframework / identifier / "Headers"
    modulemap = headers / "module.modulemap"
    if "module NoritoBridge" not in modulemap.read_text(encoding="utf-8"):
        raise SystemExit(f"staged NoritoBridge module map is invalid: {modulemap}")
    digest = hashlib.sha256()
    with binary.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    actual_hash = digest.hexdigest()
    if hashes[identifier] != actual_hash:
        raise SystemExit(f"staged NoritoBridge hash mismatch for {identifier}")

expected_top_level = {
    "Info.plist",
    "NoritoBridge.artifacts.json",
    *expected_slices,
}
privacy_marker = xcframework / ".privacy-production-enabled"
if manifest.get("privacy_production_enabled") is True:
    expected_top_level.add(privacy_marker.name)
elif manifest.get("privacy_production_enabled") is not False:
    raise SystemExit("staged NoritoBridge manifest has a non-boolean privacy mode")
if {entry.name for entry in xcframework.iterdir()} != expected_top_level:
    raise SystemExit("staged NoritoBridge has unexpected top-level artifacts")

expected_slice_entries = {"Headers", library_name}
expected_header_entries = {
    "NoritoBridge.h",
    "connect_norito_bridge.h",
    "module.modulemap",
}
for identifier in expected_slices:
    slice_path = xcframework / identifier
    headers_path = slice_path / "Headers"
    if {entry.name for entry in slice_path.iterdir()} != expected_slice_entries:
        raise SystemExit(f"staged NoritoBridge slice {identifier} has unexpected files")
    if {entry.name for entry in headers_path.iterdir()} != expected_header_entries:
        raise SystemExit(f"staged NoritoBridge slice {identifier} has unexpected headers")

def fsync_path(path):
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


directories_to_sync = []
for root, directories, files in os.walk(xcframework, topdown=True):
    root_path = Path(root)
    directories_to_sync.append(root_path)
    for name in files:
        fsync_path(root_path / name)
for directory in reversed(directories_to_sync):
    fsync_path(directory)
fsync_path(xcframework.parent)
PY

assert_bridge_source_seal "staged artifact validation"

if [[ "$ALLOW_DIRTY_SOURCE" == "1" ]]; then
  MOBILE_SDK_ALLOW_DIRTY_SOURCE=1 \
    MOBILE_SDK_APPLE_ARTIFACT_DIR="$PUBLISH_ROOT" \
    bash "$ROOT_DIR/scripts/check_mobile_sdk_artifacts.sh" --root "$ROOT_DIR" --apple-only
else
  MOBILE_SDK_APPLE_ARTIFACT_DIR="$PUBLISH_ROOT" \
    bash "$ROOT_DIR/scripts/check_mobile_sdk_artifacts.sh" --root "$ROOT_DIR" --apple-only
fi

assert_bridge_source_seal "pre-publication artifact verification"

if [[ "${NORITO_BRIDGE_PRESERVE_CARGO_TARGETS:-0}" != "1" \
  && "${NORITO_BRIDGE_SKIP_CARGO_BUILDS:-0}" != "1" ]]; then
  echo "[+] Removing generated Apple Cargo/staging intermediates before publication" >&2
  rm -rf "$CARGO_BUILD_DIR_BASE" "$STAGE_DIR"
fi

python3 - \
  "$PUBLISH_XCFRAMEWORK" "$FINAL_XCFRAMEWORK" "$FINAL_MANIFEST" \
  "$CANONICAL_MANIFEST_RELATIVE_TARGET" <<'PY'
import ctypes
import hashlib
import json
import os
from pathlib import Path
import stat
import sys
import tempfile


RENAME_EXCHANGE = 0x00000002
libc = ctypes.CDLL(None, use_errno=True)
SLICE_PATHS = {
    "ios-arm64": "libNoritoBridge.a",
    "ios-arm64_x86_64-simulator": "libNoritoBridge.a",
    "macos-arm64": "libNoritoBridge.a",
}


def object_without_duplicates(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON member: {key}")
        result[key] = value
    return result


def fsync_path(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def validate_manifest_bytes(framework: Path, manifest_bytes: bytes) -> None:
    try:
        manifest = json.loads(
            manifest_bytes.decode("utf-8"),
            object_pairs_hook=object_without_duplicates,
        )
    except (UnicodeError, ValueError, TypeError) as error:
        raise RuntimeError("live NoritoBridge manifest is malformed") from error
    hashes = manifest.get("hashes")
    if not isinstance(hashes, dict) or set(hashes) != set(SLICE_PATHS):
        raise RuntimeError("live NoritoBridge manifest has a non-canonical hash inventory")
    for identifier, library_name in SLICE_PATHS.items():
        binary = framework / identifier / library_name
        if (
            not binary.is_file()
            or binary.is_symlink()
            or (framework / identifier).is_symlink()
        ):
            raise RuntimeError(f"live NoritoBridge slice is not regular: {binary}")
        digest = hashlib.sha256()
        with binary.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
        if hashes[identifier] != digest.hexdigest():
            raise RuntimeError(f"live NoritoBridge hash mismatch for {identifier}")


def write_embedded_manifest(framework: Path, contents: bytes) -> None:
    destination = framework / "NoritoBridge.artifacts.json"
    if os.path.lexists(destination) and (
        destination.is_symlink() or not destination.is_file()
    ):
        raise RuntimeError(f"refusing unexpected embedded manifest: {destination}")
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=".NoritoBridge.artifacts.",
        suffix=".tmp",
        dir=framework,
    )
    temporary = Path(temporary_name)
    try:
        os.fchmod(descriptor, 0o644)
        with os.fdopen(descriptor, "wb", closefd=True) as handle:
            handle.write(contents)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, destination)
        fsync_path(framework)
    finally:
        if os.path.lexists(temporary):
            temporary.unlink()


def install_public_manifest_link(link: Path, target: str) -> None:
    temporary = link.parent / f".NoritoBridge.artifacts.link.{os.getpid()}"
    if os.path.lexists(temporary):
        raise RuntimeError(f"refusing unexpected manifest-link temporary: {temporary}")
    try:
        os.symlink(target, temporary)
        os.replace(temporary, link)
        fsync_path(link.parent)
    finally:
        if os.path.lexists(temporary):
            temporary.unlink()


def prepare_stable_manifest_link(
    final_framework: Path,
    final_manifest: Path,
    relative_target: str,
) -> None:
    framework_exists = os.path.lexists(final_framework)
    manifest_exists = os.path.lexists(final_manifest)

    if framework_exists:
        if final_framework.is_symlink() or not final_framework.is_dir():
            raise RuntimeError(
                f"refusing unexpected live XCFramework path: {final_framework}"
            )
        embedded = final_framework / "NoritoBridge.artifacts.json"
        if manifest_exists and final_manifest.is_symlink():
            if os.readlink(final_manifest) != relative_target:
                raise RuntimeError(
                    f"refusing non-canonical public manifest link: {final_manifest}"
                )
            if embedded.is_symlink() or not embedded.is_file():
                raise RuntimeError("canonical public manifest link has no regular target")
            contents = embedded.read_bytes()
            validate_manifest_bytes(final_framework, contents)
            return
        if manifest_exists:
            if not final_manifest.is_file() or final_manifest.is_symlink():
                raise RuntimeError(
                    f"refusing unexpected live manifest path: {final_manifest}"
                )
            contents = final_manifest.read_bytes()
            validate_manifest_bytes(final_framework, contents)
            write_embedded_manifest(final_framework, contents)
            if embedded.read_bytes() != contents:
                raise RuntimeError("embedded live manifest differs after migration")
            install_public_manifest_link(final_manifest, relative_target)
            return
        if embedded.is_symlink() or not embedded.is_file():
            raise RuntimeError(
                "live XCFramework has neither a public nor an embedded manifest"
            )
        contents = embedded.read_bytes()
        validate_manifest_bytes(final_framework, contents)
        install_public_manifest_link(final_manifest, relative_target)
        return

    if manifest_exists:
        if (
            not final_manifest.is_symlink()
            or os.readlink(final_manifest) != relative_target
        ):
            raise RuntimeError(
                "refusing an orphaned non-canonical public NoritoBridge manifest"
            )
    else:
        install_public_manifest_link(final_manifest, relative_target)


def exchange(left: Path, right: Path) -> None:
    encoded_left = os.fsencode(left)
    encoded_right = os.fsencode(right)
    if hasattr(libc, "renamex_np"):
        renamex_np = libc.renamex_np
        renamex_np.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint]
        renamex_np.restype = ctypes.c_int
        result = renamex_np(encoded_left, encoded_right, RENAME_EXCHANGE)
    elif hasattr(libc, "renameat2"):
        renameat2 = libc.renameat2
        renameat2.argtypes = [
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_uint,
        ]
        renameat2.restype = ctypes.c_int
        result = renameat2(
            -100, encoded_left, -100, encoded_right, RENAME_EXCHANGE
        )
    else:
        raise OSError("this host lacks an atomic path-exchange primitive")
    if result != 0:
        error = ctypes.get_errno()
        raise OSError(error, os.strerror(error), f"{left} <-> {right}")


def publish(staged: Path, final: Path) -> None:
    if os.path.lexists(final):
        exchange(staged, final)
    else:
        os.rename(staged, final)
    fsync_path(staged.parent)
    fsync_path(final.parent)


staged_xcframework = Path(sys.argv[1])
final_xcframework = Path(sys.argv[2])
final_manifest = Path(sys.argv[3])
relative_target = sys.argv[4]

prepare_stable_manifest_link(
    final_xcframework,
    final_manifest,
    relative_target,
)
publish(staged_xcframework, final_xcframework)
PY

echo "[+] Atomically published XCFramework and canonical manifest: $FINAL_XCFRAMEWORK" >&2
echo "[+] Public manifest link: $FINAL_MANIFEST -> $CANONICAL_MANIFEST_RELATIVE_TARGET" >&2
