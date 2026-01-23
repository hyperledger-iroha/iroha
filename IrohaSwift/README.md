# IrohaSwift

Swift SDK targeting Hyperledger Iroha v2 and Sora Nexus (Iroha v3) nodes on Apple platforms.

Features:
- Torii HTTP client (balances, transactions, subscriptions, pipeline recovery, time service, ZK attachments, prover reports, contracts)
- Health & metrics helpers (fetch `/v1/health` text probe and `/v1/metrics` Prometheus/JSON payloads)
- Norito envelope encoder (header + CRC64-XZ)
- Native NoritoBridge integration (required by default; set `IROHASWIFT_USE_BRIDGE=optional` for the Swift-only fallback) powering transfer/mint/burn builders and JSON inspection helpers
- Norito RPC HTTP helper (`NoritoRpcClient`) with binary header/query/timeout handling
- Pipeline submission helpers (POST `/v1/pipeline/transactions` with configurable retries + status polling)
- Ed25519 key management & signing via CryptoKit (iOS 15+)
- Confidential key derivation (`ConfidentialKeyset.derive`) mirroring the Rust HKDF so wallets can obtain `sk_spend`, `nk`, `ivk`, `ovk`, and `fvk` locally, plus Torii wrappers for `/v1/confidential/derive-keyset`
- Runtime capability helpers (`ToriiClient.getNodeCapabilities`, `getRuntimeMetrics`, `getRuntimeAbiActive`) mirroring the Torii `/v1/node/capabilities` and `/v1/runtime/*` surfaces
- Verifying key registry helpers (`ToriiClient.getVerifyingKey`, `listVerifyingKeys`, register/update) covering `/v1/zk/vk` operations

## Installation

`IrohaSwift` replaced the older ad-hoc Swift package names; make sure your dependency
graph points at the renamed module.

### Swift Package Manager (Xcode UI)
1. In Xcode select **File → Add Package Dependencies…**
2. Enter `https://github.com/hyperledger/iroha-swift` and pick the desired branch/tag
   (the `main` branch tracks the latest SDK snapshots in this repository).
3. Add the `IrohaSwift` library product to your application target.

### Swift Package Manager (`Package.swift`)

```swift
// Package.swift
dependencies: [
    .package(
        url: "https://github.com/hyperledger/iroha-swift",
        branch: "main"
    )
],
targets: [
    .target(
        name: "YourApp",
        dependencies: [
            .product(name: "IrohaSwift", package: "iroha-swift")
        ]
    )
]
```

When working from the monorepo, use `.package(name: "IrohaSwift", path: "../../IrohaSwift")`
instead of the Git URL so Xcode consumes the local sources.

#### NoritoBridge policy (SwiftPM)

`Package.swift` expects `dist/NoritoBridge.xcframework` to live next to the repository root and keeps the native bridge on by default. Set `IROHASWIFT_USE_BRIDGE=optional` to continue when the xcframework is missing (a warning is emitted with the expected path), or `IROHASWIFT_USE_BRIDGE=0` to force Swift-only builds even when the xcframework is present (useful for CI stubs). Runtime errors such as `ConnectCodecError.bridgeUnavailable` and `SwiftTransactionEncoderError.nativeBridgeUnavailable` echo the same hint so operators can re-enable the bridge quickly.

CI runs `.github/workflows/swift-packaging.yml` (see `ci/check_swift_spm_validation.sh` and `ci/check_swift_pod_bridge.sh`) to assert the manifest fails when the bridge is required but missing and that Swift-only builds still succeed when the bridge is disabled.

### CocoaPods

```ruby
pod 'IrohaSwift', :podspec => 'https://raw.githubusercontent.com/hyperledger/iroha/main/IrohaSwift/IrohaSwift.podspec'
```

The podspec pulls sources from this repository and requires `dist/NoritoBridge.xcframework`
next to the checkout; `pod lib lint` fails fast when the bridge is missing so releases
bundle the signed xcframework (see `docs/connect_swift_integration.md` for the bundling flow).

Usage:
```swift
import IrohaSwift

let torii = ToriiClient(baseURL: URL(string: "http://127.0.0.1:8080")!)
let sdk = IrohaSDK(baseURL: torii.baseURL)

// Generate Ed25519 keypair (CryptoKit)
let kp = try Keypair.generate()
let accountId = AccountId.make(publicKey: kp.publicKey, domain: "wonderland")

// Fetch balances
sdk.getAssets(accountId: accountId) { result in
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
    assetDefinitionId: "jpy#xst",
    quantity: "1.23",
    destination: "ed0120...@wonderland",
    description: "demo",
    ttlMs: 60_000
)
let envelope = try sdk.buildSignedTransfer(transfer: transfer, keypair: kp)
sdk.submit(envelope: envelope) { err in
    print(err as Any)
}

// Query pipeline status if needed
torii.getTransactionStatus(hashHex: envelope.hashHex) { status in
    print(status)
}

// Await pipeline completion using the helper (falls back to immediate success when
// the endpoint returns no status payload).
sdk.submitAndWait(envelope: envelope) { result in
    print("pipeline status:", result)
}
```

`IrohaSDK` trims and validates chain/account/asset identifiers before signing and fails fast on malformed inputs. Override `creationTimeProvider` when you need deterministic timestamps for fixture generation or offline signing flows.

### Subscriptions

Subscription plans live on asset definitions and are billed by triggers. Use
`bill_for.period = previous_period` for arrears billing (charge on the first for
last month) or `next_period` for fixed-price plans billed in advance.

```swift
let plan: ToriiSubscriptionPlan = [
    "provider": .string("aws@commerce"),
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

let planRequest = ToriiSubscriptionPlanCreateRequest(
    authority: "aws@commerce",
    privateKey: "provider-private-key-hex",
    planId: "aws_compute#commerce",
    plan: plan
)
try await torii.createSubscriptionPlan(planRequest)

let subscriptionRequest = ToriiSubscriptionCreateRequest(
    authority: "alice@users",
    privateKey: "subscriber-private-key-hex",
    subscriptionId: "sub-001",
    planId: "aws_compute#commerce"
)
try await torii.createSubscription(subscriptionRequest)

let usageRequest = ToriiSubscriptionUsageRequest(
    authority: "aws@commerce",
    privateKey: "provider-private-key-hex",
    unitKey: "compute_ms",
    delta: "3600000"
)
try await torii.recordSubscriptionUsage(subscriptionId: "sub-001", requestBody: usageRequest)

let actionRequest = ToriiSubscriptionActionRequest(
    authority: "aws@commerce",
    privateKey: "provider-private-key-hex"
)
try await torii.chargeSubscriptionNow(subscriptionId: "sub-001", requestBody: actionRequest)
```

### Canonical request signing

App-facing Torii endpoints accept optional `X-Iroha-Account` /
`X-Iroha-Signature` headers. Use `ToriiCanonicalRequest` to build them:

```swift
let url = URL(string: "https://torii.example/v1/accounts/alice@wonderland/assets?limit=5")!
let headers = try ToriiCanonicalRequest.buildHeaders(
    method: "get",
    url: url,
    accountId: "alice@wonderland",
    privateKey: Data(repeating: 7, count: 32)
)
var request = URLRequest(url: url)
headers.forEach { key, value in
    request.setValue(value, forHTTPHeaderField: key)
}
```

> **Roadmap ADDR-5a:** Account-scoped helpers (`ToriiClient.getAssets`, `getTransactions`, and the matching `IrohaSDK` shortcuts) now accept canonical, IH58 (preferred), or compressed (`snx1`, second-best) literals and percent-encode the `/v1/accounts/{account_id}/…` segments automatically, so wallets can forward whatever selector they surface without manually escaping `@` or trimming input.

### Account addresses

```swift
let address = try AccountAddress.fromAccount(domain: "default", publicKey: Data(repeating: 0, count: 32))
print(try address.canonicalHex())
print(try address.toIH58(networkPrefix: 753))
print(try address.toCompressedSora())
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
`submitTransaction` validates `data_model_version` from `/v1/node/capabilities` and throws
`ToriiClientError.incompatibleDataModel` if the node was built from a different release.

`ToriiClient.getMetrics()` automatically decodes JSON payloads even when Torii forgets to
set `Content-Type: application/json`, falling back to the Prometheus/text response only
when JSON decoding fails. Pass `asText: true` to force the text variant.

Swift concurrency wrappers are available on iOS 15/macOS 12 and newer:

```swift
if #available(iOS 15, macOS 12, *) {
    Task {
        let balances = try await torii.getAssets(accountId: accountId)
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
terminal status (Approved/Committed/Applied or Rejected/Expired) is observed. Tune the
behaviour via `PipelineStatusPollOptions` or by setting `sdk.pipelinePollOptions`:

```swift
var options = PipelineStatusPollOptions(successStates: Set([.approved, .committed]),
                                       failureStates: Set([.rejected, .expired]))
options.pollInterval = 0.25 // seconds between polls
options.timeout = 20        // abort if no status within 20 seconds

if #available(iOS 15, macOS 12, *) {
    let status = try await sdk.submitAndWait(envelope: envelope, pollOptions: options)
    print("hash", status.content.hash, "status", status.content.status.kind)
}
```

`PipelineTransactionState` covers the common Torii status strings (`.queued`, `.approved`,
`.committed`, `.applied`, `.rejected`, `.expired`) and falls back to `.other("NAME")` for
future values.

Completion-based variants return a `Task<Void, Never>` so callers can cancel outstanding
polls. Failures bubble up as `PipelineStatusError.failure` (rejected/expired) or
`PipelineStatusError.timeout` when no terminal status arrives in time.

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

### Offline receipts and bundles

Use `OfflineReceiptBuilder` to validate offline spend receipts and bundle submissions (spend-key
signature verification, policy enforcement, platform snapshot binding, and aggregate proof root
checks), and `OfflineWallet.buildSignedReceipt` to sign with the spend key while persisting to the
audit log or journal:

```swift
let chainId = "testnet"
let journal = try OfflineJournal(url: journalURL, key: OfflineJournalKey.derive(from: seed))
let receipt = try wallet.buildSignedReceipt(
    chainId: chainId,
    receiverAccountId: certificate.controller,
    amount: "10",
    invoiceId: "inv-001",
    platformProof: proof,
    senderCertificate: certificate,
    signingKey: spendKey,
    journal: journal
)

let claimedDelta = try OfflineReceiptBuilder.aggregateAmount(receipts: [receipt])
let resultingValue = "90" // current balance minus claimedDelta
let initialBlindingHex = "<current-blinding-hex>"
let resultingBlindingHex = "<next-blinding-hex>"
let artifacts = try OfflineBalanceProofBuilder.advanceCommitment(
    chainId: chainId,
    claimedDelta: claimedDelta,
    resultingValue: resultingValue,
    initialCommitmentHex: certificate.allowance.commitment.hexUppercased(),
    initialBlindingHex: initialBlindingHex,
    resultingBlindingHex: resultingBlindingHex
)
let balanceProof = OfflineBalanceProof(
    initialCommitment: certificate.allowance,
    resultingCommitment: artifacts.resultingCommitment,
    claimedDelta: claimedDelta,
    zkProof: artifacts.proof
)
let transfer = try OfflineReceiptBuilder.buildTransfer(
    chainId: chainId,
    receiver: certificate.controller,
    depositAccount: certificate.controller,
    receipts: [receipt],
    balanceProof: balanceProof
)
```

`OfflineReceiptBuilder` sorts receipts by `(counter, tx_id)` unless you set `sortReceipts: false`,
and bundles must not mix counter scopes (App Attest key id vs. marker/provisioned).

Balance proofs are mandatory for settlement; the builder emits the versioned 12,385-byte v1 proof
blob (delta + range proofs) that Torii validates.

To include proof attachments (for example, regulator receipts), build attachments and pass them
into the transfer:

```swift
let chainId = "testnet"
let attachment = try ProofAttachment(
    backend: "halo2/ipa",
    proof: proofBytes,
    verifyingKey: .reference(.init(backend: "halo2/ipa", name: "transfer_v1")),
    verifyingKeyCommitment: vkCommitment,
    envelopeHash: envelopeHash
)
let attachments = OfflineProofAttachmentList(attachments: [attachment])
let transfer = try OfflineReceiptBuilder.buildTransfer(
    chainId: chainId,
    receiver: certificate.controller,
    depositAccount: certificate.controller,
    receipts: [receipt],
    balanceProof: balanceProof,
    attachments: attachments
)
```

If you are composing receipts without `OfflineWallet`, `OfflineReceiptRecorder` wires the journal
and audit logger into `OfflineReceiptBuilder` while still enforcing spend-key and account-id
validation:

```swift
let logger = try OfflineAuditLogger(isEnabled: true)
let recorder = OfflineReceiptRecorder(journal: journal, auditLogger: logger)
let chainId = "testnet"
let seed = Data("receipt-seed".utf8)
let bundleSeed = Data("bundle-seed".utf8)
let receipt = try OfflineReceiptBuilder.buildSignedReceipt(
    txIdSeed: seed,
    chainId: chainId,
    receiverAccountId: certificate.controller,
    amount: "10",
    invoiceId: "inv-002",
    platformProof: proof,
    senderCertificate: certificate,
    signingKey: spendKey,
    recorder: recorder,
    timestampMs: 123
)

let transfer = try OfflineReceiptBuilder.buildTransfer(
    bundleIdSeed: bundleSeed,
    chainId: chainId,
    receiver: certificate.controller,
    depositAccount: certificate.controller,
    receipts: [receipt],
    balanceProof: balanceProof,
    sortReceipts: true
)
```

When attaching aggregate proofs, compute the Poseidon receipts root with
`OfflineReceiptBuilder.computeReceiptsRoot` and populate the envelope before submission. Use
`OfflineAggregateProofMetadataKey` to tag the FASTPQ parameter set and circuit identifiers:

```swift
let metadata: [String: ToriiJSONValue] = [
    OfflineAggregateProofMetadataKey.parameterSet: .string("fastpq-offline-v1"),
    OfflineAggregateProofMetadataKey.sumCircuit: .string("fastpq/offline_sum/v1"),
    OfflineAggregateProofMetadataKey.counterCircuit: .string("fastpq/offline_counter/v1"),
    OfflineAggregateProofMetadataKey.replayCircuit: .string("fastpq/offline_replay/v1"),
]
```

Torii builds FASTPQ witness payloads from the transfer payload
(`POST /v1/offline/transfers/proof`). Use the native bridge helper to derive proof bytes:

```swift
let sumRequest = try await torii.requestOfflineTransferProof(
    .init(transfer: transfer, kind: "sum")
)
let counterRequest = try await torii.requestOfflineTransferProof(
    .init(transfer: transfer, kind: "counter", counterCheckpoint: counterCheckpoint)
)
let replayRequest = try await torii.requestOfflineTransferProof(
    .init(transfer: transfer,
          kind: "replay",
          replayLogHeadHex: replayHeadHex,
          replayLogTailHex: replayTailHex)
)

let proofs = try OfflineReceiptBuilder.generateAggregateProofs(
    sumRequest: sumRequest,
    counterRequest: counterRequest,
    replayRequest: replayRequest
)
let envelope = try OfflineReceiptBuilder.buildAggregateProofEnvelope(
    receipts: receipts,
    proofSum: proofs.sum,
    proofCounter: proofs.counter,
    proofReplay: proofs.replay,
    metadata: metadata
)
```

Note: the native bridge emits deterministic sum/counter/replay proofs (Norito-encoded
`OfflineFastpq*Proof`), and the core verifier enforces them when `proof_mode = "required"`.

`OfflineWallet.syncOfflineState()` refreshes revocations, counter checkpoints, and verdict metadata
from the state snapshot so wallets can enforce policy deadlines without re-fetching each list.

Submission retries can be tuned with `PipelineSubmitOptions` (default: 3 retries, 0.5s
backoff, retrying 429/5xx responses and transport errors). For example:

### Confidential key derivation

Wallets can build the confidential key hierarchy locally or request it from Torii:

```swift
let seed = Data(repeating: 0x42, count: 32)
let localKeyset = try ConfidentialKeyset.derive(from: seed)

if #available(iOS 15, macOS 12, *) {
    let remoteKeyset = try await sdk.deriveConfidentialKeyset(seedHex: localKeyset.spendKeyHex)
    assert(remoteKeyset == localKeyset)
}
```

`IrohaSDK.deriveConfidentialKeyset` mirrors `POST /v1/confidential/derive-keyset` and wraps
the response so apps receive a fully populated `ConfidentialKeyset`. Provide either
`seedHex` or `seedBase64`; inputs are trimmed automatically, and invalid encodings surface
as `ConfidentialKeyDerivationError`.

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

Each initializer validates the X25519 public key (32 bytes) and XChaCha20-Poly1305 nonce
(24 bytes). Use `ConfidentialEncryptedPayload.deserialize(from:)` to parse existing Norito
bytes and `asHexDictionary()` when logging or exporting the fields.

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
    authority: AccountId.make(publicKey: keypair.publicKey, domain: "wonderland"),
    assetDefinitionId: "rose#wonderland",
    fromAccountId: "alice@wonderland",
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
    authority: AccountId.make(publicKey: keypair.publicKey, domain: "wonderland"),
    assetDefinitionId: "rose#wonderland",
    toAccountId: "bob@wonderland",
    publicAmount: "50",
    inputs: [Data(repeating: 0x10, count: 32)],
    proof: proof,
    rootHint: Data(repeating: 0x44, count: 32)
)

try await sdk.submit(unshield: request, keypair: keypair)
```

`ProofAttachment` mirrors the Norito schema (`backend`, `proof_b64`, `vk_ref`/`vk_inline`, optional
`vk_commitment_hex`/`envelope_hash_hex`), keeping Swift clients aligned with Rust and JS builders.

### Multisig spec builder

The IOS4 multisig roadmap now ships a Swift builder so apps can assemble deterministic
registration payloads before submitting `MultisigRegister` instructions. The helper mirrors
`MultisigSpec` from the executor data model, validating quorum/TTL/signatory bounds and
exporting the exact JSON layout Torii expects:

```swift
let specBuilder = MultisigSpecBuilder()
    .setQuorum(3)
    .setTransactionTtl(milliseconds: 86_400_000) // 1 day
    .addSignatory(accountId: "alice@wonderland", weight: 2)
    .addSignatory(accountId: "bob@wonderland", weight: 1)
    .addSignatory(accountId: "carol@wonderland", weight: 1)

let specPayload = try specBuilder.build()
let specJSON = try specBuilder.encodeJSON(prettyPrinted: true)
```

`MultisigSpecBuilder` enforces the 255-member limit, rejects zero-length TTLs, and ensures
the quorum can actually be met (total signatory weight ≥ quorum). The resulting
`MultisigSpecPayload` encodes signatories as the canonical `{ "account@domain": weight }`
map that Rust/CLI pipelines expect. Feed the JSON blob directly into your transaction
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
    authority: "council@sora",
    accountId: "council-multisig@sora",
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
    let policy = try await torii.getConfidentialAssetPolicy(assetDefinitionId: "rose#wonderland")
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

Register, update, and list verifying keys via the Torii helpers:

```swift
if #available(iOS 15, macOS 12, *) {
    let vkBytes = Data(repeating: 0xAA, count: 32)
    var request = ToriiVerifyingKeyRegisterRequest(
        authority: "alice@wonderland",
        privateKey: "ed25519:...",
        backend: "halo2/ipa",
        name: "payments_v1",
        version: 1,
        circuitId: "payments:v1",
        publicInputsSchemaHashHex: String(repeating: "0a", count: 32),
        gasScheduleId: "halo2-default",
        verifyingKeyBytes: vkBytes
    )
    request.metadataUriCid = "ipfs://example-metadata"
    try await torii.registerVerifyingKey(request)

    let current = try await torii.listVerifyingKeys(query: ToriiVerifyingKeyListQuery(backend: "halo2/ipa"))
    print("vk count:", current.count)
}
```

### Runtime capabilities

Query runtime adverts to surface ABI metadata:

```swift
if #available(iOS 15, macOS 12, *) {
    let capabilities = try await torii.getNodeCapabilities()
    let metrics = try await torii.getRuntimeMetrics()
    let abiActive = try await torii.getRuntimeAbiActive()
    print("supported:", capabilities.supportedAbiVersions,
          "active:", abiActive.activeVersions,
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
        description: "Add syscall",
        abiVersion: 2,
        abiHashHex: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        addedSyscalls: [42],
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

Interact with the Torii verifying-key endpoints to inspect and manage Halo2 verifier metadata:

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

Submit verifier lifecycle transactions with the typed request bodies. When embedding inline
bytes supply base64 data and let the helper compute `vk_len` automatically:

```swift
if #available(iOS 15, macOS 12, *) {
    guard let vkBytes = Data(base64Encoded: "AQID") else { return }

    var register = ToriiVerifyingKeyRegisterRequest(
        authority: "alice@wonderland",
        privateKey: "ed0120...",
        backend: "halo2/ipa",
        name: "vk_main",
        version: 1,
        circuitId: "halo2/ipa::transfer_v1",
        publicInputsSchemaHashHex: "fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3",
        gasScheduleId: "halo2_default",
        verifyingKeyBytes: vkBytes
    )
    register.maxProofBytes = 8_192
    try await torii.registerVerifyingKey(register)

    var update = ToriiVerifyingKeyUpdateRequest(
        authority: register.authority,
        privateKey: register.privateKey,
        backend: register.backend,
        name: register.name,
        version: register.version + 1,
        circuitId: register.circuitId,
        publicInputsSchemaHashHex: register.publicInputsSchemaHashHex
    )
    update.commitmentHex = "20574662a58708e02e0000000000000000000000000000000000000000000000"
    try await torii.updateVerifyingKey(update)
}
```

Completion-style overloads mirror the async variants (`registerVerifyingKey(_:completion:)`,
`updateVerifyingKey(_:completion:)`) and return a
`Task<Void, Never>` so UI layers can cancel inflight submissions if needed.

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
`ToriiTriggerEventFilter`. Pair the `lastEventId:` parameter with Torii’s `Last-Event-ID`
to resume streams without missing lifecycle updates.

### Hardware acceleration (Metal / NEON / StrongBox)

`NoritoNativeBridge` now exposes the same acceleration controls as the Rust host via
`AccelerationSettings`. Defaults match the Rust workspace (Metal enabled on Apple
platforms, CUDA disabled). Configure before encoding or interacting with the bridge:

```swift
// Enable Metal compute kernels and tweak Merkle GPU thresholds.
var accel = AccelerationSettings(enableMetal: true,
                                 merkleMinLeavesMetal: 256,
                                 preferCpuSha2MaxLeavesAarch64: 128)
accel.apply() // No-op when the native bridge is not bundled.

// Or initialize the SDK with explicit settings
let tunedSDK = IrohaSDK(baseURL: torii.baseURL, accelerationSettings: accel)

// Load the same structure from an iroha_config-compatible JSON file.
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
not call `apply()` retain the deterministic fallback behaviour.

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
self-tests on the current host). The helper falls back to `nil` when the Norito
bridge is not bundled, matching the behaviour of the setter.

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

### Connect (WalletConnect-style relay, scaffold)

The SDK ships an early `ConnectClient` wrapper that manages the raw WebSocket session. Frame encoding/decoding flows through `ConnectCodec`, which now requires the Norito bridge (throws `ConnectCodecError.bridgeUnavailable` when the XCFramework is absent). Use `ConnectCrypto` to generate Connect X25519 key pairs and derive directional session keys from the bridge:

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
let apps = try await torii.listConnectApps()
let manifest = try await torii.getConnectAdmissionManifest()
let wsRequest = try ConnectClient.makeWebSocketRequest(baseURL: torii.baseURL,
                                                       sid: session.sid,
                                                       role: .app,
                                                       token: session.tokenApp)
let connect = ConnectClient(request: wsRequest)
```

Encryption/decryption of ciphertext envelopes is handled by the bridge-backed helpers:
derive keys via `ConnectCrypto`, call `session.setDirectionKeys(_:)`, and `ConnectSession`
will decrypt ciphertext frames into `ConnectEnvelope` instances automatically (use
`nextControlFrame()` or `await session.nextEnvelope()` for decrypted payloads).

Persist Connect X25519 keys via `ConnectKeyStore` so wallet approvals can include the
attestation bundle (SHA-256 digest + device label + created-at). The default store writes
to Application Support; inject a custom directory if you need sandboxed storage.
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
let proposal = ToriiGovernanceDeployContractProposalRequest(namespace: "apps",
                                                            contractId: "demo.contract.v1",
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

## NoritoBridge packaging

The release process for the Norito Swift bindings is documented in
[`docs/norito_bridge_release.md`](../docs/norito_bridge_release.md). Follow the steps to
build the XCFramework, compute the checksum, and update both the Swift Package manifest
and the CocoaPods podspec. The resulting artifacts should share the same semantic version
as the `norito` Rust crate.
`dist/NoritoBridge.artifacts.json` should accompany the XCFramework and record the
bridge version plus per-platform SHA-256 hashes.

### NoritoBridge policy and troubleshooting
- Default builds link `dist/NoritoBridge.xcframework`. The manifest fails fast when the
  xcframework is missing; set `IROHASWIFT_USE_BRIDGE=optional` to build the Swift-only
  fallback or `IROHASWIFT_USE_BRIDGE=0` to disable bridge loading entirely (useful for CI
  or when the binary is unavailable).
- When the bridge is disabled or missing, SDK surfaces return
  `bridgeUnavailable`/`nativeBridgeUnavailable` errors that include the env hint
  (`IROHASWIFT_USE_BRIDGE`) and the expected xcframework location.
- Running without the bridge will skip native Norito encoding and Connect crypto helpers;
  use the Swift-only encoder paths and expect Connect codec calls to throw
  `ConnectCodecError.bridgeUnavailable`.
- Example: `IROHASWIFT_USE_BRIDGE=optional swift test --package-path IrohaSwift` builds
  with the fallback; restore the env var or place the xcframework to re-enable native
  helpers.

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
