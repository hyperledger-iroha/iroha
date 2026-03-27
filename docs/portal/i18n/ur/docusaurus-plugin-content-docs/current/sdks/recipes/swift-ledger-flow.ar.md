---
lang: ur
direction: rtl
source: docs/portal/docs/sdks/recipes/swift-ledger-flow.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: ar
direction: rtl
source: docs/portal/docs/sdks/recipes/swift-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6d8d50d8145f646236734aa00d49f741ac2a7616d42326044fdfcba55bd621a1
source_last_modified: "2025-11-11T10:24:06.064900+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: وصفة تدفق دفتر الأستاذ في Swift
description: استخدم IrohaSwift لسكّ الأصول وتحويلها على شبكة التطوير الافتراضية.
slug: /sdks/recipes/swift-ledger-flow
---

import SampleDownload from '@site/src/components/SampleDownload';

> المشفّر في IrohaSwift يوفّر حاليًا أدوات مساعدة للسك/التحويل؛ أما تسجيل تعريفات الأصول فما يزال يتم عبر CLI. نفّذ أمر CLI في الخطوة 1 مرة واحدة قبل تشغيل مثال Swift.

<SampleDownload
  href="/sdk-recipes/swift/Sources/LedgerFlow/main.swift"
  filename="Sources/LedgerFlow/main.swift"
  description="نزّل مثال async/await لتفتحه في Xcode أو تلصقه في حزمة Swift الخاصة بك."
/>

## 1. سجّل الأصل (CLI)

```bash
iroha --config defaults/client.toml asset definition register --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

## 2. جهّز بيانات الاعتماد

```bash
# raw 32-byte Ed25519 key in hex (use `iroha_cli tools crypto private-key export --raw` if needed)
export ADMIN_PRIVATE_KEY_RAW="4f94...<64 hex chars>..."
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

## 3. أضف IrohaSwift إلى حزمتك

```swift title="Package.swift"
.package(name: "IrohaSwift", path: "../../IrohaSwift")
```

أو استخدم عنوان Git (`https://github.com/hyperledger/iroha-swift`) في Xcode.

## 4. برنامج المثال

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

ابنِ باستخدام `swift build -c release` ثم شغّل `swift run LedgerFlow`.

## 5. تحقّق من التكافؤ

- عاين المعاملات عبر `iroha --config defaults/client.toml transaction get --hash <hash>`.
- قارن الحيازات باستخدام `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'`.
- ادمج هذه الوصفة مع وصفات Rust/Python/JavaScript لتأكيد أن كل SDK ينتج التجزئات نفسها لتدفق العرض التجريبي.
