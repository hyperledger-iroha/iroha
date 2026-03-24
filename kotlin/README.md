# Iroha Kotlin SDK

Kotlin rewrite of the `iroha_android` and `norito_java` for Hyperledger Iroha 3.

## Artifacts

Not published to Maven Central yet. Build locally and consume via `mavenLocal()`.

| Artifact | Type | Description |
|----------|------|-------------|
| `org.hyperledger.iroha.sdk:core-jvm` | JAR | Pure Kotlin/JVM — models, codec, crypto, client, offline protocol |
| `org.hyperledger.iroha.sdk:client-android` | AAR | Android keystore, device telemetry, IrohaKeyManager |
| `org.hyperledger.iroha.sdk:offline-wallet-android` | AAR | Offline wallet: JNI natives, attestation (Play Integrity, SafetyDetect) |

### Consumer usage

```kotlin
// build.gradle.kts (consumer project)
repositories {
    mavenLocal()
}

// Pure JVM — business logic modules, JUnit tests, server-side
implementation("org.hyperledger.iroha.sdk:core-jvm:0.1-SNAPSHOT")

// Android wallet without offline payments
implementation("org.hyperledger.iroha.sdk:client-android:0.1-SNAPSHOT")

// Android wallet with offline payments (adds ~14MB arm64 native lib)
implementation("org.hyperledger.iroha.sdk:offline-wallet-android:0.1-SNAPSHOT")
```

---

## Build Instructions

### Prerequisites

| Tool | Version | Required For |
|------|---------|-------------|
| JDK | 21+ | All modules |
| Android SDK | compileSdk 35 | `client-android`, `offline-wallet-android` |
| Rust | 1.92+ | Native `.so` build |
| Android NDK | 28+ | Native `.so` build |
| `cargo-ndk` | any | Native `.so` build |

### Step 1: Build core-jvm and client-android

These modules have no native dependencies — they build immediately.

```bash
# Build and run tests
./gradlew :core-jvm:build :client-android:assembleRelease --quiet

# Run core-jvm unit tests
./gradlew :core-jvm:test --console=plain
```

### Step 2: Build native libraries (for offline-wallet-android)

The `libconnect_norito_bridge.so` files are **not tracked in git** — they are built from the Rust crate at `crates/connect_norito_bridge` in the same iroha repository. The Gradle task defaults to `../..` as the iroha root (override via `iroha.dir` in `local.properties` if needed).

**One-time setup:**

```bash
# Install Rust Android targets
rustup target add aarch64-linux-android x86_64-linux-android

# Install cargo-ndk
cargo install cargo-ndk

# Verify Android NDK
echo $ANDROID_NDK_HOME  # must point to NDK 28+
```

**Build the .so files:**

```bash
./gradlew :offline-wallet-android:buildNativeLibs
```

This Gradle task:
1. Reads `iroha.dir` from `local.properties`
2. Runs `cargo ndk` for `arm64-v8a` and `x86_64` targets
3. Copies `libconnect_norito_bridge.so` into `offline-wallet-android/src/main/jniLibs/`

First build takes ~5-10 minutes (compiles all Rust dependencies). Incremental builds are faster.

**Output:**

| ABI | File | Size |
|-----|------|-----:|
| arm64-v8a | `jniLibs/arm64-v8a/libconnect_norito_bridge.so` | ~14MB |
| x86_64 | `jniLibs/x86_64/libconnect_norito_bridge.so` | ~18MB |

> **Note:** `armeabi-v7a` (32-bit ARM) is not supported due to an upstream `rkyv` crate incompatibility with 32-bit targets.

### Step 3: Publish to local Maven

```bash
# Publish all three artifacts to ~/.m2/repository/
./gradlew publishToMavenLocal
```

This makes the artifacts available to any project on the same machine via `mavenLocal()`.

**Verify:**

```bash
ls ~/.m2/repository/org/hyperledger/iroha/sdk/core-jvm/0.1-SNAPSHOT/
ls ~/.m2/repository/org/hyperledger/iroha/sdk/client-android/0.1-SNAPSHOT/
ls ~/.m2/repository/org/hyperledger/iroha/sdk/offline-wallet-android/0.1-SNAPSHOT/
```

### Quick reference

```bash
# Full build from scratch (after local.properties is configured):
./gradlew :offline-wallet-android:buildNativeLibs  # ~5-10 min first time
./gradlew publishToMavenLocal                       # ~30 sec

# Rebuild only core-jvm (no native deps):
./gradlew :core-jvm:publishToMavenLocal

# Rebuild after Rust source changes:
./gradlew :offline-wallet-android:buildNativeLibs
./gradlew :offline-wallet-android:publishToMavenLocal
```

## Motivation

### Kotlin as the standard

Kotlin is the default language for Android development. Migrating from Java makes the SDK consistent with the Android ecosystem and eliminates the friction of Java/Kotlin interop at the call site. 

### Java 8 bytecode safety

Android libraries must target Java 8 bytecode. Java 11+ API calls (`String.isBlank()`, `List.of()`, `Files.readString()`) compile fine but crash at runtime on older Android devices. Kotlin's standard library provides equivalent functions that are safe across all API levels, eliminating this class of runtime failures.

### Reflection-free

The original Java SDK used reflection in multiple places (Android API discovery, BouncyCastle loading, keystore operations). This Kotlin rewrite eliminates reflection from `client-android` entirely and isolates the remaining optional-dependency probing in `core-jvm` behind try/catch fallbacks. 

### Modular architecture

The original SDK shipped as a single monolith. This rewrite splits it into three artifacts with clear boundaries:

- **`core-jvm`** — pure JVM, no Android framework dependency. Usable in Kotlin Multiplatform modules, JUnit tests without Robolectric, server-side tools, and admin panels. Contains all protocol logic: Norito codec, transaction building, client transport, offline journal, connect protocol.

- **`client-android`** — Android keystore integration, hardware-backed key generation, device telemetry. Depends on `core-jvm` via `api()` — consumers get all core types transitively.

- **`offline-wallet-android`** — extracted specifically to isolate the `libconnect_norito_bridge.so` native library (~14MB arm64). Consumers who don't need offline payments avoid this APK size impact entirely.

### Null safety

The Java SDK required defensive null checks at every Kotlin call site (`!!`, `?:`, `?.let {}`). Kotlin's type system makes nullability explicit — parameters that accept null are declared `T?`, everything else is guaranteed non-null by the compiler. This removes most `NullPointerException` risks from consumer apps. Some risk remains at Java interop boundaries (BouncyCastle, JCA) where platform types (`T!`) may hide nullability.

### Testability without Android

`core-jvm` runs on any JVM. Consumers can unit-test transaction building, address encoding, signing, and Norito serialization with plain JUnit — no Android instrumentation, no Robolectric, no emulator.

## Side Dependencies

| Dependency | Version | Used By | Risk |
|-----------|---------|---------|------|
| `org.bouncycastle:bcprov-jdk18on` | 1.78.1 | `core-jvm` (3 files: MultisigSeedHelper, ConnectCrypto, IdentifierReceiptVerifier) | **Binary compatibility** — BouncyCastle releases are not always backward-compatible. Consumer apps that bundle a different BC version may hit `NoSuchMethodError` at runtime. The SDK loads BC via reflection with try/catch fallback; core crypto (Blake2b/2s/3, Ed25519, IrohaHash) uses only JCA and does not require BC. |
| `com.github.luben:zstd-jni` | 1.5.7-7 | `core-jvm` (Norito compression) | **Native library** — zstd-jni bundles platform-specific `.so`/`.dylib`. On Android, the JNI natives may conflict with other zstd consumers. Compression is optional; the codec falls back gracefully if zstd is unavailable. |

