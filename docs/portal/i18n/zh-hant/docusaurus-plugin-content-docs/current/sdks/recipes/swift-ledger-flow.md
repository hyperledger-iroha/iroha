---
slug: /sdks/recipes/swift-ledger-flow
lang: zh-hant
direction: ltr
source: docs/portal/docs/sdks/recipes/swift-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Swift ledger flow recipe
description: Use IrohaSwift to mint and transfer assets with the default dev network.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

從“@site/src/components/SampleDownload”導入 SampleDownload；

> IrohaSwift 的編碼器當前公開了鑄幣/傳輸助手；資產定義
> 註冊仍然通過 CLI 進行。運行步驟 1 中的 CLI 命令一次
> 在執行 Swift 示例之前。

<樣本下載
  href="/sdk-recipes/swift/Sources/LedgerFlow/main.swift"
  文件名=“來源/LedgerFlow/main.swift”
  description="下載 async/await 示例，以便您可以在 Xcode 中打開它或將其粘貼到您的 Swift 包中。"
/>

## 1. 註冊資產 (CLI)

```bash
iroha --config defaults/client.toml asset definition register --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

## 2. 準備憑證

```bash
# raw 32-byte Ed25519 key in hex (use `iroha_cli tools crypto private-key export --raw` if needed)
export ADMIN_PRIVATE_KEY_RAW="4f94...<64 hex chars>..."
export ADMIN_ACCOUNT="<katakana-i105-account-id>"
export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
```

## 3. 將 IrohaSwift 添加到您的包中

```swift title="Package.swift"
.package(name: "IrohaSwift", path: "../../IrohaSwift")
```

或者在 Xcode 中使用 Git URL (`https://github.com/hyperledger/iroha-swift`)。

## 4. 示例程序

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

使用 `swift build -c release` 構建並使用 `swift run LedgerFlow` 運行。

## 5. 驗證奇偶性

- 通過 `iroha --config defaults/client.toml transaction get --hash <hash>` 檢查交易。
- 將持有量與 `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'` 進行比較。
- 將此配方與 Rust/Python/JavaScript 相結合以確認每個 SDK
  為演示流程生成相同的哈希值。