# IrohaSwift

Swift SDK targeting Hyperledger Iroha v2 and Sora Nexus (Iroha v3) nodes on Apple platforms.

Features:
- Torii HTTP client (balances, transactions, explorer instructions/transactions/RWAs, subscriptions, VPN quote/session/receipt flows, pipeline recovery, time service, ZK attachments, prover reports, contracts)
- Offline note models, transaction builders, proof binding helpers, and readiness discovery through `/v1/offline/readiness`
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

`Package.swift` checks for `dist/NoritoBridge.xcframework` next to the repository root and fails package resolution when the bridge is missing. Runtime errors such as `ConnectCodecError.bridgeUnavailable` and `SwiftTransactionEncoderError.nativeBridgeUnavailable` include the same bridge-location hint for broken or unloaded bridge symbols.

CI runs `.github/workflows/swift-packaging.yml` (see `ci/check_swift_spm_validation.sh` and `ci/check_swift_pod_bridge.sh`) to verify bridge packaging.

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

// Query pipeline status if needed
torii.getTransactionStatus(hashHex: envelope.hashHex) { status in
    print(status)
}

// Await pipeline completion using the helper.
sdk.submitAndWait(envelope: envelope) { result in
    print("pipeline status:", result)
}
```

Wallet-scoped Torii deployments commonly require the `Authorization`,
`X-Account-Id`, and `X-Dataspace-Id` headers on every request. Use
`ToriiClientAuthentication` or `defaultHeaders` on `ToriiClient` so the SDK
attaches those headers centrally instead of repeating them at each call site.
Credential-bearing headers are rejected over plain HTTP or host-mismatched
requests by the shared transport-security check.

`TransferRequest`, `MintRequest`, `BurnRequest`, `ShieldRequest`, and `UnshieldRequest` expect
canonical unprefixed Base58 asset-definition IDs on the Swift surface.

`IrohaSDK` trims and validates chain/account/asset identifiers before signing and fails fast on malformed inputs. Override `creationTimeProvider` when you need deterministic timestamps for fixture generation or offline signing flows. `defaultSigningAlgorithm` controls the SDK helpers used by `generateSigningKey()` / `signingKey(fromSeed:)`; `Keypair` convenience APIs are Ed25519-only while native-backed algorithms use `NoritoBridge`.

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

The direct subscription mutation helpers now fail closed instead of accepting
embedded private keys. Build the equivalent subscription instructions locally,
sign them with your wallet key material, and submit the resulting transaction
through `submitTransaction` or `/v1/pipeline/transactions`.

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

`PipelineTransactionState` covers Torii status strings (`.queued`, `.approved`,
`.committed`, `.applied`, `.rejected`, `.expired`) and maps unrecognized values to
`.other("NAME")`.

Completion-based variants return a `Task<Void, Never>` so callers can cancel outstanding
polls. Failures bubble up as `PipelineStatusError.failure` (rejected/expired) or
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

Offline note issuance starts with the Torii issuer flow, where wallets supply
their own canonical note commitment to `/v1/offline/notes/issue`.
Redemption and audit payloads are submitted as direct transaction instructions.
The Swift SDK no longer publishes legacy offline HTTP helpers.

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

### Offline APIs

Torii exposes `/v1/offline/readiness` for offline HTTP discovery and keeps
the issuer POST flow for key refill plus note issuance. Wallets derive note
commitments locally and pass the bare 64-character commitment hex to
`/v1/offline/notes/issue`; Torii returns settlement lineage metadata without
deriving the note commitment from `settlement.entry_hash`. Redemption and audit
payloads are submitted as direct transaction instructions; the legacy legacy
offline HTTP routes are no longer published.
Swift exposes `OfflineNoteIssue`, `OfflineNoteRedeem`, and `OfflineNoteAuditBundle`
models plus `buildIssueOfflineNote`, `buildRedeemOfflineNote`,
`buildAuditOfflineNote`, and `buildDefundOfflineNote` transaction builders on
`IrohaSDK`. Redeem and audit builders verify that the recursive proof's public-input hash
matches the canonical Swift/Rust Norito payload before signing, so callers pass prover output
that is bound to the exact public inputs being submitted.

`buildRedeemOfflineNote` signs a single redeem instruction. It is only appropriate when the
source note's issued claim is already recorded on-chain, such as issuer-loaded notes or outputs
whose audit lineage has already been published. P2P offline cash is a bearer transfer: the
recipient must not require the note to have been pre-issued on-chain before accepting or
redeeming it. For bearer defunding, use `DefundOfflineNoteRequest` through
`buildDefundOfflineNote` or `submit(defundOfflineNote:...)`; it puts the ordered
`bearerAuditTrail` audits before the final `RedeemOfflineNote` instruction in the same signed
transaction, so the output claim is anchored and redeemed atomically.

`OfflineNoteWallet` adds the app-facing one-call flow for load, receive
request preparation, P2P pay, accept, optional audit publication, redeem
submission, and sync. Offline-to-offline pay/accept is local-final and
irrevocable: the sender immediately records spent inputs and spendable change,
while the recipient marks the matched pending output spendable after local
token and proof verification. No online sync is required for the value transfer.
Payment tokens carry the bearer audit trail needed to defund received notes; accepted P2P notes
persist that trail locally, and wallet `redeem(_:)` submits an atomic defund instead of a naked
redeem. `publishAudit` is a separate online evidence-submission step and does not change
wallet note spendability. The first release surface is dependency-injected:
apps provide Torii canonical auth, device binding, attestation, proof
generation/verification, transaction submission, and persistent storage.
`sync()` can also use an app-provided transaction-outcome resolver to finalize
redeem-pending note records after redeem finality. The SDK includes an in-memory store, a
`ToriiOfflineNoteIssuerClient` for body-signed key-refill plus note-issue
loads, and a direct `IrohaSDK` audit/redeem/defund submitter.
`KagemushaCompactPaymentTokenProver` exposes the native record-backed compact
token prover for shielded offline-offline payments. Pass a Norito-encoded
`KagemushaVerifiedFoldRecordBundle`; the bridge verifies each private hop proof
against its verifier record and returns a Norito-encoded
`KagemushaCompactPaymentToken` when an ABI 6-or-later `NoritoBridge` is
available and its Kagemusha entry point rejects the malformed availability probe.
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
recursive compact verifier-key archives; gate them with `isNativeAvailable`
and `isVerifierNativeAvailable`. The recursive-spend compact projection
verifier is exposed separately as
`verifyRecursiveSpendCompactPaymentTokenProjection(compactTokenArchive:verifierRecordArchive:blockHeight:)`;
gate it with `isProjectionVerifierNativeAvailable`. It accepts raw Norito
compact-token and verifier-record archives, rejects empty, malformed, or
oversized archives before bridge dispatch, and returns the native boolean
receiver result. ABI 7 now carries the one-hop LEN=4
compact-token proof path when the native bundle includes the packaged compact
one-hop proving-key archive and matching verifier-slice material. Production
defaults still stay on ABI 6 Reserved-lineage recursive spend until that
archive is shipped and signed for release. The proof-composition reservation
remains fail-closed for a missing packaged key, the generic compact-token
reservation, and the multi-hop verifier-batch reservation; those cases are
reserved ABI-7 state. The Swift wrapper maps the native
recursive-compact-unavailable bridge code to
`KagemushaRecursiveCompactPaymentTokenProverError.recursiveCompactUnavailable`
so wallet code can distinguish reserved admission from malformed inputs. Swift accepts additive native bridge ABI
versions at or above ABI 6 so ABI 7 bundles keep the minimum-ABI-6 privacy and
recursive-spend helpers usable.
`KagemushaRecursiveSpendProver` exposes the ABI 6 spend-again-offline cash
surface. Pass raw Norito archives to initialize the first recursive spend
bundle, append each offline hop, verify a received bundle, and build the online
redeem archive without reimplementing the accumulator or proof internals in
Swift. `KagemushaRecursiveSpendProver.preferredMode` selects
`recursive_spend_v1` when an ABI 6-or-later bridge exposes init, append, both
transition-profile helpers, the append-boundary helper, both lineage-witness
helpers, verify, and redeem, and every required symbol rejects the malformed
availability probe without returning output bytes. It falls back to
`checked_prefold_v1` for
legacy checked pre-fold runtimes. `transitionProfileInit(requestArchive:)` and
`transitionProfileAppend(requestArchive:)` return the canonical
Reserved-lineage accumulator transition profile as raw Norito archives for
fixture generation and circuit preflight.
Transaction builders expose the same Kagemusha instruction surface without
asking wallet code to reframe native archives. Use
`KagemushaInstructionTransactionRequest` for a typed `KagemushaTransfer` or
`RedeemKagemushaRecursive` instruction archive, and use
`IrohaSDK.buildKagemushaRecursiveRedeem(...)` to derive the redeem instruction
from a native recursive redeem request before signing. These builders require
valid Norito archives, reject empty, malformed, tampered, or wrong-type
instruction archives, and keep recursive redeem derivation inside the native
bridge.
`lineageAppendBoundary(profileArchive:)` derives the compact append-boundary
Norito archive from a full append transition profile with native opening
preflight material; wallet code should treat the boundary bytes as opaque
verifier material.
The append-boundary digest uses the public
`recursiveSpendLineageAppendBoundaryDomainV1` domain, plus the
`recursiveSpendLineageAppendBoundaryChainAssetBindingDomainV1` and
`recursiveSpendLineageAppendBoundaryFinalNoteBindingDomainV1` subdomains for
chain/asset and final-root/current-note binding.
`KagemushaRecursiveSpendProver.recursiveSpendLineageWitnesslessMaxHopsV1`
is `64`, and `recursiveSpendLineageTransitionCircuitWiredV1` is `true`;
witnessless Reserved-lineage online redemption is admitted for lineage bundles
whose hop count is inside that cap.
Use
`KagemushaRecursiveSpendProver.canRedeemWitnessless` or
`requiresLineageWitnessForRedeem` to make that branch from circuit id and hop
count. Use `canAppendWitnesslessLineage` before attempting a witnessless
Reserved-lineage append; it returns `true` for previous hop counts `1...63`.
`preferredAppendOutputCircuitId(previousHopCount:)` returns the recommended
append output selector for this release; it selects Reserved-lineage append
inside that range.
`canProveAppendOutputCircuitId(_:previousHopCount:)` tells wallet code whether
the selected append output can be proved in this release: semantic recursive
append is available through hop 64, and Reserved-lineage append is available
for previous hop counts `1...63`.
The semantic append path is bounded by `compactTokenMaxHops`; witnessless
Reserved-lineage append and redeem use the separate
`recursiveSpendLineageWitnesslessMaxHopsV1` cap.
`canSelectAppendOutputCircuitId(previousProofCircuitId:outputCircuitId:previousHopCount:)`
adds the previous-proof transition check before a wallet serializes the append
request.
`isSupportedPreviousProofCircuitId(_:)` and
`requiresPreviousLineageVerifierRecordForAppend(previousProofCircuitId:)` let
wallets reject unknown previous recursive proof circuits and include
`previous_lineage_verifier_record` only for Reserved-lineage previous bundles.
`requiresPreviousProofOpenEnvelopesForAppend(outputCircuitId:previousHopCount:)`
tells wallet code whether the selected append output circuit requires the
request to carry the previous recursive proof opening archive. The
`outputCircuitId` argument is the append request's `output_proof_circuit_id`;
missing or empty request values preserve semantic compatibility append. The
`previous_recursive_proof_open_envelopes_archive` field is opaque native prover
material: Swift wallet code must pass it through Norito unchanged and must not
construct, rewrite, or mutate it. The native bridge validates `vk_commitment`,
`public_inputs_schema_hash`, and `domain_tag` against the exact previous bundle
before proving or returning output bytes.
Native append streams the previous recursive proof bytes and per-hop accumulator
material into native-owned accumulator digests (`recursive_proof_chain_digest`,
lineage/aggregation transcript, fixed-window schedule/shared-manifest/table-base,
verifier-witness batch, transition-profile, append-opening-preflight,
append-boundary, scalar-projection, and previous/resulting accumulator digests);
SDK code must not derive, supply, or patch accumulator state.
Verify request archives must pass the
same public-binding preflight before the native bridge returns a
`KagemushaRecursiveSpendVerifyResultV1`: Reserved-lineage bundles require a
matching active `lineage_verifier_record`, semantic bundles must omit it, and
unsupported proof attachments are rejected as malformed requests rather than
soft invalid proof results. Production init requests and
Reserved-lineage append-output requests must also include packaged lineage key
artifacts in the raw Norito request: `lineage_verifier_key` and
`lineage_proving_key_archive`. Missing artifacts are rejected before runtime key
generation. The
previous bundle must already be Reserved-lineage before a Reserved-lineage
append output is valid; semantic previous bundles keep using semantic append
plus a record-backed lineage witness.
`normalizedAppendOutputCircuitId` and `isSupportedAppendOutputCircuitId`
helpers expose that defaulting rule for wallet-side preflight. The
`recursivePreviousProofOpenEnvelopesRequiredCountV1` and
`recursivePreviousProofOpenEnvelopesMaxBytes` expose the exactly-one-envelope
cardinality rule and native 8 MiB pre-decode cap for that archive.
Native Kagemusha prover wrappers reject empty native result archives and native
outputs larger than 64 MiB instead of treating them as successful proof
material.

### Native privacy bridge

`PrivacyNativeBridge` exposes the privacy FFI surface as generic raw Norito
archives: `capabilitiesV1()`, `buildProofV1(requestArchive:)`, and
`verifyProofV1(requestArchive:)`. The SDK does not expose algorithm-specific
production proof builders while the privacy rows remain gated. Native
availability requires ABI 6 or later, the privacy capability/build/verify
symbols, and successful Norito probe outputs whose operation-specific result
schema bytes match the called entry point.

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

`OfflineBearerCashWallet` is the app-facing Offline Bearer Cash surface. It is
the Offline Note wallet under the cash naming layer, so value is represented by
note commitments, note secrets, nullifiers, audit lineage, and issuer-signed
hardware key certificates instead of a separate mutable purse protocol.

`OfflineNoteTransferHandoff` wraps the canonical payment token into app-facing
transfer modalities. Use `qrStreamingFrameBytes(for:)` for animated/binary QR
flows, `nfcFrameBytes(for:)` for APDU-sized NFC frame exchange, and
`nearbyPayload(for:)` or `nearbyFrameBytes(for:)` for MultipeerConnectivity or
other nearby byte channels. `OfflineNoteTransferStreamReceiver` reconstructs
stream frames back into a payment token, and `OfflineNoteTransferCapabilities`
keeps NFC disabled on iOS unless the app explicitly opts in after confirming an
allowed Core NFC HCE/CardSession use case and entitlement. Apps that want the
png2-style NFC handoff can use `OfflineNoteNfcApduProtocol` directly: select
the Iroha AID, read/write the 40-byte metadata header, transfer chunks, then
commit and poll/read a local `receiptAck` payload. The default write helper
`nfcPaymentTokenWriteAPDUs(for:)` uses Android-safe 240-byte chunks because many
Android NFC stacks cannot reliably carry larger APDUs; iOS-to-iOS integrations
can opt into the extended helpers only after both peers advertise support.
`IrohaSwiftMobileTransports` adds an optional iOS CoreNFC implementation around
that same APDU protocol. Configure `IrohaOfflineNfcConfiguration` with the app's
AID, keep `cardSessionRuntimeEnabled` false unless the build has an allowed HCE
entitlement/profile, and use `IrohaOfflineNfcReaderService` plus
`IrohaOfflineNfcCardSessionController` for sender/receiver flows. The module
logs only state, counts, status words, and sanitized error codes. `IrohaSwiftTransferUI`
contains reusable SwiftUI QR/NFC/Nearby widgets and the QR fountain frame
cycler used by wallet apps that want the png2-style transfer surface without
copying transport logic.
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
`OfflineNoteNearbyEnvelope` provides the matching sorted-key JSON envelope
with unpadded base64url payloads, the pairing-image challenge used before
sending a payment token, and the local receipt ACK returned by the receiver.
None of these local transports require online sync for offline-to-offline value
transfer.
`OfflineNoteKeychainStore` writes wallet state through revisioned
ThisDeviceOnly Keychain records and deletes the previous revision after each
commit, so app-container rollback cannot revive an earlier note set without the
deleted revision item. Legacy `spendPending` records decode as `spent`, and
legacy `changePending` records decode as `spendable`.
Issuance is accepted only from an offline escrow manager with `CanManageOfflineEscrow`, and the
one-use key certificate must be signed over its canonical payload. Redemption proofs bind the
source note commitment, nullifiers, certified key payload, recipient, asset, and amount to a
previously issued note claim before escrowed value is released. Optional audit bundles bind their
token id, observed nullifiers, output commitments, and certified key payload to the proof, and the
certified key must have been issued on-ledger first.
Recursive proofs must name an active `offline_note` verifier key in WSV, carry an
`OpenVerifyEnvelope`, match `offline_note_recursive_public_inputs_schema_hash()`, and expose the
semantic Offline instance columns advertised by `/v1/offline/readiness`. The readiness
payload includes the canonical `halo2/ipa` verifier key id for `offline-note-recursive`;
wallets should submit only real prover output for that verifier.

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

## NoritoBridge packaging

The release process for the Norito Swift bindings is documented in
[`docs/norito_bridge_release.md`](../docs/norito_bridge_release.md). Follow the steps to
build the XCFramework, compute the checksum, and update both the Swift Package manifest
and the CocoaPods podspec. The resulting artifacts should share the same semantic version
as the `norito` Rust crate.
`dist/NoritoBridge.artifacts.json` should accompany the XCFramework and record the
bridge version plus per-platform SHA-256 hashes.

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
