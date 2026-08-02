# IrohaAndroid

`IrohaAndroid` provides a native Android library that wraps Hyperledger Iroha
capabilities for Kotlin/Java mobile applications. The library includes key
management (including secure-element-backed keys), transaction building and
signing, Norito serialization helpers, and networking clients for interacting
with Iroha nodes.

The current API covers the offline key management façade, Norito encoding backed
by the shared `norito-java` implementation, the Android Keystore/StrongBox
backend (with cached attestations + explicit deterministic software providers), and
network client abstractions. The instruction helpers include RWA lot builders
alongside NFT helpers:
`RegisterRwaInstruction`, `TransferRwaInstruction`,
`MergeRwasInstruction`, `RedeemRwaInstruction`,
`FreezeRwaInstruction`, `UnfreezeRwaInstruction`,
`HoldRwaInstruction`, `ReleaseRwaInstruction`,
`ForceTransferRwaInstruction`, `SetRwaControlsInstruction`,
and RWA-aware metadata setters/removers.

## Gradle quickstart

Point Gradle at the repository that hosts the SDK (the default
`ci/publish_android_sdk.sh` output lives under `artifacts/android/maven`) and
depend on the surface you need:

```kotlin
repositories {
    google()
    mavenCentral()
    maven { url = uri("../../artifacts/android/maven") } // or your Nexus
}

dependencies {
    implementation("org.hyperledger.iroha:iroha-android:<version>") // Android AAR
    implementation("org.hyperledger.iroha:iroha-android-jvm:<version>")     // JVM tooling
}
```

The sample app (`:samples-android`) can validate AAR consumption from a local
repository with:

```bash
./gradlew -p java/iroha_android :samples-android:assembleDebug \
  -PirohaAndroidUsePublished=true \
  -PirohaAndroidRepoDir=$PWD/../artifacts/android/maven
```

It uses the local snapshot repository when it exists and otherwise uses the
in-repo project dependency. Set `irohaAndroidVersion` to match the
published coordinates when consuming from Maven.

The Java peer facades reuse the Kotlin SDK artifacts published as
`org.hyperledger.iroha.sdk:core-jvm` and `client-android`. Their Maven version
is pinned by `irohaKotlinSdkVersion` in `gradle.properties`; update that pin
when publishing a new compatible Kotlin transport release. An explicit
`-PirohaKotlinSdkVersion` overrides the pin, followed by the optional shared
`irohaSdkVersion` property and finally `irohaAndroidVersion` when the more
specific properties are absent. Composite builds substitute the in-tree
Kotlin projects while preserving these exact coordinates in generated POMs.

## DA commitment and pin-intent proofs

`HttpClientTransport.newDaToriiClient()` returns the Java typed DA client. It
covers the proof-policy, commitment list/prove/verify, and pin-intent
list/prove/verify routes. Use `DaModels.Digest32` for manifest and storage-ticket
digests and `BigInteger` for unsigned 64-bit counters.

```java
final DaToriiClient da = transport.newDaToriiClient();
DaModels.PinIntentListResponse page =
    da.listPinIntents(
            new DaModels.PinIntentListRequest(BigInteger.valueOf(100), null))
        .join();
while (page.nextCursor() != null) {
  page =
      da.listPinIntents(
              new DaModels.PinIntentListRequest(
                  BigInteger.valueOf(100), page.nextCursor()))
          .join();
}
final DaModels.PinIntentProof proof =
    da.provePinIntent(
            new DaModels.PinIntentQueryRequest(
                null,
                DaModels.Digest32.fromHex(ticketHex),
                null, null, null, null))
        .join();
if (proof != null) {
  assert da.verifyPinIntent(proof).join().valid();
}
```

List routes use immutable, server-issued snapshot cursors and return an explicit
nullable `nextCursor()`. Proof routes use separate selector-only request types;
pagination fields are not accepted by proof requests.

Responses are decoded into closed typed models. The client rejects unknown
fields, malformed transparent byte wrappers, invalid checksummed hashes,
contradictory verification responses, and Merkle paths whose direction/length
does not match the advertised bundle location. Requests are capped at 64 KiB
and buffered responses at 8 MiB.

## Offline peer transport V1 (Java)

The first-release peer transport has one wire family only: IPM1 messages,
IQR1/IRQR QR frames, authenticated IPN1 Nearby records, and the NFC application
identifier `F0504B45504B524E464301`. The new `IrohaPeer*V1` APIs never fall
back to the older Kagemusha QR, Nearby service, or NFC APDU formats.

Create the common envelope with `IrohaPeerWireMessageV1`. QR senders call
`IrohaPeerQRCodecV1.encode(...)`; receivers feed camera text to
`IrohaPeerQRScanSessionV1.ingest(...)`. The scan session is bounded and its default
policy keeps at most three streams, limits header-last buffering, expires both
idle and absolute-age state, removes successful streams, and quarantines a
stream after conflicting frames. Bind optional expected profile, kind, and
schema in the scan-session constructor, and call `reset()` whenever the camera
session changes. After application-domain rejection of a structurally valid
completion, call the bounded `scanSession.quarantine(streamId)` API before
resuming capture. Scan input is exact IQR1 text; leading or trailing whitespace
is rejected. Camera capture belongs to the application, so declare and request
`android.permission.CAMERA` when the UI uses it.
Those defaults are hard V1 ceilings: three active streams, twelve pre-header
frames, 3,072 pre-header bytes, 30 seconds idle, and 180 seconds absolute.
Custom limit objects may only tighten them.

Google Nearby Connections is pinned to `19.3.0` and uses exact service ID
`org.hyperledger.iroha.offline.transfer.v1` with
`Strategy.P2P_POINT_TO_POINT`. Create discovery records through
`IrohaPeerNearbyAndroidV1.discoveryContext(...)` (or the sender-only bootstrap
sentinel), require the listener's 4-to-12 ASCII-digit verification decision, and feed
received BYTES only into `IrohaPeerNearbySecureChannelV1`. The transitive Kotlin
`IrohaPeerNearbyConnectionsTransportV1` owns advertising, discovery, send, and
stop lifecycle. A send completion is success only after the exact payload's
framework update is terminal `PayloadTransferUpdate.Status.SUCCESS`; enqueue or
connection acceptance is never delivery success.
IPN1 plaintext records are capped at 32,704 bytes; the encrypted record must
remain within 32 KiB after its 54-byte framing overhead, and adapters reserve
64 bytes by default.
Authentication records are capped at 32 KiB, operation timeouts at 300
seconds, and one receive phase admits the four-record V1 transcript. Android
defers callback-executor submission until after releasing the lifecycle
monitor, including for an injected direct executor. Epoch invalidation
suppresses callbacks not yet admitted; an already-admitted callback may finish.
Listener callbacks reject bounded overload. Terminal send completions remain
exact-once through a separately bounded serial fallback; saturation of both a
stalled configured executor and that fallback uses a nonblocking inline path,
which cannot promise the configured context or global FIFO order.

For NFC reader mode, `IrohaPeerAndroidNfcV1.transceiver(tag)` derives local
read/write limits from `IsoDep.maxTransceiveLength` and extended-length
support; a short WRITE chunk is capped at 203 bytes. HCE applications use
`IrohaPeerAndroidNfcV1.receiverBridge(...)`. Its COMMIT callback must persist
the exact payment outcome and IDA1 acknowledgement before completing; only
then can the bridge return status `9000`. The two-tap sender's
`IrohaPeerNfcSenderCheckpointStoreV1.loadOrCreateDurableCheckpoint` callback
atomically loads an exact request- and peer-bound ISC1 or creates, debits, and
stores it, returning only the durable value. The runner validates it before
BEGIN_PAYMENT, so store failure sends no BEGIN_PAYMENT and restart cannot debit
a replacement payment. The separate
`IrohaPeerNfcSenderCheckpointUpdaterV1.updateDurableCheckpoint` callback
installs the ACK-bearing ISC1 before CONFIRM_ACK; update failure sends no
confirmation. The sender treats GET_STATUS as authoritative after a retap.
Use `IrohaPeerNfcV1.runReaderExchange(...)` for the complete shared Kotlin
runner; Java supplies only callbacks for transceive, atomic checkpoint
load-or-create, and the monotonic ACK update, so it cannot drift into a second
NFC state machine. One NFC profile policy binds request, payment, and
acknowledgement to the same profile; mixed-profile sessions fail closed. A
complete NFC IPM1 value is capped at 24,660 bytes, and the portable runner's
whole-exchange default is 73,996 actions.
The bridge's separate BEGIN callback receives a transient payment-admission
context and must atomically store and return a
`IrohaPeerNfcDurablePaymentAdmissionV1` containing the exact 244-byte IPA1.
Restore that record with the Java facade; never reconstruct it from summary
fields. Both BEGIN and COMMIT callbacks have a five-second fail-closed deadline
and must be idempotent because a timeout makes the durable result ambiguous.
Late callbacks cannot mutate a newer tap. IPA1 resumes at byte zero, while IDA1
takes precedence after COMMIT and ACK-phase BEGIN is rejected.
The NFC value and the 32-KiB canonical and 24,576-byte encoded-body limits for
bounded Kagemusha handoffs are hard constructor ceilings.

The AAR merges the version-bounded Nearby/NFC permissions and ships
`@xml/iroha_peer_nfc_v1_aids`. A wallet must still register its concrete HCE
service explicitly. The service subclasses the transitive Kotlin
`IrohaPeerAsyncHostApduServiceV1` and returns one stable bridge from its
`commandHandler` property:

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

The merged manifest includes `NFC`, legacy `ACCESS_WIFI_STATE` /
`CHANGE_WIFI_STATE`, legacy `BLUETOOTH` / `BLUETOOTH_ADMIN`,
`ACCESS_COARSE_LOCATION` through API 31, `ACCESS_FINE_LOCATION` on APIs 29–31
(requested together with coarse location on Android 12),
`BLUETOOTH_ADVERTISE` / `BLUETOOTH_CONNECT` / `BLUETOOTH_SCAN` from API 31,
`NEARBY_WIFI_DEVICES` from API 32, and `ACCESS_LOCAL_NETWORK` from API 37.
Before starting a rail, the host app requests every applicable dangerous
permission for the running OS and role. Discoverers need scan, advertisers need
advertise, established connections need connect, and Android 37+ needs local
network permission when the platform requires it. `NFC` and legacy
Wi-Fi/Bluetooth state permissions are manifest-only; a missing permission is
never a reason to use an unauthenticated fallback.

The Java build consumes the default SDK's pure-JVM NFC/IPN1 state machines and
Android radio adapters through explicit Gradle composite substitutions during
repository development. Published artifacts carry the equivalent transitive
dependencies, so Java and Kotlin do not maintain divergent cryptographic or
APDU implementations. The shared vector is
`../../fixtures/offline/kagemusha_peer_transport_v2.json`.

IPM1 admits only profile `2` / schema `0x0102` as a 24,576-byte bounded
handoff for a mainline typed Kagemusha native archive. Generic IPM validates
its exact ABI21 envelope without native code; production code then performs
deeper semantic decoding through `IrohaPeerKagemushaAdapterV1`. Full ABI21
QR/NFC/native archives up to 32 MiB continue to use the independent
`KagemushaQrStream`, `KagemushaNfcProtocol`, and `KagemushaNearby`
facades. Kagemusha retains its distinct `PKK2*`/`PKKQ1` text and Bonjour
identifiers, while NFC uses the sole canonical AID
`F0504B45504B524E464301`. Nearby uses the authenticated binary `PKNB1`
envelope and its own smaller bound. Those rails are never negotiated,
reinterpreted, or used as fallback for IPM1. The
no-raw-text/no-unauthenticated-Nearby rule applies to `IrohaPeer*V1`; the
retained ABI21 family also has no old AID. Do not use profile `2` for a
sidecar/demo representation.

These transport changes are client-side and require no backend API change.

The sole first-release IPM1 profile code 2 requires schema `0x0102`.
Construction and decode enforce native-independent ABI21 NRT0 framing, the
authoritative fully-qualified kind schema, CRC64, exact compact-length flags,
and static padding (request/payment 8, ACK 0). Deeper semantics remain in the
typed adapter.
`../../fixtures/offline/kagemusha_peer_transport_v2.json` additionally pins a
qualified 49-byte structural archive through exact IPM1, IQR1, NFC, and
authenticated Nearby bytes in Swift, Kotlin, and Java. Its one-byte body is
structural-only and must not be passed to the typed adapter.
`PEER_OPTIMIZED` compression is cross-rail and
uses zlib only when it saves at least 32 bytes and one 256-byte shard.

From `java/iroha_android`, the normal checks below exercise the Java facades,
shared fixture parity, Kotlin dependency wiring, manifest contract, and Android
adapters:

```bash
./gradlew :core:check :android:testDebugUnitTest
```

For a fast portable peer-only iteration, use:

```bash
./gradlew :core:test \
  --tests 'org.hyperledger.iroha.android.offline.IrohaPeer*'
```

## Fee quotes and sponsorship

Every transaction payload requires a typed `FeePaymentIntent`. Authority-paid
and exact sponsor-program selections are explicit:

```java
import java.util.Collections;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.FeeSponsorProgramId;

FeePaymentIntent authorityPaid =
    FeePaymentIntent.authority(Collections.emptyList());
FeePaymentIntent sponsored =
    FeePaymentIntent.sponsor(
        new FeeSponsorProgramId(sponsorAccountId, "wallet_payments"),
        3L,
        Collections.emptyList());
```

An empty charge-limit list is only an initial quote draft. Freeze the complete
unsigned payload, put `requested.toJsonMap()` in its required `fee_payment`
field, and call `HttpClientTransport.quoteFees(unsignedPayload, canonicalAuth)`.
Verify that the quote retained the payer, exact sponsor program/revision, and
gas bound; replace only `fee_payment` with the returned intent before signing
and submitting the same payload. Use
`HttpClientTransport.getFeeSponsorProgram(programId, canonicalAuth)` to inspect
one exact lifecycle record before selecting its revision. Contract/IVM drafts
require a positive gas bound in the intent.

The metadata keys `fee_sponsor`, `gas_asset_id`, and `gas_limit` are retired and
rejected. Sponsor rejection never falls back to the authority.

## Atomic mixed executable batches

`Executable.batch(...)` preserves an exact interleaving of native instructions
and deployed-contract calls:

```java
Executable executable =
    Executable.batch(
        Arrays.asList(
            ExecutableBatchItem.instruction(registerInstruction),
            ExecutableBatchItem.contractCall(
                new ContractInvocation(
                    contractAddress, expectedCodeHash, "apply", argumentRecord)),
            ExecutableBatchItem.instruction(transferInstruction)));
```

The node applies the batch atomically. Empty batches are rejected. Contract
addresses must be canonical lowercase V1 Bech32m literals, and any batch that
contains a contract call must provide one positive signature-bound gas limit in
the `FeePaymentIntent`; these constraints are checked before signing.

## Native asset-lock cancellation

`CancelAssetLockInstruction` implements the V1 compare-and-cancel contract.
Supply the exact application lock ID and the positive canonical Quantity read
from finalized ledger state:

```java
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.instructions.CancelAssetLockInstruction;

CancelAssetLockInstruction cancel =
    CancelAssetLockInstruction.builder()
        .setLockId("appeal:case-42")
        .setExpectedRemainingAmount("20")
        .build();
InstructionBox instructionBox = cancel.toInstructionBox();
```

The builder derives the native escrow hash with Blake2b-256 and emits only
`escrow_id` plus `expected_remaining_amount` under the registered
`iroha_data_model::isi::escrow::CancelAssetLock` Norito wire name. The lock-ID
preimage must be nonempty exact text without surrounding whitespace or a BOM
and is bounded by `CancelAssetLockInstruction.MAX_LOCK_ID_UTF8_BYTES_V1`
(4,096 UTF-8 bytes, not characters); the on-wire `EscrowId` remains 32 bytes.
The retired one-field shape, aliases, extra fields, zero, and alternate numeric
spellings are rejected; stale expected amounts are rejected atomically by the
ledger.

## Account addresses

```java
import org.hyperledger.iroha.android.address.AccountAddress;

byte[] key = new byte[32];
AccountAddress address = AccountAddress.fromAccount(key, "ed25519");
System.out.println(address.canonicalHex());
System.out.println(address.toI105(753));

AccountAddress.DisplayFormats formats = address.displayFormats();
System.out.println(formats.i105);
System.out.println(formats.i105Warning);
```

Use `displayFormats()` whenever UI layers need to render or copy addresses so the warning text and
network prefix stay aligned with `specs/sns/address_display_guidelines.md`.

## Kagemusha proof artifacts and device registration

The Android/JVM offline surface has exactly two current pieces. `KagemushaRecursiveSpendProver`
requires native bridge ABI 21, streams the eight authenticated V4 proof artifacts into an atomic
generation install, and exposes typed `initSpendV4`, `appendSpendV4`, `verifySpendV4`, and
`buildRedeemV4` calls over the fixed native exports. `KagemushaScaledAmount` converts decimal input to positive
`u128` atomic units exactly at the authoritative asset scale and never rounds. The standalone
`DeviceAttestationRegistration` plus `RegisterOfflineDeviceAttestation` path validates finalized
KeyMint/App Attest material and builds the exact one-instruction on-chain registration transaction.
Android products remain fail-closed until the native proof backend reports available and the
matching artifact generation is installed. The exact external inventory is `ParamsIPA`, processed
proving key, processed verifying key, and final-key selector-zero bootstrap witness for each Eq/Ep
parity. Bounded circuit parameters are authenticated inline in the V4 manifest, not streamed as
extra artifacts. The protocol and JVM append builder accept one or two
inputs and enforce an eight-peer-hop ceiling natively. Inputs are canonicalized by authenticated
bundle digest; duplicate or conflicting exact-state branches fail closed. Peer request and
acknowledgement signing expose only strict P-256 device key/signature wrappers; callers never pass
native wire discriminants.

Artifact installation requires the canonical candidate-bound promotion record through
`ReleaseAuthentication`, in addition to the trusted policy, attestation, benchmark evidence, and
cryptographic review. An authenticated-but-unpromoted release cannot become active.

`newToriiClient(...)` exposes the query-free, asset-neutral `getOfflineCapability`,
`getRecipientRegistrationLineage`, `submitTopUp`, `submitRedeem`, and `getOperation`. Commands send
the typed Norito request directly with `application/x-norito` and the signed lowercase operation id
as `Idempotency-Key`; responses must be typed Norito as well. Top-up
bodies are limited to 512 KiB and redemption bodies to 48 MiB, exposed as
`MAX_TORII_TOP_UP_REQUEST_BYTES_V4` and `MAX_TORII_REDEEM_REQUEST_BYTES_V4`.
`projectReadiness` returns the live asset scale, committed height/hash, all role-specific verifier
commitments and activation windows, and the required nullable authenticated `artifactSet`. A
present set binds the V4 generation, manifest, release-policy and release-attestation digests,
issuance window, proof-pair bound, and asset scale to exact logical roles
`kagemusha_recursive_step_eq_v4_verifier_record` and
`kagemusha_recursive_step_ep_v4_verifier_record` with circuits
`kagemusha-recursive-spend-step-eq-compact-layout-v5` and
`kagemusha-recursive-spend-step-ep-compact-lineage-v5`, respectively. An absent artifact set
requires both recursive records and backend construction to be unavailable with exactly one
`recursive_v4_registry_unavailable` or `recursive_v4_registry_malformed` blocker; a present set
forbids both. `proofBackendAvailable` reports exact backend construction independently.
`recursiveLineageSupported` additionally requires the authenticated artifact set and distinct
active Eq/Ep records, while `ready` is true only when the complete blocker set is empty.
`recursive_lineage_unavailable` is present exactly when lineage is false. `prepareTopUp` accepts Torii's authoritative
`next_zero_path`; the resulting recursive init persists its own native membership witness rather
than the earlier shield-tree witness. Typed decoders restore the opening and exact canonical
top-up/redemption submissions for idempotent restart retries. Secret-bearing append/redemption
build requests are single-use and zeroized when native proving consumes them.
Each projected branch carries its complete ordered exact-state claim set and authenticated V4
artifact binding. Native `conflictsWith` compares every claim pair, rejecting equality and
ancestor/descendant overlap while allowing the two consistent sibling outputs from one split;
applications never parse lineage paths.

## Native privacy bridge

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
`PrivacyExact12FixtureBundleV1` entirely in Java; it does not load the native
bridge. Pass one canonical standard-Base64 line (without the fixture file's
final LF), or decode raw Norito bytes directly:

```java
PrivacyExact12FixtureBundleV1 bundle =
    PrivacyExact12FixtureCodecV1.decodeCanonicalBase64(fixtureLine);
byte[] canonicalArchive = PrivacyExact12FixtureCodecV1.encodeCanonical(bundle);
PrivacyExact12FixtureCodecV1.requireCanonicalArchive(receivedArchive, canonicalArchive);
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

## Multisig specs and TTL preview

```java
import org.hyperledger.iroha.android.multisig.MultisigProposalTtlPreview;
import org.hyperledger.iroha.android.multisig.MultisigSpec;
import org.hyperledger.iroha.android.address.AccountAddress;

byte[] aliceKey = new byte[32];
byte[] bobKey = new byte[32];
String alice = AccountAddress.fromAccount(aliceKey, "ed25519").toI105(753);
String bob = AccountAddress.fromAccount(bobKey, "ed25519").toI105(753);
MultisigSpec spec =
    MultisigSpec.builder()
        .setQuorum(3)
        .setTransactionTtlMs(86_400_000L)
        .addSignatory(alice, 2)
        .addSignatory(bob, 1)
        .build();

MultisigProposalTtlPreview preview = spec.enforceProposalTtl(90_000L, System.currentTimeMillis());
System.out.println("effective ttl: " + preview.effectiveTtlMs());
System.out.println("expires at: " + preview.expiresAtMs());
```

`enforceProposalTtl` rejects TTL overrides above the policy cap (`transaction_ttl_ms`) before
submission so apps can surface the same error Torii would return. Use
`previewProposalExpiry` when you only need a preview (cap + expiry) without throwing.
When registering a multisig controller, supply an explicit canonical I105 account id for a random
controller key (the controller must never be used for direct signing). Nodes now quarantine
deterministically derived controller ids and will reject registration and subsequent
propose/approve attempts that use them.

```java
import java.util.Base64;
import org.hyperledger.iroha.android.model.InstructionBox;

InstructionBox registerMultisig =
    InstructionBox.fromWirePayload(
        "<WIRE_NAME_REGISTER_MULTISIG>",
        Base64.getDecoder().decode("<WIRE_PAYLOAD_BASE64>")); // Replace with wire payload bytes.
```

## Subscriptions

```java
import java.net.URI;
import java.util.LinkedHashMap;
import java.util.Map;
import org.hyperledger.iroha.android.client.SubscriptionToriiClient;
import org.hyperledger.iroha.android.numeric.NumericV1;
import org.hyperledger.iroha.android.subscriptions.SubscriptionCreateRequest;
import org.hyperledger.iroha.android.subscriptions.SubscriptionCreateResponse;
import org.hyperledger.iroha.android.subscriptions.SubscriptionPlanCreateRequest;
import org.hyperledger.iroha.android.subscriptions.SubscriptionPlanCreateResponse;
import org.hyperledger.iroha.android.subscriptions.SubscriptionUsageRequest;

SubscriptionToriiClient client =
    SubscriptionToriiClient.builder()
        .baseUri(URI.create("https://example.com"))
        .build();

Map<String, Object> plan = new LinkedHashMap<>();
plan.put("kind", "fixed");
plan.put("price", "120");
plan.put("period", "month");

SubscriptionPlanCreateResponse planResponse =
    client.createSubscriptionPlan(
            SubscriptionPlanCreateRequest.builder()
                .authority("<provider_account_i105>")
                .privateKey("<hex>")
                .planId("aws_compute#commerce")
                .plan(plan)
                .build())
        .join();

SubscriptionCreateResponse subscriptionResponse =
    client.createSubscription(
            SubscriptionCreateRequest.builder()
                .authority("<subscriber_account_i105>")
                .privateKey("<hex>")
                .subscriptionId("sub-001$subscriptions")
                .planId("aws_compute#commerce")
                .build())
        .join();

client.recordSubscriptionUsage(
        "sub-001$subscriptions",
        SubscriptionUsageRequest.builder()
            .authority("<provider_account_i105>")
            .privateKey("<hex>")
            .unitKey("compute_ms")
            .delta(NumericV1.QuantityValue.parseCanonical("3600000"))
            .build())
    .join();
```

## Verifying key registry

`HttpClientTransport` wraps Torii's `/v1/zk/vk/register` and
`/v1/zk/vk/update` routes. The helpers validate production verifier backends,
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
rejects non-canonical Norito, another chain or authority, extra/substituted
instructions, any mismatch in the complete verifying-key record, and signing
messages that do not match the payload prehash:

```java
ClientConfig config =
    ClientConfig.builder()
        .setLocalSigningContext(new LocalSigningContext("production-chain"))
        // Configure the Torii endpoint and other client policy here.
        .build();
HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);
byte[] vkBytes = new byte[] {1, 2, 3};

VerifyingKeyTransactionDraft registerDraft =
    transport
    .registerVerifyingKey(
        VerifyingKeyRegisterRequest.builder()
            .authority("<authority_i105>")
            .backend("halo2/ipa")
            .name("vk_main")
            .version(1L)
            .circuitId("halo2/ipa::transfer_v1")
            .publicInputsSchemaHashHex("a".repeat(64))
            .gasScheduleId("halo2_default")
            .verifyingKeyBytes(vkBytes)
            .status("Active")
            .build())
    .join();

VerifyingKeyTransactionDraft updateDraft =
    transport
    .updateVerifyingKey(
        VerifyingKeyUpdateRequest.builder()
            .authority("<authority_i105>")
            .backend("halo2/ipa")
            .name("vk_main")
            .version(2L)
            .circuitId("halo2/ipa::transfer_v1")
            .publicInputsSchemaHashHex("a".repeat(64))
            .status("Withdrawn")
            .build())
    .join();

if (registerDraft.submitted()) {
    throw new IllegalStateException("Torii must return an unsigned draft");
}
byte[] registerPayload = registerDraft.transactionPayloadBytes();
byte[] registerSigningMessage = registerDraft.signingMessageBytes();
byte[] registerSignature = signer.sign(registerPayload);
```

## Layout

```
java/iroha_android
├── src
│   ├── main/java
│   │   └── org/hyperledger/iroha/android
│   │       ├── IrohaKeyManager.java
│   │       ├── SigningException.java
│   │       ├── client
│   │       │   ├── ClientConfig.java
│   │       │   ├── ClientResponse.java
│   │       │   ├── HttpClientTransport.java
│   │       │   └── IrohaClient.java
│   │       ├── crypto
│   │       │   ├── KeyProviderMetadata.java
│   │       │   ├── Ed25519Signer.java
│   │       │   ├── SoftwareKeyProvider.java
│   │       │   └── keystore
│   │       │       ├── KeystoreBackend.java
│   │       │       ├── KeystoreKeyProvider.java
│   │       │       └── KeyGenParameters.java
│   │       ├── gpu
│   │       │   └── CudaAccelerators.java
│   │       ├── model
│   │       │   ├── Executable.java
│   │       │   └── TransactionPayload.java
│   │       ├── norito
│   │       │   ├── NoritoCodecAdapter.java
│   │       │   ├── NoritoException.java
│   │       │   ├── NoritoJavaCodecAdapter.java
│   │       │   └── TransactionPayloadAdapter.java
│   │       ├── offline
│   │       │   └── KagemushaRecursiveSpendProver.java
│   │       ├── subscriptions
│   │       │   ├── SubscriptionPlanCreateRequest.java
│   │       │   ├── SubscriptionCreateRequest.java
│   │       │   ├── SubscriptionListResponse.java
│   │       │   └── SubscriptionToriiException.java
│   │       └── tx
│   │           ├── SignedTransaction.java
│   │           └── TransactionBuilder.java
│   └── test/java
│       └── org/hyperledger/iroha/android
│           ├── IrohaKeyManagerTests.java
│           ├── client/HttpClientTransportTests.java
│           ├── crypto/keystore/KeystoreKeyProviderTests.java
│           ├── gpu/CudaAcceleratorsTests.java
│           ├── norito/NoritoCodecAdapterTests.java
│           └── tx/TransactionBuilderTests.java
├── src/test/resources
│   └── transaction_payloads.json
├── schemas
│   └── norito_schema_manifest.json
```

## Modules & dependencies

- `:core` — shared Java sources used by both targets.
- `:android` — Android AAR surface (OkHttp transport, keystore helpers) published as
  `org.hyperledger.iroha:iroha-android`.
- `:jvm` — JVM jar (java.net.http transport) published as `org.hyperledger.iroha:iroha-android-jvm`.
- `:samples-android` — launchable demo that exercises Norito encoding and address helpers against the
  AAR; build with `gradle -p java/iroha_android :samples-android:assembleDebug`.

When consuming the workspace via an included build:

```kotlin
dependencies {
    implementation(project(":android")) // Android apps
    implementation(project(":jvm"))     // JVM tooling
}
```

Published artefacts can be pulled directly when the composite build is not in play:

```kotlin
dependencies {
    implementation("org.hyperledger.iroha:iroha-android:${IROHA_ANDROID_VERSION}") // Android (AAR)
    implementation("org.hyperledger.iroha:iroha-android-jvm:${IROHA_ANDROID_VERSION}") // JVM
}
```

The `:samples-android` app defaults to the in-repo `:android` project but can
consume a published Maven repo by setting `irohaAndroidUsePublished=true`
(or `ANDROID_SAMPLE_USE_PUBLISHED=1`) and pointing `irohaAndroidRepoDir` at
`artifacts/android/maven/<version>` from `ci/publish_android_sdk.sh`.
`MainActivity` renders an IH58 address from the AAR, and `SampleAddressTest`
keeps the wiring green for the published-vs-project toggle.

## Build & Test

The Gradle harness targets JDK 21 and wires the included `norito-java` build automatically. From the
repository root:

```bash
bash ci/run_android_tests.sh
# or
make android-tests
# or run a subset of tasks
ANDROID_GRADLE_TASKS=":core:check :android:testDebugUnitTest" bash ci/run_android_tests.sh
```

Use `ANDROID_HARNESS_MAINS` (comma-separated class names) to filter the main-based harnesses, and
`ANDROID_GRADLE_TASKS` to override the Gradle task list.

`gradle -p java/iroha_android :core:check` runs the shared JUnit/parameterised harnesses with
assertions enabled, enforces the pinned Norito schema manifest (`verifyNoritoSchemas`), and calls
`checkAndroidFixtures` (formerly `scripts/check_android_fixtures.py`) so the fixture manifest stays
in sync. Lint and JVM publishing are covered by the default CI task list above, and
`:android:testDebugUnitTest` exercises the Android-only harness alongside the shared tests.

Set `NORITO_JAVA_VERSION=<version>` to exercise a different Norito drop; the manifest guard fails if
it diverges from `schemas/norito_schema_manifest.json`.

`PlatformHttpTransportExecutor` now prefers the Android OkHttp factory when present (substituted in
core tests), falling back to the JDK client elsewhere. The Android module exposes
`OkHttpTransportExecutorFactory` for callers that want to reuse a shared `OkHttpClient`.

To keep JVM-only transports out of the Android artefacts, run the guard after producing an AAR:

```bash
make android-transport-guard
# or provide a custom classes.jar/aar
ANDROID_TRANSPORT_GUARD_AAR=java/iroha_android/android/build/outputs/aar/android-release.aar \
  bash ci/check_android_transport_guard.sh
# or
bash ci/check_android_transport_guard.sh /path/to/classes.jar
```

The guard is fail-closed: the compiled artifact, `jdeps`, and archive tooling
must all be present and successfully inspect the candidate. There is no
override for allowing JVM transports in an Android release.

### Transport defaults and troubleshooting

- HTTP clients use a strict runtime split: Android loads `OkHttpTransportExecutorFactory`, while JVM
  builds load `JavaHttpExecutorFactory`. Apps or services that want a custom transport should inject
  it explicitly with the relevant builder.
- Builders now auto-wire platform defaults: `HttpClientTransport.withDefaultExecutor(...)`,
  `ToriiEventStreamClient.builder()` (without `setTransportExecutor(...)`),
  `SorafsGatewayClient.builder()`, and `HttpSafetyDetectService.createDefault(...)` all pick the
  platform executor so Android apps land on the shared OkHttp client without extra wiring.
- WebSocket clients require an explicit connector. Android callers should inject
  `OkHttpWebSocketConnectorFactory.createDefault()`, while JVM callers should inject
  `JdkWebSocketConnectorFactory.createDefault()`. This keeps platform selection deterministic and
  avoids reflective discovery; `AndroidClientFactory` performs the injection for its clients.
- Android artefacts must not contain `java.net.http` bytecode. The mobile SDK artifact workflow
  assembles the Java release AAR and executes
  `ci/check_android_transport_guard.sh` (also available locally via `make android-transport-guard`)
  to fail when JVM-only classes leak into the Android bundle.
- Guard failures usually mean the wrong artefact was scanned (set `ANDROID_TRANSPORT_GUARD_AAR` to
  the release bundle if you used a custom output path) or a
  JVM-only module was added as an Android dependency. Rebuild with
  `gradle -p java/iroha_android :android:assembleRelease` and rerun the guard before publishing.
- Consumers can supply a custom `OkHttpClient` via `OkHttpTransportExecutorFactory` on Android (the
  default factory uses the shared client provider); JVM callers can opt into
  `JavaHttpExecutorFactory` when `java.net.http` is available on the module path.
- When using a custom `OkHttpClient` (for example with certificate pinning) and you need to release
  resources, call `HttpClientTransport.invalidateAndCancel()` (or
  `HttpTransportExecutor.invalidateAndCancel()`) to cancel in-flight requests and clean up the
  underlying dispatcher/connection pool.
- For Android-first apps that want a single transport surface, `AndroidClientFactory` constructs
  HTTP/Norito RPC/SSE/WebSocket/Safety Detect/SoraFS clients around a shared `OkHttpClient`,
  threading through `ClientConfig` headers/observers (including telemetry).

Tests rely on Java assertions (enabled in the Gradle test tasks). Make sure `JAVA_HOME` points to a
JDK 21 or newer installation before running the harness.

If your environment does not provide `/usr/libexec/java_home`, set `JAVA_HOME` explicitly (for
example on Homebrew-based macOS hosts):

```bash
JAVA_HOME="$(brew --prefix openjdk@21)/libexec/openjdk.jdk/Contents/Home" \
ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest \
./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --rerun-tasks
```

Android Foundations pins this workspace to **JDK 21 LTS**. Possible upgrades are only evaluated after
Oracle’s quarterly CPU releases: stage the candidate by setting `ANDROID_JDK_NEXT=1` in Buildkite
so the Gradle harness and Android fixture parity checks soak the alternate toolchain. Capture
the CI logs and record the soak decision in `artifacts/android/fixture_runs/` and `status.md` before
promoting a new JDK.

`scripts/check_android_fixtures.py` keeps the checked-in Norito fixtures and
manifest in sync with the canonical hash metadata. Verify schema pinning with:

```bash
gradle -p java/iroha_android :core:verifyNoritoSchemas
```

Refresh the manifest after Norito schema updates via:

```bash
gradle -p java/iroha_android :core:regenNoritoSchemaManifest
```

Commit the updated `schemas/norito_schema_manifest.json` alongside any Norito changes.

To produce a combined CI-friendly report that exercises both the test harness
and the fixture parity gate, run:

```bash
python3 scripts/android_test_report.py --run-tests --output artifacts/android/test_report.json
```

The helper executes `ci/run_android_tests.sh`, writes the per-step summaries
under `artifacts/android/`, and exits non-zero if either the tests or fixture
parity fail (set `--allow-failures` only when you need a report without failing
the shell).

### Deterministic export & recovery

- `SoftwareKeyProvider.exportDeterministic(...)` emits v4 bundles with per-export salt/nonce,
  `kdf_kind`, and `kdf_work_factor`. Argon2id (64 MiB, 3 iterations, parallelism = 2) is the only
  export KDF. Passphrases must be >=12 characters and the importer rejects all-zero salt/nonce
  seeds.
- `SoftwareKeyProvider` can persist deterministic exports by wiring a `KeyExportStore` plus
  `KeyPassphraseProvider` (for example, `FileKeyExportStore` on Android/JVM, or
  `InMemoryKeyExportStore` in tests). The provider rehydrates keys from the store before generating
  new material, keeping software-backed accounts stable across app restarts.
- `KeyExportBundle.decode(Base64|bytes)` accepts the v4 payload only. Treat
  salt/nonce/ciphertext errors as tampering and capture a fresh bundle rather than reusing an old
  export between devices.
- Regression coverage in `DeterministicKeyExporterTests` includes wrong passphrases and tampered
  salt/nonce/ciphertext, and all-zero seed rejection. Clear passphrase char arrays after use in
  application code.
- `KeystoreKeyProviderTests` exercises empty vs challenged attestation regeneration and the
  `android.keystore.attestation.failure` telemetry path; run
  `ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.crypto.keystore.KeystoreKeyProviderTests \
  bash ci/run_android_tests.sh` to spot-check cache and challenge matrices without rebuilding the
  full suite.

### Lint & dependency scanning (AND6 prototype)

Run

```bash
make android-lint
```

from the repository root to execute the AND6 static-analysis prototype
(`ci/check_android_javac_lint.sh`). The script reuses the lightweight javac
target used by `ci/run_android_tests.sh`, enables `-Xlint:all` with warnings treated as
errors, and then invokes `jdeps --summary` to ensure the Android surface only
depends on the approved JDK modules (`java.base`, `java.net.http`,
`jdk.httpserver`). Any new module dependency causes the lint run to fail so
publish jobs can gate on the same policy.

Every run copies the generated `jdeps` summary to
`artifacts/android/lint/jdeps-summary.txt` so CI/release artefacts always have
an up-to-date module list. Set
`ANDROID_LINT_KEEP_WORKDIR=1 make android-lint` to preserve the workspace for
manual inspection and/or provide
`ANDROID_LINT_SUMMARY_OUT=artifacts/android/lint/<tag>/jdeps-summary.txt make android-lint`
when you need an additional, versioned copy for compliance packets.

### Norito fixture rotation

The Rust xtask is the sole owner of the shared Norito RPC fixture corpus. Canonical
payloads, descriptors, manifests, schema hashes, and compact-hash vectors live in
`fixtures/norito_rpc`. The files under `java/iroha_android/src/test/resources` are a
generated Java mirror; Java keeps local `.norito` copies because its parity tests read
those payload bytes directly. Never edit the mirror or its hashes by hand.

Regenerate and verify the complete owner set from the repository root:

```bash
cargo run --locked -p xtask --features dev-tools --bin xtask -- norito-rpc-fixtures
cargo run --locked -p xtask --features dev-tools --bin xtask -- norito-rpc-verify
bash ci/check_android_fixtures.sh
```

`make android-fixtures` is a convenience wrapper around the same xtask owner and
then runs the Android parity check; `make android-fixtures-check` runs only that
check. A successful generation updates the canonical directory and every SDK
mirror together. Commit the complete generated change set, not an Android-only
subset. The parity wrapper can still write its JSON summary under
`artifacts/android/parity/` for CI evidence, but that report is not fixture source
material.

The test harness currently executes the key manager, keystore flows (with the
Android Keystore fallback backend), Norito codec round-trips that verify typed
instruction decoding, transaction builder signing, and HTTP client serialization
paths to keep the Java pathways aligned.

### Publishing snapshots (AND9)

Run

```bash
make android-publish-snapshot \
  ANDROID_PUBLISH_VERSION=0.2.0-dev.1 \
  ANDROID_PUBLISH_REPO_DIR=$PWD/artifacts/android/maven
```

(or call `./gradlew -p java/iroha_android publish -PirohaAndroidVersion=... -PirohaAndroidRepoDir=...`)
to build/publish both SDK targets (JVM jar and Android AAR), generate the
CycloneDX SBOMs, and emit runtime evidence. The helper stages outputs under
`artifacts/android/reports/<version>/{android,jvm}/` with optional Sigstore
bundles (`ANDROID_PUBLISH_SIGN=1`) and assembles the sample app against the
published AAR unless `ANDROID_PUBLISH_SKIP_SAMPLE=1` is set. Override the
staging root via `ANDROID_PUBLISH_REPORT_DIR` and Maven destinations via
`ANDROID_PUBLISH_REPO_URL`/`ANDROID_PUBLISH_REPO_DIR`.

Key evidence paths (also mirrored under `artifacts/android/reports/<version>`):

- JVM deps: `jvm/build/reports/publishing/iroha-android-jvm-<version>-runtimeClasspath.json`
- JVM runtime manifest/checksum:
  `jvm/build/reports/publishing/iroha-android-jvm-runtime-manifest.json` and
  `jvm/build/reports/publishing/iroha-android-jvm-<version>-runtime.sha256`
- Android deps:
  `android/build/reports/publishing/iroha-android-android-<version>-releaseRuntimeClasspath.json`
- Android runtime manifest/checksum:
  `android/build/reports/publishing/iroha-android-android-runtime-manifest.json` and
  `android/build/reports/publishing/iroha-android-android-<version>-runtime.sha256`
- SBOMs: recorded under `build/reports/` (JSON and XML) with the chosen path
  surfaced via the `sbom.path` field in each runtime manifest.

Every publish invocation automatically runs the manifest/SBOM tasks so
governance packets can attach the resulting JSON alongside the Maven repository
snapshot. See `specs/sdk/android/publishing_plan.md` for the full release
checklist.

### Observability Hooks


> **Instruction APIs:** The Java SDK provides typed `InstructionTemplate`
> builders for register, transfer, mint/burn, permission and role, trigger,
> governance, RWA, confidential-asset, Kaigi, and other implemented instruction
> families. `SetKeyValueInstruction` and `RemoveKeyValueInstruction` cover
> domain, account, asset-definition, NFT, RWA, and trigger metadata;
> `SetAssetKeyValueInstruction` and `RemoveAssetKeyValueInstruction` cover
> concrete asset balances. `InstructionBox.fromWirePayload(...)` carries other
> canonical Norito-framed instruction payloads without inventing a Java-side
> schema.

`ClientConfig` now exposes request-scoped instrumentation, static header support,
and deterministic retry policies. Applications can attach `ClientObserver`
implementations to capture metrics or send tracing data, register default headers
(for example, API tokens or `User-Agent` values), and configure `RetryPolicy`
instances to automatically retry transient network failures or 5xx responses with
predictable backoff.

### Crash telemetry

Enable crash telemetry by installing the built-in handler when configuring the
client:

```java
TelemetryOptions telemetryOptions = TelemetryOptions.builder()
    .setTelemetryRedaction(TelemetryOptions.Redaction.builder()
        .setSaltHex("<hex-salt>")
        .setSaltVersion("2026Q1")
        .setRotationId("rot-7")
        .build())
    .build();

ClientConfig clientConfig =
    ClientConfig.builder()
        .setBaseUri(new URI("https://torii.devnet.example"))
        .setTelemetryOptions(telemetryOptions)
        .setTelemetrySink(myTelemetrySink)
        .enableCrashTelemetryHandler()
        .build();
```

The handler records `android.crash.report.capture` automatically when uncaught
exceptions reach the process boundary. Upload pipelines can reuse the telemetry
configuration to emit `android.crash.report.upload` counters:

```java
clientConfig
    .crashTelemetryReporter()
    .ifPresent(reporter -> reporter.recordUpload(crashId, "sorafs", "success", retryCount));
```

Pass a custom `CrashTelemetryHandler.MetadataProvider` to `setCrashTelemetryMetadataProvider`
when additional crash context (e.g., watchdog buckets) is required.

### Torii streaming (SSE)

`HttpClientTransport.newEventStreamClient()` exposes the shared
`ToriiEventStreamClient`, which reuses the same base URI, telemetry observers, and
auth headers as the HTTP transport. Streaming clients consume Torii’s
server-sent event feeds and surface parsed frames via the listener interface:

```java
ToriiEventStreamClient streams = httpTransport.newEventStreamClient();
ToriiEventStream stream =
    streams.openSseStream(
        "/v1/events/sse",
        ToriiEventStreamOptions.defaultOptions(),
        new ToriiEventStreamListener() {
          @Override
          public void onEvent(ServerSentEvent event) {
            event.terminalStreamError().ifPresent(error -> {
              throw error;
            });
            System.out.println(event.event() + ": " + event.data());
          }
        });

// Remember to close the stream when your component is torn down.
stream.close();
```

Listeners receive retry hints (via `retry:` frames) so applications can reuse
Torii’s back-off guidance, and telemetry observers attached to the transport
emit the same hashed-authority metadata recorded for HTTP submissions. When the
transport supports streaming responses (OkHttp/JDK/URLConnection), frames are
parsed as they arrive; other executors buffer the response before parsing.

The canonical `/v1/events/sse` and `/v1/contracts/events/sse` feeds are
live-only and have no replay log. The client rejects every case variant of
`Last-Event-ID` before HTTP dispatch for exactly those paths; replay-capable
custom streams may still receive that header. Raw listeners receive terminal
`event: stream_error` frames. Call `ServerSentEvent.terminalStreamError()`
before category filtering to project one into a strict `ToriiStreamException`
with its stable code, server message, optional unsigned dropped-message count,
replay flag, and raw JSON. Malformed or schema-expanded terminal envelopes fail
closed as `ToriiStreamProtocolException`. The typed pipeline-status stream does
this projection automatically. Reconnecting to a canonical feed starts a new
live subscription and can have a gap.

Use `ToriiEventStreamSubscription` when a long-lived component needs automatic
reconnects:

```java
ToriiEventStreamSubscription subscription =
    ToriiEventStreamSubscription.builder(
            streams, "/v1/events/sse", ToriiEventStreamOptions.defaultOptions(), listener)
        .setInitialBackoff(Duration.ofSeconds(1))
        .setMaxBackoff(Duration.ofSeconds(30))
        .addObserver(new ToriiEventStreamObserver() {
          @Override
          public void onReconnectScheduled(Duration delay, ReconnectReason reason) {
            telemetry.incrementReconnects(reason.name(), delay);
          }

          @Override
          public void onStreamFailure(Throwable error) {
            telemetry.recordFailure(error);
          }
        })
        .build()
        .start();

// Later
subscription.close();
```

The helper honours server-provided retry hints and uses exponential
backoff when the stream fails before emitting one. Observers registered via
`addObserver` receive structured lifecycle notifications (`streamOpened`,
`streamClosed`, `streamFailure`, and `onReconnectScheduled`) so telemetry
pipelines can tag reconnect attempts, failure causes, and delay budgets without
mutating the primary listener.

### Torii streaming (WebSocket)

The WebSocket surface rides on the transport abstractions
(`TransportRequest`/`TransportWebSocket`) so JVM and Android inject their platform
connectors explicitly. Android apps should pass the OkHttp connector to keep
`java.net.http` out of the AAR; JVM apps can instead inject
`JdkWebSocketConnectorFactory.createDefault()`:

```java
import okhttp3.OkHttpClient;
import org.hyperledger.iroha.android.client.okhttp.OkHttpWebSocketConnector;

ToriiWebSocketClient wsClient =
    ToriiWebSocketClient.builder()
        .setBaseUri(URI.create("https://torii.devnet.example"))
        .setWebSocketConnector(new OkHttpWebSocketConnector(new OkHttpClient()))
        .build();

ToriiWebSocketSession session =
    wsClient.connect(
        "/ws/telemetry",
        ToriiWebSocketOptions.builder()
            .addSubprotocol("norito-stream")
            .build(),
        new ToriiWebSocketListener() {
          @Override
          public void onText(ToriiWebSocketSession session, CharSequence data, boolean last) {
            System.out.println("payload: " + data);
          }
        });
```

Sessions expose async send helpers (`sendText`, `sendBinary`, `sendPing`, `sendClose`)
and share the same observer instrumentation used by HTTP/SSE clients. Ping/pong
support depends on the connector: the JDK connector honours both, while the
OkHttp connector returns a failed future for ping/pong because OkHttp does not
expose control frames. Subprotocol negotiation is surfaced via
`ToriiWebSocketSession.subprotocol()`, and reconnect helpers remain available
through `ToriiWebSocketSubscription`.

### Connect retry policy

The Connect stack uses the dedicated `org.hyperledger.iroha.android.connect.ConnectRetryPolicy`
helper to mirror the Rust reference implementation (`connect_retry::policy`). It applies
exponential back-off with full jitter (base 5 s, capped at 60 s) and derives jitter deterministically
from the Connect session identifier so Android, Swift, and JavaScript clients wait for the exact
same delay sequence:

```java
import org.hyperledger.iroha.android.connect.ConnectRetryPolicy;

byte[] sessionId = new byte[32]; // use the Connect session id bytes
ConnectRetryPolicy policy = new ConnectRetryPolicy();
for (int attempt = 0; attempt < 5; attempt++) {
    long delayMs = policy.delayMillis(attempt, sessionId);
    Thread.sleep(delayMs);
    // reconnect logic here
}
```

The deterministic seed/attempt mapping ensures reconnect telemetry stays aligned across SDKs.

### Connect error taxonomy

`org.hyperledger.iroha.android.connect.error.ConnectError` mirrors the shared taxonomy
(`specs/connect_error_taxonomy.md`) so Android apps emit the same `category`/`code`
pairs as the Swift and JavaScript SDKs. Wrap every transport, codec, or queue failure via
`ConnectErrors.from(Throwable)` (or manually create a `ConnectError` using the builder)
before forwarding attributes to OpenTelemetry:

```java
import org.hyperledger.iroha.android.connect.error.ConnectError;
import org.hyperledger.iroha.android.connect.error.ConnectErrors;

try {
    connectClient.send(frame);
} catch (Exception ex) {
    ConnectError error = ConnectErrors.from(ex);
    telemetry.emit("connect.error", error.telemetryAttributes());
    throw error;
}
```

Queue back-pressure helpers such as `ConnectQueueError.overflow(limit)` and
`ConnectQueueError.expired(ttlMillis)` already implement `ConnectErrorConvertible`, so they
map to the `queueOverflow` and `timeout` categories automatically. Use
`ConnectError.telemetryAttributes(...)` to project overrides (fatal flag, HTTP status, or
custom `underlying` context) when surfacing the events.

### Canonical request signing

Torii app endpoints accept optional `X-Iroha-Account`, `X-Iroha-Signature`,
`X-Iroha-Timestamp-Ms`, and `X-Iroha-Nonce` headers. Use
`CanonicalRequestSigner` when calling account-scoped helpers or building ad-hoc
HTTP requests:

```java
import java.net.URI;
import org.hyperledger.iroha.android.client.CanonicalRequestSigner;

URI uri = URI.create("https://torii.example/v1/accounts/<account_i105>/assets?limit=10");
Map<String, String> headers =
    CanonicalRequestSigner.buildHeaders("get", uri, new byte[0], "<account_i105>", keyPair.getPrivate());
```

Signatures cover the canonical method/path/query/body layout plus freshness
metadata, matching the Rust verifier Torii uses on app-facing endpoints.

### Sora VPN native lease flow

`HttpClientTransport` exposes the quote-first Sora VPN endpoints. Quotes bind
the account, exit class, client metering key, XOR fee asset, escrow account, and
operator account, then return native `OpenVpnLeaseEscrow` instructions. Session
creation requires the committed hash of the exact quote-bound lease-open
transaction, and operator receipt submission returns a native `SettleVpnLease`
instruction with earned/refunded XOR amounts:

```java
ToriiCanonicalRequestAuth userAuth =
    new ToriiCanonicalRequestAuth("<account_i105>", userKeyPair.getPrivate());

VpnQuote quote = transport.createVpnQuote(
    new VpnQuoteCreateRequest("standard", "<metering_public_key_hex>"),
    userAuth).join();

VpnSession session = transport.createVpnSession(
    new VpnSessionCreateRequest(
        quote.exitClass(),
        quote.quoteId(),
        "<committed_open_lease_tx_hash>",
        quote.meteringPublicKeyHex()),
    userAuth).join();

ToriiCanonicalRequestAuth operatorAuth =
    new ToriiCanonicalRequestAuth("<operator_i105>", operatorKeyPair.getPrivate());
VpnReceipt settled = transport.submitVpnReceipt(
    new VpnReceiptSubmitRequest("<relay_receipt_hex>", "<client_voucher_hex>", quote.leaseIdHex()),
    operatorAuth).join();
```

Submit `quote.openLeaseInstruction()` and `settled.settleLeaseInstruction()` as
normal signed native instruction transactions. This keeps prepaid VPN funds in
XOR escrow until usage receipts and client vouchers prove the amount earned by
the operator.

### Pipeline Hashes

`HttpClientTransport.submitTransaction(...)` computes the canonical BLAKE2b-256
hash for every signed transaction via `SignedTransactionHasher` and surfaces it
through `ClientResponse.hashHex()`. Callers can forward the returned hash to
`waitForTransactionStatus(...)` (or other Torii polling helpers) without
reimplementing the hashing logic, and the same canonical value is preserved when
pending transactions are replayed from `PendingTransactionQueue`.

The first-release ID commits to the canonical signed `TransactionPayload`
inside `TransactionEntrypoint::External`, not to the surrounding authorization
proof. Replacing a signature or multisig proof therefore does not create a
second ID for the same intent. Proof attachments live in
`TransactionPayload.attachments()` and remain signature- and ID-bound.

Torii returns a Norito-encoded transaction submission receipt (payload +
signature) on `/v1/pipeline/transactions`. The Android SDK surfaces the raw
receipt bytes via `ClientResponse.body()` so callers can decode them with their
Norito tooling when they need the receipt fields.

### Torii Reject Codes

Torii attaches an `x-iroha-reject-code` header when admission fails. Both the
JDK and Android/OkHttp transports keep that header intact via
`ClientResponse.rejectCode()` so apps can surface the precise Torii error without
re-parsing the response body:

```java
ClientResponse response = transport.submitTransaction(transaction).join();
response.rejectCode().ifPresent(code -> {
  // e.g., PRTRY:TX_SIGNATURE_MISSING — surface to the user/telemetry
});
```

### Resolving Account Aliases

`HttpClientTransport.resolveAccountAlias(...)` posts to `/v1/aliases/resolve` and
returns an `Optional<AccountAliasResolution>`. An `Optional.empty()` value
indicates the node responded with HTTP 404 for an unknown alias; other failures
complete the future exceptionally. `AccountAliasResolution.index()` is optional
and may be `null` when the alias backend does not expose a deterministic index.

```java
Optional<AccountAliasResolution> resolved =
    client.resolveAccountAlias("some_alias@universal").join();
resolved.ifPresentOrElse(
    resolution -> System.out.println("account: " + resolution.accountId()),
    () -> System.out.println("alias not found"));
```

### Reading Kotodama Manifests

`HttpClientTransport.getContractManifest(codeHash)` reads
`/v1/contracts/code/{code_hash}` into the complete Kotodama V1 manifest model.
The strict decoder retains `seiyaku_name`, branded entrypoint kinds, exact
flat-preorder argument/return schemas, dynamic access hints,
completeness/skips, triggers, state, error codes, `kotoba`, and provenance. A
`List` node contains only `capacity` and its element subtree immediately follows
it. Unknown fields, legacy nested `element` metadata, incomplete or trailing
tapes, over-depth schemas, noncanonical Norito hashes, wrapper/manifest hash
mismatches, and inconsistent schemas are rejected before the future completes.

### Key Manager Defaults

`IrohaKeyManager.withDefaultProviders()` constructs a manager that prefers
hardware-backed keystore providers when available and also registers a software
provider for emulators or desktop JVMs. Pass custom `KeyGenParameters` when you
need to enforce StrongBox-only keys or user-authentication requirements while
retaining an explicit deterministic software provider for local testing.
If your desktop JVM lacks built-in Ed25519 support, configure the software
provider with BouncyCastle required.
Hardware-backed keys remain non-extractable; for user-managed accounts that must
roam across devices, prefer `SOFTWARE_ONLY` (or `withSoftwareProvider`) and use
`exportDeterministicKey(...)` / `importDeterministicKey(...)` to move key
material between devices securely. When you need fully exportable keys, build
the software provider with BouncyCastle enforced and a persistent export store:

```java
KeyExportStore store = new FileKeyExportStore(new File(filesDir, "keys.properties"));
KeyPassphraseProvider passphraseProvider = () -> "export-passphrase".toCharArray();
SoftwareKeyProvider provider =
    new SoftwareKeyProvider(
        SoftwareKeyProvider.ProviderPolicy.BOUNCY_CASTLE_REQUIRED,
        store,
        passphraseProvider);
IrohaKeyManager manager = IrohaKeyManager.fromProviders(List.of(provider));
```

Or use the convenience helper:

```java
IrohaKeyManager manager =
    IrohaKeyManager.withExportableSoftwareKeys(store, passphraseProvider);
```

To opt into post-quantum ML-DSA transaction signing and Kagemusha lifecycle/artifact streaming
flows, select the signing algorithm up front:

```java
IrohaKeyManager ed25519Manager = IrohaKeyManager.withSoftwareProvider();
IrohaKeyManager mlDsaManager =
    IrohaKeyManager.withSoftwareProvider(SigningAlgorithm.ML_DSA);
IrohaKeyManager gostManager =
    IrohaKeyManager.withSoftwareProvider(SigningAlgorithm.GOST_2012_256_A);

KeyGenParameters params =
    new KeyGenParameters.Builder()
        .setSigningAlgorithm(SigningAlgorithm.ML_DSA)
        .build();
IrohaKeyManager tunedManager = IrohaKeyManager.withDefaultProviders(params);
```

`ED25519` remains the default. `SECP256K1`, `BLS_NORMAL`, `BLS_SMALL`,
`ML_DSA`, the five `GOST_2012_*` variants, and `SM2` use the shared native
bridge and are software-only in this pass, so `HARDWARE_*` and `STRONGBOX_*`
preferences fail fast instead of silently falling back to an unexpected
provider.

The manager validates Ed25519 SPKI output and skips providers that return a
different algorithm (common on emulators), falling back to the next configured
provider.

`generateOrLoad(alias, preference)` accepts a `KeySecurityPreference` that
describes the required hardware tier:

- `STRONGBOX_REQUIRED` — only StrongBox-backed providers are consulted; the call
  fails if no StrongBox backend is registered or the target device cannot
  satisfy the request.
- `STRONGBOX_PREFERRED` — StrongBox providers are tried first, then other
  hardware-backed providers, and finally explicitly configured software providers.
- `HARDWARE_REQUIRED`/`HARDWARE_PREFERRED` — retain the previous semantics for
  “any hardware” while allowing deterministic software-provider selection.
- `SOFTWARE_ONLY` — bypass hardware providers entirely (useful for emulator or
  deterministic testing scenarios).

Aliases follow a predictable lifecycle: the manager looks up existing keys
across all providers in priority order, reuses the first match, and only
generates a new key when no provider has material for the alias. Hardware-backed
aliases remain pinned to the provider that created them, while software
providers are consulted only when the chosen preference allows a downgrade (for
example, `STRONGBOX_PREFERRED` on a device without StrongBox support). When an
alias is generated on a weaker route than requested, the manager records the
software copy so future lookups remain deterministic.

Call `IrohaKeyManager.providerMetadata()` to inspect the registered providers
(name, hardware capability, attestation support) before deciding which
alias/security preference to use, and use the new
`IrohaKeyManager.verifyAttestation(...)` helper to validate the attestation
chain exported by hardware providers without wiring keystore internals through
application code.

`IrohaKeyManager.hasStrongBoxProvider()` is a convenience check that reports
whether a StrongBox-backed provider is registered, enabling applications to
surface hardware posture in diagnostics or gate user-facing flows before
attempting StrongBox-only operations.

```java
IrohaKeyManager manager = IrohaKeyManager.withDefaultProviders();
for (KeyProviderMetadata meta : manager.providerMetadata()) {
    System.out.printf("%s hardware=%s level=%s attestation=%s%n",
        meta.name(), meta.hardwareBacked(), meta.securityLevel(), meta.supportsAttestationCertificates());
}
```

Use `IrohaKeyManager.verifyAttestation(...)` (or the underlying
`KeystoreKeyProvider.verifyAttestation(...)`) alongside `AttestationVerifier`
when you need to validate the StrongBox/TEE attestation chain exported by the
Android backend. The verifier checks the certificate path, decoded challenge,
and security level while surfacing parsed metadata so applications can enforce
hardware policies or forward the attestation bundle to remote services. For
lab automation run `scripts/android_keystore_attestation.sh --bundle-dir <path>
--trust-root <root.pem> [--trust-root-dir <directory>]` — it compiles the same
verifier and produces a JSON summary that should be archived with each
attestation bundle.

Need fresh attestation material? Call
`IrohaKeyManager.generateAttestation(alias, challenge)` – it uses the selected
provider (StrongBox/TEE first) and returns a `KeyAttestation`
bundle when the hardware can satisfy the request, storing the artefact in the
backing provider for subsequent verification. Pass a non-empty `challenge` to
force fresh material (cache entries are keyed by `(alias, challenge)`), and set
`KeyGenParameters.Builder.setAttestationChallenge(...)` when generating keys if
you need the challenge embedded at creation time. StrongBox preferences are
propagated to keystore parameters (`STRONGBOX_REQUIRED` forces StrongBox,
`STRONGBOX_PREFERRED` requests StrongBox), and generation errors are surfaced
directly.

To exercise CUDA acceleration on capable devices, launch the JVM with
`-Diroha.cuda.enableNative=true` and ensure `libconnect_norito_bridge` is
available on `java.library.path`. Without the flag the deterministic Java path remains
active and no native library is loaded (avoiding security warnings in CI).

Kotlin callers should use `CudaAcceleratorsKotlin.*OrNull` helpers to receive
`Long?`/`LongArray?` outputs instead of `Optional` wrappers. See the CUDA
operator guide for native setup and the hardware-qualified smoke harness
(`specs/sdk/android/gpu_operator_guide.md`). The ordinary JVM suite excludes
that GPU-only class; the nightly CUDA lane selects it explicitly and any
missing driver, JNI bridge, or CUDA result fails the lane.

`SoftwareKeyProvider.exportDeterministic(...)` emits a versioned, AES-GCM
wrapped export bundle (v4) using per-export salt/nonce. The bundle records the
signing algorithm alongside `kdf_kind` and work factor; v4 uses Argon2id
(64 MiB, 3 iterations, parallelism 2). A minimum 12 character passphrase is
enforced for deterministic exports/imports. Salt/nonce reuse is rejected and
decode guards fail fast on tampered lengths.
The companion `importDeterministic(...)` helper restores the key pair while
validating the export's public key and authentication tag, ensuring passphrase
mismatches or tampering are rejected.

`IrohaKeyManager.exportDeterministicKey(...)` / `importDeterministicKey(...)`
surface the same functionality through the manager so applications do not need
direct access to the underlying `SoftwareKeyProvider` during recovery flows.

### Pending Transaction Queue

Applications can provide a `PendingTransactionQueue` (the default implementation
`FilePendingTransactionQueue` persists base64-encoded canonical pending-transaction
records via
`ClientConfig`. When Torii submissions exhaust their retry budget, the
transport persists the signed payloads for later replay and automatically
drains the queue before sending new transactions. This keeps the mobile client
resilient to intermittent connectivity without losing deterministic ordering.
### Norito RPC Helper

Use `NoritoRpcClient` when you need to call Torii's Norito RPC endpoints
(`application/x-norito` payloads) alongside the REST pipeline. The helper
wraps the platform HTTP executor (OkHttp on Android, the JDK HTTP client
elsewhere), applies the correct binary content headers, and lets callers
override HTTP method, timeouts, headers, query parameters, and `Accept`
negotiation via `NoritoRpcRequestOptions`. The tests under
`client/NoritoRpcClientTests` demonstrate POST/GET flows, header overrides,
and error propagation, while the client builder accepts default headers (for
example `Authorization`) so instrumentation matches the REST transport. Call
`ClientConfig.toNoritoRpcClient()` for the platform default (OkHttp on Android)
or `ClientConfig.toNoritoRpcClient(HttpTransportExecutor)` when you already
have a custom executor. `HttpClientTransport.newNoritoRpcClient()` reuses
existing client configuration and telemetry hooks when spinning up a Norito
RPC transport.

### SoraFS Gateway Helpers

The `org.hyperledger.iroha.android.sorafs` package provides thin builders that map
directly to the Rust `sorafs_orchestrator` configuration. Use
`GatewayProvider.builder()` to describe gateway endpoints,
`GatewayFetchOptions.builder()` to compose telemetry/retry/transport overrides, and
`GatewayFetchRequest.builder()` to bundle everything into the JSON structure the
orchestrator expects. `TransportPolicy` and `AnonymityPolicy` mirror the CLI/SDK
labels (`soranet-first`, `anon-guard-pq`, etc.), ensuring Android clients participate
in the staged SoraNet anonymity rollout alongside the other SDKs.
The builders preserve and validate canonical protocol inputs without rewriting them:
`manifest_id_hex`, `provider_id_hex`, and the mandatory `gateway_public_key_hex` trust
anchor must be non-zero, lowercase, unprefixed 32-byte hex; stream tokens must use exact
standard Base64; and gateway URLs must be credential-free HTTPS origins with no query,
fragment, explicit port, or path.

`SorafsGatewayClient` wraps the HTTP transport so applications can submit orchestrator
requests without reimplementing header/observer plumbing. Call
`client.fetch(request)` when you only need the raw JSON/string output, or
`client.fetchSummary(request)` to receive a typed `GatewayFetchSummary` that exposes
provider receipts, anonymity ratios, and chunk metadata. The client reuses
`HttpTransportExecutor`, which means tests can provide deterministic fakes and production
code can share the same connection pool as the Torii pipeline transport.
`HttpClientTransport.newSorafsGatewayClient(...)` is a convenience helper that spawns the
gateway client using the same executor, timeout, headers, and observers as the primary Torii
transport so applications can rely on a single HTTP stack.

### Mock Torii Harness

The test suite now includes a lightweight HTTP harness (`src/test/java/org/hyperledger/iroha/android/client/mock/ToriiMockServer.java`)
that mirrors Torii's `/v1/pipeline/transactions` submission and `/v1/pipeline/transactions/status` routes. Integration tests such as
`HttpClientTransportHarnessTests` spin up the server, interact with it via `HttpClientTransport`, and assert on the recorded
requests/responses, providing end-to-end coverage for retries, headers, and offline queue replays without depending on a real Torii node.

The transaction descriptors, manifest, and matching `.norito` files under
`src/test/resources` are the generated Java mirror of `fixtures/norito_rpc`.
Regenerate them only through the canonical Rust owner:

```bash
cargo run --locked -p xtask --features dev-tools --bin xtask -- norito-rpc-fixtures
cargo run --locked -p xtask --features dev-tools --bin xtask -- norito-rpc-verify
python3 scripts/check_android_fixtures.py
```

The owner rewrites the canonical corpus first and publishes the Java mirror plus
the Python and Swift descriptor-only mirrors from the same result. Always commit
the complete generated set together; never retain an old hash, retired fixture,
or compatibility-only payload in the Java resource directory.

## License

Licensed under the Apache License, Version 2.0. See `LICENSE` for details.
