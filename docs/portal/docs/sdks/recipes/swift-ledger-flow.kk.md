---
lang: kk
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

SampleDownload файлын '@site/src/components/SampleDownload' ішінен импорттау;

> IrohaSwift кодері қазіргі уақытта жалбыз/трансфер көмекшілерін көрсетеді; актив анықтамасы
> тіркеу әлі де CLI арқылы жүзеге асырылады. 1-қадамдағы CLI пәрменін бір рет іске қосыңыз
> Swift үлгісін орындамас бұрын.

<Үлгі жүктеп алу
  href="/sdk-recipes/swift/Sources/LedgerFlow/main.swift"
  filename="Sources/LedgerFlow/main.swift"
  description="Асинхронды/күту үлгісін жүктеп алыңыз, осылайша оны Xcode ішінде аша аласыз немесе оны Swift бумасына қойыңыз."
/>

## 1. Активті тіркеу (CLI)

```bash
iroha --config defaults/client.toml asset definition register --id coffee#wonderland
```

## 2. Тіркелгі деректерін дайындаңыз

```bash
# raw 32-byte Ed25519 key in hex (use `iroha_cli tools crypto private-key export --raw` if needed)
export ADMIN_PRIVATE_KEY_RAW="4f94...<64 hex chars>..."
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
```

## 3. Пакетке IrohaSwift қосыңыз

```swift title="Package.swift"
.package(name: "IrohaSwift", path: "../../IrohaSwift")
```

немесе Xcode ішінде Git URL (`https://github.com/hyperledger/iroha-swift`) пайдаланыңыз.

## 4. Мысал бағдарлама

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

`swift build -c release` көмегімен құрастырыңыз және `swift run LedgerFlow` арқылы іске қосыңыз.

## 5. Тепе-теңдікті тексеріңіз

- `iroha --config defaults/client.toml transaction get --hash <hash>` арқылы транзакцияларды тексеріңіз.
- `iroha --config defaults/client.toml asset list filter '{"id":"coffee#wonderland##<account>"}'` холдингтерін салыстырыңыз.
- Әрбір SDK растау үшін осы рецептті Rust/Python/JavaScript рецептерімен біріктіріңіз
  демонстрациялық ағын үшін бірдей хэштерді жасайды.