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
App code should use `OfflineCashLifecycleController` around the wallet for load
actions so pending audit receipts are submitted before the issuer sees a new
note-issue request. Local exchange screens should validate a cached
`OfflineCashConfigurationSnapshot` after setup and should not fetch
capabilities when creating or accepting a device-to-device transfer.

```kotlin
val snapshot = OfflineCashConfigurationSnapshot(
    chainId = "00000042",
    assetDefinitionId = "pkr#sbp",
    offlinePaymentsEnabled = true,
    issuerPublicKeyBase64 = cachedIssuerKeyBase64,
    nativeBridgeAbiVersion = 8,
    createdAtMs = cachedAtMs,
    expiresAtMs = expiresAtMs,
)
snapshot.requireUsableForOfflineExchange(
    nowMs = currentTimeMs,
    requiredNativeBridgeAbiVersion = 7,
)

val controller = OfflineCashLifecycleController(
    wallet = offlineWallet,
    auditReceiptSynchronizer = auditReceiptSynchronizer,
)
controller.load("pkr#sbp", "500")

val transports = OfflineNoteTransferCapabilities.current(
    androidHceSupported = appHasHceEntitlement && deviceSupportsNfc,
    nearbyAvailable = true,
).supportedModalities()
```

Do not render NFC controls when `supportedModalities()` omits NFC; non-NFC
devices and app builds without HCE should use QR or Nearby only.

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
`KagemushaCompactPaymentToken` when `connect_norito_bridge` is available and
the native Kagemusha entry point rejects the empty-archive availability probe.
`KagemushaRecursiveAggregationProofBundleProver` exposes the matching
admission-neutral recursive proof-bundle path. Pass the same record-bundle
archive plus a Norito-encoded Pallas open-envelope archive to receive a
Norito-encoded `KagemushaRecursiveAggregationProofBundle`.
`KagemushaRecursiveCompactPaymentTokenProver` exposes the ABI 7
`recursive_compact_v1` compact-token surface and probes
`kagemusha-recursive-compact-v1` separately from ABI 6 recursive spend. Use
`proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes`
with record-bundle, Pallas open-envelope, and recursive compact key-artifact
archives, and `verifyRecursiveCompactPaymentToken` with compact-token and
recursive compact verifier-key archives; gate them with `isNativeAvailable()`
and `isVerifierNativeAvailable()`. The recursive-spend compact projection
verifier is exposed separately as
`verifyRecursiveSpendCompactPaymentTokenProjection(...)` and
`verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(...)`; gate it with
`isProjectionVerifierNativeAvailable()`. It accepts raw Norito compact-token
and verifier-record archives, rejects empty, malformed, oversized, or
negative signed-height inputs before JNI dispatch, accepts full `u64`
activation heights through canonical unsigned decimal `String` or `BigInteger`
overloads, and returns the native boolean receiver result. ABI 7 now carries the
one-hop LEN=4 and package-backed
multi-hop compact-token proof paths when the native bundle includes packaged
compact proving-key archives and matching verifier-slice material. Production
defaults still stay on ABI 6 Reserved-lineage recursive spend until that
artifact set is shipped and signed for release. Empty, malformed, missing, or
oversized local archives fail as `IllegalArgumentException`; the legacy
`isRecursiveCompactUnavailable(error)` helper remains for older bridge
diagnostics.
The ABI-7 launch boundary remains explicit: the one-hop LEN=4 compact-token
proof path uses a packaged compact one-hop proving-key, while release evidence
continues to track the proof-composition reservation, generic compact-token
reservation, multi-hop verifier-batch reservation, and reserved ABI-7 state.
Missing native symbols still surface as `IllegalStateException`.
`KagemushaRecursiveSpendProver` exposes the ABI 6 spend-again-offline cash
surface. Preferred mode selection chooses `recursive_spend_v1` after the JNI
native bridge ABI-version probe succeeds and init, append, both transition-profile helpers,
the append-boundary helper, both lineage-witness helpers, verify, and redeem
reject the empty-archive availability probes instead of accepting permissive
native calls.
`transitionProfileInit(requestArchive)` and
`transitionProfileAppend(requestArchive)` return the canonical Reserved-lineage
accumulator transition profile as raw Norito archives for fixture generation
and circuit preflight. `lineageAppendBoundary(profileArchive)` derives the
compact append-boundary Norito archive from a full append transition profile
with native opening preflight material. The append-boundary digest uses the
public `RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1` domain, plus
`RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1` and
`RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1` for
chain/asset and final-root/current-note binding.
`KagemushaInstructionArchives` wraps a typed `KagemushaTransfer` or
`RedeemKagemushaRecursive` instruction archive, builds a single archived
instruction transaction payload, or derives the redeem instruction from a
native recursive redeem request before constructing that payload. These helpers
require valid Norito archives, reject empty, malformed, tampered, or wrong-type
instruction archives, and keep recursive redeem derivation inside the native
bridge.
Use
`canRedeemWitnessless(circuitId, hopCount)` or
`requiresLineageWitnessForRedeem(circuitId, hopCount)` before online redeem
construction. `RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1` is `64`, and
`RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1` is `true`, so
witnessless Reserved-lineage online redemption is available for lineage bundles
inside the 64-hop cap.
Use `canAppendWitnesslessLineage(previousHopCount)` before attempting a
witnessless Reserved-lineage append; it returns `true` for previous hop counts
`1..63`.
Use `preferredAppendOutputCircuitId(previousHopCount)` as the default append
output selector; it selects Reserved-lineage append for previous hop counts `1..63`.
Use `canProveAppendOutputCircuitId(outputCircuitId, previousHopCount)` before
selecting an append output circuit; semantic recursive append returns true
through hop 64, and Reserved-lineage append returns true for previous hop
counts `1..63`.
The semantic append path is bounded by `COMPACT_TOKEN_MAX_HOPS`; witnessless
Reserved-lineage append and redeem use the separate
`RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1` cap.
Use `canSelectAppendOutputCircuitId(previousProofCircuitId, outputCircuitId,
previousHopCount)` to apply the previous-proof transition rule before
serializing an append request.
`isSupportedPreviousProofCircuitId(previousProofCircuitId)` and
`requiresPreviousLineageVerifierRecordForAppend(previousProofCircuitId)` let
wallets reject unknown previous recursive proof circuits and include
`previous_lineage_verifier_record` only for Reserved-lineage previous bundles.
`requiresPreviousProofOpenEnvelopesForAppend(outputCircuitId,
previousHopCount)` identifies whether the selected append output circuit
requires the request to carry the previous recursive proof opening archive.
`outputCircuitId` is the Norito append request's `output_proof_circuit_id`;
missing or empty request values preserve semantic compatibility append.
`KagemushaRecursiveSpendProver.buildPallasOpenEnvelopesArchive(recordBundleArchive)`
and `buildPreviousProofOpenEnvelopesArchive(previousBundleArchive)` ask the
native bridge to generate the opaque Pallas opening archives for the current-hop
record bundle and the previous recursive proof. Typed-codec callers can use
`KagemushaRecursiveSpendRequestCodecs.buildPallasOpenEnvelopesArchiveForRecordBundle(recordBundle)`
and `buildPreviousProofOpenEnvelopesArchive(previousBundle)` to route through
the same native validators.
`previous_recursive_proof_open_envelopes_archive` is opaque native prover
material: Kotlin wallet code must pass it through Norito unchanged and must not
construct, rewrite, or mutate it. The native bridge validates `vk_commitment`,
`public_inputs_schema_hash`, and `domain_tag` against the exact previous bundle
before proving or returning output bytes.
Native append streams the previous recursive proof bytes and per-hop accumulator
material into native-owned accumulator digests (`recursive_proof_chain_digest`,
lineage/aggregation transcript, fixed-window schedule/shared-manifest/table-base,
verifier-witness batch, transition-profile, append-opening-preflight,
append-boundary, scalar-projection, and previous/resulting accumulator digests);
SDK code must not derive, supply, or patch accumulator state.
Verify request archives must pass the same public-binding preflight before the
native bridge returns a `KagemushaRecursiveSpendVerifyResultV1`:
Reserved-lineage bundles require a matching active `lineage_verifier_record`,
semantic bundles must omit it, and unsupported proof attachments are rejected
as malformed requests rather than soft invalid proof results.
Production init requests and Reserved-lineage append-output requests must also
include packaged lineage key artifacts in the raw Norito request:
`lineage_verifier_key` and `lineage_proving_key_archive`. Missing artifacts are
rejected before runtime key generation.
Reserved-lineage append output is valid only when the previous bundle is
already Reserved-lineage; semantic previous bundles keep using semantic append
plus a record-backed lineage witness.
`normalizeAppendOutputCircuitId` and `isSupportedAppendOutputCircuitId` expose
that defaulting rule for wallet-side preflight.
`RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1` and
`RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES` expose the
exactly-one-envelope cardinality rule and native 8 MiB pre-decode cap for that
archive.

### Native privacy bridge

`PrivacyNativeBridge` exposes the privacy FFI surface as generic raw Norito
archives: `capabilitiesArchive()`, `buildProof(requestArchive)`, and
`verifyProof(requestArchive)`. The Kotlin SDK does not expose
algorithm-specific production proof builders while the privacy rows remain
gated. Native availability requires ABI 6 or later, the privacy
capability/build/verify JNI symbols, and successful Norito probe outputs whose
operation-specific result schema bytes match the called entry point.

All privacy request and response payloads must stay as raw Norito archives.
Kotlin validates archive magic, length, CRC, the 64 MiB native size cap, and the
operation-specific result schema before returning bytes to callers. Capability
metadata reports `privacy-production-gate-v1`, keeps `productionReady = false`,
and remains fail-closed with missing production gates and no audit references
until real proving, verification, chain admission, witness privacy checks,
deterministic testing, negative/adversarial testing, replay/nullifier rejection
testing, parser/verifier fuzzing, performance gates, and external audit signoff
are complete.

Kotlin also exposes the deterministic privacy FFI status/error-code contract
for diagnostics and cross-language parity: `STATUS_ERROR`,
`ERROR_NULL_POINTER`, `ERROR_MALFORMED_NORITO`,
`ERROR_UNSUPPORTED_ALGORITHM`, `ERROR_PRODUCTION_DISABLED`, and
`ERROR_INVALID_REQUEST`. The stable wire values are `status_error = 1`,
`null_pointer = 1`, `malformed_norito = 2`, `unsupported_algorithm = 3`,
`production_disabled = 4`, and `invalid_request = 5`; treat them as
sanitized status metadata, not proof success.

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
