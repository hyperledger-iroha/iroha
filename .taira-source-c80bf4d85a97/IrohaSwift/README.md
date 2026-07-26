# IrohaSwift

Swift SDK targeting Hyperledger Iroha v2 and Sora Nexus (Iroha v3) nodes on Apple platforms.

Features:
- Torii HTTP client (balances, transactions, explorer instructions/transactions/RWAs, subscriptions, VPN quote/session/receipt flows, pipeline recovery, time service, ZK attachments, prover reports, contracts)
- Kagemusha cash models, transaction builders, proof binding helpers, and readiness discovery through `/v1/offline/readiness`
- Health & metrics helpers (fetch `/v1/health` text probe and `/v1/metrics` Prometheus/JSON payloads)
- Norito envelope encoder (header + CRC64-XZ)
- Required Native NoritoBridge integration (`dist/NoritoBridge.xcframework`) powering transfer/mint/burn builders and JSON inspection helpers
- Norito RPC HTTP helper (`NoritoRpcClient`) with binary header/query/timeout handling
- Pipeline submission helpers (POST `/v1/pipeline/transactions` with configurable retries + status polling)
- Ed25519 signing with CryptoKit plus native-bridge secp256k1, ML-DSA, GOST R 34.10-2012, BLS normal/small, and SM2 support
- Confidential key derivation (`ConfidentialKeyset.derive`) mirroring the Rust HKDF so wallets can obtain `sk_spend`, `nk`, `ivk`, `ovk`, and `fvk` locally
- Runtime capability helpers (`ToriiClient.getNodeCapabilities`, `getRuntimeMetrics`, `getRuntimeAbiActive`) mirroring the Torii `/v1/node/capabilities` and `/v1/runtime/*` surfaces
- Verifying key registry read/mutation/event helpers (`ToriiClient.getVerifyingKey`, `listVerifyingKeys`, `registerVerifyingKey`, `updateVerifyingKey`, `streamVerifyingKeyEvents`) covering `/v1/zk/vk` operations

## Installation

`IrohaSwift` replaced the older ad-hoc Swift package names; make sure your dependency
graph points at the renamed module.

The remote coordinates below are the immutable first-release target. Do not
advertise them until the signed `0.1.0` SwiftPM tag and
`iroha-swift-v0.1.0` monorepo tag have passed the public package canary. Before
that cutover, use the local-package form documented below.

### Swift Package Manager (Xcode UI)
1. In Xcode select **File → Add Package Dependencies…**
2. Enter `https://github.com/hyperledger/iroha-swift` and select the exact
   first-release version `0.1.0`.
3. Add the `IrohaSwift` library product to your application target. Apps using
   NFC or Nearby peer transfer must also add `IrohaSwiftMobileTransports`.

### Swift Package Manager (`Package.swift`)

```swift
// Package.swift
dependencies: [
    .package(
        url: "https://github.com/hyperledger/iroha-swift",
        exact: "0.1.0"
    )
],
targets: [
    .target(
        name: "YourApp",
        dependencies: [
            .product(name: "IrohaSwift", package: "iroha-swift"),
            .product(name: "IrohaSwiftMobileTransports", package: "iroha-swift")
        ]
    )
]
```

Peer-transfer apps import both modules:

```swift
import IrohaSwift
import IrohaSwiftMobileTransports
```

The host app, not SwiftPM, owns Apple privacy strings and entitlements. Add the
keys used by the rails you enable (replace only the human-readable strings):

```xml
<!-- Info.plist: needed only when the app captures QR with the camera. -->
<key>NSCameraUsageDescription</key>
<string>Scan an offline-transfer QR code.</string>

<!-- Info.plist: Google Nearby. Keep the Bonjour service exact. -->
<key>NSBonjourServices</key>
<array>
    <string>_F2EBA4BCB49B._tcp</string>
</array>
<key>NSBluetoothAlwaysUsageDescription</key>
<string>Discover a nearby device for an offline transfer.</string>
<key>NSLocalNetworkUsageDescription</key>
<string>Exchange an offline transfer with a nearby device.</string>

<!-- Info.plist: Core NFC reader mode. Keep the AID exact. -->
<key>NFCReaderUsageDescription</key>
<string>Exchange an offline transfer over NFC.</string>
<key>com.apple.developer.nfc.readersession.iso7816.select-identifiers</key>
<array>
    <string>F0504B45504B524E464301</string>
</array>
```

Reader builds also need the Near Field Communication Tag Reading capability,
which produces this entitlement:

```xml
<key>com.apple.developer.nfc.readersession.formats</key>
<array>
    <string>TAG</string>
</array>
```

Receiver/CardSession builds additionally require an Apple-provisioned HCE
profile containing the following entitlements. Do not make CardSession a
runtime fallback: require iOS 17.4 or newer and proceed only when
`IrohaPeerNfcCardSessionControllerV1.availability(...)` reports an eligible
device.

```xml
<key>com.apple.developer.nfc.hce</key>
<true/>
<key>com.apple.developer.nfc.hce.iso7816.select-identifier-prefixes</key>
<array>
    <string>F0504B45504B524E464301</string>
</array>
```

When working from the monorepo, use `.package(name: "IrohaSwift", path: "../../IrohaSwift")`
instead of the Git URL so Xcode consumes the local sources.

#### NoritoBridge policy (SwiftPM)

`Package.swift` checks for `dist/NoritoBridge.xcframework` next to the repository root and fails package resolution when the bridge is missing. Runtime errors such as `ConnectCodecError.bridgeUnavailable` and `SwiftTransactionEncoderError.nativeBridgeUnavailable` include the same bridge-location hint for broken or unloaded bridge symbols.

The default bridge build deliberately keeps real privacy proving and verification
fail-closed. After the privacy production-gate evidence has been approved, build
an opt-in Apple artifact with:

```bash
scripts/build_norito_xcframework.sh --privacy-production-enabled
```

That option passes the existing `privacy-production-enabled` Cargo feature to
every Apple slice and marks the XCFramework plus its artifact manifest. The
`Mobile SDK Artifacts` manual workflow exposes the same default-off option.
Do not use skip-build mode with this option: the builder rejects that ambiguous
combination rather than labeling pre-existing libraries as production-enabled.

CI runs `.github/workflows/mobile_sdk_artifacts.yml` (see `ci/check_swift_spm_validation.sh` and `ci/check_swift_pod_bridge.sh`) to verify bridge packaging and mandatory missing-artifact rejection.

### CocoaPods

```ruby
pod 'IrohaSwift', :podspec => 'https://raw.githubusercontent.com/hyperledger-iroha/iroha/iroha-swift-v0.1.0/IrohaSwift/IrohaSwift.podspec'
```

The podspec pulls sources from this repository and requires `dist/NoritoBridge.xcframework`
next to the checkout; `pod lib lint` fails fast when the bridge is missing so releases
bundle the signed xcframework (see `docs/connect_swift_integration.md` for the bundling flow).

Usage:
```swift
import IrohaSwift

let toriiURL = URL(string: "https://torii.example")!
let sdk = IrohaSDK(baseURL: toriiURL)
let pqSDK = IrohaSDK(baseURL: toriiURL, defaultSigningAlgorithm: .mlDsa)
let gostSDK = IrohaSDK(baseURL: toriiURL, defaultSigningAlgorithm: .gost2012_256A)

// Generate a signing key using the SDK default (Ed25519 unless overridden)
let signingKey = try sdk.generateSigningKey()
let accountId = AccountId.make(publicKey: try signingKey.publicKey())
let asset = "66owaQmAQMuHxPzxUN3bqZ6FJfDa"

let walletToken = "<wallet-session-token>"
let toriiAuth = try ToriiClientAuthentication.bearerToken(
    walletToken,
    accountId: accountId,
    dataspaceId: "mibank.paynet"
)
let torii = ToriiClient(baseURL: toriiURL, authentication: toriiAuth)

// Account onboarding requires the dedicated route token explicitly. It remains
// separate from an optional global X-API-Token configured on the client. Plan
// first, then apply the exact stateless receipt; neither body contains a key or token.
// The bundled Norito bridge encodes the exact receipt body and verifies its
// domain-separated hash, pinned chain, and authority signature before either
// call returns/submits.
// An older/missing bridge fails closed; JSON is never used as receipt hash input.
let onboardingIntent = try ToriiAccountOnboardingPlanRequest(
    alias: "merchant@paynet",
    accountId: accountId
)
let onboardingReceipt = try await torii.planAccountOnboarding(
    onboardingIntent,
    onboardingToken: routeToken,
    expectedAuthority: configuredOnboardingAuthority,
    expectedChainId: configuredChainId
)
let onboarding = try await torii.applyAccountOnboarding(
    onboardingReceipt,
    onboardingToken: routeToken,
    expectedAuthority: configuredOnboardingAuthority,
    expectedChainId: configuredChainId
)

// Operator alias setup is plan-only on Torii. The wallet verifies the plan
// hash and byte-identical instruction frames, signs one ordinary transaction,
// and submits it through the existing pipeline endpoint.
let setupPlan = try await torii.planAliasSetup(setupRequest, canonicalAuth: canonicalAuth)
try await sdk.submitAliasSetupPlan(
    setupRequest,
    plan: setupPlan,
    bodyEncoder: encodeCanonicalAliasPlanBody,
    feePayment: feePayment,
    signingKey: signingKey
)

// Or opt into any native-bridge signing algorithm explicitly.
let pqSigningKey = try pqSDK.generateSigningKey()
let gostSigningKey = try gostSDK.signingKey(fromSeed: Data("seed".utf8))

// Fetch balances through the credentialed Torii client
torii.getAssets(accountId: accountId, asset: asset, scope: "global") { result in
    print(result)
}

// List attachments published via the Torii app API
torii.listAttachments { result in
    print("attachments:", result)
}

// Submit transfer (WIP encoder)
let transfer = TransferRequest(
    chainId: "00000000-0000-0000-0000-000000000000",
    authority: accountId,
    assetDefinitionId: "66owaQmAQMuHxPzxUN3bqZ6FJfDa",
    quantity: "1.23",
    destination: "<destination_account_i105>",
    description: "demo",
    ttlMs: 60_000
)
let envelope = try sdk.buildSignedTransfer(transfer: transfer, signingKey: signingKey)
sdk.submit(envelope: envelope) { err in
    print(err as Any)
}

// Interleave canonical instruction frames and deployed-contract calls in one
// atomic transaction. All items share the signed gas limit.
let invocation = try TransactionContractInvocation(
    contractAddress: contractAddress,
    expectedCodeHash: expectedCodeHash,
    entrypoint: "apply",
    arguments: argumentRecord
)
let mixedEnvelope = try sdk.buildSignedExecutableBatch(
    chainId: "00000000-0000-0000-0000-000000000000",
    authority: accountId,
    entries: [
        .instruction(registerFrame),
        .contractCall(invocation),
        .instruction(transferFrame),
    ],
    feePayment: .authority(chargeLimits: [], gasLimit: 500_000),
    signingKey: signingKey
)

// Query pipeline status if needed
torii.getTransactionStatus(hashHex: envelope.hashHex) { status in
    print(status)
}

// Await pipeline completion using the helper.
sdk.submitAndWait(envelope: envelope) { result in
    print("pipeline status:", result)
}
```

Executable batches must be non-empty. Contract-call entries require a positive
signature-bound gas limit and an exact lowercase V1 Bech32m contract address;
invalid payloads are rejected before signing.

### Cancel an asset lock with an exact state precondition

`CancelAssetLockInstructionV1` implements the first-release two-field
compare-and-cancel contract. The convenience initializer hashes exact,
nonblank lock-id text without surrounding whitespace or a BOM with native
Blake2b-256, sets Iroha's hash marker bit, and emits the checksummed `EscrowId`
literal. The preimage is bounded by
`CancelAssetLockInstructionV1.maxLockIdUTF8BytesV1` (4,096 UTF-8 bytes, not
characters), while the on-wire `EscrowId` remains 32 bytes. Internal bytes are
never trimmed or normalized. The expected remaining amount must use canonical
positive `Quantity` spelling:

```swift
let cancellation = try CancelAssetLockInstructionV1(
    lockId: "appeal-case-2048",
    expectedRemainingAmount: "250"
)
let instructionJSON = try cancellation.noritoJSON()
let instructionFrame = try cancellation.transactionInstructionFrame()
```

Use the `escrowId:` initializer when the exact canonical marked hash literal
comes from finalized ledger state. `noritoArchive()`, `decodeNoritoArchive(_:)`,
`decodeBareJSON(_:)`, and `decodeInstructionJSON(_:)` enforce the byte-canonical
two-field V1 shape. Missing `expected_remaining_amount`, zero or noncanonical
quantities, aliases, extra fields, malformed hash literals, legacy one-field
archives, and trailing bytes all fail closed. Old development state carrying
the retired one-field layout must be discarded and reseeded.

Lease renewal and native auto-renew use the same local-signing flow through
`planAliasLeaseRenewal`, `planAliasAutoRenew`, and
`submitAliasLifecyclePlan`. An exact auto-renew no-op returns without creating
or submitting an empty transaction. Visibility-aware reads are available as
signed or unsigned overloads of `resolveAccountAlias`,
`resolveAccountAliasIndex`, and `aliasesByAccount`; signed calls emit the
canonical Iroha account/signature/timestamp/nonce headers. Alias plan and
intent values never contain API tokens or private keys.
The default alias frame codec uses the bundled Rust instruction registry to
typed-decode and canonically re-encode every complete planner frame; an older
or missing bridge fails closed. Advanced callers may still inject an equivalent
registry codec for testing or alternate packaging.

Wallet-scoped Torii deployments commonly require the `Authorization`,
`X-Account-Id`, and `X-Dataspace-Id` headers on every request. Use
`ToriiClientAuthentication` or `defaultHeaders` on `ToriiClient` so the SDK
attaches those headers centrally instead of repeating them at each call site.
Credential-bearing headers are rejected over plain HTTP or host-mismatched
requests by the shared transport-security check.

`TransferRequest`, `MintRequest`, `BurnRequest`, `ShieldRequest`, and `UnshieldRequest` expect
canonical unprefixed Base58 asset-definition IDs on the Swift surface.

`IrohaSDK` trims and validates chain/account/asset identifiers before signing and fails fast on malformed inputs. Override `creationTimeProvider` when you need deterministic timestamps for fixture generation or offline signing flows. `defaultSigningAlgorithm` controls the SDK helpers used by `generateSigningKey()` / `signingKey(fromSeed:)`; `Keypair` convenience APIs are Ed25519-only while native-backed algorithms use `NoritoBridge`.

### Offline peer transport V1

`IrohaPeerWireMessageV1` is the only first-release request/payment/ACK envelope.
Profile `1` requires schema `1` and allows a 24,576-byte encoded body; profile
`2` requires schema `0x0102` and allows a 24,576-byte bounded whole-offer body
(24,660 bytes including the fixed 84-byte IPM1 header). Canonical
bytes are capped at 32 KiB. Offline Note bytes remain opaque. Its text-based
apps can use `IrohaPeerCanonicalTextPayloadCodecV1` for an exact UTF-8 round
trip; the codec rejects profile `2`, whose bytes must instead be a kind-matched
ABI21 archive. Profile-2 construction and decode validate NRT0 v0.0, no
compression, exact compact-length flags, CRC64, the authoritative
fully-qualified schema, and static padding (request/payment 8, ACK 0) without
requiring the native bridge. The typed adapter performs deeper semantics.
The shared `fixtures/offline/kagemusha_peer_transport_v2.json` vector pins the
same qualified 49-byte structural archive through IPM1, IQR1, NFC, and an
authenticated Nearby record. Its one-byte body is structural-only and must not
be sent to the typed adapter.

QR uses bounded multi-stream `IQR1` scanning with idle and absolute expiry.
Its standard values are hard V1 ceilings: three active streams, twelve
pre-header frames, 3,072 pre-header bytes, 30 seconds idle, and 180 seconds
absolute; custom policies may only tighten them.
Bind optional expected profile, kind, and schema when constructing the scan
session; wrong-schema streams are quarantined before completion. The
`.peerOptimized` compression policy is shared by all rails and uses zlib only
when it saves at least 32 bytes and one 256-byte shard. If wallet-domain
validation rejects a structurally valid completion, call
`scanSession.quarantine(streamID:)` before resuming capture. Scan input is exact
IQR1 text with no whitespace trimming; explicit Swift scanner uptimes are
throwing and must be finite and nonnegative.
Nearby uses Google Connections point-to-point service
`org.hyperledger.iroha.offline.transfer.v1`, mandatory matching 4...12 ASCII digits,
and canonical Base64URL-no-padding ASCII IPD1 discovery. Only the sender may
start with the zero bootstrap; it adopts the receiver's advertised nonzero
request context before the `IPN1` certificate-bound P-256/HKDF/AES-GCM
session. The adapter marks
a BYTES send complete only after its terminal transfer update succeeds. NFC
uses AID `F0504B45504B524E464301`, exact ISC1 sender checkpoints, 244-byte IPA1
durable BEGIN records, IDA1 durable ACK records, min(local, peer) chunk
negotiation, and GET_STATUS recovery after
ambiguous RF loss. The complete reader runner applies a whole-exchange
73,996-action default even when a peer advertises one-byte chunks. One NFC
profile policy binds request, payment, and acknowledgement to the same profile;
mixed-profile sessions fail closed. Its
`loadOrCreateDurableCheckpoint` callback is one atomic load-or-create/debit/store
boundary and must return the exact durable request- and peer-bound ISC1; the
runner validates it before BEGIN_PAYMENT. `updateDurableCheckpoint` separately
installs the ACK-bearing ISC1 before CONFIRM_ACK. Failure at either durability
boundary emits neither the command it gates nor a replacement debit.

Wire limits are hard-capped at 32 KiB canonical, 24,576 Offline Note encoded,
and 24,576 bounded Kagemusha encoded bytes. NFC messages cannot exceed 24,660
bytes. Nearby timeouts must be finite, positive, and at most 300 seconds; its
receive budget admits the four-record V1 transcript and fails closed on a
fifth. Epoch invalidation suppresses callbacks not yet admitted; an
already-admitted application callback may finish.
Listener callbacks are bounded and reject overload. Terminal send completions
remain exact-once through a separately bounded fallback; if both callback lanes
are stalled and saturated, the final nonblocking path runs inline and therefore
does not promise the configured callback context or global FIFO order.

The portable wire, QR, Nearby cryptography, and NFC state machines live in
`IrohaSwift`; Google Nearby and Core NFC lifecycle adapters live in
`IrohaSwiftMobileTransports`. These `IrohaPeer*V1` APIs have no
MultipeerConnectivity, legacy AID, raw-text, or unauthenticated BYTES fallback.
That scope does not remove the independent Kagemusha ABI21 bulk family.

The application entry points are:

- QR: produce display strings with
  `IrohaPeerQRCodecV1.staticCompleteTextCandidate(...)` or
  `animatedFrameTexts(...)`; feed scanner text to
  `IrohaPeerQRScanSessionV1.ingest(...)`, and call `reset()` when the camera
  session or expected kind changes. The producer preflights the exact Base45
  length before building a complete frame and emits animated strings directly,
  reusing each repeated header string without changing the V1 wire sequence.
- Nearby: authenticate records with `IrohaPeerNearbySessionV1`, and own radio
  lifecycle through `IrohaPeerNearbyConnectionsTransportV1.startAdvertising`,
  `startDiscovering`, `send`, and `stop`. A `send` completion means delivery
  only after the exact payload's terminal transfer update is `.success`; queue
  acceptance is never success.
- NFC: use `IrohaPeerNfcReaderServiceV1.run(...)` for reader mode and
  `IrohaPeerNfcCardSessionControllerV1.start(...)` for an eligible receiver.
  The admission callback receives an ephemeral context and must atomically
  persist and return a distinct `IrohaPeerNfcDurablePaymentAdmissionV1`; pass
  that decoded IPA1 back as `restoredPaymentAdmission` after relaunch. Admission
  and commit callbacks are idempotent because their default and maximum
  five-second deadline makes a timeout ambiguous. A callback that ignores task
  cancellation cannot hold the CardSession past that deadline. Admission and
  COMMIT share one process-wide, queue-free lease: timeout/cancel does not
  release it until the actual callback returns, and retaps fail immediately
  with a distinct saturation failure instead of spawning more tasks. A callback
  that never returns therefore requires process restart. Its late value is not
  installed or published and is loaded from durable storage on the next start.
  IPA1 resumes at byte zero; IDA1 wins after COMMIT. Exact/restored BEGIN
  and COMMIT replays publish the idempotent `.paymentAdmitted` and
  `.acknowledgementReady` state events.
  Reader contact retries default to three attempts over three seconds and are
  hard-capped at ten attempts and 30 seconds. A connect slot is claimed before
  an attempt, so duplicate detection callbacks cannot consume retry budget.
  Apps with a custom transceiver can call the portable
  `IrohaPeerNfcReaderExchangeV1.run(...)` directly, preserving both durable
  checkpoint callbacks.

The bounded Retail V1 Kagemusha handoff uses profile `2` and only schema `0x0102`.
`IrohaPeerKagemushaAdapterV1` rejects every other schema before invoking the
native archive decoder, and its IPM1 adapter fails explicitly above the 24,576
byte whole-offer body ceiling (24,660 bytes with the IPM1 header). The
independent ABI21 APIs remain
`KagemushaQRStreamCodec`, `KagemushaNFCProtocol`, and
`KagemushaNearbyExchange`, with distinct `PKK2*`/`PKKQ1`, the canonical
`F0504B45504B524E464301` SDK NFC AID, and
Bonjour/Multipeer identifiers. They are never negotiated, reinterpreted, or
used as fallback for Retail V1. Full QR, NFC, and native ABI21 archives up to
32 MiB continue to use those rails; Kagemusha Nearby's JSON/text envelope has
its own smaller bound. The profile identifier must not be used for a different
sidecar or demo encoding.

This transport hardening is client-side and requires no backend API change.

The canonical cross-SDK vectors live in `../fixtures/offline/peer_*.json`.
From this directory, run the portable/mobile suites and the mainline Kagemusha
adapter boundary with:

```bash
swift test --filter IrohaPeer
swift test --filter KagemushaPeerTransportTests
```

See the [peer transport V1 guide](../docs/source/peer_transport_v1.md) for byte
layouts, fixture hashes, Android permissions, and durability boundaries.

### Kagemusha offline cash lifecycle

IrohaSwift exposes only Kagemusha offline cash. There is no runtime product-mode
field or wallet-selectable offline API. The native artifact wire contract is
authenticated internally and is not another public API. It has no `mode` field;
the manifest schema/version, ABI, proof backend, transcript, and circuit IDs
identify the exact contract.

Use the typed `KagemushaRecursiveSpend` and
`KagemushaRecursiveSpendCodecs` APIs for top-up, recipient-request creation,
split/append, receiver verification and acknowledgement, and full or partial
redemption. Amounts are canonical atomic `u128` values paired with the
asset-definition scale; callers must reject excess decimal precision instead of
rounding.

Wallet applications own encrypted note state and peer transport. Persist the
opaque bundle, recipient output, optional sender change, artifact binding, and
operation status at each commit boundary. Fetch the complete ABI-21/V4 artifact set,
wrap each one-shot source in `KagemushaRecursiveSpendArtifactStream`, and acquire
it through a `KagemushaRecursiveSpendArtifactCoordinator` created with
`.authenticated(...)` from deployment-provisioned release trust. Keep every proof
operation inside the returned lease's `withInstalledArtifactSet` callback. The
coordinator verifies the exact manifest generation and eight-file inventory:
`ParamsIPA`, processed proving key, processed verifying key, and final-key
selector-zero bootstrap witness for each Eq/Ep parity. The two bounded circuit
parameter records are authenticated inline in the manifest rather than streamed
as extra files. The coordinator verifies lengths, offsets, and digests;
serializes install, use, rotation, and uninstall; and fails stale leases closed.
No network or artifact access belongs on the offline send or receive path.

### Push Devices

`ToriiClient.registerPushDevice` and `unregisterPushDevice` wrap `/v1/notify/devices`. Apps obtain their FCM/APNs token from the platform SDK, then submit the token with canonical request auth for the owning account:

```swift
let body = ToriiPushDeviceRequest(accountId: accountId,
                                  platform: "FCM",
                                  token: fcmToken,
                                  topics: ["activity"])
try await torii.registerPushDevice(body, canonicalAuth: auth)
try await torii.unregisterPushDevice(body, canonicalAuth: auth)
```

### Subscriptions

Subscription plans live on asset definitions and are billed by triggers. Use
`bill_for.period = previous_period` for arrears billing (charge on the first for
last month) or `next_period` for fixed-price plans billed in advance.

```swift
let plan: ToriiSubscriptionPlan = [
    "provider": .string("<provider_account_i105>"),
    "billing": .object([
        "cadence": .object([
            "kind": .string("monthly_calendar"),
            "detail": .object([
                "anchor_day": .number(1),
                "anchor_time_ms": .number(0)
            ])
        ]),
        "bill_for": .object([
            "period": .string("previous_period"),
            "value": .null
        ]),
        "retry_backoff_ms": .number(86_400_000),
        "max_failures": .number(3),
        "grace_ms": .number(604_800_000)
    ]),
    "pricing": .object([
        "kind": .string("usage"),
        "detail": .object([
            "unit_price": .string("0.024"),
            "unit_key": .string("compute_ms"),
            "asset_definition": .string("usd#pay")
        ])
    ])
]

```

Direct subscription mutation helpers are not exposed. Build the equivalent
subscription instructions locally, sign them with wallet key material, and
submit the resulting transaction through `submitTransaction` or
`/v1/pipeline/transactions`.

### Canonical request signing

App-facing Torii endpoints accept optional `X-Iroha-Account`,
`X-Iroha-Signature`, `X-Iroha-Timestamp-Ms`, and `X-Iroha-Nonce` headers.
Use `ToriiCanonicalRequest` to build them; it signs the canonical request plus
the freshness metadata and auto-generates timestamp/nonce values when you do not
pass them explicitly:

```swift
let url = URL(string: "https://torii.example/v1/accounts/<account_i105>/assets?limit=5")!
let headers = try ToriiCanonicalRequest.buildHeaders(
    method: "get",
    url: url,
    accountId: "<account_i105>",
    privateKey: Data(repeating: 7, count: 32)
)
var request = URLRequest(url: url)
headers.forEach { key, value in
    request.setValue(value, forHTTPHeaderField: key)
}
```

### Sora VPN native lease flow

`ToriiClient` exposes the quote-first Sora VPN flow used by native XOR lease
escrow. Request a signed quote, submit the returned `OpenVpnLeaseEscrow`
transaction with the wallet, then create the VPN session with the committed
payment transaction hash and the same metering public key:

```swift
let auth = ToriiCanonicalRequestAuth(
    accountId: "<account_i105>",
    privateKey: Data(repeating: 7, count: 32)
)
let quote = try await torii.createVpnQuote(
    ToriiVpnQuoteCreateRequest(meteringPublicKeyHex: meteringPublicKeyHex),
    canonicalAuth: auth
)
// Submit quote.txInstructions as a signed transaction, then pass its hash:
let session = try await torii.createVpnSession(
    ToriiVpnSessionCreateRequest(
        quoteId: quote.quoteId,
        paymentTransactionHash: paymentHash,
        meteringPublicKeyHex: meteringPublicKeyHex
    ),
    canonicalAuth: auth
)
```

Relay operators submit cumulative receipt/voucher evidence with
`submitVpnReceipt`; the response carries a `SettleVpnLease` instruction so the
operator receives only earned XOR and the customer gets the refundable balance.

> **Account selectors:** Account-scoped helpers (`ToriiClient.getAssets`, `getTransactions`, and matching `IrohaSDK` shortcuts) accept canonical I105 account ids or on-chain account aliases (`name@dataspace` / `name@domain.dataspace`). Torii resolves aliases to canonical account ids before serving the response.

### Detached asset transfers

Use the SDK-owned two-phase `/v1/assets/transfer` flow for online payments. It
prepares exactly one numeric transfer, requires an explicit balance scope and
short creation/TTL window, and never accepts a private key, nonce, arbitrary
metadata, aliases, or legacy field spellings.

```swift
let request = ToriiAssetTransferRequest(
    authority: authority,
    assetDefinitionId: assetDefinitionId,
    assetBalanceScope: "dataspace:10", // or exactly "global"
    amount: "750",
    destination: destination,
    memo: "invoice 42",
    feePayment: .authority(chargeLimits: [], gasLimit: nil),
    creationTimeMs: torii.recommendedCreationTimeMs(),
    transactionTtlMs: 120_000
)
let draft = try await torii.prepareDetachedAssetTransfer(request)

// SigningKey keeps signing local. The public-key/signature overload is also
// available for Keychain or hardware-backed signers.
let submitted = try await torii.submitDetachedAssetTransfer(
    draft,
    signingKey: signingKey
)
let finality = try await torii.waitForDetachedAssetTransferFinality(
    draft,
    submittedResponse: submitted
)
```

Preparation fails closed unless ABI-21 native inspection proves the versioned
scaffold has the exact authority, chain, definition, source scope, amount,
destination, memo, typed fee payer, creation time, TTL, and no extra metadata.
The prepare route obtains the canonical fee quote and replaces only the charge
maxima before returning the scaffold. To select sponsorship, pass
`.sponsor(programId:programRevision:chargeLimits:gasLimit:)` with one exact
`FeeSponsorProgramId` and non-zero immutable revision; there is no account-only
sponsor selector or authority fallback.
Submission locally verifies Ed25519 authority/signature binding, uses native
finalization, and requires Torii's final transaction and entrypoint hashes to
match. `IrohaSDK` forwards the same prepare, submit, and finality methods.

For locally assembled transactions, build the complete unsigned payload first,
then call `quoteAndApplyFees(unsignedPayload:canonicalAuth:)`. Sign the returned
payload without changing any other field. `quoteFees` and
`getFeeSponsorProgram` expose the underlying account-signed
`/v1/fees/quote` and exact program lookup routes. Transaction metadata named
`fee_sponsor`, `gas_asset_id`, or `gas_limit` is retired and rejected.

### Kotodama contract manifests

`ToriiClient.fetchContractManifest(codeHashHex:)` reads
`/v1/contracts/code/{hash}` as a strict `ToriiContractManifestRecord`. The model preserves
the `seiyaku`/`誓約` identity, the branded `kotoage`/`言挙げ`, `hajimari`/`始まり`, and
`kaizen`/`改善` lifecycle surface, exact flat-preorder argument and return schemas, bounded
access hints, triggers, state and error declarations, `kotoba`, and provenance. Unknown
fields, mismatched convenience hashes, English lifecycle aliases, and callbacks that bypass
a declared `kotoage` entrypoint are rejected during decoding. V1 type schemas use one flat
preorder node tape: a `List` node carries only `capacity`, and its element subtree follows it
immediately. The decoder rejects the retired nested `element` field, incomplete or overlong
tapes, and forged `AccountView`, `AssetView`, `AssetDefinitionView`, `DomainView`, `NftView`,
or `QueryPage<View>` shapes.

Contract alias and state reads are also SDK-owned. Use
`resolveContractAlias(_:)` for `/v1/contracts/aliases/resolve` and construct a
throwing `ToriiContractStateQuery` with one typed target (`.address` or
`.alias`) and one typed selector (`.path`, `.paths`, or `.prefix`) before calling
`queryContractState(_:)`. Responses reject unknown fields, duplicate JSON keys,
non-canonical base64, selector/target substitution, invalid pagination, and the
retired per-entry `decode_error` shape. A Torii JSON decode failure is instead a
top-level `ToriiClientError.httpStatus` carrying Torii's stable error envelope.

Wallets must use the two-step detached call flow when the signing key is held by
the client:

```swift
let draft = try await torii.prepareDetachedContractCall(
    ToriiContractCallRequest(
        authority: authority,
        contractAlias: "bisp::hbl.sbp",
        entrypoint: "spend_to_merchant",
        payload: .object(["amount": .string("750")]),
        transactionTtlMs: 120_000,
        gasLimit: 500_000
    )
)
let signature = try signAfterUserPresence(draft.signingMessage)
let response = try await torii.submitDetachedContractCall(
    draft,
    publicKeyHex: publicKeyHex,
    signatureB64: signature.base64EncodedString()
)
let finality = try await torii.waitForDetachedContractCallFinality(
    draft,
    submittedResponse: response
)
```

`ToriiContractCallDraft` retains the normalized request and all resolved
contract, ABI, entrypoint, gas, sponsor, payload, time, and TTL bindings. Submit
accepts only the public key and detached signature; it fails closed unless the
returned receipt and queued pipeline status match the draft exactly.
`waitForDetachedContractCallFinality` then uses the canonical `scope=auto`
pipeline lookup and returns only an applied, globally scoped, state-resolved
status with a positive block height.

### Explorer instruction history

Torii explorer endpoints expose instruction-level data, including transfer details.
Use `getExplorerTransfers` to fetch a page and derive transfer records:

```swift
if #available(iOS 15.0, macOS 12.0, *) {
    let params = ToriiExplorerInstructionsParams(page: 1,
                                                 perPage: 50,
                                                 kind: "Transfer",
                                                 assetDefinitionId: "<base58-asset-definition-id>")
    let transfers = try await torii.getExplorerTransfers(params: params,
                                                         matchingAccount: "<account_i105>")
    for record in transfers {
        switch record.details {
        case .asset(let asset):
            print("transfer:", asset.amount,
                  asset.assetDefinitionId ?? "unknown asset",
                  "from:", asset.senderAccountId ?? "unknown",
                  "to:", asset.destinationAccountId)
        case .assetBatch(let entries):
            for entry in entries {
                print("batch transfer:", entry.amount,
                      entry.assetDefinitionId,
                      "from:", entry.senderAccountId,
                      "to:", entry.receiverAccountId)
            }
        }
    }
}
```

If you prefer a flattened, UI-ready shape, ask for transfer summaries:

```swift
if #available(iOS 15.0, macOS 12.0, *) {
    let summaries = try await torii.getExplorerTransferSummaries(
        params: ToriiExplorerInstructionsParams(page: 1, perPage: 50, kind: "Transfer"),
        matchingAccount: "<account_i105>"
    )
    for summary in summaries {
        print(summary.direction, summary.amount, summary.assetDefinitionId)
    }
}
```

For batch transfers, `transferIndex` tracks the entry position within the instruction payload.
Convenience flags `isIncoming`, `isOutgoing`, and `isSelfTransfer` help with UI direction labels.
If you need to recompute direction for another account or show counterparties, use
`direction(relativeTo:)` and `counterpartyAccountId(relativeTo:)`. Direction helpers also accept
`isIncoming(relativeTo:)`, `isOutgoing(relativeTo:)`, and `isSelfTransfer(relativeTo:)`.
Use `signedAmount(relativeTo:)` when you need a simple +/‑ string for UI totals.
Summaries conform to `Identifiable`, using `transactionHash|instructionIndex|transferIndex` as the
stable identifier.
Use `matchingAccount` or `assetDefinitionId` to filter transfer records and summaries.

For a one-shot transaction history helper, use `getTransactionHistory` (alias of
`getAccountTransferHistory`):

```swift
if #available(iOS 15.0, macOS 12.0, *) {
    let history = try await torii.getTransactionHistory(accountId: "<account_i105>",
                                                        page: 1,
                                                        perPage: 50)
    for item in history {
        print(item.isIncoming ? "in" : "out", item.amount, item.assetDefinitionId)
    }
}
```

You can also pass `assetDefinitionId` or `assetId` to narrow results. The `assetId` filter matches
the source internal asset balance-bucket literal (`<base58-asset-definition-id>#<canonical-i105-account-id>`) as reported by explorer
transfers.
Transaction-scoped helpers (`getExplorerTransactionTransferSummaries`,
`streamTransactionTransferSummaries`) accept the same filters.

To stream multiple pages, use `iterateAccountTransferHistory`:

```swift
if #available(iOS 15.0, macOS 12.0, *) {
    for try await item in torii.iterateAccountTransferHistory(accountId: "<account_i105>",
                                                              perPage: 25) {
        print(item.direction, item.amount, item.assetDefinitionId)
    }
}
```

You can also list transaction summaries or fetch a transaction detail payload:

```swift
if #available(iOS 15.0, macOS 12.0, *) {
    let txPage = try await torii.getExplorerTransactions(
        params: ToriiExplorerTransactionsParams(page: 1, perPage: 25)
    )
    if let first = txPage.items.first {
        let detail = try await torii.getExplorerTransactionDetail(hashHex: first.hash)
        print("transaction status:", detail.status)
    }
}
```

To fetch a single instruction payload, use `getExplorerInstructionDetail` with the transaction hash
and instruction index.

If you need transfer details for a specific transaction, call
`getExplorerTransactionTransferSummaries(hashHex:matchingAccount:)`.
Use `streamTransactionTransferSummaries` or `transactionTransferSummariesPublisher` to keep
receiving live transfer updates for that transaction.

For RWA lots, use the dedicated explorer and chain-state helpers:

```swift
if #available(iOS 15.0, macOS 12.0, *) {
    let lots = try await torii.getExplorerRwas(
        params: ToriiExplorerRwasParams(
            page: 1,
            perPage: 25,
            ownedBy: "<account_i105>",
            domain: "commodities"
        )
    )
    if let first = lots.items.first {
        let detail = try await torii.getExplorerRwaDetail(rwaId: first.id)
        print(detail.quantity, detail.heldQuantity, detail.primaryReference)
    }

    let rwaIds = try await torii.listRwas(options: ToriiListOptions(limit: 10))
    print(rwaIds.items.map(\.id))
}
```

For local instruction composition, `RwaInstructionBuilders` and the matching
`IrohaSDK` convenience methods now cover the dedicated RWA instruction family.
The richer registration/merge/control-policy payloads stay as `NoritoJSON`
objects so callers can pass canonical Rust-side JSON shapes directly:

```swift
let newRwa = try NoritoJSON.fromJSONObject([
    "domain": "commodities",
    "quantity": "10.5",
    "spec": ["scale": 1],
    "primary_reference": "vault-cert-001",
    "metadata": ["origin": "AE"],
    "parents": [],
    "controls": ["freeze_enabled": true]
])
let registerRwa = try sdk.buildRegisterRwa(rwa: newRwa)
let transferRwa = try sdk.buildTransferRwa(
    sourceAccountId: "<source_i105>",
    rwaId: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef$commodities",
    quantity: "1.5",
    destinationAccountId: "<destination_i105>"
)

let metadata = try NoritoJSON(["serial": "vault-01"])
let setMetadata = SetMetadataRequest(
    chainId: "00000000-0000-0000-0000-000000000000",
    authority: "<source_i105>",
    target: .rwa("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef$commodities"),
    key: "serial",
    value: metadata
)
```

To react to new blocks as they commit, subscribe to the explorer SSE streams:

```swift
if #available(iOS 15.0, macOS 12.0, *) {
    for try await instruction in torii.streamExplorerInstructions() {
        if let details = instruction.transferDetails() {
            print("transfer:", details)
        }
    }
}
```

Use `streamExplorerTransactions()` if you only need transaction summaries.
Combine users can call `explorerInstructionsPublisher` / `explorerTransactionsPublisher`.
For a UI-ready transfer feed, use `streamExplorerTransferSummaries(matchingAccount:)` or
`explorerTransferSummariesPublisher`.
Transfer stream helpers accept `matchingAccount`, `assetDefinitionId`, and `assetId` filters.
If you need history plus live updates in one stream, use `streamAccountTransferHistory`.
Combine users can call `accountTransferHistoryPublisher` for the same flow.

### Account addresses

```swift
let address = try AccountAddress.fromAccount(publicKey: Data(repeating: 0, count: 32))
print(try address.canonicalHex())
print(try address.toI105(networkPrefix: 753))
```

Account address domain labels are canonicalized to lowercase ASCII and must not contain whitespace
or reserved characters (`@`, `#`, `$`). Use canonical ASCII/punycode labels when working with IDNs.
Account addresses also validate public key lengths for known algorithms (ed25519 requires 32 bytes;
secp256k1 requires 33 bytes when enabled), and reject empty keys.

### Pipeline submission defaults

`IrohaSDK` posts signed payloads to `/v1/pipeline/transactions` and polls
`/v1/pipeline/transactions/status` until the transaction reaches a terminal state. The
helpers in `TxBuilder` (for example `submitAndWait(transfer:keypair:)`) wrap the same
flow. No additional configuration is required when targeting Torii builds that ship the
pipeline surface.
If `/v1/pipeline/transactions/status` responds with `404`, Torii likely restarted or
evicted the in-memory status cache; the SDK treats this as "pending" and continues polling.
Pipeline submissions include an `Idempotency-Key` header derived from the transaction
hash so retries stay safe; override `sdk.pipelineSubmitOptions.idempotencyKeyFactory` or
set it to `nil` to disable the header when integrating with custom gateways.

### Metadata & governance helpers

`TxBuilder` includes Norito-backed builders for metadata edits and governance actions.
Use the new `NoritoJSON` helper to encode values deterministically before signing:

```swift
let metadata = try NoritoJSON(["region": "eu-west", "tier": 2])
let setMetadata = try SetMetadataRequest(chainId: chainId,
                                         authority: accountId,
                                         target: .account(accountId),
                                         key: "profile",
                                         value: metadata)
let envelope = try sdk.buildSetMetadata(request: setMetadata, signingKey: signingKey)
try await sdk.submit(envelope: envelope)
```

To observe the async flow directly:

```swift
if #available(iOS 15.0, macOS 12.0, *) {
    Task {
        let envelope = try sdk.buildSignedTransfer(transfer: transfer, keypair: kp)
        let status = try await sdk.submitAndWait(envelope: envelope) // POSTS + polls
        print("final state:", status.content.status.state)
    }
}
```

If you need the immediate submission receipt without waiting for a terminal state,
call `torii.submitTransaction(data: envelope.norito)` directly. The returned
`ToriiSubmitTransactionResponse` includes the receipt payload and signature; use
`receipt.hash` (or `receipt.payload.txHash`) to poll with `torii.getTransactionStatus(hashHex:)`.
`submitTransaction` validates the transaction submit schema from `/v1/node/capabilities`
(`data_model_version` + `signed_transaction_schema_hash_hex`) and throws
`ToriiClientError.dataModelMismatch` or
`ToriiClientError.transactionSchemaMismatch` if the node was built from a mismatched release.

`ToriiClient.getMetrics()` requests JSON and requires `Content-Type: application/json`.
Pass `asText: true` to request the text/Prometheus variant.

Swift concurrency wrappers are available on iOS 15/macOS 12 and newer:

```swift
if #available(iOS 15, macOS 12, *) {
    Task {
        let balances = try await torii.getAssets(accountId: accountId, asset: asset, scope: "global")
        print("balances:", balances)

        try await sdk.submit(transfer: transfer, keypair: kp)

        let status = try await sdk.submitAndWait(transfer: transfer, keypair: kp)
        print("final status:", status.content.status.kind)

        let timeSnapshot = try await sdk.getTimeNow()
        print("network time", timeSnapshot.now)
    }
}

### Pipeline status polling

`IrohaSDK` exposes `submitAndWait` helpers (envelope + transfer/mint/burn variants) that
POST to `/v1/pipeline/transactions` and poll `/v1/pipeline/transactions/status` until a
canonical `Applied` status or a failure (`Rejected`/`Expired`) is observed. `Approved` and
`Committed` remain progress states. Tune timing and failure handling via
`PipelineStatusPollOptions` or by setting `sdk.pipelinePollOptions`:

```swift
var options = PipelineStatusPollOptions()
options.pollInterval = 0.25 // seconds between polls
options.timeout = 20        // abort if no status within 20 seconds

if #available(iOS 15, macOS 12, *) {
    let status = try await sdk.submitAndWait(envelope: envelope, pollOptions: options)
    print("hash", status.content.hash, "status", status.content.status.kind)
}
```

`PipelineTransactionState` covers Torii status strings (`.queued`, `.approved`,
`.committed`, `.applied`, `.rejected`, `.expired`) and maps unrecognized values to
`.other("NAME")`.

Completion-based variants return a `Task<Void, Never>` so callers can cancel outstanding
polls. The success state is intentionally not configurable: only exact `Applied` proves
execution. Failures bubble up as `PipelineStatusError.failure` (rejected/expired) or
`PipelineStatusError.timeout` when no terminal status arrives in time. When Torii includes
`rejection_reason`, it is exposed via `PipelineStatusError.rejectionReason` and the localized
error message.

Need to monitor a transaction initiated elsewhere? Use the dedicated helper:

```swift
if #available(iOS 15, macOS 12, *) {
    do {
        let status = try await sdk.pollPipelineStatus(hashHex: "deadbeef")
        print(status.content.status.kind)
    } catch {
        print("pipeline error:", error)
    }
}
```

### Offline transaction queue

Set `sdk.pendingTransactionQueue` to automatically persist signed envelopes when submissions
exhaust their retry budget (for example, while offline). The SDK drains the queue before
each new submission and replays stored envelopes in FIFO order:

```swift
let queueURL = FileManager.default
    .urls(for: .documentDirectory, in: .userDomainMask)[0]
    .appendingPathComponent("pending.queue")
sdk.pendingTransactionQueue = try FilePendingTransactionQueue(fileURL: queueURL)
```

`FilePendingTransactionQueue` stores base64-encoded `SignedTransactionEnvelope` blobs, so
operators can archive or inspect them later. When Torii rejects a replayed transaction the
SDK surfaces `IrohaSDKError.toriiRejected` and leaves the remaining entries untouched so
apps can decide how to remediate.

### Kagemusha Torii API

`ToriiClient` uses only the canonical direct Torii lifecycle:
`GET /v1/offline/readiness`, `POST /v1/offline/top-up`,
`POST /v1/offline/redeem`, and `GET /v1/offline/operations/{operation_id}`.
Use `getKagemushaReadiness(assetDefinitionId:)`, `submitKagemushaTopUp`,
`submitKagemushaRedeem`, and `getKagemushaOperationStatus(operationId:)`.

`KagemushaTopUpRequest` and `KagemushaRedeemRequest` accept only the corresponding
typed Kagemusha Norito archive. They derive the lowercase idempotency key from
the embedded nonzero operation ID; callers cannot override it. Top-up archives
are limited to 512 KiB and redeem archives to 48 MiB, exactly matching Torii.
Keep a submitted
operation and its input note until the operation status reaches final chain
state. A transport timeout or unknown state is not permission to create a new
operation ID.

Artifact and readiness validation requires exact bridge ABI 21 and manifest
schema `kagemusha.offline.recursive_spend.artifact_manifest.v4`. The V4
manifest's eight streamed artifacts are content-addressed and installed
atomically through `KagemushaRecursiveSpendArtifactInstallSessionV4`; a partial,
corrupt, unpromoted, or role-substituted generation never becomes active.
`KagemushaRecursiveSpendReleaseAuthenticationV4` requires the canonical
candidate-bound promotion record in addition to policy, attestation, benchmark,
and review bytes. Circuit parameters remain authenticated inline in the Eq/Ep
profiles.

`ToriiKagemushaReadiness.artifactSet` is required but nullable. A non-null value
binds the authenticated generation, manifest, release-policy and
release-attestation digests, issuance window, proof-pair bound, and asset scale
to both exact recursive verifier records:
`kagemusha_recursive_step_eq_v4_verifier_record` with
`kagemusha-recursive-spend-step-eq-compact-layout-v5`, and
`kagemusha_recursive_step_ep_v4_verifier_record` with
`kagemusha-recursive-spend-step-ep-compact-lineage-v5`. Authenticated
backend construction is not proof admission. A null artifact set requires both
recursive records and backend construction to be unavailable with exactly one
`recursive_v4_registry_unavailable` or `recursive_v4_registry_malformed`
blocker. `proofBackendAvailable` reports exact backend construction
independently.
`recursiveLineageSupported` is true only when the artifact set, distinct active
Eq/Ep records, and backend are all available; its inverse is represented by
`recursive_lineage_unavailable`. `ready` is true only when the complete blocker
set is empty, so unrelated issuer or transfer blockers do not erase valid
backend and lineage facts.

Top-up uses `KagemushaTopUpShieldBuildRequestV4`,
`KagemushaRecursiveSpendTopUpUnsignedV4`, and an authorization over the
canonical ABI-21 digest. After direct Torii submission and authenticated
finality verification, initialize the offline branch with
`KagemushaRecursiveSpendInitLocalRequestV4` and
`KagemushaRecursiveSpend.initSpendV4`. Offline transfer is receiver-initiated:
verify the nonce-bound `KagemushaRecipientPaymentRequest`, construct a
`KagemushaRecursiveSpendAppendLocalRequestV4` with recipient and optional change
branches, call `appendSpendV4`, verify the result locally, and send only the V4
recipient peer-payment archive.

The receiver calls `verifySpendV4`, checks the exact asset, scale, amount,
recipient commitment, verifier window, hop bound, and lineage requirements,
then durably stores the bundle before creating a
`KagemushaReceiverAcknowledgement`. Under `cash_handoff_v1`, the sender has
already irreversibly consumed its inputs and durably bound/signed the exact
payment before transport handoff. `verifiedForSender` verifies a delivery
receipt only; it can never accept, roll back, replace, or claw back the spend.
Replayed peer payments and acknowledgements remain idempotent at the wallet
operation layer.

Redemption uses `KagemushaRecursiveSpendRedeemLocalRequestV4`, the retained
unshield-v3 primitive proof APIs, and
`KagemushaRecursiveSpendRedeemUnsignedV4`. Full redeem has no change branch;
partial redeem binds one offline change branch to the same proof and operation
ID. `buildRedeemV4` produces the authorization-bound build result; finalizing it
returns the canonical V4 request submitted by `submitKagemushaRedeem`.

All accumulator, proof, verifier-record, and finality-proof archives are opaque
to wallet code. The first-release wire API has no separate lineage-witness
archive. Do not reconstruct or mutate proof material outside the typed codecs.

### Native privacy bridge

`PrivacyNativeBridge` exposes the privacy FFI surface as generic raw Norito
archives: `capabilitiesV1()`, `buildProofV1(requestArchive:)`, and
`verifyProofV1(requestArchive:)`. Approved typed proof-builder aliases for
admitted production privacy entrypoints, including
`buildZkAceAuthorizationProofV1(requestArchive:)`, dispatch through the same
production archive paths and remain fail-closed while the privacy rows are
gated. Planned catalog entrypoints stay unexported until their production gates
pass. Native availability in the first release requires exact ABI 21, the privacy
capability/build/verify symbols, and successful Norito probe outputs whose
operation-specific result schema bytes match the called entry point.

All privacy request and response payloads must stay as raw Norito archives.
Swift validates archive magic, length, CRC, the 64 MiB native size cap, and the
operation-specific result schema before returning bytes to callers. Capability
metadata reports `privacy-production-gate-v1`, keeps `productionReady = false`,
and remains fail-closed with missing production gates and no audit references
until real proving, verification, chain admission, witness privacy checks,
deterministic testing, negative/adversarial testing, replay/nullifier rejection
testing, parser/verifier fuzzing, performance gates, and external audit signoff
are complete.

Swift also exposes the deterministic privacy FFI status/error-code contract for
diagnostics and cross-language parity: `ffiStatusError`, `ffiErrorNullPointer`,
`ffiErrorMalformedNorito`, `ffiErrorUnsupportedAlgorithm`,
`ffiErrorProductionDisabled`, and `ffiErrorInvalidRequest`. The stable wire
values are `status_error = 1`, `null_pointer = 1`, `malformed_norito = 2`,
`unsupported_algorithm = 3`, `production_disabled = 4`, and
`invalid_request = 5`; treat them as sanitized status metadata, not proof success.
The confidential-v2 Swift wallet helpers expose
`ConfidentialNoteOpening`, `ConfidentialNoteCommitment.deriveFromOpening`,
`ConfidentialNoteNullifier`, `ConfidentialOwnerTag`,
`ConfidentialNoteEncryption.encryptNote`,
`ConfidentialNoteDecryption.decryptNote`,
`ConfidentialNoteDecryption.decryptNoteWithOwnerTag`,
`PrivacyConfidentialWitnessV1`, `buildConfidentialTransferProofRequestV1`,
`buildConfidentialUnshieldProofRequestV1`,
`LocalZkAssetMerklePathProvider`, and
`ToriiClient.getMerklePathForCommitment(asset:commitment:)`. Default note
decryption derives the expected owner tag from the supplied spend key;
diversified notes must use the explicit expected-owner-tag overload. Decrypted
note plaintext rejects noncanonical length varints before reconstructing the
opening. Confidential note and witness byte-vector contents keep their raw
bytes after the vector length. Direct verifier-record hashes use packed fixed
arrays, hashes inside `Option` or `Vec` use ConstVec element framing, and all
Iroha `Hash` values retain their marker bit. The verifier-record `status` field
uses the canonical four-byte `u32` enum discriminant. Swift
Merkle providers reject ambiguous local frontiers and Torii responses with
duplicate JSON keys, noncanonical integer
fields, non-lowercase fixed32 hex, depth/count drift, root drift,
direction-bit drift, or non-verifying paths before wallet code receives proof
material.

Submission retries can be tuned with `PipelineSubmitOptions` (default: 3 retries, 0.5s
backoff, retrying 429/5xx responses and transport errors). For example:

### Confidential key derivation

Wallets derive the confidential key hierarchy locally:

```swift
let seed = Data(repeating: 0x42, count: 32)
let localKeyset = try ConfidentialKeyset.derive(from: seed)

if #available(iOS 15, macOS 12, *) {
    let sdkKeyset = try await sdk.deriveConfidentialKeyset(seedHex: localKeyset.spendKeyHex)
    assert(sdkKeyset == localKeyset)
}
```

`IrohaSDK.deriveConfidentialKeyset` is a local convenience wrapper around
`ConfidentialKeyset.derive`. No Torii request is made. Provide either
`seedHex` or `seedBase64`; inputs are trimmed automatically, all-zero spend keys are
rejected as inert material, and invalid encodings surface as
`ConfidentialKeyDerivationError`.

### Confidential encrypted payloads

Construct memo envelopes for confidential transfers:

```swift
let payload = try ConfidentialEncryptedPayload(
    ephemeralPublicKey: Data(ephemeralPublicKeyBytes),
    nonce: Data(nonceBytes),
    ciphertext: memoCiphertext
)

let noritoBytes = try payload.serializedPayload()      // bare Norito struct bytes
let envelope = try payload.noritoEnvelope()            // header + CRC64-XZ
```

Each initializer validates the X25519 public key length and rejects low-order
public keys, enforces the XChaCha20-Poly1305 nonce length (24 bytes), and
requires non-empty ciphertext. Use `ConfidentialEncryptedPayload.deserialize(from:)`
to parse existing Norito bytes and `asHexDictionary()` when logging or exporting
the fields.

### Confidential gas schedule

Operators can inspect the active confidential verification costs directly from Torii:

```swift
if #available(iOS 15.0, macOS 12.0, *) {
    if let schedule = try await sdk.getConfidentialGasSchedule() {
        print("proof base:", schedule.proofBase)
        print("per nullifier:", schedule.perNullifier)
    } else {
        print("node has not advertised confidential gas knobs yet")
    }
}
```

`getConfidentialGasSchedule()` wraps `GET /v1/configuration`, parsing the logger/network/
queue sections along with `confidential_gas` when present. When the node has not enabled
confidential proofs yet the helper simply returns `nil`, mirroring the Python/JS DTOs.

### Configuration snapshots

`getConfiguration()` returns the typed snapshot, including transport defaults for streaming:

```swift
if #available(iOS 15.0, macOS 12.0, *) {
    let snapshot = try await sdk.getConfiguration()
    if let soranet = snapshot.transport?.streaming?.soranet {
        print("SoraNet exit:", soranet.exitMultiaddr)
        print("Provision queue cap:", soranet.provisionQueueCapacity)
    }
}
```

### Shield transaction builder

`ShieldRequest` wires encrypted payloads into a `zk::Shield` instruction:

```swift
let payload = try ConfidentialEncryptedPayload(
    ephemeralPublicKey: Data(repeating: 0x11, count: 32),
    nonce: Data(repeating: 0x22, count: 24),
    ciphertext: memoCiphertext
)

let request = try ShieldRequest(
    chainId: chainId,
    authority: AccountId.make(publicKey: keypair.publicKey),
    assetDefinitionId: "66owaQmAQMuHxPzxUN3bqZ6FJfDa",
    fromAccountId: "<account_i105>",
    amount: "42",
    noteCommitment: noteCommitmentBytes, // 32 bytes
    payload: payload,
    ttlMs: 120
)

try await sdk.submit(shield: request, keypair: keypair)
```

The SDK validates the 32-byte commitment, enforces the encrypted payload layout, and signs
the Norito transaction before submitting it to `/v1/pipeline/transactions`. Use
`submitAndWait(shield:pollOptions:)` to block until Torii reports a terminal status.

### Unshield transaction builder

`UnshieldRequest` assembles `zk::Unshield` instructions with proof attachments:

```swift
let proof = try ProofAttachment(
    backend: "halo2/ipa",
    proof: Data(repeating: 0xAB, count: 48),
    verifyingKey: .reference(.init(backend: "halo2/ipa", name: "vk_unshield"))
)

let request = try UnshieldRequest(
    chainId: chainId,
    authority: AccountId.make(publicKey: keypair.publicKey),
    assetDefinitionId: "66owaQmAQMuHxPzxUN3bqZ6FJfDa",
    toAccountId: "<recipient_account_i105>",
    publicAmount: "50",
    inputs: [Data(repeating: 0x10, count: 32)],
    proof: proof,
    rootHint: Data(repeating: 0x44, count: 32)
)

try await sdk.submit(unshield: request, keypair: keypair)
```

`ProofAttachment` emits registry-bound envelopes (`backend`, `proof_b64`, `vk_ref`, optional
`vk_commitment_hex`/`envelope_hash_hex`); embedded key bytes are not accepted by the Swift builder.

### Multisig spec builder

The IOS4 multisig roadmap now ships a Swift builder so apps can assemble deterministic
registration payloads before submitting `MultisigRegister` instructions. The helper mirrors
`MultisigSpec` from the executor data model, validating quorum/TTL/signatory bounds and
exporting the exact JSON layout Torii expects:

```swift
let specBuilder = MultisigSpecBuilder()
    .setQuorum(3)
    .setTransactionTtl(milliseconds: 86_400_000) // 1 day
    .addSignatory(accountId: "<account_i105>", weight: 2)
    .addSignatory(accountId: "<signatory_b_i105>", weight: 1)
    .addSignatory(accountId: "<signatory_c_i105>", weight: 1)

let specPayload = try specBuilder.build()
let specJSON = try specBuilder.encodeJSON(prettyPrinted: true)
```

`MultisigSpecBuilder` enforces the 255-member limit, rejects zero-length TTLs, and ensures
the quorum can actually be met (total signatory weight ≥ quorum). The resulting
`MultisigSpecPayload` encodes signatories as `{ "<encoded_account_id>": weight }`.
Feed the JSON blob directly into your transaction
builder or store it alongside governance approvals for reproducibility. Use
`specPayload.previewProposalExpiry(requestedTtlMs:now:)` to surface the effective TTL
and approximate expiry for proposal/relayer flows; it clamps overrides to the policy cap
and flags when a requested TTL was reduced for UX messaging. Call
`specPayload.enforceProposalExpiry(requestedTtlMs:)` to reject overrides above the cap
before submitting a proposal so clients surface the same error the node would emit.

Submit the registration via the new Norito-backed transaction builders:

```swift
let request = MultisigRegisterRequest(
    chainId: "sora-mainnet",
    authority: "<authority_account_i105>",
    accountId: "<multisig_account_i105>",
    spec: specPayload,
    ttlMs: 120_000
)

// completion handler variant
try sdk.submitAndWait(multisigRegister: request, keypair: councilKeypair) { result in
    switch result {
    case .success(let status):
        print("multisig account registered:", status.kind)
    case .failure(let error):
        print("error:", error)
    }
}

// or async/await
if #available(iOS 15.0, macOS 12.0, *) {
    let status = try await sdk.submitAndWait(multisigRegister: request, keypair: councilKeypair)
    print("registered multisig:", status.kind)
}
```
Choose a controller account id in the same domain as the signatories (the key can be random and
discarded because direct multisig signing is forbidden). Deterministically derived multisig keys are
quarantined; registration requires a non-derivable account id.

The SDK routes the request through the Norito native bridge so transactions are signed
locally and submitted through `/v1/pipeline/transactions` with the same deterministic
encoding the CLI uses.

### Inspect confidential asset policies

Wallets and auditors can poll an asset definition’s confidential policy and pending
transition metadata via `/v1/confidential/assets/{definition_id}/transitions`:

```swift
if #available(iOS 15, macOS 12, *) {
    let policy = try await torii.getConfidentialAssetPolicy(assetDefinitionId: "66owaQmAQMuHxPzxUN3bqZ6FJfDa")
    if let pending = policy.pendingTransition {
        print("Next mode:", pending.newMode, "opens at", pending.windowOpenHeight ?? pending.effectiveHeight)
    }
}
```

`ToriiConfidentialAssetPolicy` exposes the active/pending modes, verifier parameter ids,
and the derived window-open height so UI layers can display countdowns without manual JSON
decoding. The completion-based overload mirrors the async helper for apps that still rely
on callback-first code.

### Verifying key registry

Inspect verifying keys via the Torii helpers:

```swift
if #available(iOS 15, macOS 12, *) {
    let detail = try await torii.getVerifyingKey(backend: "halo2/ipa", name: "payments_v1")
    let current = try await torii.listVerifyingKeys(query: ToriiVerifyingKeyListQuery(backend: "halo2/ipa"))
    print("vk status:", detail.record.status, "count:", current.count)
}
```

### Runtime capabilities

Query runtime adverts to surface ABI metadata:

```swift
if #available(iOS 15, macOS 12, *) {
    let capabilities = try await torii.getNodeCapabilities()
    let metrics = try await torii.getRuntimeMetrics()
    let abiActive = try await torii.getRuntimeAbiActive()
    print("abi:", capabilities.abiVersion,
          "signed_tx_schema:", capabilities.signedTransactionSchemaHashHex ?? "missing",
          "active:", abiActive.abiVersion,
          "upgrades:", metrics.upgradeEventsTotal)
}
```

Completion-based APIs (`getNodeCapabilities(completion:)`, etc.) are also available when
Swift concurrency is not an option.

Generate upgrade instructions via the runtime helpers:

```swift
if #available(iOS 15, macOS 12, *) {
    let manifest = ToriiRuntimeUpgradeManifest(
        name: "Upgrade Foo",
        description: "Refresh runtime provenance",
        abiVersion: 1,
        abiHashHex: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        addedSyscalls: [],
        startHeight: 1_000,
        endHeight: 1_200
    )

    let proposal = try await torii.proposeRuntimeUpgrade(manifest: manifest)
    let proposalInstructions = proposal.txInstructions

    let activation = try await torii.activateRuntimeUpgrade(idHex: String(repeating: "a", count: 64))
    let activationInstructions = activation.txInstructions
    let cancellation = try await torii.cancelRuntimeUpgrade(idHex: String(repeating: "a", count: 64))
    let cancellationInstructions = cancellation.txInstructions
    // Feed the returned `txInstructions` into your transaction builder / submit pipeline.
}
```

```swift
sdk.pipelineSubmitOptions = PipelineSubmitOptions(maxRetries: 5,
                                                 initialBackoffSeconds: 0.25,
                                                 backoffMultiplier: 1.5)
```
Pipeline submissions always use `/v1/pipeline/transactions` and `/v1/pipeline/transactions/status`.

### Verifying key registry

Interact with the Torii verifying-key endpoints to inspect and monitor Halo2 verifier metadata:

```swift
if #available(iOS 15, macOS 12, *) {
    let detail = try await torii.getVerifyingKey(backend: "halo2/ipa", name: "vk_main")
    print("vk status:", detail.record.status)

    let idsOnly = try await torii.listVerifyingKeys(
        query: ToriiVerifyingKeyListQuery(backend: "halo2/ipa", idsOnly: true)
    )
    print("known ids:", idsOnly.map(\.id.name))
}
```

Direct register/update helpers post the Torii app API payloads with explicit
`authority` and `private_key` fields, and validate backend labels plus inline
verifier-key commitments before sending:

```swift
if #available(iOS 15, macOS 12, *) {
    try await torii.registerVerifyingKey(
        ToriiVerifyingKeyRegisterRequest(
            authority: "alice",
            privateKey: "ed25519:...",
            backend: "halo2/ipa",
            name: "vk_main",
            version: 1,
            circuitId: "halo2/ipa::transfer_v1",
            publicInputsSchemaHashHex: String(repeating: "a", count: 64),
            gasScheduleId: "halo2_default",
            verifyingKeyBytes: Data([1, 2, 3]),
            status: .active
        )
    )
}
```

Completion-style overloads still mirror the async read and event-stream helpers
so UI layers can cancel inflight work if needed.

```swift
if #available(iOS 15, macOS 12, *) {
    let stream = torii.streamVerifyingKeyEvents(
        filter: ToriiVerifyingKeyEventFilter(backend: "halo2/ipa", name: "vk_main")
    )

    Task.detached {
        do {
            for try await message in stream {
                switch message.event {
                case .registered(let id, _):
                    print("registered:", id)
                case .updated(_, let record):
                    print("updated to version", record.version)
                @unknown default:
                    break
                }
            }
        } catch {
            print("stream error:", error)
        }
    }
}
```

If you need to observe proof verification outcomes, reuse the same streaming helpers:

```swift
if #available(iOS 15, macOS 12, *) {
    let proofs = torii.streamProofEvents(
        filter: ToriiProofEventFilter(backend: "halo2/ipa", proofHashHex: String(repeating: "a", count: 64))
    )

    Task.detached {
        do {
            for try await message in proofs {
                switch message.event {
                case .verified(let body):
                    print("verified:", body.id.proofHashHex)
                case .rejected(let body):
                    print("rejected:", body.id.proofHashHex)
                }
            }
        } catch {
            print("proof stream error:", error)
        }
    }
}
```

Trigger lifecycle events expose the same async sequence shape:

```swift
if #available(iOS 15, macOS 12, *) {
    let triggers = torii.streamTriggerEvents(
        filter: ToriiTriggerEventFilter(triggerId: "nightly-tick")
    )

    Task.detached {
        for try await message in triggers {
            switch message.event {
            case .created(let id):
                print("trigger created:", id)
            case .deleted(let id):
                print("trigger deleted:", id)
            case .extended(let details):
                print("extended by", details.delta)
            case .shortened(let details):
                print("shortened by", details.delta)
            case .metadataInserted(let change):
                print("metadata inserted:", change.key)
            case .metadataRemoved(let change):
                print("metadata removed:", change.key)
            }
        }
    }
}
```

Adjust the event set by toggling the `includeCreated`, `includeDeleted`, `includeExtended`,
`includeShortened`, `includeMetadataInserted`, and `includeMetadataRemoved` flags on
`ToriiTriggerEventFilter`. The canonical `/v1/events/sse` feed is live-only: its
Swift helpers expose no resume argument and never emit `Last-Event-ID`. A reconnect
can therefore have a gap. If Torii emits terminal `event: stream_error`, the typed
helpers fail with `ToriiClientError.stream(ToriiStreamError)`, preserving the stable
code, message, optional dropped-message count, and replay flag. Malformed terminal
error payloads fail closed as `ToriiClientError.invalidPayload` rather than being
silently filtered as an unrelated event.

### Hardware acceleration (Metal / NEON / StrongBox)

`NoritoNativeBridge` now exposes the same acceleration controls as the Rust host via
`AccelerationSettings`. Defaults match the Rust workspace (Metal enabled on Apple
platforms, CUDA disabled). Configure before encoding or interacting with the bridge:

```swift
// Enable Metal compute kernels and tweak Merkle GPU thresholds.
var accel = AccelerationSettings(enableMetal: true,
                                 merkleMinLeavesMetal: 256,
                                 preferCpuSha2MaxLeavesAarch64: 128)
accel.apply() // Applies to the required native bridge.

// Or initialize the SDK with explicit settings
let tunedSDK = IrohaSDK(baseURL: torii.baseURL, accelerationSettings: accel)

// Load the same structure from an iroha_config JSON file.
if let configURL = Bundle.main.url(forResource: "acceleration", withExtension: "json") {
    do {
        let configSettings = try AccelerationSettings.fromJSONFile(at: configURL)
        let sdkFromConfig = IrohaSDK(baseURL: torii.baseURL, accelerationSettings: configSettings)
        _ = sdkFromConfig // use in your app
    } catch {
        assertionFailure("Invalid acceleration config: \\(error)")
    }
}
```

Setting values to `nil` keeps the engine defaults; negative numbers are ignored. The
bridge automatically applies the default configuration on startup so projects that do
not call `apply()` use the same deterministic defaults.

To surface telemetry and parity evidence in dashboards, read the runtime state before
publishing metrics:

```swift
if let state = AccelerationSettings.runtimeState() {
    print("Metal supported:", state.metal.supported,
          "configured:", state.metal.configured,
          "available:", state.metal.available,
          "parity OK:", state.metal.parityOK)
    print("CUDA supported:", state.cuda.supported)
}
```

`runtimeState()` returns both the applied configuration and the Metal/CUDA runtime
status exposed by the bridge (`available` reflects whether the backend passed parity
self-tests on the current host). The helper returns `nil` when the Norito bridge
symbols are unavailable, matching the behaviour of the setter.

### Norito fixtures & parity

Swift shares the canonical Norito fixtures with Android. Mirror them into
`IrohaSwift/Fixtures` before updating tests or dashboards:

```bash
make swift-fixtures
# or:
scripts/swift_fixture_regen.sh
```

Verify the copied fixtures remain byte-identical to the Android source:

```bash
make swift-fixtures-check
```

Run both the fixture parity check and dashboard validation in one shot:

```bash
make swift-ci
```

The script copies `.norito` artifacts plus supporting JSON manifests from
`java/iroha_android/src/test/resources` (override with
`SWIFT_FIXTURE_SOURCE`/`SWIFT_FIXTURE_OUT`). Keeping the synced directory committed lets
dashboards and regression tests diff Swift fixtures independently of the Android tree.

When the Rust exporter publishes the canonical archive, set
`SWIFT_FIXTURE_ARCHIVE=/path/to/norito-fixtures.tar.gz` (or `.zip`) before running
`make swift-fixtures`. The regeneration script extracts the archive to a temporary
directory, mirrors the contents into `IrohaSwift/Fixtures`, and records the archive
path, digest, and `source_kind=archive` in `artifacts/swift_fixture_regen_state.json`
so CI cadence checks and dashboards continue to track ownership.

### Connect (WalletConnect-style relay)

The SDK ships `ConnectClient` and `ConnectSession` helpers for WebSocket
session management, typed frame exchange, and encrypted envelope handling.
Frame encoding/decoding flows through `ConnectCodec`, which requires the Norito
bridge (throws `ConnectCodecError.bridgeUnavailable` when the XCFramework is
absent). Use `ConnectCrypto` to generate Connect X25519 key pairs and derive
directional session keys from the bridge:

```swift
let connectURL = URL(string: "wss://node.example/v1/connect/ws?sid=\(sid)&role=app")!
// token = token_app or token_wallet from /v1/connect/session
var connectRequest = URLRequest(url: connectURL)
connectRequest.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
let connect = ConnectClient(request: connectRequest)

Task {
    await connect.start()
    do {
        let keyPair = try ConnectCrypto.generateKeyPair()
        let open = ConnectOpen(appPublicKey: keyPair.publicKey,
                               appMetadata: ConnectAppMetadata(name: "Demo dApp", iconURL: nil, description: nil),
                               constraints: ConnectConstraints(chainID: "00000000-0000-0000-0000-000000000000"),
                               permissions: ConnectPermissions(methods: ["sign"]))
        let frame = ConnectFrame(sessionID: Data(),
                                 direction: .appToWallet,
                                 sequence: 0,
                                 kind: .control(.open(open)))
        try await connect.send(frame: frame) // NoritoBridge handles frame encoding when linked.
        while true {
            let received = try await connect.receiveFrame()
            print("frame seq:", received.sequence)
        }
    } catch {
        print("connect setup failed: \(error)")
    }
}
```

`ToriiClient` now exposes the Connect REST surface (`/v1/connect/status`, `/v1/connect/session`, `/v1/connect/app/*`) so you can create sessions, manage the app registry/policy/manifest, and build the WebSocket URL deterministically:

```swift
let torii = ToriiClient(baseURL: URL(string: "https://torii.example")!)
let session = try await torii.createConnectSession(sid: "demo-session")
// Keep tokenManagement server-side for cleanup/status; wallet/app launch URIs carry tokenRelay.
let apps = try await torii.listConnectApps()
let manifest = try await torii.getConnectAdmissionManifest()
let wsRequest = try ConnectClient.makeWebSocketRequest(baseURL: torii.baseURL,
                                                       sid: session.sid,
                                                       role: .app,
                                                       token: session.tokenApp)
let connect = ConnectClient(request: wsRequest)
```

Wallet approval code can derive the relay binding with
`ConnectCrypto.relayAuthHash(sessionID:relayToken:)` before signing the approval
preimage. Keep `session.tokenManagement` server-side for deletion and
per-session status calls.

Encryption/decryption of ciphertext envelopes is handled by the bridge-backed helpers:
derive keys via `ConnectCrypto`, call `session.setDirectionKeys(_:)`, and `ConnectSession`
will decrypt ciphertext frames into `ConnectEnvelope` instances automatically (use
`nextControlFrame()` or `await session.nextEnvelope()` for decrypted payloads).

Persist Connect X25519 keys via `ConnectKeyStore` so wallet approvals can include the
attestation bundle (SHA-256 digest + device label + created-at). The default store writes
to Application Support; inject a custom directory if you need sandboxed storage. Integrity
checks use a canonical JSON ordering; noncanonical HMAC orderings are rejected.
> After deriving direction keys (e.g., via `ConnectCrypto.deriveDirectionKeys`), call
> `ConnectSession.setDirectionKeys(_:)` to unlock automatic decryption of encrypted
> control frames. Use `ConnectEnvelope.decrypt(frame:symmetricKey:)` for direct access
> to the decrypted payload when you need to inspect non-control envelopes.

#### Retry policy

`ConnectRetryPolicy` mirrors the Rust reference implementation (`connect_retry::policy`) so every SDK samples the same exponential back-off with full jitter (base 5 s, cap 60 s). Provide the Connect session identifier as the seed to keep reconnection jitter deterministic across platforms:

```swift
let policy = ConnectRetryPolicy()
let seed = sessionID // 32-byte Data from the Connect session
for attempt in 0..<5 {
    let delayMs = policy.delayMillis(forAttempt: UInt32(attempt), seed: seed)
    try await Task.sleep(nanoseconds: UInt64(delayMs) * 1_000_000)
    try await connect.start()
}
```

The Android and JavaScript SDKs use the same seed/attempt mapping, so reconnect back-off remains identical regardless of the client stack.

Status:
- Envelope encoder is complete; transaction payload encoder is under active development.
- Nexus/Torii v3 surface coverage is in progress; see the workspace `roadmap.md` for the active backlog.
- PRs welcome for additional endpoints and full Norito encoding coverage.

### Governance API helpers

`ToriiClient` now wraps the governance REST endpoints so apps can draft contract deployment proposals, submit ballots, and fetch referendum state without reimplementing the HTTP layer. The responses include Norito transaction skeletons (`tx_instructions`) that you can feed into the SDK transaction builders:

```swift
let proposal = ToriiGovernanceDeployContractProposalRequest(contractAlias: "demo::universal",
                                                            codeHashHex: "f0…",
                                                            abiHashHex: "e1…",
                                                            abiVersion: "1")
let draft = try await torii.submitGovernanceDeployContractProposal(proposal)

// Convert the instruction skeleton into a signed transaction envelope
// (TxBuilder helpers reuse the Norito payload emitted by Torii).
// try txBuilder.submit(envelope: yourConversionHelper(draft.txInstructions))

let tally = try await torii.getGovernanceTally(id: "referendum-123")
print("approve:", tally.approve, "reject:", tally.reject)
```

The same helpers are exposed on `IrohaSDK` via convenience methods (for example,
`sdk.submitGovernancePlainBallot(...)`, `sdk.getGovernanceProposal(idHex:)`). Unlock statistics (`/v1/gov/locks/stats`) accept optional `height` and `referendum_id` filters.

### Norito RPC helper

Use `NoritoRpcClient` when you need direct access to the binary RPC surface (see roadmap
task NRPC-3B). The helper mirrors the JavaScript client and centralises the
`application/x-norito` headers, optional query parameters, and timeout handling.

```swift
import IrohaSwift

let rpc = NoritoRpcClient(
    baseURL: URL(string: "https://torii.dev.sora.net")!,
    session: URLSession(configuration: .ephemeral),
    defaultHeaders: ["User-Agent": "SwiftNRPC/1.0"]
)
let payload = try noritoEncode(typeName: "PipelineSubmitRequestV1",
                               payload: signedEnvelopeBytes)

if #available(iOS 15.0, macOS 12.0, *) {
    Task {
        let response = try await rpc.call(
            path: "/v1/pipeline/submit",
            payload: payload,
            params: ["dry_run": "false"]
        )
        print("submit response bytes:", response.count)
    }
}
```

- Relative/absolute paths are supported and query parameters are percent-encoded.
- `Content-Type`/`Accept` default to `application/x-norito` with per-call overrides and
  removal (`headers: ["Accept": nil]`).
- `NoritoRpcError` exposes the HTTP status code + textual body for non-2xx responses.
- Regression tests live in `IrohaSwift/Tests/IrohaSwiftTests/NoritoRpcClientTests.swift`.

## SoraFS replication-order instructions

`SorafsReplicationInstructionBuilders` emits the exact native V1 JSON variants
and can schema-close them again with `decode(_:)`:

```swift
let issue = try SorafsReplicationInstructionBuilders.issueReplicationOrder(
    orderId: orderId,
    orderPayload: replicationOrderBytes,
    issuedEpoch: 20,
    deadlineEpoch: 28
)
let complete = try SorafsReplicationInstructionBuilders.completeReplicationOrder(
    orderId: orderId,
    providerId: providerId,
    completionEpoch: 27
)
let expire = try SorafsReplicationInstructionBuilders.expireReplicationOrder(
    orderId: orderId,
    expirationEpoch: 29
)
```

IDs must be non-zero lowercase 64-hex strings. Issue validates canonical,
bounded `ReplicationOrderV1` framing, the embedded order ID, target/provider
assignment policy, and deadline ordering. Completion is provider-specific and
always contains `order_id`, `provider_id`, and `completion_epoch`; missing,
legacy, or unknown fields are rejected by the decoder.

## NoritoBridge packaging

The release process for the Norito Swift bindings is documented in
[`docs/norito_bridge_release.md`](../docs/norito_bridge_release.md). Follow the steps to
build the XCFramework, compute the checksum, and update both the Swift Package manifest
and the CocoaPods podspec. The resulting artifacts should share the same semantic version
as the `norito` Rust crate.
The canonical `NoritoBridge.artifacts.json` is embedded in the XCFramework and
records the bridge version plus per-platform SHA-256 hashes.
`dist/NoritoBridge.artifacts.json` is the stable relative symlink to that embedded
manifest; publishing the XCFramework therefore switches both binaries and evidence
through one atomic directory exchange.

### NoritoBridge policy and troubleshooting
- Builds require `dist/NoritoBridge.xcframework`; package resolution fails when the
  artifact is missing or malformed.
- Broken bridge symbols surface `bridgeUnavailable`/`nativeBridgeUnavailable` errors
  that include the expected xcframework location.
- Example: `swift test --package-path IrohaSwift` requires the bridge artifact to be
  materialized first.

## SwiftUI demo and CI

A SwiftUI wallet example (`examples/ios/NoritoDemoXcode`) showcases token balances,
Torii WebSocket subscriptions, and IRH transfers. The Xcode project, Swift sources, and
configuration templates are checked into the repository. Launch the demo by supplying the
Norito bridge XCFramework and populating the `.env` file (keys such as `TORII_NODE_URL`,
`CONNECT_SESSION_ID`, `CONNECT_TOKEN_APP`, `CONNECT_TOKEN_WALLET`, and `CONNECT_CHAIN_ID`
are read on startup). Validation hooks live in `scripts/ci/verify_norito_demo.sh` and will
be extended to run `xcodebuild` once macOS CI runners are available.

For contributor setup and Torii mock ledger instructions, refer to
[`docs/norito_demo_contributor.md`](../docs/norito_demo_contributor.md).

## Development commands

- Run the package tests:

  ```bash
  swift test --package-path IrohaSwift
  ```

- Render/validate the parity + CI dashboards (uses sample feeds by default):

  ```bash
  make swift-dashboards
  ```

  Use `SWIFT_PARITY_FEED` / `SWIFT_CI_FEED` environment variables to point at
  exporter output when available.

- Sync the Norito fixtures used for Swift parity/dashboards:

  ```bash
  make swift-fixtures
  ```

## Documentation & Integration Guides

- SDK overview and APIs: [`docs/source/sdk/swift/index.md`](../docs/source/sdk/swift/index.md)
- Connect quickstart (high-level SDK flow + CryptoKit reference): [`docs/connect_swift_ios.md`](../docs/connect_swift_ios.md)
- Xcode integration guide (NoritoBridgeKit, ChaChaPoly framing, ConnectSession wiring): [`docs/connect_swift_integration.md`](../docs/connect_swift_integration.md)
- SwiftUI demo contributor guide (local Torii setup, acceleration toggles): [`docs/norito_demo_contributor.md`](../docs/norito_demo_contributor.md)
