# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Build all modules
./gradlew build --quiet

# Run all tests
./gradlew :core-jvm:test --console=plain

# Build a specific module
./gradlew :core-jvm:build --quiet
./gradlew :client-android:assembleRelease --quiet

# Build generated native .so files and provenance outside the source tree
export MOBILE_SDK_ANDROID_ARTIFACT_DIR=/absolute/non-symlink/path/to/android-artifacts
mkdir -p "$MOBILE_SDK_ANDROID_ARTIFACT_DIR"
./gradlew :client-android:buildNativeLibs

# Build the production-featured generated native artifacts
./gradlew :client-android:buildNativeLibs -PprivacyProductionEnabled=true

# Publish to local Maven
./gradlew publishToMavenLocal
```

## Architecture

Three published SDK modules and a separate JVM `tools` application (`iroha_kotlin_sdk`) serve Kotlin and Java consumers.

### Module: `core-jvm` (JAR, pure Kotlin/JVM)
All protocol logic — no Android dependencies. HTTP/SSE uses the Kotlin-owned
OkHttp adapter, with scoped client lifetimes and borrowed injected backends.
WebSockets use the injected Kotlin-owned Netty engine with explicit NIO/JDK TLS
resources, bounded complete messages and one upgrade attempt. No engine is
selected through platform discovery or a process-global client:
- **`sdk.norito`** — Norito binary codec (TypeAdapter, NoritoCodec, NoritoEncoder/Decoder, compression)
- **`sdk.core.model`** — transaction models, 84 instruction types, InstructionBox, Executable (sealed class)
- **`sdk.crypto`** — Blake2b/2s/3, Ed25519, IrohaHash, and Argon2id key export (JCA + direct BouncyCastle dependency)
- **`sdk.gpu`** — one bounded batch CUDA API for both JVM languages, with explicit backend injection/native loading; device qualification runs through `cudaHardwareTest`, separately from host tests
- **`sdk.crypto.keystore.attestation`**, **`KeyAttestation`** — Android-free evidence records, certificate verification and governed revocation policy shared with Android clients and offline JVM tooling
- **`sdk.address`** — account/asset address encoding (IH58, Bech32M)
- **`sdk.tx`** — transaction building, signing, offline envelopes, norito adapters
- **`sdk.client`** — Torii HTTP/WS/SSE client, JSON, transport, queue
- **`sdk.offline`** — aggregate-balance KAGEMUSHA V1 models, canonical
  `kgm1:` peer transports, and hardware lifecycle binding contracts
- **`sdk.connect`** — connect protocol (BouncyCastle)
- **`sdk.telemetry`** — telemetry sink, options, providers
- **`sdk.multisig`**, **`sdk.subscriptions`**, **`sdk.sorafs`**, **`sdk.nexus`** — feature packages

### Module: `client-android` (AAR, depends on `core-jvm`)
Android-specific additions:
- **`sdk.crypto.keystore`** — Android Keystore provisioning, provider dispatch and platform integration; pure evidence verification belongs to `core-jvm`
- **`sdk.telemetry`** — AndroidDeviceProfileProvider, AndroidNetworkContextProvider
- **`sdk.IrohaKeyManager`** — key provider orchestrator

### Module: `kagemusha-wallet-android` (AAR)
KAGEMUSHA wallet orchestration and Android/JNI device lifecycle integration live here.
Keep pure wire and cryptographic contracts in `core-jvm` and general Android client integration
in `client-android`. Device qualification remains separate from JVM/native unit tests.

### Module: `tools` (JVM application)

`iroha-attestation` verifies collected evidence through the `core-jvm` verifier.
It owns command parsing, bounded certificate/ZIP input and atomic JSON output.
It has no Android dependency or Maven SDK publication. Trust roots, expected
challenge/SPKI, governed snapshot commitment and evaluation time are supplied
independently of the evidence bundle. Run `:tools:test :tools:installDist`.

## JDK Compatibility

All modules enforce **JDK 8 API compatibility at compile time** via `-Xjdk-release=8` in `freeCompilerArgs`. The compiler uses JDK 21 (`jvmToolchain(21)`) but restricts the available API surface to JDK 8. Using any JDK 9+ API (e.g. `Optional.isEmpty()`, `BigInteger.TWO`, `URLEncoder.encode(String, Charset)`, `Arrays.compareUnsigned()`) will cause a compilation error. Use Kotlin stdlib equivalents or JDK 8 overloads instead.

**Do not remove `-Xjdk-release=8` from any module's `freeCompilerArgs`.** This flag is the only compile-time guard against JDK 9+ API usage. Without it, incompatible calls compile silently and crash at runtime on Android.

## API Design Rules

### No data classes in public API
**Never use `data class` for public library API types.** Use regular immutable classes with explicit construction invariants, defensive copying and intentional `equals`/`hashCode` semantics. Avoid automatic copying or rendering of sensitive protocol state. This is the first release: remove superseded interfaces and migrate callers without compatibility aliases or wrappers.

### No Reflection
**Never use `java.lang.reflect.*` in this SDK.** This library is consumed by Android apps that use R8/D8 shrinking. Reflection forces consumers to add keep rules for every reflected class/method, complicating ProGuard/R8 configuration. Use unchecked casts, explicit type checks, or Kotlin type system features instead.

### Defensive Copying
All mutable collections and byte arrays are copied on construction and access. Use `toList()`, `toMap()`, `copyOf()` patterns.

### Java Interop
- `@JvmStatic` on companion object factory methods
- `@JvmField` on public properties where Java callers need field-style access
- No `Optional` in Kotlin API — use nullable types (`T?`)
- Authenticated Torii operations use `RequestSigner`; private keys stay with the application or its explicit software signer.

## Key Patterns
- **Two instruction representations**: typed (structured fields) vs wire (opaque `ByteArray` + wire name); `InstructionBox` unifies both
- **Source layout**: main sources under `src/main/java/` (Kotlin files, retained path from Java migration), tests under `src/test/kotlin/`
- **Native libraries**: `.so` files built from Rust via
  `./gradlew :client-android:buildNativeLibs`, not tracked in git. Raw cargo-ndk
  output is isolated below
  `$MOBILE_SDK_ANDROID_ARTIFACT_DIR/gradle-build/iroha_kotlin_sdk/client-android/native/cargo-ndk/<mode>/`;
  compiler state is separately isolated in the sibling
  `native/cargo-target/<mode>/` via `CARGO_TARGET_DIR`; canonically stripped
  authoritative bytes and provenance live in the sibling `generated/` tree.
  Cargo-ndk output first lands in a
  transient per-ABI staging directory so unrelated workspace `cdylib` outputs
  cannot enter the exact raw inventory. The build verifies its race-stable
  Android source seal after every ABI and immediately before and after each raw
  or stripped/provenance promotion, and binds the dependency-closure fingerprint
  into provenance. AGP packages those generated outputs and explicitly excludes
  `src/main/jniLibs`. Every ABI uses canonical Rust 1.93.1 `cargo`, `rustc`, and
  `rustdoc`, plus `CARGO_BUILD_JOBS=1`, `CARGO_INCREMENTAL=0`,
  `CARGO_NET_OFFLINE=true`, and `RUSTC_BOOTSTRAP=1`. The Cargo command always
  includes `--locked --offline --jobs 1 -Z unstable-options --lockfile-path`
  with the canonical repository-root `Cargo.lock`; alternate locks and legacy
  overrides are rejected.

## Testing

JUnit 5 with `@ParameterizedTest` / `@MethodSource` for data-driven tests. Test companion objects provide argument lists via `@JvmStatic` methods.

## Version Catalog

Dependencies managed in `gradle/libs.versions.toml`: Kotlin 2.3.10, AGP 9.0.1, JUnit 5.11.4, BouncyCastle 1.78.1, zstd-jni 1.5.7-7, OkHttp 4.12.0, Netty 4.2.17.Final.
