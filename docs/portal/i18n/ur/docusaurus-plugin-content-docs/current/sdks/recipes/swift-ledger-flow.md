---
lang: ur
direction: rtl
source: docs/portal/docs/sdks/recipes/swift-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /sdks/recipes/swift-ledger-flow
title: سویفٹ لیجر فلو ترکیب
description: ڈیفالٹ ڈیولپمنٹ نیٹ ورک کے ساتھ اثاثے منٹ اور ٹرانسفر کرنے کے لیے IrohaSwift استعمال کریں۔
---

import SampleDownload from '@site/src/components/SampleDownload';

> IrohaSwift کا انکوڈر فی الحال mint/transfer مددگار فنکشنز فراہم کرتا ہے؛ اثاثہ تعریف کی رجسٹریشن ابھی بھی CLI کے ذریعے ہوتی ہے۔ Swift نمونہ چلانے سے پہلے قدم 1 کی CLI کمانڈ ایک بار چلائیں۔

<SampleDownload
  href="/sdk-recipes/swift/Sources/LedgerFlow/main.swift"
  filename="Sources/LedgerFlow/main.swift"
  description="async/await مثال ڈاؤن لوڈ کریں تاکہ اسے Xcode میں کھول سکیں یا اپنے Swift پیکج میں پیسٹ کر سکیں۔"
/>

## 1. اثاثہ رجسٹر کریں (CLI)

```bash
iroha --config defaults/client.toml asset definition register --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

## 2. اسناد تیار کریں

```bash
# raw 32-byte Ed25519 key in hex (use `iroha_cli tools crypto private-key export --raw` if needed)
export ADMIN_PRIVATE_KEY_RAW="4f94...<64 hex chars>..."
export ADMIN_ACCOUNT="<katakana-i105-account-id>"
export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
```

## 3. IrohaSwift کو اپنی پیکج میں شامل کریں

```swift title="Package.swift"
.package(name: "IrohaSwift", path: "../../IrohaSwift")
```

یا Xcode میں Git URL (`https://github.com/hyperledger/iroha-swift`) استعمال کریں۔

## 4. مثال پروگرام

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
            fputs("ledger flow failed: \(error)
", stderr)
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

`swift build -c release` سے بلڈ کریں اور `swift run LedgerFlow` کے ساتھ چلائیں۔

## 5. برابری کی تصدیق

- `iroha --config defaults/client.toml transaction get --hash <hash>` کے ذریعے ٹرانزیکشنز دیکھیں۔
- `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'` سے ہولڈنگز کا موازنہ کریں۔
- اس ترکیب کو Rust/Python/JavaScript والی ترکیبوں کے ساتھ ملائیں تاکہ یہ تصدیق ہو کہ ہر SDK ڈیمو فلو کے لیے ایک ہی ہیشز تیار کرتا ہے۔

