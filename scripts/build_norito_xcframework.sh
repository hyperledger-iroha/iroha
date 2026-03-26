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
#
# Outputs into ./dist/NoritoBridge.xcframework

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)
CRATE_DIR="$ROOT_DIR/crates/connect_norito_bridge"
INC_DIR="$CRATE_DIR/include"
OUT_DIR="$ROOT_DIR/dist"
BUILD_DIR="$ROOT_DIR/build/norito_bridge"
STAGE_DIR="$BUILD_DIR/stage"

LIB_CRATE_NAME="connect_norito_bridge"
FRAMEWORK_NAME="NoritoBridge"
STATIC_LIB_NAME="libNoritoBridge.a"
FRAMEWORK_BUNDLE_ID="${FRAMEWORK_BUNDLE_ID:-org.hyperledger.iroha.NoritoBridge}"

: "${IPHONEOS_DEPLOYMENT_TARGET:=15.0}"
: "${IPHONESIMULATOR_DEPLOYMENT_TARGET:=15.0}"
: "${MACOSX_DEPLOYMENT_TARGET:=12.0}"
export IPHONESIMULATOR_DEPLOYMENT_TARGET
export MACOSX_DEPLOYMENT_TARGET
CARGO_BUILD_DIR="$BUILD_DIR/cargo-ios${IPHONEOS_DEPLOYMENT_TARGET//./_}-sim${IPHONESIMULATOR_DEPLOYMENT_TARGET//./_}"

BRIDGE_VERSION=""
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
    *)
      echo "[-] Unknown argument: $1" >&2
      echo "    Usage: $0 [--bridge-version <version>]" >&2
      exit 1
      ;;
  esac
  shift
done

echo "[+] Using iOS deployment target (device): $IPHONEOS_DEPLOYMENT_TARGET" >&2
echo "[+] Using iOS deployment target (simulator): $IPHONESIMULATOR_DEPLOYMENT_TARGET" >&2

rm -rf "$CARGO_BUILD_DIR" "$STAGE_DIR" "$OUT_DIR/NoritoBridge.xcframework"
mkdir -p "$STAGE_DIR" "$OUT_DIR"

DEVICE_TRIPLE="aarch64-apple-ios"
SIM_ARM_TRIPLE="aarch64-apple-ios-sim"
SIM_X64_TRIPLE="x86_64-apple-ios"
MACOS_TRIPLE="aarch64-apple-darwin"

echo "[+] Building Rust static libraries (release)" >&2
echo "    Targets: $DEVICE_TRIPLE, $SIM_ARM_TRIPLE, $SIM_X64_TRIPLE, $MACOS_TRIPLE" >&2

echo "    (Make sure you have installed targets via: rustup target add $DEVICE_TRIPLE $SIM_ARM_TRIPLE $SIM_X64_TRIPLE $MACOS_TRIPLE)" >&2

# Rust uses IPHONEOS_DEPLOYMENT_TARGET for both iOS device and simulator targets,
# while cc-based dependencies also honor IPHONESIMULATOR_DEPLOYMENT_TARGET.
env IPHONEOS_DEPLOYMENT_TARGET="$IPHONEOS_DEPLOYMENT_TARGET" \
  NORITO_SKIP_BINDINGS_SYNC=1 \
  CARGO_TARGET_DIR="$CARGO_BUILD_DIR" \
  cargo build -p "$LIB_CRATE_NAME" --release --target "$DEVICE_TRIPLE"
env IPHONEOS_DEPLOYMENT_TARGET="$IPHONESIMULATOR_DEPLOYMENT_TARGET" \
  IPHONESIMULATOR_DEPLOYMENT_TARGET="$IPHONESIMULATOR_DEPLOYMENT_TARGET" \
  NORITO_SKIP_BINDINGS_SYNC=1 \
  CARGO_TARGET_DIR="$CARGO_BUILD_DIR" \
  cargo build -p "$LIB_CRATE_NAME" --release --target "$SIM_ARM_TRIPLE"
env IPHONEOS_DEPLOYMENT_TARGET="$IPHONESIMULATOR_DEPLOYMENT_TARGET" \
  IPHONESIMULATOR_DEPLOYMENT_TARGET="$IPHONESIMULATOR_DEPLOYMENT_TARGET" \
  NORITO_SKIP_BINDINGS_SYNC=1 \
  CARGO_TARGET_DIR="$CARGO_BUILD_DIR" \
  cargo build -p "$LIB_CRATE_NAME" --release --target "$SIM_X64_TRIPLE"
env MACOSX_DEPLOYMENT_TARGET="$MACOSX_DEPLOYMENT_TARGET" \
  NORITO_SKIP_BINDINGS_SYNC=1 \
  CARGO_TARGET_DIR="$CARGO_BUILD_DIR" \
  cargo build -p "$LIB_CRATE_NAME" --release --target "$MACOS_TRIPLE"

LIB_DEV="$CARGO_BUILD_DIR/$DEVICE_TRIPLE/release/lib${LIB_CRATE_NAME}.a"
LIB_SIM_ARM="$CARGO_BUILD_DIR/$SIM_ARM_TRIPLE/release/lib${LIB_CRATE_NAME}.a"
LIB_SIM_X64="$CARGO_BUILD_DIR/$SIM_X64_TRIPLE/release/lib${LIB_CRATE_NAME}.a"
LIB_MAC="$CARGO_BUILD_DIR/$MACOS_TRIPLE/release/lib${LIB_CRATE_NAME}.a"

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

# Stage iOS static libraries.
cp "$LIB_DEV" "$LIB_DEV_STAGED"
cp "$SIM_UNI" "$LIB_SIM_STAGED"
cp "$LIB_MAC" "$LIB_MAC_STAGED"

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

echo "[+] Creating XCFramework" >&2
xcodebuild -create-xcframework \
  -library "$LIB_DEV_STAGED" -headers "$HEADERS_DEV" \
  -library "$LIB_SIM_STAGED" -headers "$HEADERS_SIM" \
  -library "$LIB_MAC_STAGED" -headers "$HEADERS_MAC" \
  -output "$OUT_DIR/${FRAMEWORK_NAME}.xcframework"

echo "[+] XCFramework created: $OUT_DIR/${FRAMEWORK_NAME}.xcframework" >&2

IOS_BIN="$OUT_DIR/${FRAMEWORK_NAME}.xcframework/ios-arm64/${STATIC_LIB_NAME}"
SIM_BIN="$OUT_DIR/${FRAMEWORK_NAME}.xcframework/ios-arm64_x86_64-simulator/${STATIC_LIB_NAME}"
MAC_BIN="$OUT_DIR/${FRAMEWORK_NAME}.xcframework/macos-arm64/${STATIC_LIB_NAME}"
if [[ ! -f "$IOS_BIN" || ! -f "$SIM_BIN" || ! -f "$MAC_BIN" ]]; then
  echo "[-] Missing XCFramework binaries needed to emit NoritoBridge.artifacts.json" >&2
  exit 1
fi

IOS_HASH=$(shasum -a 256 "$IOS_BIN" | awk '{print $1}')
SIM_HASH=$(shasum -a 256 "$SIM_BIN" | awk '{print $1}')
MAC_HASH=$(shasum -a 256 "$MAC_BIN" | awk '{print $1}')

cat > "$OUT_DIR/NoritoBridge.artifacts.json" <<EOF
{
  "version": "$BRIDGE_VERSION",
  "hashes": {
    "ios-arm64": "$IOS_HASH",
    "ios-arm64_x86_64-simulator": "$SIM_HASH",
    "macos-arm64": "$MAC_HASH"
  }
}
EOF
echo "[+] Wrote artifact manifest: $OUT_DIR/NoritoBridge.artifacts.json" >&2
