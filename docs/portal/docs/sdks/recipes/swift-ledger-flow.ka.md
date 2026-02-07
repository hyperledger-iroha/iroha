---
lang: ka
direction: ltr
source: docs/portal/docs/sdks/recipes/swift-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 77894eaa2a9997d03b1a2aa447b1df0b259b6f58fd3f330d2e75ef8c9bf85143
source_last_modified: "2026-01-22T16:26:46.513609+00:00"
translation_last_reviewed: 2026-02-07
title: Swift ledger flow recipe
description: Use IrohaSwift to mint and transfer assets with the default dev network.
slug: /sdks/recipes/swift-ledger-flow
translator: machine-google-reviewed
---

SampleDownload-ის იმპორტი '@site/src/components/SampleDownload'-დან;

> IrohaSwift-ის შიფრატორი ამჟამად ავლენს პიტნის/გადაცემის დამხმარეებს; აქტივ-განმარტება
> რეგისტრაცია კვლავ ხდება CLI-ის მეშვეობით. ერთხელ გაუშვით CLI ბრძანება ნაბიჯი 1-ში
> Swift ნიმუშის შესრულებამდე.

<SampleDownload
  href="/sdk-recipes/swift/Sources/LedgerFlow/main.swift"
  filename="Sources/LedgerFlow/main.swift"
  description="ჩამოტვირთეთ async/wait მაგალითი, რათა გახსნათ ის Xcode-ში ან ჩასვით თქვენს Swift პაკეტში."
/>

## 1. დაარეგისტრირეთ აქტივი (CLI)

```bash
iroha --config defaults/client.toml asset definition register --id coffee#wonderland
```

## 2. მოამზადეთ რწმუნებათა სიგელები

```bash
# raw 32-byte Ed25519 key in hex (use `iroha_cli tools crypto private-key export --raw` if needed)
export ADMIN_PRIVATE_KEY_RAW="4f94...<64 hex chars>..."
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
```

## 3. დაამატეთ IrohaSwift თქვენს პაკეტს

```swift title="Package.swift"
.package(name: "IrohaSwift", path: "../../IrohaSwift")
```

ან გამოიყენეთ Git URL (`https://github.com/hyperledger/iroha-swift`) Xcode-ში.

## 4. პროგრამის მაგალითი

```swift title="Sources/LedgerFlow/main.swift"
import Foundation
import IrohaSwift

@main
struct LedgerFlow {
    static func main() async {
        guard #available(macOS 13.0, iOS 16.0, *) else {
            fatalError("IrohaSwift async APIs require macOS 13 / iOS 16 or newer")
        }
        do {
            try await run()
        } catch {
            fputs("ledger flow failed: \(error)\n", stderr)
            exit(1)
        }
    }

    @available(macOS 13.0, iOS 16.0, *)
    static func run() async throws {
        let env = ProcessInfo.processInfo.environment
        let adminAccount = env["ADMIN_ACCOUNT"]!
        let receiverAccount = env["RECEIVER_ACCOUNT"]!
        guard let privateBytes = Data(hexString: env["ADMIN_PRIVATE_KEY_RAW"] ?? "") else {
            fatalError("Set ADMIN_PRIVATE_KEY_RAW to a 32-byte Ed25519 key in hex")
        }

        let keypair = try Keypair(privateKeyBytes: privateBytes)
        let torii = ToriiClient(baseURL: URL(string: "http://127.0.0.1:8080")!)
        let sdk = IrohaSDK(toriiClient: torii)

        let assetDefinition = "coffee#wonderland"
        let adminAssetId = TxBuilder.makeAssetId(assetDefinitionId: assetDefinition, accountId: adminAccount)

        // Mint 250 units into the admin account.
        let mint = MintRequest(chainId: "00000000-0000-0000-0000-000000000000",
                               authority: adminAccount,
                               assetDefinitionId: assetDefinition,
                               quantity: "250",
                               destination: adminAccount,
                               ttlMs: 60_000)
        try await sdk.submitAndWait(mint: mint, keypair: keypair)

        // Transfer 50 units to the demo receiver.
        let transfer = TransferRequest(chainId: mint.chainId,
                                       authority: adminAccount,
                                       assetDefinitionId: assetDefinition,
                                       quantity: "50",
                                       destination: receiverAccount,
                                       description: "swift-recipe",
                                       ttlMs: 60_000)
        try await sdk.submitAndWait(transfer: transfer, keypair: keypair)

        // Query the receiver’s balances.
        let balances = try await sdk.getAssets(accountId: receiverAccount, limit: 5)
        print("Receiver balances:")
        for balance in balances where balance.assetDefinitionId == assetDefinition {
            print("  \(balance.assetDefinitionId): \(balance.quantity)")
        }
    }
}
```

ააშენეთ `swift build -c release`-ით და გაუშვით `swift run LedgerFlow`-ის გამოყენებით.

## 5. შეამოწმეთ პარიტეტი

- შეამოწმეთ ტრანზაქციები `iroha --config defaults/client.toml transaction get --hash <hash>`-ის საშუალებით.
- შეადარეთ ჰოლდინგი `iroha --config defaults/client.toml asset list filter '{"id":"coffee#wonderland##<account>"}'`-თან.
- შეუთავსეთ ეს რეცეპტი Rust/Python/JavaScript-თან, რათა დაადასტუროთ ყველა SDK
  აწარმოებს იგივე ჰეშებს დემო ნაკადისთვის.