#!/bin/sh
# Isolated physical-device fixture; never emits a production SDK artifact.
set -eu
fixture_root=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd -P)
iroha_root=$(CDPATH='' cd -- "$fixture_root/../../../.." && pwd -P)
cd "$iroha_root"
export IPHONEOS_DEPLOYMENT_TARGET=18.0
# blake3 suppresses cc-rs rerun metadata and watches only global CFLAGS. An
# explicit no-LTO setting (also enforced by blake3 itself) invalidates its older
# SDK-default C object while the target flags below supply the deployment floor.
export CFLAGS="${CFLAGS:+$CFLAGS }-fno-lto"
# cc-rs tracks target CFLAGS, while some dependencies do not track the deployment
# variable. Keep every native C/C++ object on the same floor in the warm lane.
export CFLAGS_aarch64_apple_ios="${CFLAGS_aarch64_apple_ios:+$CFLAGS_aarch64_apple_ios }-miphoneos-version-min=18.0"
export CXXFLAGS_aarch64_apple_ios="${CXXFLAGS_aarch64_apple_ios:+$CXXFLAGS_aarch64_apple_ios }-miphoneos-version-min=18.0"
exec cargo iroha-fast -- build --offline \
  --manifest-path "$fixture_root/Cargo.toml" \
  --target aarch64-apple-ios --release --target-dir "$fixture_root/target"
