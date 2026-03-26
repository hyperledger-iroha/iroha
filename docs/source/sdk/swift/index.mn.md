---
lang: mn
direction: ltr
source: docs/source/sdk/swift/index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8d41aebe9f16cb2796bf6ba1a13c6b8cfce4d070a52e1b1a81fc1ebbfaa83080
source_last_modified: "2026-02-05T14:42:48.419847+00:00"
translation_last_reviewed: 2026-02-07
title: Iroha Swift SDK Overview
summary: Landing page for installing IrohaSwift, running the quickstart, and understanding the Norito pipeline/connect helpers referenced by IOS2/IOS5 roadmap tasks.
---

# Iroha Swift SDK

The Swift SDK (IrohaSwift) targets iOS and macOS clients that require deterministic
Norito encoding, `/v1/pipeline` submission, and the Connect/WebSocket surfaces used in
Sora Nexus. It ships as a Swift Package (`IrohaSwift/Package.swift`) and can also be
embedded via CocoaPods or XCFramework ZIPs.

## Installing IrohaSwift

The package formerly published under ad-hoc names has been renamed to `IrohaSwift`.
Point your dependency manager at the new Git URL to pick up the latest snapshots from
this repository.

- **Xcode SPM UI:** `File → Add Package Dependencies…` →
  `https://github.com/hyperledger/iroha-swift` (select the `main` branch or a tagged
  release) → add the `IrohaSwift` product to your targets.
- **`Package.swift`:**

  ```swift
  dependencies: [
      .package(
          url: "https://github.com/hyperledger/iroha-swift",
          branch: "main"
      )
  ],
  targets: [
      .target(
          name: "DemoApp",
          dependencies: [
              .product(name: "IrohaSwift", package: "iroha-swift")
          ]
      )
  ]
  ```

- **CocoaPods:** `pod 'IrohaSwift', :podspec => 'https://raw.githubusercontent.com/hyperledger/iroha/main/IrohaSwift/IrohaSwift.podspec'`

When developing from a checked-out workspace you can keep using the relative path variant
(`.package(name: "IrohaSwift", path: "../../IrohaSwift")`) to avoid fetching over the
network.

### Bridge delivery and platform minimums
- Toolchain/platform: Swift 5.9+ with iOS 15+ or macOS 12+ for both SPM and CocoaPods.
- Bridge: `dist/NoritoBridge.xcframework` and `dist/NoritoBridge.artifacts.json` must ship with the app or pod (the manifest records the bridge version plus per-platform SHA-256 hashes). `ci/check_swift_spm_validation.sh` exercises the manifest with and without the bridge and fails when a required build sees a missing xcframework; `ci/check_swift_pod_bridge.sh` lints the podspec with the bundled bridge so pod consumers stay in parity with SwiftPM binary targets. Both run in `.github/workflows/swift-packaging.yml`.
- Policy: bridge loading is automatic. When `dist/NoritoBridge.xcframework` is present, native helpers are enabled; when it is absent, Swift-only fallback is used. SDK helper failures surface `bridgeUnavailable`/`nativeBridgeUnavailable` with the expected bridge location.

## Quickstart

```swift
import IrohaSwift

let torii = ToriiClient(baseURL: URL(string: "http://127.0.0.1:8080")!)
var sdk = IrohaSDK(baseURL: torii.baseURL)

let keypair = try Keypair.generate()
let accountId = AccountId.make(publicKey: keypair.publicKey)

let transfer = TransferRequest(
    chainId: "00000000-0000-0000-0000-000000000000",
    authority: accountId,
    assetDefinitionId: "66owaQmAQMuHxPzxUN3bqZ6FJfDa",
    quantity: "1.23",
    destination: accountId,
    description: "demo",
    ttlMs: 60_000
)

if #available(iOS 15.0, macOS 12.0, *) {
    Task {
        let balances = try await torii.getAssets(accountId: accountId)
        print("balances", balances)

        let status = try await sdk.submitAndWait(transfer: transfer, keypair: keypair)
        print("pipeline status", status.content.status.kind)
    }
}
```

## SM2 Cryptography

`Sm2Keypair` wraps the NoritoBridge SM2 helpers so Swift clients can derive
deterministic keys from seeds, compute canonical multihashes, and sign or verify
messages without reimplementing the algorithm. When the bridge is not linked the
APIs surface `Sm2Error.bridgeUnavailable`.

```swift
let seed = Data("iroha-rust-sdk-sm2-deterministic-fixture".utf8)
let pair = try Sm2Keypair.deriveFromSeed(distid: "iroha-sdk-sm2-fixture", seed: seed)

let message = Data("swift sm2 demo".utf8)
let signature = try pair.sign(message: message)

print("prefixed multihash", try pair.publicKeyPrefixed())
print("SM2 ZA", try pair.computeZA().map { String(format: "%02X", $0) }.joined())

if try pair.verify(message: message, signature: signature) {
    print("signature verified")
}
```

Use `Sm2Keypair.defaultDistid()` to query the runtime default distinguishing
identifier and `Sm2Error.invalidKeyLength`/`invalidSignatureLength` guards when
marshalling raw buffers. The canonical fixture in `fixtures/sm/sm2_fixture.json`
is reused by Rust, Python, JavaScript, and Swift; CI enforces parity via
`ci/check_sm2_sdk_fixtures.sh`.

## Pipeline Submission & Polling

- `submitAndWait` performs `POST /v1/pipeline/transactions` and polls
  `/v1/pipeline/transactions/status` until the transaction reaches a terminal state.
- A `404` from `/v1/pipeline/transactions/status` means Torii has no cached status yet
  (for example after a restart); the Swift SDK treats this as "pending" and keeps polling.
- `pollPipelineStatus` monitors a hash that may have been submitted by another SDK or CLI.
- `PipelineStatusPollOptions` configures polling interval, timeout, max attempts, and the
  typed `PipelineTransactionState` sets used to classify success/failure. Defaults treat
  Approved/Committed/Applied as success and Rejected/Expired as failure.
- `PipelineSubmitOptions` controls retry behaviour for transaction submission
  (defaults: 3 retries, 0.5s backoff, multiplier 2.0, retrying 429/5xx and transport
  errors).
- `pipelineEndpointMode` toggles between the modern `/v1/pipeline/*` endpoints and the
  Torii nodes that have not adopted the pipeline routes yet.
- Completion-based APIs return a `Task<Void, Never>` so callers can cancel outstanding
  polls from UI layers.
- The `NoritoDemoXcode` sample ships with the pipeline helpers enabled out of the box; it
  surfaces live status transitions (`Queued`, `Approved`, etc.) while polling.
- CI smoke coverage for the XcodeGen template and SwiftUI demos runs via
  `ci/check_swift_samples.sh`; see `docs/source/sdk/swift/swift_sample_smoke_tests.md`
  for destinations, skips, and DerivedData paths used in IOS5 sample gates.
- Roadmap owners can follow the end-to-end adoption runbook in
  [`pipeline_adoption_guide.md`](pipeline_adoption_guide.md), which documents the
  retry/idempotency knobs, evidence capture expectations, and telemetry hooks that gate
  IOS2-WB2.

## Norito RPC helper

Roadmap item **NRPC-3B** adds a Swift helper that mirrors the JavaScript
`NoritoRpcClient`. Use it when you need direct access to the binary
`application/x-norito` endpoints (submitters, manifests, or future RPC
extensions) without re-implementing header logic or timeout plumbing.

```swift
import IrohaSwift

let session = URLSession(configuration: .ephemeral)
let rpc = NoritoRpcClient(
    baseURL: URL(string: "https://torii.dev.sora.net")!,
    session: session,
    defaultHeaders: ["User-Agent": "SwiftNRPC/1.0"],
    timeout: 10
)

let requestBody = try noritoEncode(typeName: "PipelineSubmitRequestV1",
                                   payload: Data(pipelineBytes))

if #available(iOS 15.0, macOS 12.0, *) {
    Task {
        let response = try await rpc.call(
            path: "/v1/pipeline/submit",
            payload: requestBody,
            params: ["dry_run": "false"]
        )
        // `response` contains the binary Norito payload returned by Torii.
        print("submit response bytes:", response.count)
    }
}
```

Key facts:

- Accepts absolute or relative paths and handles query parameters/percent encoding.
- Defaults `Content-Type`/`Accept` to `application/x-norito`, with overrides/removal
  supported via the `headers` and `accept` parameters.
- Propagates per-call timeouts (seconds) and exposes the HTTP status/body via
  `NoritoRpcError` on non-2xx responses.
- Regression tests live in
  `IrohaSwift/Tests/IrohaSwiftTests/NoritoRpcClientTests.swift`.

### Offline queueing

Set `IrohaSDK.pendingTransactionQueue` when a client needs to stage submissions while
offline. With a queue configured, the SDK:

- Persists every `SignedTransactionEnvelope` that exhausts its retry budget (network
  errors, 429/5xx responses) via the pluggable `PendingTransactionQueue`.
- Flushes the queue before sending new envelopes, replaying entries in FIFO order while
  preserving their Norito payloads and transaction hashes.
- Requeues entries automatically when replay attempts fail so operators can retry later or
  inspect the on-disk artefacts.

`FilePendingTransactionQueue` stores base64-encoded JSON records (one per line) and works
well for iOS/macOS apps that can supply an Application Support path:

```swift
let queueURL = FileManager.default
    .urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
    .appendingPathComponent("pending.queue")
sdk.pendingTransactionQueue = try FilePendingTransactionQueue(fileURL: queueURL)
```

When Torii rejects a replayed transaction the SDK surfaces `IrohaSDKError.toriiRejected`
and leaves the remaining entries untouched, allowing wallets to present the failure and
decide whether to discard or resubmit the affected envelope.

### Offline circulation modes

`OfflineWallet` now exposes `OfflineWalletCirculationMode` so apps can distinguish between
ledger-reconcilable allowances and pure offline/bearer campaigns:

```swift
let wallet = try OfflineWallet(
    toriiClient: torii,
    auditLoggingEnabled: true,
    circulationMode: .ledgerReconcilable) { mode, notice in
        bannerView.show(title: notice.headline, message: notice.details)
    }

wallet.setCirculationMode(.offlineOnly)

guard wallet.requiresLedgerReconciliation else {
    logger.notice("Skipping Torii sync: offline bearer mode active")
    return
}

try await wallet.fetchTransfers(params: ToriiOfflineListParams(limit: 25))
```

`ToriiOfflineListParams` mirrors the convenience filters exposed by Torii —
pass `assetId`, `controllerId`, `receiverId`, `depositAccountId`,
`certificateExpiresBeforeMs/AfterMs`, `policyExpiresBeforeMs/AfterMs`,
`refreshBeforeMs/AfterMs`, `attestationNonceHex`, `verdictIdHex`,
`requireVerdict`, or `onlyMissingVerdict` directly to the struct instead of
composing JSON predicates. The helper lowercases verdict IDs and rejects invalid
combinations before the request is executed, keeping the Swift surface aligned
with the OA11 roadmap guarantees.

`OfflineReceiptChallenge.encode(chainId, ...)` reuses the shared native helper to emit the canonical
Norito payload plus the chain-bound `irohaHash`/`clientDataHash` pair that Apple App Attest and
Android KeyMint expect. Receipt `amount` strings must use the allowance's canonical scale (asset
definition scale when specified; otherwise the allowance amount scale) to match the ledger
verifier. Use the `expectedScale` overload to enforce scale locally, and call it before generating
platform proofs so every device feeds the exact same bytes into the attestation
chain.【IrohaSwift/Sources/IrohaSwift/OfflineReceiptChallenge.swift:1】【IrohaSwift/Tests/IrohaSwiftTests/OfflineReceiptChallengeTests.swift:1】

### Offline receipt builders

`OfflineReceiptBuilder` validates receipts and bundles before submission, including spend-key
signature verification, account-id/policy checks, platform snapshot policy binding, aggregate
proof root matching, and challenge-hash verification for App Attest/provisioned proofs. Use
`OfflineWallet.buildSignedReceipt` to sign with the spend key and append to the journal and audit
log in one call. Pass `chainId` so the challenge hash is bound to the target network:

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

Balance proofs are required for settlement; `OfflineBalanceProofBuilder` emits the versioned
12,385-byte v1 proof blob (delta + range proofs) that Torii expects.

If you need deterministic IDs or direct journal/audit wiring without `OfflineWallet`, use
`OfflineReceiptRecorder` alongside the builder:

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
    OfflineAggregateProofMetadataKey.sumCircuit: .string("fastpq/offline_sum/v2"),
    OfflineAggregateProofMetadataKey.counterCircuit: .string("fastpq/offline_counter/v2"),
    OfflineAggregateProofMetadataKey.replayCircuit: .string("fastpq/offline_replay/v2"),
]
```

Torii builds FASTPQ witness payloads from the transfer payload
(`POST /v1/offline/transfers/proof`). Feed the JSON into
`OfflineReceiptBuilder.generateAggregateProofs` to get proof bytes (requires the native bridge):

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

### Inspector provisioning proofs

Swift ships `AndroidProvisionedProof` so kiosk tooling and POS wallets can load
the `proof.json` artefacts emitted by `cargo xtask offline-provision`, validate
the Norito hash literal + inspector signature, and re-encode the manifest
before attaching it to OA10.3 allowances:

```swift
let proofURL = URL(fileURLWithPath: "fixtures/offline_provision/kiosk-demo/proof.json")
let proof = try AndroidProvisionedProof.load(from: proofURL)
let manifest = proof.deviceManifest
let canonical = try proof.encodedData(prettyPrinted: true)
```

The helper normalises the canonical hash literal (`hash:...#....`), exposes
`deviceId`/`challengeHashData` for downstream attestations, and keeps the
inspector signature in uppercase hex so OA10.3a flows align with the Norito
schema documented in `offline_allowance.md`.【IrohaSwift/Sources/IrohaSwift/AndroidProvisionedProof.swift:1】

Use the `.notice` payload to surface disclosures/localised copy and fall back to the default handler
when no custom UI is supplied. Additional risk guidance lives in `docs/source/offline_bearer_mode.md`.

### Offline cash setup, load, and refresh

Use the `ToriiOfflineCash*Request` and `ToriiOfflineCashEnvelope` models when calling the
authenticated offline cash endpoints directly. The returned envelope is authoritative for the current
`account_id` / device binding tuple:

```swift
let envelope = try await authenticatedTransport.post(
    "/v1/offline/cash/setup",
    body: ToriiOfflineCashSetupRequest(
        accountId: controllerId,
        assetDefinitionId: assetDefinitionId,
        deviceBinding: deviceBinding,
        deviceProof: deviceProof
    ),
    decode: ToriiOfflineCashEnvelope.self
)

print("lineage id", envelope.lineageState.lineageId)
print("balance", envelope.lineageState.balance)
```

To move value offline, call `/v1/offline/cash/load`. The issuer debits the online balance, updates the
offline cash lineage, and returns the new authoritative envelope. Authorization refreshes use
`/v1/offline/cash/refresh` for the same lineage and never mint value:

```swift
let loaded = try await authenticatedTransport.post(
    "/v1/offline/cash/load",
    body: ToriiOfflineCashLoadRequest(
        operationId: UUID().uuidString,
        lineageId: envelope.lineageState.lineageId,
        accountId: controllerId,
        assetDefinitionId: assetDefinitionId,
        amount: "100.00",
        deviceBinding: deviceBinding,
        deviceProof: deviceProof
    ),
    decode: ToriiOfflineCashEnvelope.self
)

let refreshed = try await authenticatedTransport.post(
    "/v1/offline/cash/refresh",
    body: ToriiOfflineCashRefreshRequest(
        operationId: UUID().uuidString,
        lineageId: loaded.lineageState.lineageId,
        accountId: controllerId,
        deviceBinding: deviceBinding,
        deviceProof: deviceProof
    ),
    decode: ToriiOfflineCashEnvelope.self
)
```

Use `/v1/offline/cash/sync`, `/v1/offline/cash/redeem`, and `getOfflineRevocationBundle()` for the rest of
the offline cash lifecycle. The pre-release allowance/certificate/settlement helpers were removed from
the public SDK surface.

### Offline audit logging

When `auditLoggingEnabled` is `true`, `OfflineWallet` writes `{sender, receiver, asset, amount, timestamp}` entries to
`Documents/offline_audit_log.json` (or a custom `storageURL`). Use `fetchTransfersWithAudit` to reconcile bundles and
`recordTransferAudit(_:)` for bespoke flows:

```swift
let wallet = try OfflineWallet(
    toriiClient: torii,
    auditLoggingEnabled: true,
    auditStorageURL: customDirectory?.appendingPathComponent("audit.json"))

// Automatically capture every bundle that Torii returns.
let transfers = try await wallet.fetchTransfersWithAudit(params: ToriiOfflineListParams(limit: 100))

// Manually log a bundle (e.g. after custom filtering).
if let first = transfers.items.first {
    wallet.recordTransferAudit(first)
}

// Export/clear the journal when regulators request it.
let json = try wallet.exportAuditJSON()
try wallet.clearAuditLog()
```

`recordTransferAudit(_:)` inspects the transfer payload, falls back to receiver/deposit metadata when receipts are missing,
and keeps the log deterministic so the OA5.1 audit toggle can be flipped per jurisdiction without bespoke plumbing.

### Revocation bundle journal

Offline cash now uses the signed revocation bundle returned by `/v1/offline/revocations` plus the
current `OfflineSpendAuthorization` carried in each lineage envelope. Persist the latest envelope and
revocation bundle together so wallets can fail closed for send when authorization or revocation
freshness expires.

```swift
let bundle = try await torii.getOfflineRevocationBundle()
print("revocation bundle expires", bundle.expiresAtMs)
```

### Counter journal

Offline cash mutations and offline transfer receipts continue to use monotonic App Attest / marker
counters. Persist the counters alongside the lineage envelope and transfer journal; there is no
separate counter-summary endpoint in the offline cash cutover.

## SoraFS orchestrator client

`SorafsOrchestratorClient` wraps the same native Norito bridge used by the CLI parity harness, making
it easy to rerun multi-provider fetches without shelling out to `sorafs_cli`. The async API returns
both the assembled payload bytes and the typed `SorafsGatewayFetchReport` structure:

```swift
if #available(iOS 15.0, macOS 12.0, *) {
    let client = SorafsOrchestratorClient()
    Task {
        let parity = try await client.fetch(
            plan: orchestratorFixture.plan,
            providers: orchestratorFixture.providerSpecs(at: fixturesDir, payload: payloadBytes),
            options: SorafsGatewayFetchOptions(telemetryRegion: "ci")
        )
        print("provider reports", parity.report.providerReports)
    }
}
```

- `fetch(plan:providers:options:)` accepts strongly typed fixtures and `SorafsGatewayFetchOptions`.
- `fetchRaw(planJSON:providersJSON:optionsJSON:)` replays the canonical JSON blobs under
  `fixtures/sorafs_orchestrator/`.
- Both methods accept a `cancellationHandler` so UI layers can tear down inflight fetches when a task
  is cancelled.

See `IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift` and the parity suite
(`IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`) for reference usage.

### DA manifest + proof-of-availability helpers

`ToriiClient.getDaManifestBundle(storageTicketHex:)` calls `/v1/da/manifests/{ticket}` and returns the
canonical manifest bytes, decoded Norito JSON, and chunk plan (`ToriiDaManifestBundle`). Pair it with
`ToriiClient.fetchDaPayloadViaGateway(...)` to mirror the `iroha app da prove-availability` flow inside Swift:

```swift
let torii = ToriiClient(baseURL: toriiURL)
let manifest = try await torii.getDaManifestBundle(storageTicketHex: ticketHex)
let providers = [
    try SorafsGatewayProvider(
        name: "gw-usw2",
        providerIdHex: "<provider hex>",
        baseURL: URL(string: "https://gateway-usw2.example")!,
        streamTokenB64: creds.streamTokenB64
    )
]
let session = try await torii.fetchDaPayloadViaGateway(
    manifestBundle: manifest,
    providers: providers,
    options: SorafsGatewayFetchOptions(telemetryRegion: "us-west-2")
)
print("assembled bytes", session.gatewayResult.payload.count)
print("scoreboard", session.gatewayResult.report.scoreboard ?? [])
print("telemetry region", session.gatewayResult.report.telemetryRegion ?? "<unset>")

The `telemetryRegion` mirrors the CLI’s `--telemetry-region` flag so evidence bundles and
scoreboard metadata line up between Swift and the Rust tooling.
```

`fetchDaPayloadViaGateway` accepts either a storage ticket (it will refetch the manifest) or a cached
`ToriiDaManifestBundle`, derives the chunker handle automatically, and reuses `SorafsOrchestratorClient`
under the hood. The helper returns `ToriiDaGatewayFetchResult`, which exposes the manifest metadata,
chunk plan JSON, final payload bytes, and the orchestrator report so SDKs can persist the same evidence
bundle as the CLI. See `ToriiClientTests` for regression coverage.

When `proofSummaryOptions` are supplied the client invokes the native bridge’s
`connect_norito_da_proof_summary` helper and decodes the JSON into a typed `ToriiDaProofSummary` /
`ToriiDaProofRecord` structure. This mirrors the `iroha app da prove-availability` output (hashes, offsets,
per-proof Merkle paths) without forcing apps to parse raw JSON. Options control sampling (`sampleCount`,
`sampleSeed`) and can force specific leaf indexes for deterministic tests. The proof engine is provided by
`NativeDaProofSummaryGenerator` by default, but a custom `DaProofSummaryGenerating` implementation can be
injected for mocks or pre-computed summaries:

```swift
let summaryOptions = ToriiDaProofSummaryOptions(sampleCount: 2, sampleSeed: 0xDEADBEEF)
let session = try await torii.fetchDaPayloadViaGateway(
    manifestBundle: manifest,
    providers: providers,
    proofSummaryOptions: summaryOptions
)
if let summary = session.proofSummary {
    print("blob hash", summary.blobHashHex)
    print("first proof leaf bytes", summary.proofs.first?.leafBytes.count ?? 0)
}
```

#### Proof summary artefacts

`ToriiDaProofSummaryArtifact` converts a `ToriiDaProofSummary` (from `fetchDaPayloadViaGateway` or a
direct `NativeDaProofSummaryGenerator` call) into the Norito JSON bundle emitted by
`iroha app da prove-availability`. Pair it with `DaProofSummaryArtifactEmitter.emit(...)` to optionally write
the artefact to disk while still receiving the parsed struct for post-processing:

```swift
let summary = try NativeDaProofSummaryGenerator.shared.makeProofSummary(
    manifest: manifest.manifestBytes,
    payload: session.gatewayResult.payload,
    options: ToriiDaProofSummaryOptions(sampleCount: 2)
)
let proofResult = try DaProofSummaryArtifactEmitter.emit(
    summary: summary,
    manifestPath: "artifacts/manifest.json",
    payloadPath: "artifacts/payload.bin",
    outputURL: URL(fileURLWithPath: "/tmp/proof_summary.json")
)
print("proofs emitted", proofResult.artifact.proofCount)
```

When a summary is not available yet, pass the manifest/payload bytes plus optional sampling options and
the emitter will invoke `NativeDaProofSummaryGenerator` (or any injected `DaProofSummaryGenerating`
implementation) before returning the artefact:

```swift
let generated = try DaProofSummaryArtifactEmitter.emit(
    manifestBytes: manifest.manifestBytes,
    payloadBytes: session.gatewayResult.payload,
    proofOptions: ToriiDaProofSummaryOptions(sampleCount: 4, sampleSeed: 0),
    outputURL: nil    // skip writing to disk, work with the in-memory artefact
)
```

The emitted JSON mirrors the CLI schema (`manifest_path`, `blob_hash`, `proofs[].leaf_bytes_b64`, etc.),
so Swift automation can archive PoR evidence alongside the orchestrator reports without shelling out to
the CLI.

### DA ingest submission

`ToriiClient.submitDaBlob(_:)` mirrors `iroha app da submit`, building the Norito request body, signing it,
posting to `/v1/da/ingest`, and decoding the receipt. Use `ToriiDaBlobSubmission` to describe the payload,
erasure profile, retention policy, optional metadata, and signing material:

```swift
var submission = ToriiDaBlobSubmission(
    payload: payloadData,
    laneId: 42,
    epoch: 7,
    sequence: 1,
    metadata: [
        ToriiDaMetadataEntry(key: "da.stream", value: Data("taikai".utf8))
    ],
    clientBlobId: digest32Data,              // 32-byte digest (BLAKE3 recommended)
    privateKeyHex: signerHex,
    codec: "application/octet-stream"
)
let ingest = try await torii.submitDaBlob(submission)
print("status:", ingest.status, "duplicate:", ingest.duplicate)
if let receipt = ingest.receipt {
    print("storage ticket", receipt.storageTicketHex)
}
```

`ToriiDaBlobSubmission` defaults match the CLI (chunk size 256 KiB, RS 12/10 profile, `da.default`
retention tag). When the NoritoBridge XCFramework is linked the builder hashes the payload with BLAKE3
automatically, but environments without the bridge must still provide a 32-byte `clientBlobId`
(the CLI’s `blake3(payload)` output matches). Signers can pass a raw Ed25519 seed (`privateKey`),
hex string (`privateKeyHex`), or a pre-computed `signatureHex` +
`submitterPublicKeyHex`. Metadata entries accept raw `Data` values with visibility/encryption flags so the
JSON matches Torii’s Norito schema.

`submitDaBlob` returns `ToriiDaIngestSubmitResult` which exposes the acceptance status, the optional
`ToriiDaIngestReceipt` (decoded digests, queued timestamp, operator signature, `rentQuote` micro values),
the `sora-pdp-commitment` response header, and the signing artefacts (client blob id, submitter, signature)
that were sent to Torii.

## Hardware Acceleration

`AccelerationSettings` mirrors the Rust `AccelerationConfig` (Metal/NEON toggles, Merkle
thresholds). Apply settings before Norito bridge usage:

```swift
var accel = AccelerationSettings(enableMetal: true, merkleMinLeavesMetal: 256)
accel.apply()
sdk.accelerationSettings = accel

if let url = Bundle.main.url(forResource: "client", withExtension: "toml") {
    // Automatically detects JSON or TOML `iroha_config` files and normalises zero/default values.
    sdk.accelerationSettings = (try? AccelerationSettings.fromIrohaConfigFile(at: url)) ?? accel
}
```

`AccelerationSettings.fromIrohaConfig`/`fromIrohaConfigFile` accept the full
`iroha_config` document (JSON or TOML). They locate the `accel` section, normalise
zero-as-default fields, and return settings ready to apply (falling back to defaults if
no `accel` section exists) so Rust and Swift can share configuration artefacts.

For production apps, `AccelerationSettingsLoader.load(...)` threads the
`NORITO_ACCEL_CONFIG_PATH` environment override (developer/testing convenience) and the
bundled `acceleration.{json,toml}` or `client.{json,toml}` files before falling back to
defaults:

```swift
let accel = AccelerationSettingsLoader.load(
    environmentKey: "NORITO_ACCEL_CONFIG_PATH",
    environment: ProcessInfo.processInfo.environment,
    bundle: .main
)
sdk.accelerationSettings = accel
```

The loader reuses the same parsing/normalisation logic and logs which source supplied
the configuration so mobile telemetry can attach provenance to the chosen Metal/NEON
thresholds.

Call `AccelerationSettings.runtimeState()` when exporting telemetry so dashboards can
record whether Metal/CUDA backends were detected, configured, and healthy on the host
that produced each evidence bundle:

```swift
if let runtime = AccelerationSettings.runtimeState() {
    telemetryEmitter(.metalEnabled, runtime.metal.available)
    telemetryEmitter(.metalParity, runtime.metal.parityOK)
    telemetryEmitter(.cudaSupported, runtime.cuda.supported)
    if let reason = runtime.metal.lastError {
        telemetryEmitter(.metalDisableReason, reason)
    }
}
```

The helper reports both the applied configuration and runtime flags (supported,
configured, available, parity) plus the backend disable/error message surfaced by
the Rust bridge. It returns `nil` when the Norito bridge is unavailable so unit tests
and CLI tools can remain portable; the Swift bridge frees the FFI buffers once the
strings are copied so callers do not need manual cleanup.

## Telemetry & Redaction Readiness

- `docs/source/sdk/swift/telemetry_redaction.md` — outlines the IOS7/IOS8 telemetry
  redaction plan, signal inventory, governance artefacts, and the hashing/bucketing rules
  that keep Swift observability in lockstep with Rust and Android.
- `dashboards/data/swift_schema.sample.json` — sample schema snapshot for the new signal
  inventory; `dashboards/data/mobile_parity.sample.json` now includes the `telemetry`
  block consumed by `swift_status_export.py` and `scripts/render_swift_dashboards.sh`. Use
  `scripts/swift_collect_redaction_status.py` + `scripts/swift_enrich_parity_feed.py` to automatically
  inject salt/override data, and manage manual overrides via
  `python3 scripts/swift_status_export.py telemetry-override …`.
- `docs/source/sdk/swift/telemetry_chaos_checklist.md` — scenario checklists for override/salt
  rehearsals so telemetry alerts stay validated ahead of IOS7 council gates.

## Release & Reproducibility

- `docs/source/sdk/swift/reproducibility_checklist.md` — step-by-step evidence bundle
  for IOS8 releases covering Norito fixtures, `make bridge-xcframework`, dashboard feeds,
  and checksum capture so auditors can replay Swift SDK builds.

## Support & SLA Playbook

The IOS8 roadmap requires a published support policy before partner pilots can move
forward. The [Swift SDK Support Playbook](support_playbook.md) documents the ownership
matrix, severity/SLA expectations, release gating artefacts, telemetry/chaos drills, and
partner communication flow so Release, Docs, SRE, and Support share a single checklist
for pilots, GA, hotfixes, and LTS maintenance windows.

## Connect & WebSockets

`ConnectClient`, `ConnectFrames`, and `ConnectSession` expose the WalletConnect-style
flows used by Nexus. Frames now require the native Norito bridge for encode/decode and
fail closed with `ConnectCodecError.bridgeUnavailable` when the XCFramework is missing.
See `ConnectClientTests` for usage.

`ConnectCrypto` provides NoritoBridge-backed helpers for Connect X25519 key generation,
public-key derivation, and directional symmetric key output. When the bridge is not
linked these helpers raise `ConnectCryptoError.bridgeUnavailable`.

After the approval handshake, call `ConnectSession.setDirectionKeys(_:)` with the derived
keys to decrypt ciphertext frames automatically. Use `ConnectSession.nextEnvelope()` when
you need the full decrypted payload (sign results, encrypted controls), or
`ConnectEnvelope.decrypt(frame:symmetricKey:)` for manual inspection.

### Session identifiers & directional keys

- Use `ConnectSid.generate(chainId:appPublicKey:nonce16:)` to reproduce the strawman SID
  derivation (`BLAKE2b-256("iroha-connect|sid|" || chain || pk || nonce)`) before posting
  to `/v1/connect/session`. The helper stores the raw bytes plus the base64url form needed
  for the REST payload.
- `ConnectCrypto.deriveDirectionKeys(sharedSecret:sid:)` expands the shared secret via
  the bridge-backed HKDF (`iroha-connect|k_app` / `iroha-connect|k_wallet` labels) so
  both directions get a deterministic ChaCha20-Poly1305 key. Feed the resulting
  `ConnectDirectionKeys` into `ConnectSession.setDirectionKeys(_:)` immediately after the
  approval frame arrives.
- Wallets should persist the X25519 keypair via `ConnectKeyStore`: the default store
  writes to Application Support with an attestation bundle (SHA-256 of the public key,
  device label, created-at). Bridge-backed keys load automatically when you call
  `generateOrLoad(label:)`, and the returned attestation can be forwarded with approval
  frames. Integrity checks use a canonical JSON ordering while legacy orderings remain
  accepted for backward compatibility. Secure Enclave storage can be layered later by
  swapping the keystore backing.
- Queue/journal telemetry exports via `ConnectQueueJournal` + `ConnectQueueStateTracker`
  (see `ConnectQueueDiagnosticsTests`/`ConnectReplayRecorderTests`). Use
  `ConnectSessionDiagnostics.snapshot()` when wiring events into dashboards; evidence
  bundles can be emitted with `ConnectReplayRecorder.exportBundle`.
- Enforce inbound flow-control windows by passing `flowControl:` to `ConnectSession`
  or calling `setFlowControlWindow(_:)`; tokens are consumed per ciphertext frame and
  can be replenished with `grantFlowControl(direction:tokens:)` to mirror wallet-issued
  windows.

### Flow control, journalling, telemetry

- Each direction maintains a 64-bit `sequence`. `ConnectSession.sequenceOverflowGuard` trips
  `ConnectError.sequenceOverflow` before wrap-around and triggers the rotation handshake
  (`Control::RotateKeys`) so queues never reuse nonces.
- Wallet-issued flow-control windows surface as `ConnectSession.FlowControl` values.
  Read them via `ConnectSession.nextControlFrame()` and only dequeue plaintext envelopes
  when a token is available to avoid overrunning the wallet.
- Journals now derive from `ConnectQueueStateTracker` and `ConnectSessionDiagnostics`.
  Call `ConnectQueueStateTracker.updateSnapshot` whenever queue depth or health changes,
  and `recordMetric(_:)` to append NDJSON rows (`metrics.ndjson`) so `iroha connect queue inspect`
  can summarise the telemetry bundle. When you need to export evidence, call
  `ConnectSessionDiagnostics.exportJournalBundle(to:)` and
  `ConnectSessionDiagnostics.exportQueueMetrics(to:)`—both methods copy the
  `state.json`, `app_to_wallet.queue`, `wallet_to_app.queue`, and `metrics.ndjson` files into
  a temporary directory alongside the Norito manifest expected by the CLI. Queue files are
  stream-parsed with a default cap of 32 records and 1 MiB per direction; oversize or truncated
  files raise `ConnectQueueError` instead of being loaded wholesale.
- After reconnecting, call `ConnectSession.resumeSummary()` to emit the `{seqAppMax,
  seqWalletMax, queueDepths}` payload required by the telemetry plan. Hook the result into
  your `ConnectEventObserver` to drive `connect.resume_latency_ms` and
  `connect.replay_success_total`.
- Use `ConnectSession.eventStream(filter:)` (iOS 15/macOS 12+) to iterate `ConnectEvent`
  values directly, or `eventsPublisher(filter:)` when you need a Combine pipeline for SwiftUI.
  The payloads cover sign requests/results, display prompts, control-close/reject envelopes,
  and the new `ConnectBalanceSnapshot` payload emitted by `/v1/connect/ws`.
- `ConnectSession.balanceStream(accountID:)` / `balancePublisher(accountID:)` surface
  the Norito-provided balance snapshots. Each snapshot carries queue diagnostics sourced
  from `ConnectSessionDiagnostics`, so the SDK exports `connect.queue_*` metrics without
  additional plumbing and UI clients can render real-time queue depth/latency indicators.

### Additional guides
- See `connect_dev_quickstart.md` for end-to-end setup (SPM/Pods, bridge bundling, Connect lifecycle) and offline queue/journal recipes with bounded defaults and troubleshooting.
- See `offline.md` for detailed offline queue/journal flows (Connect, pipeline, wallet) and evidence/export steps.
- See `connect_samples.md` for sample project outlines (SwiftUI app + CLI harness) and testing tips.

For higher-level walkthroughs, see:

- `docs/connect_swift_integration.md` — full Xcode integration guide covering NoritoBridgeKit,
  ConnectClient/ConnectSession wiring, and ChaChaPoly envelope handling.
- `docs/norito_demo_contributor.md` — SwiftUI demo setup (local Torii), acceleration toggles, and telemetry tips.

## Torii REST Coverage

`ToriiClient` currently ships helpers for:

- **Accounts:** `getAssets`, `getTransactions` (both accept optional `assetId` filters),
  attachment upload/list/delete, trigger management, and general query envelopes. The
  `getExplorerAccountQr(accountId:)`
  helper wraps `/v1/explorer/accounts/{account_id}/qr` and returns the inline SVG, literal, and
  metadata defined in {doc}`sns/address_display_guidelines` so explorers can embed share-ready
  preferred i105 QR payloads without reimplementing the renderer
  (omit the format to use i105 or use canonical Katakana i105 output).
- **Explorer:** `getExplorerInstructions` and `getExplorerTransactions` wrap
  `/v1/explorer/instructions` and `/v1/explorer/transactions` with
  `ToriiExplorerInstructionsParams`/`ToriiExplorerTransactionsParams` filters (including
  optional `assetId` and `account` scoping). Fetch a single
  transaction with `getExplorerTransactionDetail(hashHex:)` or a single instruction with
  `getExplorerInstructionDetail(hashHex:index:)`. Use
  `getExplorerTransactionTransfers`/`getExplorerTransactionTransferSummaries` to derive transfer
  details for a single transaction (optionally filtering by `matchingAccount`, `assetDefinitionId`,
  or `assetId`), or `streamTransactionTransferSummaries` for history+live streaming of a single
  transaction. For transfer history, use
  `getExplorerTransfers`/`getExplorerTransferSummaries` (support `matchingAccount`,
  `assetDefinitionId`, and `assetId` filters), or the convenience helpers
  `getAccountTransferHistory` (alias: `getTransactionHistory`) and `iterateAccountTransferHistory`
  (iOS 15/macOS 12+) which page instructions with `kind: "Transfer"` and emit UI-ready
  `ToriiExplorerTransferSummary` records.
  These helpers accept `assetDefinitionId` or `assetId` filters (the asset-id filter matches the
  source asset literal in transfer payloads). Transfer summaries also expose `sourceAssetId` and
  `destinationAssetId` convenience accessors when they can be derived from the asset definition and
  account ids, plus `transferIndex` to track the entry position within batch transfer payloads.
  Convenience flags `isIncoming`, `isOutgoing`, and `isSelfTransfer` assist with UI direction
  labels. Use `direction(relativeTo:)` and `counterpartyAccountId(relativeTo:)` to recompute
  direction or display counterparties for a different account; `isIncoming(relativeTo:)`,
  `isOutgoing(relativeTo:)`, and `isSelfTransfer(relativeTo:)` are available for quick checks.
  To resolve asset ids relative to a specific account, use `assetId(relativeTo:)` and
  `counterpartyAssetId(relativeTo:)`. Use `signedAmount(relativeTo:)` when you need a +/‑ string
  for UI totals.
  Transfer summaries also conform to `Identifiable` with a stable
  `transactionHash|instructionIndex|transferIndex` identifier.
  Live updates are available via `streamExplorerInstructions` and `streamExplorerTransactions`
  (SSE, iOS 15/macOS 12+). Combine callers can use
  `explorerInstructionsPublisher`/`explorerTransactionsPublisher`. Use
  `streamExplorerTransfers`/`streamExplorerTransferSummaries` when you want transfer-only SSE feeds,
  and `explorerTransfersPublisher`/`explorerTransferSummariesPublisher` in Combine pipelines. These
  transfer stream helpers accept the same `matchingAccount`, `assetDefinitionId`, and `assetId`
  filters as the history helpers. Use
  `streamAccountTransferHistory` to emit historical transfer summaries and then keep streaming live
  updates without stitching the two flows manually; Combine callers can use
  `accountTransferHistoryPublisher`.

  Asset-definition helpers now target canonical unprefixed Base58 IDs and dotted aliases (`name#domain.dataspace` / `name#dataspace`). Asset-definition list/get/query responses may include `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`; alias selectors resolve against latest committed block time and stop resolving after grace, while direct reads can still report `expired_pending_cleanup` until sweep.
- **Domains & registries:** `listDomains(options:)` wraps `/v1/domains` with typed
  pagination/filtering via `ToriiListOptions`/`ToriiListFilter`/`ToriiListSort`, while
  `iterateDomains(pageSize:maxItems:)` (iOS 15/macOS 12+) emits an
  `AsyncThrowingStream<ToriiDomainRecord>` that walks the full dataset behind the same
  options. Use `.json(.object([...]))` for Norito-format filters or `.fields(["name",
  "-created_at"])` to render standard `sort` clauses—the helpers take care of encoding and
  offset bookkeeping.
- **Contracts:** register/deploy/fetch manifest/code bytes.
- **Pipeline:** `submitTransaction` (Norito envelopes, returns the submission receipt payload, and
  enforces `data_model_version` from `/v1/node/capabilities` with
  `ToriiClientError.incompatibleDataModel` on mismatch), `getTransactionStatus`, and recovery
  snapshots via `getPipelineRecovery(height:)`.
- **Network time:** `getTimeNow` for `/v1/time/now` snapshots.
- **Zero-knowledge:** prover reports/attachments list/count/delete operations and verifying key registry read/event helpers (`getVerifyingKey`, `listVerifyingKeys`, `streamVerifyingKeyEvents`).
- **Confidential assets:** derive the wallet key hierarchy locally through `deriveConfidentialKeyset`, build memo envelopes with
  `ConfidentialEncryptedPayload`, submit shielded debits via `ShieldRequest` +
  `submit(shield:keypair:)`, **unshield confidential balances via `ProofAttachment` +**
  `UnshieldRequest` **and** `submit(unshield:keypair:)`, and inspect rollout windows with
  `getConfidentialAssetPolicy(assetDefinitionId:)`, which wraps
  `GET /v1/confidential/assets/{definition_id}/transitions` and exposes pending transition
  metadata (transition id, conversion window, derived window-open height). Use
  `getConfidentialGasSchedule()` when you need the active verification multipliers that
  Torii reads from `confidential_gas` in `/v1/configuration`.
- **Runtime & capabilities:** `getNodeCapabilities`, `getRuntimeMetrics`, `getRuntimeAbiActive`,
  `getRuntimeAbiHash`, `listRuntimeUpgrades`, and the helper trio
  (`proposeRuntimeUpgrade`, `activateRuntimeUpgrade`, `cancelRuntimeUpgrade`) mirroring the
  `/v1/node/capabilities` and `/v1/runtime/*` surfaces with typed instruction bundles.
- **Governance:** draft deployment proposals (`submitGovernanceDeployContractProposal`),
  submit plain/ZK ballots, finalize or enact referenda, and fetch proposal/lock/tally/lock-stat
  snapshots via the typed helpers. Responses that include `tx_instructions` can be fed
  directly into `TxBuilder` to produce signed transactions.

> **Roadmap ADDR-5a:** Account-aware helpers (`getAssets`, `getTransactions`, and the matching `IrohaSDK` wrappers) accept i105/canonical literals and percent-encode `/v1/accounts/{account_id}/…` paths automatically so wallets can forward whatever selector they display without manual escaping.

Upcoming work (tracked under IOS3) includes governance endpoints, additional query
builders, and WebSocket/SSE subscribers shared with Android/JS.

### Rendering account addresses

Swift mirrors the Rust/JS/Python helpers via `AccountAddress`. When building wallet or explorer
UI, use the canonical format described in [`docs/source/sns/address_display_guidelines.md`](../../sns/address_display_guidelines.md):

```swift
let address = try AccountAddress.fromAccount(
    domain: "default",
    publicKey: Data(repeating: 0, count: 32)
)
let formats = address.displayFormats(networkPrefix: 753)

print("i105", formats.i105)
```

Account address domain labels are canonicalized to lowercase ASCII and must not contain whitespace
or reserved characters (`@`, `#`, `$`). Use canonical ASCII/punycode labels when working with IDNs.
Account addresses also validate public key lengths for known algorithms (ed25519 requires 32 bytes;
secp256k1 requires 33 bytes when enabled), and reject empty keys.

Show i105 as the copy/share target (and QR payload), and highlight when the implicit `default` domain is in use. This keeps
Swift parity with the Android/JS samples and prevents IME corruption of half-width kana.

To embed the share-ready SVG exposed by ADDR-6b, call
`ToriiClient.getExplorerAccountQr(accountId:)` and reuse the inline payload:

```swift
let qr = try await torii.getExplorerAccountQr(
    accountId: formats.i105,
)
print("SVG payload", qr.svg)
```

## Verifying Key Registry

`ToriiClient` wraps `/v1/zk/vk/*` so wallets can inspect registry state without hand-rolling JSON:

```swift
if #available(iOS 15, macOS 12, *) {
    let detail = try await torii.getVerifyingKey(backend: "halo2/ipa", name: "vk_main")
    let idsOnly = try await torii.listVerifyingKeys(
        query: ToriiVerifyingKeyListQuery(backend: "halo2/ipa", idsOnly: true)
    )
    print("active:", detail.record.status, "ids:", idsOnly.map(\.id.name))
}
```

Mutation DTOs remain useful when you are assembling locally signed transactions, but the direct Torii register/update/deprecate helpers now fail closed instead of accepting embedded private keys. Build the verifier-management instructions locally, sign them with your wallet key, and submit the resulting transaction through the pipeline helpers.

Completion-style overloads still mirror the async read and event-stream helpers so UIKit/SwiftUI layers can cancel work if the user dismisses a flow mid-flight.


For proof verification outcomes, the proof event stream follows the same pattern:

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

Trigger lifecycle updates are exposed through `streamTriggerEvents`:

```swift
if #available(iOS 15, macOS 12, *) {
    let triggers = torii.streamTriggerEvents(
        filter: ToriiTriggerEventFilter(triggerId: "nightly-tick")
    )

    Task.detached {
        do {
            for try await message in triggers {
                switch message.event {
                case .created(let id):
                    print("created", id)
                case .deleted(let id):
                    print("deleted", id)
                case .extended(let payload):
                    print("extended by", payload.delta)
                case .shortened(let payload):
                    print("shortened by", payload.delta)
                case .metadataInserted(let change):
                    print("metadata inserted", change.key)
                case .metadataRemoved(let change):
                    print("metadata removed", change.key)
                }
            }
        } catch {
            print("trigger stream error:", error)
        }
    }
}
```

Fine-tune the lifecycle events by flipping the `includeCreated`, `includeDeleted`,
`includeExtended`, `includeShortened`, `includeMetadataInserted`, and
`includeMetadataRemoved` switches on the filter. Provide `lastEventId:` when calling
`streamTriggerEvents` to propagate Torii’s `Last-Event-ID` header and resume seamlessly.

## Fixture Parity

Swift reuses the canonical Android fixture corpus. Sync and verify before updating tests
or dashboards:

```bash
make swift-fixtures        # rsync from java/iroha_android/... into IrohaSwift/Fixtures
make swift-fixtures-check  # confirm byte-identical parity
make swift-ci              # run fixture parity + dashboard validation bundle
```

CI pipelines run `ci/check_swift_fixtures.sh` to enforce parity automatically.
`make swift-ci` also validates the dashboard feeds; when running in CI ensure the
Buildkite agents expose `ci/xcframework-smoke:<lane>:device_tag` metadata so the rendered
summary identifies which simulator or StrongBox lane produced each result.

For cadence details and escalation procedures see:

- `docs/source/swift_fixture_cadence_pre_read.md` for the governance decision,
  rotation calendar, and SLA definition shared with Android/Python.
- `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` for the operational
  brief that maps scheduled/event-driven/fallback runs to metrics, dashboards,
  and status reporting obligations.
- `docs/source/sdk/swift/fixture_regen_playbook.md` for the regeneration +
  rollback steps, provenance manifest expectations, and evidence hand-off
  between rotation owners.

## Support & Operations

Operational expectations, SLAs, release evidence, and partner communication
flows now live in `docs/source/sdk/swift/support_playbook.md`. Review that
playbook before sharing pilot/GA builds so parity dashboards, telemetry
redaction policy, reproducibility proofs, and localized docs stay aligned with
`roadmap.md` (IOS8) and the weekly updates captured in `status.md`.
