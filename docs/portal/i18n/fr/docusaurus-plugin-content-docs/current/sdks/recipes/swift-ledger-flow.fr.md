---
lang: fr
direction: ltr
source: docs/portal/docs/sdks/recipes/swift-ledger-flow.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: fr
direction: ltr
source: docs/portal/docs/sdks/recipes/swift-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6d8d50d8145f646236734aa00d49f741ac2a7616d42326044fdfcba55bd621a1
source_last_modified: "2025-11-11T10:24:06.064900+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Recette de flux du registre Swift
description: Utilisez IrohaSwift pour frapper et transférer des actifs avec le réseau de développement par défaut.
slug: /sdks/recipes/swift-ledger-flow
---

import SampleDownload from '@site/src/components/SampleDownload';

> L’encodeur d’IrohaSwift expose actuellement des helpers de mint/transfer ; l’enregistrement des définitions d’actifs se fait encore via la CLI. Exécutez la commande CLI de l’étape 1 une fois avant de lancer l’exemple Swift.

<SampleDownload
  href="/sdk-recipes/swift/Sources/LedgerFlow/main.swift"
  filename="Sources/LedgerFlow/main.swift"
  description="Téléchargez l'exemple async/await pour l'ouvrir dans Xcode ou le coller dans votre package Swift."
/>

## 1. Enregistrez l’actif (CLI)

```bash
iroha --config defaults/client.toml asset definition register --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

## 2. Préparez les identifiants

```bash
# raw 32-byte Ed25519 key in hex (use `iroha_cli tools crypto private-key export --raw` if needed)
export ADMIN_PRIVATE_KEY_RAW="4f94...<64 hex chars>..."
export ADMIN_ACCOUNT="soraカタカナ..."
export RECEIVER_ACCOUNT="soraカタカナ..."
```

## 3. Ajoutez IrohaSwift à votre package

```swift title="Package.swift"
.package(name: "IrohaSwift", path: "../../IrohaSwift")
```

ou utilisez l’URL Git (`https://github.com/hyperledger/iroha-swift`) dans Xcode.

## 4. Programme d’exemple

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

Compilez avec `swift build -c release` et exécutez `swift run LedgerFlow`.

## 5. Vérifier la parité

- Inspectez les transactions via `iroha --config defaults/client.toml transaction get --hash <hash>`.
- Comparez les avoirs avec `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'`.
- Combinez cette recette avec celles en Rust/Python/JavaScript pour confirmer que chaque SDK produit les mêmes hashes pour le flux de démo.
