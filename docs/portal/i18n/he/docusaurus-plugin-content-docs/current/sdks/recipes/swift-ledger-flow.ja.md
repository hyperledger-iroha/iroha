---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sdks/recipes/swift-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: aa821681d708acb861054c5d788a8431022dce424d9e2301378177a2b9693900
source_last_modified: "2026-01-30T15:07:26+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/sdks/recipes/swift-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /sdks/recipes/swift-ledger-flow
title: מתכון זרימת לדג'ר ב-Swift
description: השתמשו ב-IrohaSwift כדי להטביע ולהעביר נכסים ברשת הפיתוח ברירת המחדל.
---

import SampleDownload from '@site/src/components/SampleDownload';

> המקודד של IrohaSwift מספק כרגע עוזרי mint/transfer; רישום הגדרות נכסים עדיין מתבצע דרך ה-CLI. הריצו את פקודת ה-CLI בשלב 1 פעם אחת לפני שמריצים את דוגמת Swift.

<SampleDownload
  href="/sdk-recipes/swift/Sources/LedgerFlow/main.swift"
  filename="Sources/LedgerFlow/main.swift"
  description="הורידו את דוגמת ה-async/await כדי לפתוח אותה ב-Xcode או להדביק אותה בחבילת Swift שלכם."
/>

## 1. רשמו את הנכס (CLI)

```bash
iroha --config defaults/client.toml asset definition register --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

## 2. הכינו פרטי גישה

```bash
# raw 32-byte Ed25519 key in hex (use `iroha_cli tools crypto private-key export --raw` if needed)
export ADMIN_PRIVATE_KEY_RAW="4f94...<64 hex chars>..."
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
```

## 3. הוסיפו את IrohaSwift לחבילה שלכם

```swift title="Package.swift"
.package(name: "IrohaSwift", path: "../../IrohaSwift")
```

או השתמשו בכתובת ה-Git (`https://github.com/hyperledger/iroha-swift`) ב-Xcode.

## 4. תוכנית לדוגמה

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

בנו עם `swift build -c release` והפעילו `swift run LedgerFlow`.

## 5. אימות תאימות

- עיינו בעסקאות באמצעות `iroha --config defaults/client.toml transaction get --hash <hash>`.
- השוו אחזקות עם `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'`.
- שלבו את המתכון הזה עם מתכוני Rust/Python/JavaScript כדי לוודא שכל SDK מפיק את אותם האשים עבור זרימת הדמו.
