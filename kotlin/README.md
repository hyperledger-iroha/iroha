# Iroha Kotlin SDK

Kotlin rewrite of the `iroha_android` and `norito_java` for Hyperledger Iroha 3.

## Artifacts

Not published to Maven Central yet. Build locally and consume via `mavenLocal()`.

| Artifact | Type | Description |
|----------|------|-------------|
| `org.hyperledger.iroha.sdk:core-jvm` | JAR | Pure Kotlin/JVM models, codecs, cryptography, clients, and KAGEMUSHA V1 wire support |
| `org.hyperledger.iroha.sdk:client-android` | AAR | Android keystore, device telemetry, IrohaKeyManager, shared JNI bridge for ML-DSA-65 / KAGEMUSHA flows |
| `org.hyperledger.iroha.sdk:kagemusha-wallet-android` | AAR | KAGEMUSHA wallet integration built on `client-android`; use this artifact for Android KAGEMUSHA |

### Consumer usage

```kotlin
// build.gradle.kts (consumer project)
repositories {
    mavenLocal()
}

// Pure JVM — business logic modules, JUnit tests, server-side
implementation("org.hyperledger.iroha.sdk:core-jvm:0.1.0")

// Android wallet without KAGEMUSHA payments
implementation("org.hyperledger.iroha.sdk:client-android:0.1.0")

// Android wallet with KAGEMUSHA payments
implementation("org.hyperledger.iroha.sdk:kagemusha-wallet-android:0.1.0")
```

`KagemushaWalletV1` is the Android-free aggregate-balance orchestrator. It opens only with an
`KagemushaHardwareProviderV1` that attests the complete exact-next counter, rollback-resistant
journal/inbox/outbox, trusted-time, atomic recovery, and hardware-epoch rotation contract. Incoming
payments are acknowledged only after durable staging; duplicate delivery returns the provider's
same durable ACK. Sends and redemptions require the native provider to fold the staged credits
needed to cover the amount; unrelated backlog must not delay an already-covered spend.
`foldReceiveCredit()` and `drainPendingCredits()` expose exact single-credit folds and a
stable-snapshot drain without a cumulative count limit. The drain releases the lane after each fold for queued
foreground work. Concurrent epoch rotation interrupts the drain; start a new pass for the new
epoch's watermark. Continuous background scheduling remains an integration
requirement. `KagemushaAndroidWalletV1.openProduction(...)` additionally binds an OEM adapter to
the native device service. Stock KeyMint and StrongBox remain online-only because signing keys do
not supply the required non-forking persistence contract; there is no software fallback.
Staging advances native inbox bookkeeping, not the monetary-state journal. Core's typed mint
reservation/inbox implementation is under validation; its SDK-to-OEM operation-16 adapter is
still required. A completed MintFold is a separate proved transition, not a staging result.
Managed KAGEMUSHA X25519 types enforce only the canonical 32-byte nonzero wire shape. They do
not perform scalar multiplication or low-order probing; the shared native core authenticates
canonical X25519 elements during object and complete three-message exchange validation before monetary use.
Logical sequence and durable journal revision are per hardware epoch. Authenticated rotation carries
the full balance and replay root into the exact successor epoch, replaces the device-policy binding,
and resets both counters to zero. `rotateHardwareEpoch()` does not first drain the inbox, so it
remains callable with saturated counters and pending receipts. The native provider must arrange
rollover before counter exhaustion; the managed wallet does not schedule automatic rotation.

### Transaction identity

Every ordinary `TransactionPayload` requires a nominal, immutable `NetworkId` parsed from the
exact canonical checksummed 32-byte genesis-header hash literal. The Norito codec emits it only as
`TransactionDomain::Network`; the genesis-only domain is not constructible through the SDK and is
rejected while decoding. JSON transaction surfaces use
`"domain":{"kind":"network","value":"<canonical NetworkId>"}` and reject the retired `chain`,
`chainId`, and `chain_id` identity fields. Security-sensitive SDK payloads and canonical HTTP
signatures use this exact `NetworkId`; human-readable deployment labels are display-only and are
never accepted as a signing domain.

`SignedTransactionHasher` computes the first-release external transaction ID
from `TransactionEntrypoint::External` plus the canonical signed
`TransactionPayload`. Authorization signatures and multisig proofs remain in
the submitted `SignedTransaction` wire but do not create alternate IDs for the
same intent. Proof attachments are carried by `TransactionPayload.attachments`,
so adding, removing, or replacing an attachment changes the signature preimage
and transaction ID.

### One-shot signed HTTP requests

Signed transactions, signed queries, transaction batches, and every request carrying an Iroha
nonce are dispatched at most once. The default URLConnection transport does not follow 307/308
redirects or retry connection/status failures. Custom `HttpTransportExecutor` implementations must
honor `TransportRequest.replayPolicy`: only unsigned, bodyless `GET`, `HEAD`, and `OPTIONS` requests
are `RETRY_SAFE`; all other requests are `ONE_SHOT`.

Raw `witness_base64` body authentication is not an SDK surface. Multisig writes must use a
canonical signed transaction or a closed typed signed intent.

`prepareContractCall` accepts a draft receipt only when `payload_digest_hex` is
the exact lowercase BLAKE3-256 digest of the canonical UTF-8 JSON request
payload. An omitted payload hashes the empty byte sequence; noncanonical hex or
a digest mismatch fails closed before the draft is returned.

Canonical request builders keep I105 as the semantic SDK identity but emit its lowercase
canonical-hex address in `X-Iroha-Account`, which is safe on strict ASCII HTTP stacks. Active
canonical ASCII aliases are emitted unchanged. Signed JSON `account_id` fields retain the caller's
exact canonical spelling: I105 remains I105, and a canonical body-auth alias remains unchanged.
Alias inputs must already use the exact canonical `label@dataspace` or
`label@domain.dataspace` lowercase-ASCII shape. The signer applies only bounded
structural preflight; Torii remains authoritative for UTS-46, active bindings,
and controller verification.
The first-release signing domain is an exact genesis-derived
`hash:<64 uppercase hex digits>#<4 uppercase CRC-16 digits>` `NetworkId` whose decoded 32-byte
value carries the V1 marker bit. Canonical nonces contain 1--256 visible ASCII bytes. Methods are
non-empty ASCII HTTP tokens of at most 32 bytes, and URI signers require an exact root-relative
ASCII raw path of at most 64 KiB; absolute inputs must be hierarchical HTTP(S) URIs with an
authority and no fragment.

Sora VPN receipt submission returns the native `SettleVpnLease` instruction
with exact status `settlement_pending`. That status remains provisional until
the instruction commits; only a receipt read from committed WSV state uses
`settled`. The parser also retains the exact `disconnected`, `expired`, and
`replaced` lifecycle values.

Identifier resolve/claim-receipt and RAM-LFE execute/receipt-verify calls require a per-call
`ToriiCanonicalRequestAuth` and `ClientConfig.localSigningContext`. The transport signs the exact
POST path and body once, rejects caller-supplied canonical headers, and requires a claim-receipt
path account to be the same exact canonical I105 account as the signer.

If `submitTransaction` cannot obtain an authoritative admission result, it fails with
`AmbiguousTransactionSubmissionException`. Use its `hashHex` or `reconcileWith(client)` to query
pipeline status. The exact binary endpoint accepts only HTTP `202`; any other non-ambiguous HTTP
response fails with `TransactionSubmissionHttpException`, retaining the hash, status, reject code,
and bounded response detail. Never resend the same signed bytes. `RetryPolicy` applies only to
caller-managed replay-safe reads, and configured pending queues are explicit local staging:
submission neither fills nor drains them.

Public pipeline status contains only the canonical transaction hash—exactly
`[0-9a-f]{63}[13579bdf]`, including the Iroha `HashOf` marker—closed status kind,
optional committed height, read scope, and resolution source. The parser rejects rejection
text, diagnostics, trigger completions, batch outcomes, unknown kinds, and noncanonical
metadata. Status scope is exactly `local` or `global`; `auto` is not a first-release value, and
`waitForTransactionStatus(...)` always requests `global`. Transaction-hash request values,
status responses, and Torii receipt headers are never trimmed, case-folded, prefix-stripped, or
decoded from byte-shaped values. Status reads accept only an exact HTTP `200` envelope or `404`
not-found response; `202` and `204` are protocol errors. State-resolved `Rejected` and `Expired`
are the only failures; every other non-success status remains progress. Negative polling
intervals and timeouts are rejected rather than clamped. Detailed transaction reads require an involved account or operator to send a
one-shot canonical signed `FindTransactions` query bound to the exact genesis-derived
`NetworkId`; Kotlin intentionally exposes no details helper until its generated signed-query
surface supports that contract.

### DA commitment and pin-intent proofs

`HttpClientTransport.newDaToriiClient()` returns the typed DA client. It covers
the proof-policy, commitment list/prove/verify, and pin-intent
list/prove/verify routes. DA digests use `DaModels.Digest32`; proof counters use
`BigInteger` so the full unsigned 64-bit wire range remains exact.

```kotlin
val da = transport.newDaToriiClient()
var page = da.listPinIntents(
    DaModels.PinIntentListRequest(limit = BigInteger.valueOf(100)),
).join()
while (page.nextCursor != null) {
    page = da.listPinIntents(
        DaModels.PinIntentListRequest(
            limit = BigInteger.valueOf(100),
            cursor = page.nextCursor,
        ),
    ).join()
}
val proof = da.provePinIntent(
    DaModels.PinIntentQueryRequest(
        storageTicket = DaModels.Digest32.fromHex(ticketHex),
    ),
).join()
if (proof != null) {
    check(da.verifyPinIntent(proof).join().valid)
}
```

List routes use immutable, server-issued snapshot cursors and return an explicit
nullable `nextCursor`. Proof routes use separate selector-only request types;
pagination fields are not accepted by proof requests.

Responses are decoded into closed typed models. The client rejects unknown
fields, malformed transparent byte wrappers, invalid checksummed hashes,
contradictory verification responses, and Merkle paths whose direction/length
does not match the advertised bundle location. Requests are capped at 64 KiB
and buffered responses at 8 MiB.

### Authoritative Sumeragi status and operational diagnostics

`HttpClientTransport.getSumeragiStatus()` reads only
`GET /v1/sumeragi/status` into the closed protocol-v4
`SumeragiV2Status` model. `getSumeragiDiagnostics()` separately reads
`GET /v1/sumeragi/diagnostics` into `SumeragiDiagnosticsStatus`; diagnostics
are durable operational evidence and must not be treated as consensus
authority.

```kotlin
val status = transport.getSumeragiStatus().join()
check(status.protocolVersion == 4)
println("height=${status.height} view=${status.view} leader=${status.leader}")

val diagnostics = transport.getSumeragiDiagnostics().join()
diagnostics.nativeAmxParticipantApplications.forEach { row ->
    println("lane=${row.laneId} height=${row.participantHeight} state=${row.state}")
}
```

Every JSON `u64` remains lossless as `BigInteger`. Status responses are capped
at 1 MiB and diagnostics at 16 MiB; both routes require the exact JSON content
type, a canonical matching `Content-Length` when supplied, fatal UTF-8, closed
fields and tags, and current Native AMX V2 evidence. The parsers reject
status/diagnostics swaps, legacy receipt shapes, unordered or oversized Native
participant rows, and inconsistent carrier identities.

### KAGEMUSHA peer transports

`KagemushaNoritoV1` is the canonical KAGEMUSHA wire codec. Kotlin/JVM and Android
encode the same three-message payment exchange—signed exact-amount request with its
recipient key, post-commit proof-bearing payment, and acknowledgement.
Mint authorization, mint credit, and redemption vouchers are separately framed;
`kgm1:` is the sole text transport. There is no intent, ticket, request-mode, or
alternate compatibility path. QR, NFC, and Nearby consume
`../fixtures/offline/kagemusha_v1.json`. Public wire
size and verification work are independent of balance history; no hop, input,
origin, ancestry, fan-in, or proof-depth limit is encoded.

Online reserve top-ups use the same payer authority as the debit. Build one
`TopUpKagemushaV1Instruction` from the proof-bearing request, put that sole
instruction in a transaction, and sign it with `TransactionBuilder`.
`TransactionBuilder` always signature-binds `QueuePlanSynced` for public
submission. Send the resulting `SignedTransaction` and the request's exact
nonzero 32-byte `operationId` through
`KagemushaToriiClientV1.submitTopUp(...)`. The client posts the canonical
versioned signed-transaction bytes unchanged to `/v1/kagemusha/top-up` and
uses the lowercase operation ID as `Idempotency-Key`; there is no unsigned or
request-only top-up overload. The embedded request ceiling is 16 KiB so both
maximum-size recursive proof parities remain usable.

### Fee quotes and sponsorship

Every transaction payload requires a typed `FeePaymentIntent`. Select the
authority directly, or bind sponsorship to one exact on-chain program and
immutable revision:

```kotlin
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.FeeSponsorProgramId

val authorityPaid = FeePaymentIntent.authority(emptyList())
val sponsored = FeePaymentIntent.sponsor(
    FeeSponsorProgramId(sponsorAccountId, "wallet_payments"),
    3,
    emptyList(),
)
```

The empty charge-limit list is only the initial quote draft. Freeze the complete
unsigned payload, include `fee_payment = requested.toJsonMap()`, and call
`HttpClientTransport.quoteFees(unsignedPayload, canonicalAuth)`. Verify that the
response preserved the payer, exact program/revision, and gas bound; replace
only `fee_payment` with `FeeQuoteResponse.intent`, then sign and submit that same
payload. Use `getFeeSponsorProgram(programId, canonicalAuth)` to inspect one
exact lifecycle record before selecting its revision. Contract/IVM drafts must
include a positive gas bound in the intent.

The metadata keys `fee_sponsor`, `gas_asset_id`, and `gas_limit` are retired and
rejected. A sponsor rejection never falls back to charging the authority.

For a live aggregate Hijiri adjustment, use the separate native-Norito route:

```kotlin
val request = ValidationFeeHijiriQuoteRequestV1(accountId, qualifyingTransferCount)
val quote = transport.postValidationFeeHijiriQuote(request, canonicalAuth).join()
```

This operation requires `libconnect_norito_bridge` ABI 23 and an HTTPS Torii
base URL. It signs the exact bounded Norito request with `Cache-Control: no-store`,
requires a private, non-stored, uncompressed `application/x-norito` response,
and exposes the typed projection only after native canonical decode,
arithmetic/hash validation, and exact request binding. `canonicalAuth` may be
the quoted account or a direct signatory of that multisig account; Torii checks
the live relationship. The returned assurance explicitly describes an
authenticated live evaluation, not an independently witness-verified proof;
admission-bound policy and Hijiri hashes detect a quote that became stale.

### Atomic mixed executable batches

Use `Executable.batchBuilder()` when one transaction must interleave native
instructions and deployed-contract calls:

```kotlin
val executable = Executable.batchBuilder()
    .addInstruction(registerInstruction)
    .addContractCall(
        ContractInvocation(contractAddress, expectedCodeHash, "apply", argumentRecord),
    )
    .addInstruction(transferInstruction)
    .build()
```

The item order is canonical and the node applies the whole batch atomically.
Empty batches are rejected. Contract addresses must be canonical lowercase V1
Bech32m literals, and any batch containing a contract call needs one positive,
signature-bound gas limit in its `FeePaymentIntent`; these constraints are
checked before the payload can be encoded or signed.

### Native asset-lock cancellation

`CancelAssetLockInstruction` implements the V1 compare-and-cancel contract.
Supply the exact application lock ID and the positive canonical Quantity read
from finalized ledger state:

```kotlin
import org.hyperledger.iroha.sdk.core.model.instructions.CancelAssetLockInstruction

val cancel = CancelAssetLockInstruction(
    lockId = "appeal:case-42",
    expectedRemainingAmount = "20",
)
val instructionBox = cancel.toInstructionBox()
```

The typed constructor derives the native `EscrowId` with Blake2b-256 and emits
only `escrow_id` plus `expected_remaining_amount`. The instruction pair uses
the canonical `iroha.instruction.v1::escrow::CancelAssetLock` wire ID; its
payload frame retains the concrete `iroha_data_model::isi::escrow::CancelAssetLock`
Norito schema name. The lock-ID
preimage must be nonempty exact text without surrounding whitespace or a BOM
and is bounded by `CancelAssetLockInstruction.MAX_LOCK_ID_UTF8_BYTES_V1`
(4,096 UTF-8 bytes, not characters); the on-wire `EscrowId` remains 32 bytes.
The retired one-field shape, aliases, extra fields, zero, and alternate numeric
spellings are rejected; stale expected amounts are rejected atomically by the
ledger.

### Torii server-sent events

`HttpClientTransport.newEventStreamClient()` does not synthesize an account
identity; without auth-bearing default headers its requests remain fully
anonymous and public-only. It still inherits the HTTP client's base URI,
default headers, and observers. Use `newEventStreamClient(canonicalAuth)` with
a configured `LocalSigningContext` to add a canonical account identity. The
client generates all four canonical headers after path resolution, filter
normalisation, and option-query assembly, so the signature is bound to the
exact final URI;
precomputed or partial canonical headers are rejected before dispatch. The
canonical `/v1/events/sse` and `/v1/contracts/events/sse` feeds are live-only
and have no replay log. `ToriiEventStreamClient` therefore rejects every case
variant of `Last-Event-ID` before dispatch for exactly those two paths; custom
streams that provide replay may still receive the header through
`ToriiEventStreamOptions`.

Raw listeners receive terminal `event: stream_error` frames. Call
`ServerSentEvent.terminalStreamError()` before application-event projection to
obtain a strict `ToriiStreamException` containing the stable code, server
message, optional unsigned dropped-message count, replay flag, and raw JSON.
Malformed or schema-expanded terminal envelopes fail closed as
`ToriiStreamProtocolException`; they must not be filtered as unrelated events.
A reconnect to either canonical feed starts a new live subscription and can
have a gap.

### Native privacy bridge

`PrivacyNativeBridge` exposes local build metadata only.
`compiledProfileCatalogV1()` returns this binary's canonical typed
`PrivacyCompiledProfileCatalogV1` Norito archive, and
`protocolsV1()` exposes the closed `ProtocolIdV1` enum in exact wire order. The
generic proof request/build/verify ABI and free-form algorithm selectors are
absent; proofs must use protocol-specific typed APIs. The local catalog never
establishes activation or readiness; proof submission requires a fresh
committed `/v1/privacy/capabilities` snapshot from live Torii.

Genesis `confidential_features` and `zk_policy_hash` values are opaque consensus
fingerprints, never client-side proof or backend selectors.
`ClientConfigManifestLoader` rejects those keys (and their camel-case aliases)
at any depth. Proof construction must use Torii's committed
`/v1/privacy/capabilities` response and the on-chain verifying-key registry.

`PrivacyExact12FixtureCodecV1` decodes the first-release
`PrivacyExact12FixtureBundleV1` entirely in Kotlin; it does not load the native
bridge. Pass one canonical standard-Base64 line (without the fixture file's
final LF), or decode raw Norito bytes directly:

```kotlin
val bundle = PrivacyExact12FixtureCodecV1.decodeCanonicalBase64(fixtureLine)
val canonicalArchive = PrivacyExact12FixtureCodecV1.encodeCanonical(bundle)
PrivacyExact12FixtureCodecV1.requireCanonicalArchive(receivedArchive, canonicalArchive)
```

The codec requires the exact schema, version, twelve-row order, uncompressed
`COMPACT_LEN` layout, and configured field/aggregate limits. It rejects
alternate Base64, truncation, trailing or unknown data, and reordered rows.
Use `requireCanonicalArchive` with an independently trusted fixture when exact
cross-row and cross-field identity matters.

The registry has exactly twelve IDs: `zk-ace-pq-authorization-v1`,
`anonymous-pgc-k-out-of-n-v1`, `verange-transparent-range-v1`,
`iroha-zk-ams-v1`, `vega-existing-credential-zk-v1`,
`iroha-zk-x509-stark-p256-v1`,
`iroha-jindo-polynomial-commitment-v1`,
`iroha-bootle-lantern-anoncred-v1`, `orchard-halo2-actions-v1`,
`monero-fcmp-plus-plus-v1`, `iroha-ivm-private-note-stark-v1`, and
`pq-masp-stark-v1`. Parsing is exact: aliases, retired IDs, case changes, and
whitespace normalization fail closed.

### Shared Java transaction fixtures

Kotlin/JVM and the mirrored Java Android SDK validate the same Rust-owned
transaction corpus. The authority is `../fixtures/norito_rpc`: Kotlin's
`AndroidFixtureSupport` resolves the descriptors and all 27 canonical
`.norito` payloads there, while the owner publication also writes the identical
descriptor-and-blob set into `../java/iroha_android/src/test/resources` for
Java's classpath-based tests. There is no Kotlin-local fixture copy and the
generated Java resource directory is never a regeneration input. Rotate both
consumers only through the two-root `norito-rpc-fixtures` owner workflow and
finish with `norito-rpc-verify`.

---

## Build Instructions

Rust/Kotlin parity tests require a freshly built `kotlin-fixture-gen`
executable supplied explicitly through `IROHA_KOTLIN_FIXTURE_GEN_BIN`. Relative
paths are resolved from the repository root. The test runner rejects an unset,
blank, missing, non-file, or non-executable value and never invokes Cargo:

```bash
export IROHA_KOTLIN_FIXTURE_GEN_BIN=/absolute/path/to/kotlin-fixture-gen
./gradlew :core-jvm:test --console=plain
```

### Prerequisites

| Tool | Version | Required For |
|------|---------|-------------|
| JDK | 21 | All modules |
| Android SDK | compileSdk 35 | `client-android` |
| Rust | exactly 1.93.1 | Native `.so` build |
| Android NDK | exactly 28.0.12674087-beta2 (r28-beta2) | Native `.so` build |
| `cargo-ndk` | exactly 4.1.2 | Native `.so` build |

### Step 1: Build core-jvm

The pure JVM module has no native dependency and builds immediately. Android
variant assembly is covered in the next step because AGP is causally wired to
the generated native bridge task.

```bash
# Build and run tests
./gradlew :core-jvm:build --quiet

# Run core-jvm unit tests
./gradlew :core-jvm:test --console=plain
```

### Step 2: Build native libraries (for `client-android`)

The `libconnect_norito_bridge.so` files are **not tracked in git** — they are built from the Rust crate at `crates/connect_norito_bridge` in the same iroha repository. The Gradle task now lives on `client-android`, which owns the shared native bridge used for ML-DSA-65 signing and KAGEMUSHA V1 device lifecycle operations. It defaults to `../..` as the iroha root (override via `iroha.dir` in `local.properties` if needed).

**One-time setup:**

```bash
# Install Rust Android targets
rustup target add --toolchain 1.93.1 aarch64-linux-android x86_64-linux-android

# Install cargo-ndk
cargo install cargo-ndk --version 4.1.2 --locked

# Select the authenticated Android NDK and an external artifact root
export ANDROID_NDK_HOME=/absolute/path/to/android-ndk/28.0.12674087
export MOBILE_SDK_ANDROID_ARTIFACT_DIR=/absolute/non-symlink/path/to/android-artifacts
mkdir -p "$MOBILE_SDK_ANDROID_ARTIFACT_DIR"
```

**Build the .so files:**

```bash
# Build the capability-only native bridge.
./gradlew :client-android:buildNativeLibs
```

This Gradle task (and every `client-android` release assembly):
1. Reads `iroha.dir` from `local.properties`
2. Captures the exact Android-target dependency-closure source seal, then runs
   locked `cargo ndk` separately for `arm64-v8a` and `x86_64`, checking that
   seal after every ABI build. Each cargo-ndk destination is transient because
   Cargo can copy unrelated workspace `cdylib` outputs there; only the exact
   `libconnect_norito_bridge.so` name is promoted into the authoritative raw
   directory under the external
   `$MOBILE_SDK_ANDROID_ARTIFACT_DIR/gradle-build/iroha_kotlin_sdk/client-android/native/cargo-ndk/<mode>/`.
   Compiler state remains isolated in its sibling
   `native/cargo-target/<mode>/` through a mode-specific `CARGO_TARGET_DIR`.
   Every raw and stripped/provenance
   promotion re-authenticates the saved source commit and selected dependency-
   closure fingerprint immediately before and after the promotion; the source
   sampler itself rejects commit or fingerprint drift during authentication.
3. Copies the raw libraries to a distinct generated directory, then canonically
   strips only those copies with the selected Android NDK's
   `llvm-strip --strip-unneeded`
4. Writes the authoritative libraries under the external
   `client-android/generated/jniLibs/<mode>/` subtree
5. Generates the external
   `client-android/generated/nativeProvenance/<mode>/iroha/native-build-provenance-v1.json`
   with the ABI, feature state, source commit/scoped dirty bit, dependency-
   closure `source_fingerprint_sha256`, toolchain identity, and raw/stripped
   sizes and hashes

AGP 9.0.1 registers both generated directories through
`addGeneratedSourceDirectory`, so the release AAR preserves those exact bytes
and embeds the provenance at
`assets/iroha/native-build-provenance-v1.json`. `src/main/jniLibs` is excluded;
ignored or hand-copied source-tree `.so` files cannot enter an AAR. The mobile
artifact checker rejects an unstripped library, stale source fingerprint,
malformed provenance, extra native file (including another Rust `cdylib`), or
any size/hash difference among raw cargo-ndk output, generated stripped output,
provenance, and the AAR.

Debug/JVM unit-test compilation deliberately does not register the shipping JNI
and provenance directories, so it never launches Cargo/NDK merely to compile
tests. An actual Debug app or instrumentation run that needs native calls must
pass `-PirohaDebugNativeBridge=true` with the same external artifact root and
authenticated NDK configuration. This registers the maintained generated JNI
and provenance outputs for the Debug AAR; it does not copy source-tree libraries,
enable native proving, or qualify an Offline device provider. Only exact `true`
and `false` property values are accepted. For example:

```bash
./gradlew :client-android:assembleDebug -PirohaDebugNativeBridge=true
```

The property also applies to this SDK when an Android app includes it as a
composite build. Release packaging always includes the bridge independently of
this Debug property. An unchanged raw build is reusable only while its saved source seal still
matches the live checkout; release packaging always re-runs the inexpensive
strip/provenance phase and its final seal check.

The production-gated form passes `--features privacy-production-enabled` to
`connect_norito_bridge`; the default form intentionally omits that feature so
unaudited native proving remains disabled.

For every ABI, Gradle resolves canonical `cargo`, `rustc`, and `rustdoc`
executables from exact Rust 1.93.1. It requires one job, incremental compilation
off, offline dependency resolution, and `RUSTC_BOOTSTRAP=1`, then invokes the
Cargo build with the exact root lock contract:

```text
build --locked --offline --jobs 1 -Z unstable-options \
  --lockfile-path <canonical-iroha-root>/Cargo.lock
```

There is no alternate-lock or compatibility override.

The first build takes ~5-10 minutes because it compiles all Rust dependencies.
The isolated target can reuse dependency artifacts, but compiler incremental
state remains disabled.

**Output:**

| ABI | File |
|-----|------|
| arm64-v8a | `$MOBILE_SDK_ANDROID_ARTIFACT_DIR/gradle-build/iroha_kotlin_sdk/client-android/generated/jniLibs/<mode>/arm64-v8a/libconnect_norito_bridge.so` |
| x86_64 | `$MOBILE_SDK_ANDROID_ARTIFACT_DIR/gradle-build/iroha_kotlin_sdk/client-android/generated/jniLibs/<mode>/x86_64/libconnect_norito_bridge.so` |

`<mode>` is `default` unless the property is exactly
`-PprivacyProductionEnabled=true`, in which case it is `production`. Any value
other than the exact strings `true` and `false` is rejected.

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
ls ~/.m2/repository/org/hyperledger/iroha/sdk/kagemusha-wallet-android/0.1.0/
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
registry fields, height ranges, and inline verifier-key commitments before
sending the request. Neither request accepts or transmits a private key. Torii
returns HTTP 200 with an unsigned transaction draft. SDK `Signer`
implementations apply Iroha's prehash themselves, so pass
`transactionPayloadBytes()` to `Signer.sign`; use `signingMessageBytes()` only
with an external primitive that signs an already-prehashed message. Attach the
signature to the transaction payload and use the standard transaction ingress.
The `ClientConfig` used by the transport must include an immutable
`LocalSigningContext`; read-only clients may omit it, but draft-producing
mutation routes fail before network I/O when it is absent. The draft parser
binds the exact canonical genesis-derived `NetworkId` and rejects
non-canonical Norito, another network or authority, extra/substituted
instructions, any mismatch in the complete verifying-key record, and signing
messages that do not match the payload prehash:

```kotlin
val networkId = NetworkId.parse("<canonical_network_id_hash_literal>")
val config = ClientConfig.builder()
    .setLocalSigningContext(LocalSigningContext(networkId))
    // Configure the Torii endpoint and other client policy here.
    .build()
val transport = HttpClientTransport.withExecutor(executor, config)
val vkBytes = byteArrayOf(1, 2, 3)

val registerDraft = transport.registerVerifyingKey(
    VerifyingKeyRegisterRequest(
        authority = "<authority_i105>",
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

val updateDraft = transport.updateVerifyingKey(
    VerifyingKeyUpdateRequest(
        authority = "<authority_i105>",
        backend = "halo2/ipa",
        name = "vk_main",
        version = 2,
        circuitId = "halo2/ipa::transfer_v1",
        publicInputsSchemaHashHex = "a".repeat(64),
        status = "Withdrawn",
    )
).join()

check(!registerDraft.submitted)
val registerPayload = registerDraft.transactionPayloadBytes()
val registerSigningMessage = registerDraft.signingMessageBytes()
val registerSignature = signer.sign(registerPayload)
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

For Android Keystore Ed25519 aliases, required hardware preferences are checked
against the selected key's `KeyInfo`, not only provider capability or the
generation request. Unknown provenance fails closed; preferred policies may
downgrade and expose the measured route. Custom `KeyProvider` implementations
must override `outcomeFor(...)` to prove a hardware route, and a preference set
through `KeystoreKeyProvider.withPreference(...)` remains effective for plain
`generate(...)` calls. Deterministic Ed25519 export/import in
`core-jvm` also derives the public key from the private seed at both boundaries,
rejecting substituted public keys and inconsistent input pairs without changing
the v4 bundle layout.

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

## Musubi V1 registry reads

`MusubiToriiClientV1` is the exact-network authenticated client for the twelve typed
`/v1/musubi/queries/*` POST routes. Its builder requires `LocalSigningContext`, and every method
requires `ToriiCanonicalRequestAuth`. Each exact raw body/path is signed with the configured
`NetworkId` and dispatched with one-shot replay policy. The `sdk.musubi` models preserve structural
package IDs, immutable namespace bindings, canonical structured SemVer
requirements, exact unsigned integers, finalized cursors, archive commitments,
and one exact genesis-derived `NetworkId` without legacy aliases or compatibility decoding.
Unknown fields, unsupported ABI/edition versions, and noncanonical names or
requirements are rejected. Each manifest, verification-lock parent, and
resolver row must use a distinct parent-local alias for every dependency.

The canonical cross-SDK JSON contract is
[`fixtures/musubi/sdk_v1.json`](../fixtures/musubi/sdk_v1.json), owned by the Rust
`iroha_data_model::musubi` surface. Canonical or witness headers cannot be injected through
default transport headers; the client derives them only from the explicit signing values.

The `/v1/musubi/queries/search` route exposes bounded exact-token
description and keyword discovery with a search-specific finalized projection
cursor. It is intentionally independent of dependency resolution.

`findArchiveRetention` submits a sorted, bounded exact archive batch and binds
the response to the requested identities and optional finalized snapshot before
returning any prune classification.

`MusubiInstructionsV1` supplies typed field-to-Norito constructors for immutable
namespace registration; package-maintainer invitation, acceptance, revocation,
role replacement, and removal; archive registration, location addition or
renewal, and location retirement; release publication, yank, and unyank;
permanent alias registration; exact release-digest assertion; package metadata
replacement; and Parliament-enacted package ownership recovery,
permanent-alias retargeting, artifact takedown, and registry-policy replacement.
Each builder exposes `barePayload()`, `concreteFrame()`, and
`toInstructionBox()`; transaction encoding preserves the dynamic pair inline,
while standalone boxes use Rust's exact tuple schema. All nineteen cases and
their four wire layers are checked against
[`fixtures/musubi/instructions_v1.json`](../fixtures/musubi/instructions_v1.json).

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

The original Java SDK used reflection in multiple places (Android API discovery, BouncyCastle loading, keystore operations). Kotlin production sources are reflection-free across `core-jvm`, `client-android`, and `kagemusha-wallet-android`; `scripts/check_kotlin_no_reflection.sh` enforces that contract. BouncyCastle is linked directly, Android keystore APIs are guarded by platform-version checks, and WebSocket clients require callers to inject a connector with `setWebSocketConnector(...)` instead of discovering one at runtime.

### Modular architecture

The original SDK shipped as a single monolith. This rewrite splits it into three artifacts with clear boundaries:

- **`core-jvm`** — pure JVM, no Android framework dependency. Usable in Kotlin Multiplatform modules, JUnit tests without Robolectric, server-side tools, and admin panels. Contains all protocol logic: Norito codec, transaction building, client transport, connect protocol.

- **`client-android`** — Android keystore integration, hardware-backed key generation, device telemetry, and the shared JNI bridge used for ML-DSA-65 signing. Depends on `core-jvm` via `api()` — consumers get all core types transitively.

### Null safety

The Java SDK required defensive null checks at every Kotlin call site (`!!`, `?:`, `?.let {}`). Kotlin's type system makes nullability explicit — parameters that accept null are declared `T?`, everything else is guaranteed non-null by the compiler. This removes most `NullPointerException` risks from consumer apps. Some risk remains at Java interop boundaries (BouncyCastle, JCA) where platform types (`T!`) may hide nullability.

### Testability without Android

`core-jvm` runs on any JVM. Consumers can unit-test transaction building, address encoding, signing, and Norito serialization with plain JUnit — no Android instrumentation, no Robolectric, no emulator.

## Side Dependencies

| Dependency | Version | Used By | Risk |
|-----------|---------|---------|------|
| `org.bouncycastle:bcprov-jdk18on` | 1.78.1 | `core-jvm` crypto, connect, and deterministic key export | **Binary compatibility** — BouncyCastle releases are not always backward-compatible. Consumer apps that force a different BC version may hit linkage errors at runtime. The SDK links the pinned provider directly and fails clearly when the mandatory implementation is broken; it never probes BouncyCastle through reflection. |
| `com.github.luben:zstd-jni` | 1.5.7-7 | `core-jvm` (Norito compression) | **Native library** — zstd-jni bundles platform-specific `.so`/`.dylib`. On Android, the JNI natives may conflict with other zstd consumers. Compression requires the native library to be available. |
