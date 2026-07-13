# Iroha Kotlin SDK

Kotlin rewrite of the `iroha_android` and `norito_java` for Hyperledger Iroha 3.

## Artifacts

Not published to Maven Central yet. Build locally and consume via `mavenLocal()`.

| Artifact | Type | Description |
|----------|------|-------------|
| `org.hyperledger.iroha.sdk:core-jvm` | JAR | Pure Kotlin/JVM models, codec, crypto, clients, and ABI-19 artifact streaming |
| `org.hyperledger.iroha.sdk:client-android` | AAR | Android keystore, device telemetry, IrohaKeyManager, shared JNI bridge for ML-DSA / offline flows |

### Consumer usage

```kotlin
// build.gradle.kts (consumer project)
repositories {
    mavenLocal()
}

// Pure JVM — business logic modules, JUnit tests, server-side
implementation("org.hyperledger.iroha.sdk:core-jvm:0.1.0")

// Android wallet without offline payments
implementation("org.hyperledger.iroha.sdk:client-android:0.1.0")

// Android wallet with offline payments
implementation("org.hyperledger.iroha.sdk:offline-wallet-android:0.1.0")
```

### Torii server-sent events

`HttpClientTransport.newEventStreamClient()` opens SSE feeds with the same base
URI, authentication headers, and observers as the HTTP client. The canonical
`/v1/events/sse` and `/v1/contracts/events/sse` feeds are live-only and have no
replay log. `ToriiEventStreamClient` therefore rejects every case variant of
`Last-Event-ID` before dispatch for exactly those two paths; custom streams that
provide replay may still receive the header through `ToriiEventStreamOptions`.

Raw listeners receive terminal `event: stream_error` frames. Call
`ServerSentEvent.terminalStreamError()` before application-event projection to
obtain a strict `ToriiStreamException` containing the stable code, server
message, optional unsigned dropped-message count, replay flag, and raw JSON.
Malformed or schema-expanded terminal envelopes fail closed as
`ToriiStreamProtocolException`; they must not be filtered as unrelated events.
A reconnect to either canonical feed starts a new live subscription and can
have a gap.

### Kagemusha proof artifacts

`core-jvm` exposes the exact ABI-19 typed Kagemusha init, fractional append/change, verification,
and redemption builders through the fixed native surface. It also provides exact scaled amounts,
Kagemusha V3 artifact streaming and backend-capability checks, plus the sole current
`DeviceAttestationRegistration` / `RegisterOfflineDeviceAttestation` transaction path. The latter
validates finalized platform material and emits exactly one native registration instruction.
Artifact streaming installs
exactly six Pasta artifacts atomically: transition and state parameters, proving keys, and verifying
keys. The top-up-finality roster is authenticated release metadata, not a seventh proof-key stream.

Lifecycle calls fail closed until the proof backend and the exact manifest-bound artifact set are
available. Request and result archives stay typed and canonically framed while recursive proof,
membership, note-opening, and accumulator details remain native-owned opaque bytes.
The protocol and JVM append builder accept one or two inputs and support up to eight peer hops.
Inputs are canonicalized by authenticated bundle digest; duplicate or conflicting exact-state
branches fail closed. `projectReadiness` supplies the
authoritative scale, committed height/hash, and role-specific verifier commitments/windows.
`prepareTopUp` accepts only Torii's authoritative `next_zero_path` and retains the local note
opening. After top-up finality, `projectInitResult` persists the recursive init result's own
membership witness rather than the earlier shield-tree witness. Persisted openings and submission
archives are restored with typed decoders so idempotent retries reuse exact canonical bytes.
Secret-bearing append and redeem requests are single-use and zeroized after native consumption.
Each projected branch carries its complete ordered exact-state claim set and authenticated V3
artifact binding. Native `conflictsWith` compares every claim pair, rejecting equality and
ancestor/descendant overlap while allowing the two consistent sibling outputs from one split;
wallet code never parses lineage paths.

---

## Build Instructions

### Prerequisites

| Tool | Version | Required For |
|------|---------|-------------|
| JDK | 21+ | All modules |
| Android SDK | compileSdk 35 | `client-android` |
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

### Step 2: Build native libraries (for `client-android`)

The `libconnect_norito_bridge.so` files are **not tracked in git** — they are built from the Rust crate at `crates/connect_norito_bridge` in the same iroha repository. The Gradle task now lives on `client-android`, which owns the shared native bridge used for ML-DSA signing and the typed Kagemusha lifecycle/artifact streaming. It defaults to `../..` as the iroha root (override via `iroha.dir` in `local.properties` if needed).

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
# Default developer build: privacy proof builders stay fail-closed.
./gradlew :client-android:buildNativeLibs

# Production-gated build: only use after the privacy production gate evidence is complete.
./gradlew :client-android:buildNativeLibs -PprivacyProductionEnabled=true
```

This Gradle task:
1. Reads `iroha.dir` from `local.properties`
2. Runs `cargo ndk` for `arm64-v8a` and `x86_64` targets
3. Copies `libconnect_norito_bridge.so` into `client-android/src/main/jniLibs/`

The production-gated form passes `--features privacy-production-enabled` to
`connect_norito_bridge`; the default form intentionally omits that feature so
unaudited native proving remains disabled.

First build takes ~5-10 minutes (compiles all Rust dependencies). Incremental builds are faster.

**Output:**

| ABI | File | Size |
|-----|------|-----:|
| arm64-v8a | `client-android/src/main/jniLibs/arm64-v8a/libconnect_norito_bridge.so` | ~14MB |
| x86_64 | `client-android/src/main/jniLibs/x86_64/libconnect_norito_bridge.so` | ~18MB |

> **Note:** `armeabi-v7a` (32-bit ARM) is not supported due to an upstream `rkyv` crate incompatibility with 32-bit targets.

### Step 3: Publish to local Maven

```bash
# Publish all three artifacts to ~/.m2/repository/
./gradlew publishToMavenLocal
```

This makes the artifacts available to any project on the same machine via `mavenLocal()`.

**Verify:**

```bash
ls ~/.m2/repository/org/hyperledger/iroha/sdk/core-jvm/0.1.0/
ls ~/.m2/repository/org/hyperledger/iroha/sdk/client-android/0.1.0/
ls ~/.m2/repository/org/hyperledger/iroha/sdk/offline-wallet-android/0.1.0/
```

### Quick reference

```bash
# Full build from scratch (after local.properties is configured):
./gradlew :client-android:buildNativeLibs          # ~5-10 min first time
./gradlew publishToMavenLocal                       # ~30 sec

# Rebuild only core-jvm (no native deps):
./gradlew :core-jvm:publishToMavenLocal

# Rebuild after Rust source changes:
./gradlew :client-android:buildNativeLibs
./gradlew :client-android:publishToMavenLocal
```

## Push Device Registration

`core-jvm` includes thin Torii helpers for `/v1/notify/devices`. Android apps still obtain FCM tokens from their app layer; the SDK only encodes the signed Torii request:

```kotlin
val request = PushDeviceRequest(accountId, "FCM", fcmToken, listOf("activity"))
transport.registerPushDevice(request, canonicalAuth).join()
transport.unregisterPushDevice(request, canonicalAuth).join()
```

## Verifying Key Registry

`core-jvm` exposes Torii helpers for `/v1/zk/vk/register` and
`/v1/zk/vk/update`. They validate production verifier backends, required
signing fields, height ranges, and inline verifier-key commitments before
sending the request:

```kotlin
val vkBytes = byteArrayOf(1, 2, 3)

transport.registerVerifyingKey(
    VerifyingKeyRegisterRequest(
        authority = "alice",
        privateKey = "ed25519:...",
        backend = "halo2/ipa",
        name = "vk_main",
        version = 1,
        circuitId = "halo2/ipa::transfer_v1",
        publicInputsSchemaHashHex = "a".repeat(64),
        gasScheduleId = "halo2_default",
        verifyingKeyBytes = vkBytes,
        status = "Active",
    )
).join()

transport.updateVerifyingKey(
    VerifyingKeyUpdateRequest(
        authority = "alice",
        privateKey = "ed25519:...",
        backend = "halo2/ipa",
        name = "vk_main",
        version = 2,
        circuitId = "halo2/ipa::transfer_v1",
        publicInputsSchemaHashHex = "a".repeat(64),
        status = "Withdrawn",
    )
).join()
```

## Signing Algorithm Selection

Android apps can now choose the transaction signing
algorithm explicitly:

```kotlin
import org.hyperledger.iroha.sdk.IrohaKeyManager
import org.hyperledger.iroha.sdk.crypto.SigningAlgorithm
import org.hyperledger.iroha.sdk.crypto.keystore.KeyGenParameters

val ed25519Manager = IrohaKeyManager.withSoftwareProvider()
val mlDsaManager = IrohaKeyManager.withSoftwareProvider(SigningAlgorithm.ML_DSA)
val gostManager = IrohaKeyManager.withSoftwareProvider(SigningAlgorithm.GOST_2012_256_A)

val tunedManager = IrohaKeyManager.withDefaultProviders(
    KeyGenParameters.Builder()
        .setSigningAlgorithm(SigningAlgorithm.ML_DSA)
        .build()
)
```

`ED25519` remains the default. `SECP256K1`, `BLS_NORMAL`, `BLS_SMALL`,
`ML_DSA`, the five `GOST_2012_*` variants, and `SM2` use the shared native
bridge and are software-only in this SDK pass, so hardware/StrongBox
preferences fail fast instead of silently downgrading.

## Resolving Account Aliases

`HttpClientTransport.resolveAccountAlias` posts to Torii's `/v1/aliases/resolve`
endpoint and returns the mapped account id. `AccountAliasResolution.index` is
optional and may be absent for backends that do not expose a deterministic
alias index. Unknown aliases surface as `Optional.empty()` without throwing:

```kotlin
val client = HttpClientTransport.createDefault(config)

val resolved = client.resolveAccountAlias("some_alias@universal").join()
if (resolved.isPresent) {
    val record = resolved.get()
    println("account_id=${record.accountId} source=${record.source}")
} else {
    println("alias not found")
}
```

## Reading Kotodama Manifests

`HttpClientTransport.getContractManifest(codeHash)` reads
`/v1/contracts/code/{code_hash}` into the complete Kotodama V1 manifest model.
The decoder preserves `seiyaku_name`, branded `kotoage`/`hajimari`/`kaizen`
kinds, exact flat-preorder argument and return schemas, access completeness,
triggers, state, error-code, `kotoba`, and provenance metadata. A `List` node
contains only `capacity` and its element subtree immediately follows it. The
decoder rejects unknown fields, legacy nested `element` metadata, incomplete or
trailing tapes, over-depth schemas, noncanonical Norito hash literals,
inconsistent convenience hashes, and drifted interface schemas before returning
the record.

## Motivation

`core-jvm` now ships typed builders for the first dedicated RWA instruction
slice alongside the existing NFT helpers: `RegisterRwaInstruction`,
`TransferRwaInstruction`, `MergeRwasInstruction`, `RedeemRwaInstruction`,
`FreezeRwaInstruction`, `UnfreezeRwaInstruction`, `HoldRwaInstruction`,
`ReleaseRwaInstruction`, `ForceTransferRwaInstruction`,
`SetRwaControlsInstruction`, and RWA-aware `SetKeyValueInstruction` /
`RemoveKeyValueInstruction` targets.

### Kotlin as the standard

Kotlin is the default language for Android development. Migrating from Java makes the SDK consistent with the Android ecosystem and eliminates the friction of Java/Kotlin interop at the call site.

### Java 8 bytecode safety

Android libraries must target Java 8 bytecode. Java 11+ API calls (`String.isBlank()`, `List.of()`, `Files.readString()`) crash at runtime on older Android devices. All modules enforce JDK 8 API compatibility at compile time via `-Xjdk-release=8` — using JDK 9+ APIs is a compilation error, not a silent runtime failure. Kotlin's standard library provides equivalent functions that are safe across all API levels.

### Reflection-free

The original Java SDK used reflection in multiple places (Android API discovery, BouncyCastle loading, keystore operations). This Kotlin rewrite eliminates reflection from `client-android` entirely and keeps optional-dependency probing isolated in `core-jvm`.

### Modular architecture

The original SDK shipped as a single monolith. This rewrite splits it into three artifacts with clear boundaries:

- **`core-jvm`** — pure JVM, no Android framework dependency. Usable in Kotlin Multiplatform modules, JUnit tests without Robolectric, server-side tools, and admin panels. Contains all protocol logic: Norito codec, transaction building, client transport, connect protocol.

- **`client-android`** — Android keystore integration, hardware-backed key generation, device telemetry, and the shared JNI bridge used for ML-DSA signing. Depends on `core-jvm` via `api()` — consumers get all core types transitively.

### Null safety

The Java SDK required defensive null checks at every Kotlin call site (`!!`, `?:`, `?.let {}`). Kotlin's type system makes nullability explicit — parameters that accept null are declared `T?`, everything else is guaranteed non-null by the compiler. This removes most `NullPointerException` risks from consumer apps. Some risk remains at Java interop boundaries (BouncyCastle, JCA) where platform types (`T!`) may hide nullability.

### Testability without Android

`core-jvm` runs on any JVM. Consumers can unit-test transaction building, address encoding, signing, and Norito serialization with plain JUnit — no Android instrumentation, no Robolectric, no emulator.

## Side Dependencies

| Dependency | Version | Used By | Risk |
|-----------|---------|---------|------|
| `org.bouncycastle:bcprov-jdk18on` | 1.78.1 | `core-jvm` (3 files: MultisigSeedHelper, ConnectCrypto, IdentifierReceiptVerifier) | **Binary compatibility** — BouncyCastle releases are not always backward-compatible. Consumer apps that bundle a different BC version may hit `NoSuchMethodError` at runtime. The SDK loads BC via reflection only when explicitly required; core crypto (Blake2b/2s/3, Ed25519, IrohaHash) uses only JCA and does not require BC. |
| `com.github.luben:zstd-jni` | 1.5.7-7 | `core-jvm` (Norito compression) | **Native library** — zstd-jni bundles platform-specific `.so`/`.dylib`. On Android, the JNI natives may conflict with other zstd consumers. Compression requires the native library to be available. |
