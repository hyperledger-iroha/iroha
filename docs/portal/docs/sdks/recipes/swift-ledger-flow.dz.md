---
lang: dz
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

filight Sample load འདི་ '@site/src/ཆ་ཤས་/ཆ་ཤས་/དཔེ་ཚད་ཕབ་ལེན་';

> IrohaSwift’s incoder གིས་ ད་ལྟོ་ mint/transfer གྲོགས་རམ་པ་ཚུ་ ཕྱིར་བཏོན་འབདཝ་ཨིན། རྒྱུ་ནོར་ངེས་ཚད།
> ཐོ་བཀོད་འདི་ད་ལྟོ་ཡང་ CLI བརྒྱུད་དེ་འབྱུང་དོ་ཡོདཔ་ཨིན། རིམ་པ་༡ པའི་ནང་ལུ་སི་ཨེལ་ཨའི་བརྡ་བཀོད་འདི་གཡོག་བཀོལ།
> སུའིཕཊི་དཔེ་ཚད་ལག་ལེན་འཐབ་པའི་ཧེ་མ།

<དཔེ་ཚད་ཕབ་ལེན་འབད།
  href="/sdk-ལེན་/མགྱོགས་མྱུར་/འབྱུང་ཁུངས།
  filen="འབྱུང་ཁུངས་/ལེ་ཇར་ཕོལོ་/མའེན་.ཨིསི་ཝིཕཊ།"
  secont="ཨ་སིན་ཀ་/བསྒུག་སྡོད་པའི་དཔེ་འདི་ཕབ་ལེན་འབད་དེ་ ཁྱོད་ཀྱིས་ ཨེགསི་ཀོཌི་ནང་ཁ་ཕྱེ་ནི་དང་ ཡང་ན་ ཁྱོད་རའི་སུའིཕཊི་ཐུམ་སྒྲིལ་ནང་ལུ་སྦྱར་ཚུགས།"
།/>།

## ༡ རྒྱུ་ནོར་(CLI)ཐོ་བཀོད་འབད།

```bash
iroha --config defaults/client.toml asset definition register --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

## 2. ངོ་སྤྲོད་ཀྱི་ངོ་བོ།

```bash
# raw 32-byte Ed25519 key in hex (use `iroha_cli tools crypto private-key export --raw` if needed)
export ADMIN_PRIVATE_KEY_RAW="4f94...<64 hex chars>..."
export ADMIN_ACCOUNT="soraカタカナ..."
export RECEIVER_ACCOUNT="soraカタカナ..."
```

## 3. ཁྱོད་ཀྱི་ཐུམ་སྒྲིལ་ལུ་ IrohaSwift ཁ་སྐོང་རྐྱབས།

I18NF0000002X

ཡང་ན་ ཨེགསི་ཀོཌི་ནང་ གིཊི་ཡུ་ཨར་ཨེལ་ (I18NI0000005X) ལག་ལེན་འཐབ།

## 4. དཔེར་བརྗོད།

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

I18NI000000006X དང་ཅིག་ཁར་བཟོ་སྟེ་ I18NI000000007X ལག་ལེན་འཐབ་སྟེ་ གཡོག་བཀོལ།

## 5. ཆ་སྙོམས་བདེན་པ།

- ཚོང་འབྲེལ་ཚུ་ I18NI0000008X བརྒྱུད་དེ་ བརྟག་ཞིབ་འབད།
- I18NI0000009X དང་ཅིག་ཁར་ བདག་དབང་ཚུ་ ག་བསྡུར་འབད།
- འདི་བཟོ་ཐངས་འདི་ རཱསི་/པི་ཐོན་/ཇ་བ་ཨིསི་ཀིརིཔ་ཚུ་དང་གཅིག་ཁར་ མཉམ་བསྡོམས་འབད་དེ་ ཨེསི་ཌི་ཀེ་རེ་རེ་ ངེས་གཏན་བཟོ་ནི་ལུ་ ངེས་གཏན་བཟོ་ནི་ལུ་ཨིན།
  བརྡ་སྟོན།