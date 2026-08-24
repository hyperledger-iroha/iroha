# Iroha Kotlin SDK

Kotlin rewrite of the `iroha_android` and `norito_java` for Hyperledger Iroha 3.

## Artifacts

Not published to Maven Central yet. Build locally and consume via `mavenLocal()`.

| Artifact | Type | Description |
|----------|------|-------------|
| `org.hyperledger.iroha.sdk:core-jvm` | JAR | Pure Kotlin/JVM models, codec, crypto, clients, and ABI-21/V4 artifact streaming |
| `org.hyperledger.iroha.sdk:client-android` | AAR | Android keystore, device telemetry, IrohaKeyManager, shared JNI bridge for ML-DSA / offline flows |
| `org.hyperledger.iroha.sdk:offline-wallet-android` | AAR | Offline-wallet integration built on `client-android`; use this artifact for Android offline cash |

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
pipeline status. Never resend the same signed bytes. `RetryPolicy` applies only to caller-managed
replay-safe reads, and configured pending queues are explicit local staging: submission neither
fills nor drains them.

Public pipeline status contains only the canonical transaction hash, closed status kind,
optional committed height, read scope, and resolution source. The parser rejects rejection
text, diagnostics, trigger completions, batch outcomes, unknown kinds, and noncanonical
metadata. Detailed transaction reads require an involved account or operator to send a
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

### Offline peer transports

`core-jvm` owns the portable IPM1 wire, bounded multi-stream IQR1 scanner,
authenticated IPN1 session, and NFC V1 APDU/durable-checkpoint state machines.
`client-android` owns the Android IsoDep, HCE, and Google Nearby Connections
adapters; `offline-wallet-android` depends on and re-exports that Android
surface. The application entry points are:

- Wire/QR: create `IrohaPeerWireMessageV1`, render with
  `IrohaPeerQRCodecV1.encode(...)`, and ingest camera results with
  `IrohaPeerQRScanSessionV1.ingest(...)`. Bind optional expected profile, kind,
  and schema in the scan-session constructor, then reset when capture stops.
  After application-domain rejection of a structurally valid completion, call
  the bounded `scanSession.quarantine(streamId)` API before resuming capture.
  Scan input is exact IQR1 text; leading or trailing whitespace is rejected.
  Three active streams, twelve pre-header frames, 3,072 pre-header bytes,
  30 seconds idle, and 180 seconds absolute are hard V1 ceilings; custom
  policies may only tighten them.
- Nearby: authenticate IPN1 with `IrohaPeerNearbySessionV1` and own radio
  lifecycle with `IrohaPeerNearbyConnectionsTransportV1.startAdvertising`,
  `startDiscovering`, `send`, and `stop`.
- NFC: use `IrohaPeerIsoDepTransceiverV1` with
  `IrohaPeerNfcReaderExchangeV1.run(...)` for reader mode; use one stable
  `IrohaPeerNfcReceiverApduBridgeV1` behind an
  `IrohaPeerAsyncHostApduServiceV1` subclass for HCE.

Google Nearby is pinned to `play-services-nearby:19.3.0`, uses only
point-to-point service `org.hyperledger.iroha.offline.transfer.v1`, requires a
human decision over Google's 4-to-12 ASCII verification digits, uses strict canonical
Base64URL-no-padding ASCII IPD1 discovery with sender-only zero bootstrap and
receiver-context adoption, accepts only BYTES,
and completes sends only at terminal `PayloadTransferUpdate.Status.SUCCESS`. Repeated
or conflicting lifecycle starts preserve the live operation until an explicit
stop, and every timer/payload callback is bound to its activation epoch so a
stale callback after restart cannot resolve or poison the new transfer.
IPN1 plaintext records are capped at 32,704 bytes; the encrypted record must
remain within 32 KiB after its 54-byte framing overhead, and adapters reserve
64 bytes by default. Authentication records are capped at 32 KiB, operation
timeouts at 300 seconds, and one receive phase admits the four-record V1
transcript. Callback executor submission is deferred until after releasing the
lifecycle monitor, so even a direct executor never runs application code in a
transport state lock. Epoch invalidation suppresses callbacks not yet admitted;
an already-admitted callback may finish.
Listener callbacks reject bounded overload. Terminal send completions remain
exact-once through a separately bounded serial fallback; saturation of both a
stalled configured executor and that fallback uses a nonblocking inline path,
which cannot promise the configured context or global FIFO order.

The merged library manifest declares version-bounded Wi-Fi, Bluetooth,
location, nearby-device, local-network, and optional NFC/HCE capabilities.
Legacy Wi-Fi state/change permissions end at API 31; Google Nearby's manifest
contract starts `NEARBY_WIFI_DEVICES` at API 32 with `neverForLocation`.
Applications must request every applicable dangerous permission at runtime;
the SDK never interprets a missing permission as permission to fall back to an
unauthenticated transport. Android 37+ consumers must request
`ACCESS_LOCAL_NETWORK` when required by the platform.

Concretely, the AAR merges `NFC`, legacy `ACCESS_WIFI_STATE` /
`CHANGE_WIFI_STATE`, legacy `BLUETOOTH` / `BLUETOOTH_ADMIN`,
`ACCESS_COARSE_LOCATION` through API 31, `ACCESS_FINE_LOCATION` on APIs 29–31
(requested together with coarse location on Android 12),
`BLUETOOTH_ADVERTISE` / `BLUETOOTH_CONNECT` / `BLUETOOTH_SCAN` from API 31,
`NEARBY_WIFI_DEVICES` from API 32, and `ACCESS_LOCAL_NETWORK` from API 37.
Before starting a rail, request the permissions from that list that are both
dangerous on the running OS and needed by the role; a discoverer needs scan, an
advertiser needs advertise, and an established connection needs connect.
`NFC` and the legacy Wi-Fi/Bluetooth state declarations are manifest-only.
Camera QR capture is app-owned, so a camera-based UI must separately declare
and request `android.permission.CAMERA`.

For HCE, subclass `IrohaPeerAsyncHostApduServiceV1`, declare that concrete
service with `android.permission.BIND_NFC_SERVICE`, and reference
`@xml/iroha_peer_nfc_v1_aids`. Return one stable
`IrohaPeerNfcReceiverApduBridgeV1` from the service's `commandHandler`
property. Its COMMIT response remains pending until the application has
durably stored the exact payment outcome and IDA1 ACK. One process-wide,
queue-free worker owns at most one five-second durability lease. RF deactivation
or `reset()` detaches that tap's response without starting another callback;
the accepted callback may finish until its deadline, but operation and
activation identities prevent it from installing or replying into a later tap.
Neither path discards receiver protocol or durable state. A late successful
write remains durable and the next tap restores IPA1/IDA1 from the idempotent
store.

BEGIN_PAYMENT is protected by the same five-second boundary. Storage receives
an ephemeral `IrohaPeerNfcPaymentAdmissionContextV1` and atomically returns a
distinct `IrohaPeerNfcDurablePaymentAdmissionV1` after storing its exact
244-byte IPA1 encoding. Restore IPA1 directly—never reconstruct it from
projected fields. A restored admission reports zero received bytes so a retap
rewrites safely; IDA1 wins after COMMIT and later BEGIN_PAYMENT is rejected.
Both callbacks must be idempotent because timeout makes persistence outcome
ambiguous and late completion is deliberately suppressed in memory.

The application manifest declaration for that subclass is:

```xml
<service
    android:name=".OfflinePeerHostApduService"
    android:exported="true"
    android:permission="android.permission.BIND_NFC_SERVICE">
    <intent-filter>
        <action android:name="android.nfc.cardemulation.action.HOST_APDU_SERVICE" />
    </intent-filter>
    <meta-data
        android:name="android.nfc.cardemulation.host_apdu_service"
        android:resource="@xml/iroha_peer_nfc_v1_aids" />
</service>
```

For reader mode, pass `IrohaPeerIsoDepTransceiverV1.localLimits` and
`transceiveForReader` to `IrohaPeerNfcReaderExchangeV1`. The runner reads a
fresh request, then calls
`IrohaPeerNfcSenderCheckpointStoreV1.loadOrCreateDurableCheckpoint` as one
atomic load-or-create/debit/store boundary. The exact durable checkpoint is
validated against the request and peer before BEGIN_PAYMENT, so a store failure
sends no BEGIN_PAYMENT and a restart cannot create a second debit. The separate
`IrohaPeerNfcSenderCheckpointUpdaterV1.updateDurableCheckpoint` persists the
ACK-bearing ISC1 before CONFIRM_ACK; its failure sends no confirmation. The
runner reconciles every retap with GET_STATUS, bursts successful value chunks,
and returns immediately after confirmation. Short-APDU
devices cap WRITE data at 203 bytes after V1 metadata; extended-capable
same-platform peers may negotiate up to 4,096 bytes. The planning and reducer
types remain available for applications that need lower-level orchestration.
The default whole-exchange budget is 73,996 actions: enough for three
protocol-maximum messages at one-byte chunks plus all phase controls, while a
smaller caller-supplied budget can fail hostile tiny-chunk peers before value
creation. One NFC profile policy binds request, payment, and acknowledgement to
the same profile; mixed-profile sessions fail closed. A complete NFC IPM1 value
is capped at 24,660 bytes. That is a hard constructor ceiling. Wire policies
likewise cannot exceed 32 KiB canonical or 24,576 encoded bytes for the bounded
Kagemusha handoff.

IPM1 admits only profile `2` / schema `0x0102` as a 24,576-byte bounded
handoff for a mainline typed Kagemusha native archive. Generic IPM validates
its exact ABI21 envelope without native code; `IrohaPeerKagemushaAdapterV1`
then performs deeper typed semantic decoding. Full ABI21
QR/NFC/native archives up to 32 MiB continue to use the independent
`KagemushaQrStreamCodec`, `KagemushaNfcProtocol`, and
`KagemushaNearbyEnvelopeCodec` rails. Kagemusha retains its distinct
`PKK2*`/`PKKQ1` text and Bonjour identifiers, while NFC uses the sole canonical
AID `F0504B45504B524E464301`. Nearby uses the authenticated binary `PKNB1`
envelope and its own smaller bound. Those rails are never negotiated,
reinterpreted, or used as fallback for IPM1. `IrohaPeer*V1` has no
unauthenticated Nearby, raw-text, alternate profile representation, alias, or
migration fallback.

These transport changes are client-side and require no backend API change.

The sole first-release IPM1 profile code 2 requires schema `0x0102`.
Construction and decode enforce native-independent ABI21 NRT0 framing, the
authoritative fully-qualified kind schema, CRC64, exact compact-length flags,
and static padding (request/payment 8, ACK 0). Deeper semantics remain in the
typed adapter.
`PEER_OPTIMIZED` compression is cross-rail and
uses zlib only when it saves at least 32 bytes and one 256-byte shard.

Do not stop at generic structural acceptance for a production profile-2
payload; wrap and decode it through `IrohaPeerKagemushaAdapterV1` so deeper
typed semantics are enforced. The canonical
`../fixtures/offline/kagemusha_peer_transport_v2.json` vector
pins a qualified 49-byte structural archive and its exact IPM1, IQR1, NFC, and
authenticated Nearby bytes across Swift, Kotlin, and Java. Its one-byte body is
not semantically valid and must not be passed to the typed adapter. From this
directory, run both portable and Android adapter coverage with:

```bash
./gradlew :core-jvm:test \
  --tests 'org.hyperledger.iroha.sdk.offline.IrohaPeer*' --console=plain
./gradlew :client-android:testDebugUnitTest \
  --tests 'org.hyperledger.iroha.sdk.offline.IrohaPeer*' --console=plain
```

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
only `escrow_id` plus `expected_remaining_amount` under the registered
`iroha_data_model::isi::escrow::CancelAssetLock` Norito wire name. The lock-ID
preimage must be nonempty exact text without surrounding whitespace or a BOM
and is bounded by `CancelAssetLockInstruction.MAX_LOCK_ID_UTF8_BYTES_V1`
(4,096 UTF-8 bytes, not characters); the on-wire `EscrowId` remains 32 bytes.
The retired one-field shape, aliases, extra fields, zero, and alternate numeric
spellings are rejected; stale expected amounts are rejected atomically by the
ledger.

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

ABI V1 exposes no generic shield, shielded-transfer, or unshield instruction or
native signer method; confidential movement uses the typed Kagemusha lifecycle.
`core-jvm` exposes the exact ABI-21/V4 typed Kagemusha init, fractional append/change,
verification, and redemption builders through the fixed native surface. It also provides exact
scaled amounts, V4 artifact streaming and backend-capability checks, plus the sole current
`DeviceAttestationRegistration` / `RegisterOfflineDeviceAttestation` transaction path. The latter
validates finalized platform material and emits exactly one native registration instruction.

The clean Offline Cash V1 lifecycle additionally requires one rollback-resistant
intent slot, an exact-next monetary counter, trusted time, authenticated terminal
recovery, and an authenticated staged-payment outbox. Android KeyMint's one-use
key API does not provide that atomic state machine. Call
`OfflineCashDeviceLifecycleBridgeV1.production()` to discover the complete
optional native contract; missing symbols or any partial capability produce
`ONLINE_ONLY`, with no KeyMint-only, TEE, or software fallback. The bridge accepts
only the bounded V1 command frame and does not expose an old V4/V5 selector.
See [`specs/offline_cash_device_bridge_v1.md`](../specs/offline_cash_device_bridge_v1.md)
for exact frame offsets, feature bits, and optional native entry points.

Artifact streaming installs
exactly eight Pasta artifacts atomically: `ParamsIPA`, processed proving key, processed verifying
key, and final-key selector-zero bootstrap witness for each Eq/Ep parity. Each profile's bounded
circuit parameters are authenticated inline in the V4 manifest, not streamed as a ninth or tenth
artifact. The top-up-finality roster is authenticated release metadata outside the exact eight-role
cryptographic inventory. `ReleaseAuthentication` also requires the canonical candidate-bound
promotion record alongside the trusted policy, attestation, benchmark evidence, and cryptographic
review; an authenticated-but-unpromoted release cannot be installed.

`KagemushaRecursiveSpendProver.newToriiClient` requires a genesis-derived
`LocalSigningContext`. Its receiver-lineage method additionally requires a per-call
`ToriiCanonicalRequestAuth` and signs the exact NetworkId, POST path, and Norito selector body with
fresh metadata; the resulting transport request is one-shot and cannot be redirected or retried.

Lifecycle calls fail closed until the proof backend and the exact manifest-bound artifact set are
available. Request and result archives stay typed and canonically framed while recursive proof,
membership, note-opening, and accumulator details remain native-owned opaque bytes.
The protocol and JVM append builder accept one or two inputs and support up to eight peer hops.
Inputs are canonicalized by authenticated bundle digest; duplicate or conflicting exact-state
branches fail closed. `getOfflineCapability` takes no selector and accepts only
the four-field `cash_handoff_v1` response: bridge ABI 22, maximum hop count 8,
and `ready=true`. Asset scale, committed snapshot, verifier identities, and
release bindings are supplied through the exact command and proof types that
consume them rather than a separate readiness archive.
`prepareTopUp` accepts only Torii's authoritative `next_zero_path` and retains the local note
opening. Init results do not yet carry a proof-bound output membership witness, so the JVM surface
intentionally does not project or restore a spendable init branch. Persisted openings and
submission archives use typed decoders so idempotent retries reuse exact canonical bytes.
Secret-bearing append and redeem requests are single-use and zeroized after native consumption.
Each projected branch carries its complete ordered exact-state claim set and authenticated V4
artifact binding. Native `conflictsWith` compares every claim pair, rejecting equality and
ancestor/descendant overlap while allowing the two consistent sibling outputs from one split;
wallet code never parses lineage paths.
Torii command bodies use distinct exact ceilings: 512 KiB for top-up and 48 MiB for redemption,
exposed as `MAX_TORII_TOP_UP_REQUEST_BYTES_V4` and
`MAX_TORII_REDEEM_REQUEST_BYTES_V4`.

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

The registry has exactly twelve IDs: `zk-ace-pq-authorization-v0`,
`anonymous-pgc-k-out-of-n-v1`, `verange-transparent-range-v1`,
`iroha-zk-ams-v1`, `vega-existing-credential-zk-v0`,
`iroha-zk-x509-stark-p256-v0`,
`iroha-jindo-polynomial-commitment-v0`,
`iroha-bootle-lantern-anoncred-v1`, `orchard-halo2-actions-v1`,
`monero-fcmp-plus-plus-v1`, `iroha-ivm-private-note-stark-v1`, and
`pq-masp-stark-v0`. Parsing is exact: aliases, retired IDs, case changes, and
whitespace normalization fail closed.

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

The `libconnect_norito_bridge.so` files are **not tracked in git** — they are built from the Rust crate at `crates/connect_norito_bridge` in the same iroha repository. The Gradle task now lives on `client-android`, which owns the shared native bridge used for ML-DSA signing and the typed Kagemusha lifecycle/artifact streaming. It defaults to `../..` as the iroha root (override via `iroha.dir` in `local.properties` if needed).

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
tests. An unchanged raw build is reusable only while its saved source seal still
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
registry fields, height ranges, and inline verifier-key commitments before
sending the request. Neither request accepts or transmits a private key. Torii
returns HTTP 200 with an unsigned transaction draft. SDK `Signer`
implementations apply Iroha's prehash themselves, so pass
`transactionPayloadBytes()` to `Signer.sign`; use `signingMessageBytes()` only
with a raw/HSM primitive that signs an already-prehashed message. Attach the
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

The original Java SDK used reflection in multiple places (Android API discovery, BouncyCastle loading, keystore operations). Kotlin production sources are reflection-free across `core-jvm`, `client-android`, and `offline-wallet-android`; `scripts/check_kotlin_no_reflection.sh` enforces that contract. BouncyCastle is linked directly, Android keystore APIs are guarded by platform-version checks, and WebSocket clients require callers to inject a connector with `setWebSocketConnector(...)` instead of discovering one at runtime.

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
| `org.bouncycastle:bcprov-jdk18on` | 1.78.1 | `core-jvm` crypto, connect, and deterministic key export | **Binary compatibility** — BouncyCastle releases are not always backward-compatible. Consumer apps that force a different BC version may hit linkage errors at runtime. The SDK links the pinned provider directly and fails clearly when the mandatory implementation is broken; it never probes BouncyCastle through reflection. |
| `com.github.luben:zstd-jni` | 1.5.7-7 | `core-jvm` (Norito compression) | **Native library** — zstd-jni bundles platform-specific `.so`/`.dylib`. On Android, the JNI natives may conflict with other zstd consumers. Compression requires the native library to be available. |
