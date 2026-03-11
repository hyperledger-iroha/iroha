---
lang: my
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

'@site/src/components/SampleDownload' မှ SampleDownload ကို တင်သွင်းပါ။

> IrohaSwift ၏ကုဒ်ပြောင်းကိရိယာသည် လက်ရှိတွင် mint/transfer helpers များကို ဖော်ထုတ်ပေးပါသည်။ ပိုင်ဆိုင်မှု-အဓိပ္ပါယ်
> မှတ်ပုံတင်ခြင်းကို CLI မှတဆင့် ပြုလုပ်ဆဲဖြစ်သည်။ အဆင့် 1 တွင် CLI အမိန့်ကို တစ်ကြိမ်လုပ်ဆောင်ပါ။
> Swift နမူနာကို မလုပ်ဆောင်မီ။

<နမူနာဒေါင်းလုဒ်လုပ်ပါ။
  href="/sdk-recipes/swift/Sources/LedgerFlow/main.swift"
  filename="Sources/LedgerFlow/main.swift"
  description="async/ait ဥပမာကို ဒေါင်းလုဒ်လုပ်ပါ၊ သို့မှသာ ၎င်းကို Xcode ဖြင့် ဖွင့်နိုင်သည် သို့မဟုတ် ၎င်းကို သင်၏ Swift ပက်ကေ့ဂျ်တွင် ကူးထည့်နိုင်ပါသည်။"
/>

## 1. ပိုင်ဆိုင်မှု (CLI) ကို မှတ်ပုံတင်ပါ။

```bash
iroha --config defaults/client.toml asset definition register --id coffee#wonderland
```

## 2. အထောက်အထားများကို ပြင်ဆင်ပါ။

```bash
# raw 32-byte Ed25519 key in hex (use `iroha_cli tools crypto private-key export --raw` if needed)
export ADMIN_PRIVATE_KEY_RAW="4f94...<64 hex chars>..."
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
```

## 3. IrohaSwift ကို သင့်ပက်ကေ့ဂျ်တွင် ထည့်ပါ။

```swift title="Package.swift"
.package(name: "IrohaSwift", path: "../../IrohaSwift")
```

သို့မဟုတ် Xcode တွင် Git URL (`https://github.com/hyperledger/iroha-swift`) ကိုသုံးပါ။

## 4. ဥပမာ ပရိုဂရမ်

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

`swift build -c release` ဖြင့် တည်ဆောက်ပြီး `swift run LedgerFlow` ကို အသုံးပြု၍ လုပ်ဆောင်ပါ။

## 5. တန်းတူညီမျှမှုကို အတည်ပြုပါ။

- `iroha --config defaults/client.toml transaction get --hash <hash>` မှတစ်ဆင့် အရောင်းအ၀ယ်များကို စစ်ဆေးပါ။
- ပိုင်ဆိုင်မှုများကို `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'` နှင့် နှိုင်းယှဉ်ပါ။
- SDK တိုင်းကို အတည်ပြုရန် ဤစာရွက်ကို Rust/Python/JavaScript တို့နှင့် ပေါင်းစပ်ပါ။
  သရုပ်ပြစီးဆင်းမှုအတွက် တူညီသော hashe များကို ထုတ်လုပ်သည်။