---
slug: /sdks/recipes/swift-ledger-flow
lang: zh-hans
direction: ltr
source: docs/portal/docs/sdks/recipes/swift-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Swift ledger flow recipe
description: Use IrohaSwift to mint and transfer assets with the default dev network.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

从“@site/src/components/SampleDownload”导入 SampleDownload；

> IrohaSwift 的编码器当前公开了铸币/传输助手；资产定义
> 注册仍然通过 CLI 进行。运行步骤 1 中的 CLI 命令一次
> 在执行 Swift 示例之前。

<样本下载
  href="/sdk-recipes/swift/Sources/LedgerFlow/main.swift"
  文件名=“来源/LedgerFlow/main.swift”
  description="下载 async/await 示例，以便您可以在 Xcode 中打开它或将其粘贴到您的 Swift 包中。"
/>

## 1. 注册资产 (CLI)

```bash
iroha --config defaults/client.toml asset definition register --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

## 2. 准备凭证

```bash
# raw 32-byte Ed25519 key in hex (use `iroha_cli tools crypto private-key export --raw` if needed)
export ADMIN_PRIVATE_KEY_RAW="4f94...<64 hex chars>..."
export ADMIN_ACCOUNT="soraカタカナ..."
export RECEIVER_ACCOUNT="soraカタカナ..."
```

## 3. 将 IrohaSwift 添加到您的包中

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

使用 `swift build -c release` 构建并使用 `swift run LedgerFlow` 运行。

## 5. 验证奇偶性

- 通过 `iroha --config defaults/client.toml transaction get --hash <hash>` 检查交易。
- 将持有量与 `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'` 进行比较。
- 将此配方与 Rust/Python/JavaScript 相结合以确认每个 SDK
  为演示流程生成相同的哈希值。