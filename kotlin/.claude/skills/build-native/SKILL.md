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

- Rust toolchain (1.92+) with Android targets: `aarch64-linux-android`, `x86_64-linux-android`
- `cargo-ndk` installed: `cargo install cargo-ndk`
- `ANDROID_NDK_HOME` environment variable set (NDK 28+)

## Build with Gradle task

```bash
# Fail-closed default backend
./gradlew :client-android:buildNativeLibs -PprivacyProductionEnabled=false

# Real production privacy/Kagemusha backend
./gradlew :client-android:buildNativeLibs -PprivacyProductionEnabled=true
```

The property accepts only the exact strings `true` and `false`. The task reads
`iroha.dir` from `local.properties`, runs locked `cargo ndk`, and places raw
libraries under `client-android/build/native/cargo-ndk/<mode>/`. Each mode also
uses its own `client-android/build/native/cargo-target/<mode>/` as
`CARGO_TARGET_DIR`, so default and production compiler state cannot mix. A separate
typed task copies and strips them into
`client-android/build/generated/jniLibs/<mode>/` without modifying the raw
files. It also generates
`client-android/build/generated/nativeProvenance/<mode>/iroha/native-build-provenance-v1.json`.
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

## Do not manually stage shipping libraries

Running `cargo ndk -o src/main/jniLibs` bypasses canonical stripping,
provenance, and AGP task dependencies. `src/main/jniLibs` is intentionally
excluded from every variant. Use the Gradle task above for any artifact that
will be tested, published, or shipped.

## Validate environment

```bash
which cargo-ndk || cargo install cargo-ndk
echo $ANDROID_NDK_HOME
rustup target list --installed | grep android
```

If targets are missing:
```bash
rustup target add aarch64-linux-android x86_64-linux-android
```

**Target ABIs:**
- `arm64-v8a` — production devices (required)
- `x86_64` — emulators (required for development)
- `armeabi-v7a` — skip (upstream `rkyv` crate incompatible with 32-bit)

## Verify

```bash
mode=default # or production
ls -lh client-android/build/generated/jniLibs/$mode/arm64-v8a/libconnect_norito_bridge.so
ls -lh client-android/build/generated/jniLibs/$mode/x86_64/libconnect_norito_bridge.so
test -f client-android/build/generated/nativeProvenance/$mode/iroha/native-build-provenance-v1.json
unzip -p client-android/build/outputs/aar/client-android-release.aar \
  assets/iroha/native-build-provenance-v1.json
```

Run `../scripts/check_mobile_sdk_artifacts.sh --android-only
--require-built-android` from the Iroha repository root to authenticate the
generated files, manifest, dynamic symbols, and exact AAR bytes together.

## Known issues

- **armeabi-v7a fails** — `rkyv` crate has a `const` evaluation overflow on 32-bit targets. This is upstream; skip this ABI.
- **Long build time** — First build compiles all Rust dependencies (~5-10 min). Incremental builds are faster.
- **Rust version** — Project requires rustc 1.92+. Check with `rustc --version`.
