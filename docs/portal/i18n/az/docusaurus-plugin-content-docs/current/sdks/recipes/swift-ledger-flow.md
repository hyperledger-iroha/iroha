---
slug: /sdks/recipes/swift-ledger-flow
lang: az
direction: ltr
source: docs/portal/docs/sdks/recipes/swift-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Swift ledger flow recipe
description: Use IrohaSwift to mint and transfer assets with the default dev network.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

SampleDownload-ı '@site/src/components/SampleDownload'dan idxal edin;

> IrohaSwift-in kodlayıcısı hazırda nanə/transfer köməkçilərini ifşa edir; aktiv tərifi
> qeydiyyat hələ də CLI vasitəsilə baş verir. 1-ci addımda CLI əmrini bir dəfə işə salın
> Swift nümunəsini icra etməzdən əvvəl.

<Nümunə Yüklə
  href="/sdk-recipes/swift/Sources/LedgerFlow/main.swift"
  fayl adı = "Mənbələr/LedgerFlow/main.swift"
  description="Async/await nümunəsini yükləyin ki, onu Xcode-da aça və ya Swift paketinizə yapışdıra biləsiniz."
/>

## 1. Aktivi qeydiyyatdan keçirin (CLI)

```bash
iroha --config defaults/client.toml asset definition register --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

## 2. Etibarnamələri hazırlayın

```bash
# raw 32-byte Ed25519 key in hex (use `iroha_cli tools crypto private-key export --raw` if needed)
export ADMIN_PRIVATE_KEY_RAW="4f94...<64 hex chars>..."
export ADMIN_ACCOUNT="<katakana-i105-account-id>"
export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
```

## 3. Paketinizə IrohaSwift əlavə edin

```swift title="Package.swift"
.package(name: "IrohaSwift", path: "../../IrohaSwift")
```

və ya Xcode-da Git URL (`https://github.com/hyperledger/iroha-swift`) istifadə edin.

## 4. Nümunə proqram

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

        let assetDefinition = "7Sp2j6zDvJFnMoscAiMaWbWHRDBZ"
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

`swift build -c release` ilə qurun və `swift run LedgerFlow` istifadə edərək işə salın.

## 5. Pariteti yoxlayın

- `iroha --config defaults/client.toml transaction get --hash <hash>` vasitəsilə əməliyyatları yoxlayın.
- `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'` ilə holdinqləri müqayisə edin.
- Hər SDK-nı təsdiqləmək üçün bu resepti Rust/Python/JavaScript reseptləri ilə birləşdirin
  demo axını üçün eyni hashları istehsal edir.