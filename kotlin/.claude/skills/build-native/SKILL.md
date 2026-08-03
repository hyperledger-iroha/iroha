---
name: build-native
description: Build connect_norito_bridge native .so files from Rust source. Use when user says "build native", "build .so", "rebuild native libs", "update native bridge", or "cargo ndk build".
---

# Build Native Libraries

Builds `libconnect_norito_bridge.so` for Android from the Rust crate at
`crates/connect_norito_bridge` in the Iroha repository. The authoritative
shipping bytes are the canonically stripped generated outputs consumed by the
`client-android` AAR, never a source-tree `jniLibs` directory.

## Prerequisites

- Exact Rust 1.93.1 with Android targets: `aarch64-linux-android`, `x86_64-linux-android`
- Exact `cargo-ndk` 4.1.2: `cargo install cargo-ndk --version 4.1.2 --locked`
- Python 3.12 and JDK 21
- `ANDROID_NDK_HOME` set to exact NDK 28.0.12674087-beta2 (r28-beta2)
- A pre-created canonical, writable, non-symbolic
  `MOBILE_SDK_ANDROID_ARTIFACT_DIR` outside the Iroha source tree

## Build with Gradle task

```bash
export MOBILE_SDK_ANDROID_ARTIFACT_DIR=/absolute/non-symlink/path/to/iroha-android-artifacts
mkdir -p "$MOBILE_SDK_ANDROID_ARTIFACT_DIR"

# Fail-closed default backend
./gradlew :client-android:buildNativeLibs -PprivacyProductionEnabled=false

# Real production privacy/Kagemusha backend
./gradlew :client-android:buildNativeLibs -PprivacyProductionEnabled=true
```

The property accepts only the exact strings `true` and `false`. The task reads
`iroha.dir` from `local.properties`, runs locked and offline `cargo ndk` with one
job, and redirects every Gradle/native output below
`$MOBILE_SDK_ANDROID_ARTIFACT_DIR/gradle-build/iroha_kotlin_sdk/`. Each mode has
its own `client-android/native/cargo-target/<mode>/` subtree there, so default
and production compiler state cannot mix. A separate typed task copies and
strips raw libraries into the external generated `jniLibs/<mode>/` subtree
without modifying them. It also generates the external
`generated/nativeProvenance/<mode>/iroha/native-build-provenance-v1.json`.
Each ABI is built through a transient cargo-ndk destination; only the exact
`libconnect_norito_bridge.so` file is promoted, so another workspace `cdylib`
cannot enter the raw inventory. The Android dependency-closure source seal is
verified after each ABI build and stripping. Its fingerprint is recorded in the
manifest together with both raw and stripped byte sizes and SHA-256 hashes; the
strip task fails if source or raw files change during the build.
AGP packages those outputs and embeds the same manifest at
`assets/iroha/native-build-provenance-v1.json` in the AAR.
Only release variants register those shipping outputs. Ordinary debug/JVM unit
tests remain Cargo-free; an unchanged raw artifact is reused only after its
saved source seal authenticates against the current checkout, and release
packaging still re-verifies the seal while regenerating stripped provenance.

The task resolves `cargo`, `rustc`, and `rustdoc` from exact Rust 1.93.1 and
rejects any mismatched identity. Every ABI build receives exactly
`CARGO_BUILD_JOBS=1`, `CARGO_INCREMENTAL=0`, `CARGO_NET_OFFLINE=true`, and
`RUSTC_BOOTSTRAP=1`, plus explicit canonical `RUSTC`, `RUSTDOC`, and
`CARGO_TARGET_DIR` values. Its Cargo portion is always:

```text
build --locked --offline --jobs 1 -Z unstable-options \
  --lockfile-path <canonical-iroha-root>/Cargo.lock
```

An alternate, relative, missing, or symbolic lockfile is rejected. There is no
Android compatibility environment variable for selecting another lockfile.

## Do not manually stage shipping libraries

Running `cargo ndk -o src/main/jniLibs` bypasses canonical stripping,
provenance, and AGP task dependencies. `src/main/jniLibs` is intentionally
excluded from every variant. Use the Gradle task above for any artifact that
will be tested, published, or shipped.

## Validate environment

```bash
cargo ndk --version
rustup run 1.93.1 rustc --version
python3.12 --version
echo $ANDROID_NDK_HOME
rustup target list --toolchain 1.93.1 --installed | grep android
```

If targets are missing:
```bash
rustup target add --toolchain 1.93.1 aarch64-linux-android x86_64-linux-android
```

**Target ABIs:**
- `arm64-v8a` — production devices (required)
- `x86_64` — emulators (required for development)
- `armeabi-v7a` — skip (upstream `rkyv` crate incompatible with 32-bit)

## Verify

```bash
mode=default # or production
build_root="$MOBILE_SDK_ANDROID_ARTIFACT_DIR/gradle-build/iroha_kotlin_sdk/client-android"
ls -lh "$build_root/generated/jniLibs/$mode/arm64-v8a/libconnect_norito_bridge.so"
ls -lh "$build_root/generated/jniLibs/$mode/x86_64/libconnect_norito_bridge.so"
test -f "$build_root/generated/nativeProvenance/$mode/iroha/native-build-provenance-v1.json"
unzip -p "$build_root/outputs/aar/client-android-release.aar" \
  assets/iroha/native-build-provenance-v1.json
```

From the Iroha repository root, authenticate the generated files, manifest,
dynamic symbols, and exact AAR bytes together with a separate external
source-seal target:

```bash
export CARGO_TARGET_DIR="$MOBILE_SDK_ANDROID_ARTIFACT_DIR/source-seal-cargo"
mkdir -p "$CARGO_TARGET_DIR"
export CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true
export RUSTC_BOOTSTRAP=1
export RUSTC="$(rustup which --toolchain 1.93.1 rustc)"
export RUSTDOC="$(rustup which --toolchain 1.93.1 rustdoc)"
scripts/check_mobile_sdk_artifacts.sh --android-only --require-built-android
```

## Known issues

- **armeabi-v7a fails** — `rkyv` crate has a `const` evaluation overflow on 32-bit targets. This is upstream; skip this ABI.
- **Long build time** — First build compiles all Rust dependencies (~5-10 min).
  The isolated target may reuse dependency artifacts, but compiler incremental
  state remains disabled.
- **Rust version** — Shipping native builds require exact rustc 1.93.1.
