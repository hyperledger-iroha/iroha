#!/usr/bin/env bash
set -euo pipefail

# Build a NoritoBridge.xcframework from the Rust connect_norito_bridge crate.
# - Produces a static XCFramework with umbrella header and modulemap.
# - Requires: rustup + cargo, xcodebuild, lipo.
#
# Usage:
#   scripts/build_norito_xcframework.sh
#
# Outputs into ./dist/NoritoBridge.xcframework

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)
CRATE_DIR="$ROOT_DIR/crates/connect_norito_bridge"
INC_DIR="$CRATE_DIR/include"
OUT_DIR="$ROOT_DIR/dist"
BUILD_DIR="$ROOT_DIR/build/norito_bridge"

LIB_CRATE_NAME="connect_norito_bridge"
FRAMEWORK_NAME="NoritoBridge"

: "${IPHONEOS_DEPLOYMENT_TARGET:=19.0}"
: "${IPHONESIMULATOR_DEPLOYMENT_TARGET:=19.0}"
export IPHONEOS_DEPLOYMENT_TARGET
export IPHONESIMULATOR_DEPLOYMENT_TARGET

echo "[+] Using iOS deployment target (device): $IPHONEOS_DEPLOYMENT_TARGET" >&2
echo "[+] Using iOS deployment target (simulator): $IPHONESIMULATOR_DEPLOYMENT_TARGET" >&2

DEVICE_TRIPLE="aarch64-apple-ios"
SIM_ARM_TRIPLE="aarch64-apple-ios-sim"
SIM_X64_TRIPLE="x86_64-apple-ios"
MAC_ARM_TRIPLE="aarch64-apple-darwin"

echo "[+] Building Rust static libraries (release)" >&2
echo "    Targets: $DEVICE_TRIPLE, $SIM_ARM_TRIPLE, $SIM_X64_TRIPLE, $MAC_ARM_TRIPLE" >&2

echo "    (Make sure you have installed targets via: rustup target add $DEVICE_TRIPLE $SIM_ARM_TRIPLE $SIM_X64_TRIPLE $MAC_ARM_TRIPLE)" >&2

cargo build -p "$LIB_CRATE_NAME" --release --target "$DEVICE_TRIPLE"
cargo build -p "$LIB_CRATE_NAME" --release --target "$SIM_ARM_TRIPLE"
cargo build -p "$LIB_CRATE_NAME" --release --target "$SIM_X64_TRIPLE"
cargo build -p "$LIB_CRATE_NAME" --release --target "$MAC_ARM_TRIPLE"

LIB_DEV="$ROOT_DIR/target/$DEVICE_TRIPLE/release/lib${LIB_CRATE_NAME}.a"
LIB_SIM_ARM="$ROOT_DIR/target/$SIM_ARM_TRIPLE/release/lib${LIB_CRATE_NAME}.a"
LIB_SIM_X64="$ROOT_DIR/target/$SIM_X64_TRIPLE/release/lib${LIB_CRATE_NAME}.a"
LIB_MAC_ARM="$ROOT_DIR/target/$MAC_ARM_TRIPLE/release/lib${LIB_CRATE_NAME}.a"
LIB_MAC_DYLIB="$ROOT_DIR/target/$MAC_ARM_TRIPLE/release/lib${LIB_CRATE_NAME}.dylib"

if [[ ! -f "$LIB_DEV" || ! -f "$LIB_SIM_ARM" || ! -f "$LIB_SIM_X64" || ! -f "$LIB_MAC_ARM" || ! -f "$LIB_MAC_DYLIB" ]]; then
  echo "[-] Missing built libraries. Did the cargo builds succeed?" >&2
  exit 1
fi

rm -rf "$BUILD_DIR" "$OUT_DIR/NoritoBridge.xcframework"
mkdir -p "$BUILD_DIR" "$OUT_DIR"

echo "[+] Creating simulator universal static library" >&2
SIM_UNI="$BUILD_DIR/${FRAMEWORK_NAME}-sim-universal.a"
lipo -create -output "$SIM_UNI" "$LIB_SIM_ARM" "$LIB_SIM_X64"

echo "[+] Staging Frameworks" >&2
FW_DEV_ROOT="$BUILD_DIR/device"
FW_SIM_ROOT="$BUILD_DIR/simulator"
FW_MAC_ROOT="$BUILD_DIR/macos"
FW_DEV="$FW_DEV_ROOT/${FRAMEWORK_NAME}.framework"
FW_SIM="$FW_SIM_ROOT/${FRAMEWORK_NAME}.framework"
FW_MAC="$FW_MAC_ROOT/${FRAMEWORK_NAME}.framework"

mkdir -p "$FW_DEV/Headers" "$FW_DEV/Modules" "$FW_SIM/Headers" "$FW_SIM/Modules" "$FW_MAC/Headers" "$FW_MAC/Modules"

# Copy static libs into framework roots (no extension inside framework)
cp "$LIB_DEV" "$FW_DEV/$FRAMEWORK_NAME"
cp "$SIM_UNI" "$FW_SIM/$FRAMEWORK_NAME"
cp "$LIB_MAC_DYLIB" "$FW_MAC/$FRAMEWORK_NAME"
install_name_tool -id "@rpath/${FRAMEWORK_NAME}.framework/${FRAMEWORK_NAME}" "$FW_MAC/$FRAMEWORK_NAME"

# Copy headers (umbrella + C header)
cp "$INC_DIR/connect_norito_bridge.h" "$FW_DEV/Headers/connect_norito_bridge.h"
cp "$INC_DIR/NoritoBridge.h" "$FW_DEV/Headers/NoritoBridge.h"
cp "$INC_DIR/connect_norito_bridge.h" "$FW_SIM/Headers/connect_norito_bridge.h"
cp "$INC_DIR/NoritoBridge.h" "$FW_SIM/Headers/NoritoBridge.h"
cp "$INC_DIR/connect_norito_bridge.h" "$FW_MAC/Headers/connect_norito_bridge.h"
cp "$INC_DIR/NoritoBridge.h" "$FW_MAC/Headers/NoritoBridge.h"

# Copy modulemap
cp "$CRATE_DIR/module.modulemap.template" "$FW_DEV/Modules/module.modulemap"
cp "$CRATE_DIR/module.modulemap.template" "$FW_SIM/Modules/module.modulemap"
cp "$CRATE_DIR/module.modulemap.template" "$FW_MAC/Modules/module.modulemap"

echo "[+] Creating XCFramework" >&2
xcodebuild -create-xcframework \
  -framework "$FW_DEV" \
  -framework "$FW_SIM" \
  -framework "$FW_MAC" \
  -output "$OUT_DIR/${FRAMEWORK_NAME}.xcframework"

echo "[+] XCFramework created: $OUT_DIR/${FRAMEWORK_NAME}.xcframework" >&2

BRIDGE_VERSION="${NORITO_BRIDGE_VERSION:-}"
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

MAC_BIN="$OUT_DIR/${FRAMEWORK_NAME}.xcframework/macos-arm64/${FRAMEWORK_NAME}.framework/${FRAMEWORK_NAME}"
IOS_BIN="$OUT_DIR/${FRAMEWORK_NAME}.xcframework/ios-arm64/${FRAMEWORK_NAME}.framework/${FRAMEWORK_NAME}"
SIM_BIN="$OUT_DIR/${FRAMEWORK_NAME}.xcframework/ios-arm64_x86_64-simulator/${FRAMEWORK_NAME}.framework/${FRAMEWORK_NAME}"
if [[ ! -f "$MAC_BIN" || ! -f "$IOS_BIN" || ! -f "$SIM_BIN" ]]; then
  echo "[-] Missing XCFramework binaries needed to emit NoritoBridge.artifacts.json" >&2
  exit 1
fi

MAC_HASH=$(shasum -a 256 "$MAC_BIN" | awk '{print $1}')
IOS_HASH=$(shasum -a 256 "$IOS_BIN" | awk '{print $1}')
SIM_HASH=$(shasum -a 256 "$SIM_BIN" | awk '{print $1}')

cat > "$OUT_DIR/NoritoBridge.artifacts.json" <<EOF
{
  "version": "$BRIDGE_VERSION",
  "hashes": {
    "macos-arm64": "$MAC_HASH",
    "ios-arm64": "$IOS_HASH",
    "ios-arm64_x86_64-simulator": "$SIM_HASH"
  }
}
EOF
echo "[+] Wrote artifact manifest: $OUT_DIR/NoritoBridge.artifacts.json" >&2
