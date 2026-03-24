---
slug: /sdks/recipes/swift-ledger-flow
lang: ba
direction: ltr
source: docs/portal/docs/sdks/recipes/swift-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Swift ledger flow recipe
description: Use IrohaSwift to mint and transfer assets with the default dev network.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

импорт SapleDownload '@site/src/компоненттар/SampleDownload';

> IrohaSwift’s кодер әлеге ваҡытта мәтрүшкә/трансфер ярҙамсыларын фашлай; актив-билдәләү
> теркәү һаман да CLI аша була. CLI командаһын бер тапҡыр 1-се аҙымда эшләгеҙ
> Свифт өлгөһөн башҡарыр алдынан.

<СэмплДау-лог
  href="/sdk-рецепттар/свифт/Сығанаҡтар/ЛеджерФлоу/төп.свифт".
  файл исеме="Сығанаҡтар/ЛеджерФлоу/төп.свифт".
  тасуирлама="Скачать асинк/көтөп миҫал, шулай итеп, һеҙ уны Xcode-ла аса ала йәки һеҙҙең Swift пакетына йәбештерә ала."
/>

## 1. активты теркәү (CLI)

```bash
iroha --config defaults/client.toml asset definition register --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

## 2. Инаныу ҡағыҙҙарын әҙерләгеҙ

```bash
# raw 32-byte Ed25519 key in hex (use `iroha_cli tools crypto private-key export --raw` if needed)
export ADMIN_PRIVATE_KEY_RAW="4f94...<64 hex chars>..."
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
```

## 3. Һеҙҙең пакетҡа IrohaSwift өҫтәгеҙ

```swift title="Package.swift"
.package(name: "IrohaSwift", path: "../../IrohaSwift")
```

йәки Xcode-ла Git URL-адресын (I18NI000000005X) ҡулланыу.

## 4. Миҫал программаһы

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

`swift build -c release` менән төҙөү һәм `swift run LedgerFlow` ҡулланып эшләй.

## 5. Паритетты раҫлау

- `iroha --config defaults/client.toml transaction get --hash <hash>` аша операцияларҙы тикшерергә.
- `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'` менән сағыштырыу.
- Был рецепт менән берләштереү өсөн тут/Python/JavaScript уларҙы раҫлау өсөн һәр SDK .
  демо ағымы өсөн бер үк хеш етештерә.