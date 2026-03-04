#!/usr/bin/env bash
set -euo pipefail

# Build and (optionally) run Norito with aarch64 + simd acceleration.
#
# Usage:
#   scripts/build_norito_aarch64_simd.sh [linux|mac] [build|test|bench]
#
# Defaults: target=mac, action=build

TARGET_OS="${1:-mac}"
ACTION="${2:-build}"

case "$TARGET_OS" in
  mac)
    TARGET_TRIPLE="aarch64-apple-darwin"
    ;;
  linux)
    TARGET_TRIPLE="aarch64-unknown-linux-gnu"
    ;;
  *)
    echo "Unknown target OS: $TARGET_OS (expected 'mac' or 'linux')" >&2
    exit 1
    ;;
esac

echo "Ensuring rust target: $TARGET_TRIPLE"
rustup target add "$TARGET_TRIPLE" >/dev/null 2>&1 || true

FEATURES="json simd-accel"

case "$ACTION" in
  build)
    echo "Building norito for $TARGET_TRIPLE with features: $FEATURES"
    cargo build -p norito --features "$FEATURES" --target "$TARGET_TRIPLE"
    ;;
  test)
    echo "Testing norito for $TARGET_TRIPLE with features: $FEATURES"
    cargo test -p norito --features "$FEATURES" --target "$TARGET_TRIPLE" --no-run
    ;;
  bench)
    echo "Compiling benches for $TARGET_TRIPLE with features: $FEATURES"
    cargo bench -p norito --features "$FEATURES" --target "$TARGET_TRIPLE" --no-run
    ;;
  *)
    echo "Unknown action: $ACTION (expected 'build', 'test', or 'bench')" >&2
    exit 1
    ;;
esac

echo "Done. Artifacts under target/$TARGET_TRIPLE/"

