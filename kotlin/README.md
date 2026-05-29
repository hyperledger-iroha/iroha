# Iroha Kotlin SDK

Kotlin rewrite of the `iroha_android` and `norito_java` for Hyperledger Iroha 3.

## Artifacts

Not published to Maven Central yet. Build locally and consume via `mavenLocal()`.

| Artifact | Type | Description |
|----------|------|-------------|
| `org.hyperledger.iroha.sdk:core-jvm` | JAR | Pure Kotlin/JVM — models, codec, crypto, client, offline protocol |
| `org.hyperledger.iroha.sdk:client-android` | AAR | Android keystore, device telemetry, IrohaKeyManager, shared JNI bridge for ML-DSA / offline flows |
| `org.hyperledger.iroha.sdk:offline-wallet-android` | AAR | Offline wallet APIs and attestation (Play Integrity, SafetyDetect) layered on `client-android` |

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

// Android wallet with offline payments
implementation("org.hyperledger.iroha.sdk:offline-wallet-android:0.1-SNAPSHOT")
```

### Offline Note wallet flow

`core-jvm` exposes `OfflineBearerCashWallet` over the Offline Note engine for
the one-call app actions: load, prepare receive, pay, accept, optional audit
publication, redeem, and sync. Offline-to-offline `pay` and `accept` are the
local-final value transfer: the sender marks inputs spent and change spendable
immediately, and the recipient marks the matched pending output spendable after
local token and proof verification. No online sync is required for that
transfer. Audit publication is an explicit optional online step that submits
evidence but does not affect spendability. Wallets derive note commitments,
input nullifiers, and payment token ids locally, then delegate Torii issuance,
device attestation, proof generation/verification, persistence, and direct
audit/redeem transaction submission through injectable interfaces. The `sync()`
call uses an optional transaction-outcome resolver to reconcile redeem-pending
note records once the app's Torii/outcome index observes redeem finality.
JVM core includes an in-memory store, `IrohaOfflineNoteTransactionSubmitter`,
and `ToriiOfflineNoteIssuerClient` for Torii key-refill plus note-issue
loads. Apps provide canonical auth and a device-binding provider; Android
secure storage remains in the platform wallet layer. The Android
`AndroidOfflineNoteSecureStore` rotates a non-exportable Android Keystore key
on every committed wallet-state revision and rejects app-data rollback or
cloned preference snapshots when the old revision key is no longer present.
`KagemushaCompactPaymentTokenProver` exposes the native record-backed compact
token prover for shielded offline-offline payments. Pass a Norito-encoded
`KagemushaVerifiedFoldRecordBundle`; the JNI bridge verifies each private hop
proof against its verifier record and returns a Norito-encoded
`KagemushaCompactPaymentToken` when `connect_norito_bridge` is available.
Legacy `SPEND_PENDING` records are migrated to `SPENT`, and legacy
`CHANGE_PENDING` records are migrated to `SPENDABLE`.
`OfflineNoteTransferHandoff` exposes one integration surface for local token
handoff modalities: `qrStreamingFrameBytes(token)` for animated/binary QR,
`nfcFrameBytes(token)` for APDU-sized NFC frame exchange, and
`nearbyPayload(token)` / `nearbyFrameBytes(token)` for Nearby Connections,
Bluetooth, Wi-Fi Direct, or any app-owned byte channel. The receiver
`OfflineNoteTransferStreamReceiver` accepts those stream frames and returns a
decoded payment token when complete. Android apps can call
`AndroidOfflineNoteTransferCapabilities.current(context)` from
`offline-wallet-android` to include NFC only on devices that advertise HCE.
For png2-style NFC, bind `OfflineNoteNfcApduProtocol` to an Android
`HostApduService`/`IsoDep` reader: select the Iroha AID, exchange metadata,
transfer bounded chunks, commit, then poll/read a local `RECEIPT_ACK`. The
default `nfcPaymentTokenWriteApdus(token)` uses 240-byte chunks because Android
NFC APDU limits vary by device; extended iOS-to-iOS chunks are exposed only as
an explicit opt-in. `OfflineNoteNearbyEnvelope` provides the sorted-key
Nearby JSON envelope with unpadded base64url payloads and a human-verifiable
pairing challenge for Google Nearby Connections, Bluetooth, Wi-Fi Direct, or
another reliable byte channel.
The app-facing receiver rejects completed streams whose QR envelope kind is not
a payment token, and direct payload decoding enforces the payment-token content
type before Norito decode. The QR stream decoder also rejects non-canonical
frame/envelope lengths, header counter drift, data/parity count or chunk-length
mismatches, out-of-range wire fields, poisoned parity recovery, payload-hash
mismatches, and conflicting repeated headers or chunks.
The NFC APDU parser fails closed on nonzero Le bytes for no-data commands,
non-canonical zero-length reads, and direct read helpers with invalid requested
lengths; no-offset APDUs also reject smuggled nonzero P1/P2 bytes. Nearby
decoding rejects fractional versions, unknown fields inside legacy
pairing-challenge objects, and challenge/receipt ACK content-type downgrades
instead of ignoring smuggled JSON.

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

### Step 2: Build native libraries (for `client-android` and offline-wallet consumers)

The `libconnect_norito_bridge.so` files are **not tracked in git** — they are built from the Rust crate at `crates/connect_norito_bridge` in the same iroha repository. The Gradle task now lives on `client-android`, which owns the shared native bridge used for ML-DSA signing and offline-wallet helpers. It defaults to `../..` as the iroha root (override via `iroha.dir` in `local.properties` if needed).

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
./gradlew :client-android:buildNativeLibs
```

This Gradle task:
1. Reads `iroha.dir` from `local.properties`
2. Runs `cargo ndk` for `arm64-v8a` and `x86_64` targets
3. Copies `libconnect_norito_bridge.so` into `client-android/src/main/jniLibs/`

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
ls ~/.m2/repository/org/hyperledger/iroha/sdk/core-jvm/0.1-SNAPSHOT/
ls ~/.m2/repository/org/hyperledger/iroha/sdk/client-android/0.1-SNAPSHOT/
ls ~/.m2/repository/org/hyperledger/iroha/sdk/offline-wallet-android/0.1-SNAPSHOT/
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
./gradlew :offline-wallet-android:publishToMavenLocal
```

## Push Device Registration

`core-jvm` includes thin Torii helpers for `/v1/notify/devices`. Android apps still obtain FCM tokens from their app layer; the SDK only encodes the signed Torii request:

```kotlin
val request = PushDeviceRequest(accountId, "FCM", fcmToken, listOf("activity"))
transport.registerPushDevice(request, canonicalAuth).join()
transport.unregisterPushDevice(request, canonicalAuth).join()
```

## Signing Algorithm Selection

Android apps can now choose the transaction and offline-wallet signing
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

- **`core-jvm`** — pure JVM, no Android framework dependency. Usable in Kotlin Multiplatform modules, JUnit tests without Robolectric, server-side tools, and admin panels. Contains all protocol logic: Norito codec, transaction building, client transport, offline journal, connect protocol.

- **`client-android`** — Android keystore integration, hardware-backed key generation, device telemetry, and the shared JNI bridge used for ML-DSA signing. Depends on `core-jvm` via `api()` — consumers get all core types transitively.

- **`offline-wallet-android`** — offline wallet APIs and attestation helpers layered on `client-android`.

### Null safety

The Java SDK required defensive null checks at every Kotlin call site (`!!`, `?:`, `?.let {}`). Kotlin's type system makes nullability explicit — parameters that accept null are declared `T?`, everything else is guaranteed non-null by the compiler. This removes most `NullPointerException` risks from consumer apps. Some risk remains at Java interop boundaries (BouncyCastle, JCA) where platform types (`T!`) may hide nullability.

### Testability without Android

`core-jvm` runs on any JVM. Consumers can unit-test transaction building, address encoding, signing, and Norito serialization with plain JUnit — no Android instrumentation, no Robolectric, no emulator.

## Side Dependencies

| Dependency | Version | Used By | Risk |
|-----------|---------|---------|------|
| `org.bouncycastle:bcprov-jdk18on` | 1.78.1 | `core-jvm` (3 files: MultisigSeedHelper, ConnectCrypto, IdentifierReceiptVerifier) | **Binary compatibility** — BouncyCastle releases are not always backward-compatible. Consumer apps that bundle a different BC version may hit `NoSuchMethodError` at runtime. The SDK loads BC via reflection only when explicitly required; core crypto (Blake2b/2s/3, Ed25519, IrohaHash) uses only JCA and does not require BC. |
| `com.github.luben:zstd-jni` | 1.5.7-7 | `core-jvm` (Norito compression) | **Native library** — zstd-jni bundles platform-specific `.so`/`.dylib`. On Android, the JNI natives may conflict with other zstd consumers. Compression requires the native library to be available. |
