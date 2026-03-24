---
name: build-native
description: Build connect_norito_bridge native .so files from Rust source. Use when user says "build native", "build .so", "rebuild native libs", "update native bridge", or "cargo ndk build".
---

# Build Native Libraries

Builds `libconnect_norito_bridge.so` for Android from the Rust crate at `crates/connect_norito_bridge` in the iroha repository. The Kotlin SDK lives next to the `java/` directory in the same repo, so the Rust source is at `../../crates/connect_norito_bridge` relative to this project.

## Prerequisites

- Rust toolchain (1.92+) with Android targets: `aarch64-linux-android`, `x86_64-linux-android`
- `cargo-ndk` installed: `cargo install cargo-ndk`
- `ANDROID_NDK_HOME` environment variable set (NDK 28+)

## Build with Gradle task

```bash
./gradlew :offline-wallet-android:buildNativeLibs
```

The task reads `iroha.dir` from `local.properties` and runs `cargo ndk`. Output goes to `offline-wallet-android/src/main/jniLibs/`.

## Manual build

### 1. Validate environment

```bash
which cargo-ndk || cargo install cargo-ndk
echo $ANDROID_NDK_HOME
rustup target list --installed | grep android
```

If targets are missing:
```bash
rustup target add aarch64-linux-android x86_64-linux-android
```

### 2. Build

From the iroha repository root:

```bash
cargo ndk -t arm64-v8a -t x86_64 \
  -o <kotlin-sdk-dir>/offline-wallet-android/src/main/jniLibs \
  build --release -p connect_norito_bridge
```

**Target ABIs:**
- `arm64-v8a` — production devices (required)
- `x86_64` — emulators (required for development)
- `armeabi-v7a` — skip (upstream `rkyv` crate incompatible with 32-bit)

### 3. Verify

```bash
ls -lh offline-wallet-android/src/main/jniLibs/arm64-v8a/libconnect_norito_bridge.so  # ~14MB
ls -lh offline-wallet-android/src/main/jniLibs/x86_64/libconnect_norito_bridge.so      # ~18MB
```

## Known issues

- **armeabi-v7a fails** — `rkyv` crate has a `const` evaluation overflow on 32-bit targets. This is upstream; skip this ABI.
- **Long build time** — First build compiles all Rust dependencies (~5-10 min). Incremental builds are faster.
- **Rust version** — Project requires rustc 1.92+. Check with `rustc --version`.
