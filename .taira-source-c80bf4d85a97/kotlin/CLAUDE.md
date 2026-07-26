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

# Build the default generated native .so files and provenance from Rust source
./gradlew :client-android:buildNativeLibs

# Build the production-featured generated native artifacts
./gradlew :client-android:buildNativeLibs -PprivacyProductionEnabled=true

# Publish to local Maven
./gradlew publishToMavenLocal
```

## Architecture

Two-module Kotlin SDK (`iroha_kotlin_sdk`) for the Hyperledger Iroha SDK.

### Module: `core-jvm` (JAR, pure Kotlin/JVM)
All protocol logic — no Android dependencies:
- **`sdk.norito`** — Norito binary codec (TypeAdapter, NoritoCodec, NoritoEncoder/Decoder, compression)
- **`sdk.core.model`** — transaction models, 84 instruction types, InstructionBox, Executable (sealed class)
- **`sdk.crypto`** — Blake2b/2s/3, Ed25519, IrohaHash, and Argon2id key export (JCA + direct BouncyCastle dependency)
- **`sdk.address`** — account/asset address encoding (IH58, Bech32M)
- **`sdk.tx`** — transaction building, signing, offline envelopes, norito adapters
- **`sdk.client`** — Torii HTTP/WS/SSE client, JSON, transport, queue
- **`sdk.offline`** — ABI-21/V4 typed Kagemusha lifecycle, exact scaled
  amounts, peer transports, exact eight-role artifact streaming with inline
  circuit parameters, and fail-closed exact-release readiness projection
  (independent backend/lineage facts, with `ready` true only for an empty
  blocker set)
- **`sdk.connect`** — connect protocol (BouncyCastle)
- **`sdk.telemetry`** — telemetry sink, options, providers
- **`sdk.multisig`**, **`sdk.subscriptions`**, **`sdk.sorafs`**, **`sdk.nexus`** — feature packages

### Module: `client-android` (AAR, depends on `core-jvm`)
Android-specific additions:
- **`sdk.crypto.keystore`** — Android Keystore, attestation (AttestationVerifier, AttestationResult)
- **`sdk.telemetry`** — AndroidDeviceProfileProvider, AndroidNetworkContextProvider
- **`sdk.IrohaKeyManager`** — key provider orchestrator

## JDK Compatibility

All modules enforce **JDK 8 API compatibility at compile time** via `-Xjdk-release=8` in `freeCompilerArgs`. The compiler uses JDK 21 (`jvmToolchain(21)`) but restricts the available API surface to JDK 8. Using any JDK 9+ API (e.g. `Optional.isEmpty()`, `BigInteger.TWO`, `URLEncoder.encode(String, Charset)`, `Arrays.compareUnsigned()`) will cause a compilation error. Use Kotlin stdlib equivalents or JDK 8 overloads instead.

**Do not remove `-Xjdk-release=8` from any module's `freeCompilerArgs`.** This flag is the only compile-time guard against JDK 9+ API usage. Without it, incompatible calls compile silently and crash at runtime on Android.

## API Design Rules

### No data classes in public API
**Never use `data class` for public library API types.** Data classes expose `copy()`, `componentN()`, and `toString()` as part of the binary contract. Adding, removing, or reordering properties breaks backward compatibility. Use regular classes with explicit `equals`/`hashCode` instead. See: https://kotlinlang.org/docs/api-guidelines-backward-compatibility.html

### No Reflection
**Never use `java.lang.reflect.*` in this SDK.** This library is consumed by Android apps that use R8/D8 shrinking. Reflection forces consumers to add keep rules for every reflected class/method, complicating ProGuard/R8 configuration. Use unchecked casts, explicit type checks, or Kotlin type system features instead.

### Defensive Copying
All mutable collections and byte arrays are copied on construction and access. Use `toList()`, `toMap()`, `copyOf()` patterns.

### Java Interop
- `@JvmStatic` on companion object factory methods
- `@JvmField` on public properties where Java callers need field-style access
- No `Optional` in Kotlin API — use nullable types (`T?`)

## Key Patterns
- **Two instruction representations**: typed (structured fields) vs wire (opaque `ByteArray` + wire name); `InstructionBox` unifies both
- **Source layout**: main sources under `src/main/java/` (Kotlin files, retained path from Java migration), tests under `src/test/kotlin/`
- **Native libraries**: `.so` files built from Rust via
  `./gradlew :client-android:buildNativeLibs`, not tracked in git. Raw cargo-ndk
  output is isolated under `client-android/build/native/cargo-ndk/<mode>/`;
  compiler state is separately isolated under
  `client-android/build/native/cargo-target/<mode>/` via `CARGO_TARGET_DIR`;
  canonically stripped authoritative bytes live under
  `client-android/build/generated/jniLibs/<mode>/`, with matching provenance in
  `build/generated/nativeProvenance/<mode>/`. Cargo-ndk output first lands in a
  transient per-ABI staging directory so unrelated workspace `cdylib` outputs
  cannot enter the exact raw inventory. The build verifies its race-stable
  Android source seal after every ABI and immediately before and after each raw
  or stripped/provenance promotion, and binds the dependency-closure fingerprint
  into provenance. AGP packages those generated outputs and explicitly excludes
  `src/main/jniLibs`.

## Testing

JUnit 5 with `@ParameterizedTest` / `@MethodSource` for data-driven tests. Test companion objects provide argument lists via `@JvmStatic` methods.

## Version Catalog

Dependencies managed in `gradle/libs.versions.toml`: Kotlin 2.3.10, AGP 9.0.1, JUnit 5.11.4, BouncyCastle 1.78.1, zstd-jni 1.5.7-7.
