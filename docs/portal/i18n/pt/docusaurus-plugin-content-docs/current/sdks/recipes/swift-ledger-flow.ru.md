---
lang: pt
direction: ltr
source: docs/portal/docs/sdks/recipes/swift-ledger-flow.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: ru
direction: ltr
source: docs/portal/docs/sdks/recipes/swift-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6d8d50d8145f646236734aa00d49f741ac2a7616d42326044fdfcba55bd621a1
source_last_modified: "2025-11-11T10:24:06.064900+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Рецепт потока реестра на Swift
description: Используйте IrohaSwift для выпуска и перевода активов в стандартной dev-сети.
slug: /sdks/recipes/swift-ledger-flow
---

import SampleDownload from '@site/src/components/SampleDownload';

> Кодировщик IrohaSwift пока предоставляет хелперы для mint/transfer; регистрация определений активов все еще выполняется через CLI. Выполните команду CLI на шаге 1 один раз перед запуском Swift-примера.

<SampleDownload
  href="/sdk-recipes/swift/Sources/LedgerFlow/main.swift"
  filename="Sources/LedgerFlow/main.swift"
  description="Скачайте пример async/await, чтобы открыть его в Xcode или вставить в свой Swift-пакет."
/>

## 1. Зарегистрируйте актив (CLI)

```bash
iroha --config defaults/client.toml asset definition register --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

## 2. Подготовьте учетные данные

```bash
# raw 32-byte Ed25519 key in hex (use `iroha_cli tools crypto private-key export --raw` if needed)
export ADMIN_PRIVATE_KEY_RAW="4f94...<64 hex chars>..."
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

## 3. Добавьте IrohaSwift в пакет

```swift title="Package.swift"
.package(name: "IrohaSwift", path: "../../IrohaSwift")
```

или используйте Git URL (`https://github.com/hyperledger/iroha-swift`) в Xcode.

## 4. Пример программы

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

Соберите `swift build -c release` и запустите `swift run LedgerFlow`.

## 5. Проверьте паритет

- Просмотрите транзакции через `iroha --config defaults/client.toml transaction get --hash <hash>`.
- Сравните балансы с `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'`.
- Совместите этот рецепт с Rust/Python/JavaScript, чтобы подтвердить, что каждый SDK выдает одинаковые хеши для демо-потока.
