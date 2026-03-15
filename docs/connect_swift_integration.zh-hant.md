---
lang: zh-hant
direction: ltr
source: docs/connect_swift_integration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b937a75e50aa77c02fcab0a11dae1b1cc182f88c179d6f90aa69181afa80d1b
source_last_modified: "2026-01-05T18:22:23.394597+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## 在 Xcode iOS 項目中集成 NoritoBridgeKit

本指南介紹如何將 Rust Norito 橋 (XCFramework) 和 Swift 包裝器集成到 iOS 應用程序中，然後使用與 Rust 主機相同的 Norito 編解碼器通過 WebSocket 交換 Iroha Connect 幀。

先決條件
- NoritoBridge.xcframework zip（由 CI 工作流程構建）和 Swift 助手 `NoritoBridgeKit.swift`（如果您不直接使用演示項目，請複制 `examples/ios/NoritoDemo/Sources` 下的版本）。
- Xcode 15+、iOS 13+ 目標。

選項 A：Swift 包管理器（推薦）
1) 使用 `crates/connect_norito_bridge/` 中的 `Package.swift.template` 發布二進制 SPM（填寫來自 CI 的 URL 和校驗和）。
2) 在 Xcode 中：文件 → 添加軟件包… → 輸入 SPM 存儲庫 URL → 將 `NoritoBridge` 產品添加到您的目標。
3) 將 `NoritoBridgeKit.swift` 添加到您的應用程序目標（拖到您的項目中，確保勾選“如果需要則復制”）。

選項 B：CocoaPods
1) 從 `NoritoBridge.podspec.template` 創建 Podspec（填寫 `s.source` zip URL）。
2) `pod trunk push NoritoBridge.podspec`。
3) 在你的 Podfile 中：`pod 'NoritoBridge'` → `pod install`。
4) 將 `NoritoBridgeKit.swift` 添加到您的應用程序目標。

進口
```swift
import Foundation
import CryptoKit               // ChaChaPoly / HKDF
import IrohaSwift              // ConnectClient / ConnectSession / ConnectCrypto
import NoritoBridge            // Clang module from the XCFramework
// Ensure NoritoBridgeKit.swift is part of the target
```

### 引導連接會話

`ConnectClient` 處理 WebSocket，而 `ConnectSession` 協調控制
幀和密文信封。下面的代碼片段展示了 dApp 如何打開會話，
派生連接密鑰，並等待批准響應。

```swift
let connectURL = URL(string: "wss://node.example/v1/connect/ws?sid=\(sidB64)&role=app")!
var connectRequest = URLRequest(url: connectURL)
connectRequest.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
let connectClient = ConnectClient(request: connectRequest)
let sessionID = Data(base64Encoded: sidB64)!

Task {
    await connectClient.start()

    let keyPair = try ConnectCrypto.generateKeyPair()
    var connectSession = ConnectSession(sessionID: sessionID, client: connectClient)

    let open = ConnectOpen(
        appPublicKey: keyPair.publicKey,
        appMetadata: ConnectAppMetadata(name: "Demo dApp", iconURL: nil, description: nil),
        constraints: ConnectConstraints(chainID: "00000000-0000-0000-0000-000000000000"),
        permissions: ConnectPermissions(methods: ["sign"], events: [])
    )
    try await connectSession.sendOpen(open: open)

    if case .approve(let approval) = try await connectSession.nextControlFrame() {
        let directionKeys = try ConnectCrypto.deriveDirectionKeys(localPrivateKey: keyPair.privateKey,
                                                                  peerPublicKey: approval.walletPublicKey,
                                                                  sessionID: sessionID)
        connectSession.setDirectionKeys(directionKeys)
        // Ready to decrypt ciphertext envelopes
        let firstEnvelope = try await connectSession.nextEnvelope()
        print("payload:", firstEnvelope.payload)
    }
}
```

### 發送密文幀（簽名請求等）

當 dApp 需要請求籤名時，它使用 Norito 橋助手進行編碼
一個信封，使用 ChaChaPoly 加密有效負載，並將其包裝在 `ConnectFrame` 中。

```swift
let bridge = NoritoBridgeKit()
let seq = nextSequence()
let txBytes = Data([0x01, 0x02, 0x03])
let envelope = try bridge.encodeEnvelopeSignRequestTx(sequence: seq, txBytes: txBytes)

let aad = ConnectAEAD.header(sessionID: sessionID, direction: .appToWallet, sequence: seq)
let nonce = ConnectAEAD.nonce(sequence: seq)
let ciphertext = try ChaChaPoly.seal(envelope,
                                     using: SymmetricKey(data: directionKeys.appToWallet),
                                     nonce: nonce,
                                     authenticating: aad).combined

let frame = ConnectFrame(sessionID: sessionID,
                         direction: .appToWallet,
                         sequence: seq,
                         kind: .ciphertext(ConnectCiphertext(payload: ciphertext)))
try await connectClient.send(frame: frame)
```

`ConnectAEAD.header` / `ConnectAEAD.nonce` 是方便的助手（請參閱
`docs/connect_swift_ios.md`) 從共享 `connect:v1` 標頭定義構建。他們
如果您不想添加其他實用程序，則很容易內聯：

```swift
enum ConnectAEAD {
    static func header(sessionID: Data, direction: ConnectDirection, sequence: UInt64) -> Data {
        var buffer = Data()
        buffer.append("connect:v1".data(using: .utf8)!)
        buffer.append(sessionID)
        buffer.append(direction == .appToWallet ? 0 : 1)
        var seq = sequence.littleEndian
        withUnsafeBytes(of: &seq) { buffer.append(contentsOf: $0) }
        buffer.append(1) // Ciphertext kind
        return buffer
    }

    static func nonce(sequence: UInt64) -> ChaChaPoly.Nonce {
        var bytes = Data(count: 12)
        var seq = sequence.littleEndian
        bytes.replaceSubrange(4..<12, with: withUnsafeBytes(of: &seq) { Data($0) })
        return try! ChaChaPoly.Nonce(data: bytes)
    }
}
```

### 接收/解密幀

`ConnectSession` 已經暴露了 `nextEnvelope()`，它在定向時解密有效負載
鍵已配置。如果您需要手動訪問（例如匹配現有解碼器
pipeline），您可以調用較低級別的幫助程序：

```swift
func decryptFrame(_ frame: ConnectFrame,
                  symmetricKey: SymmetricKey,
                  sessionID: Data) throws -> ConnectEnvelope {
    guard case .ciphertext(let payload) = frame.kind else {
        throw ConnectEnvelopeError.unsupportedFrameKind
    }
    let aad = ConnectAEAD.header(sessionID: sessionID,
                                 direction: frame.direction,
                                 sequence: frame.sequence)
    let nonce = ConnectAEAD.nonce(sequence: frame.sequence)
    let box = try ChaChaPoly.SealedBox(combined: payload.payload)
    let plaintext = try ChaChaPoly.open(box, using: symmetricKey, authenticating: aad)
    return try ConnectEnvelope.decode(jsonData: plaintext)
}
```

`NoritoBridgeKit` 還公開了幫助程序，例如 `decodeCiphertextFrame`、`decodeEnvelopeJson`、
和 `decodeSignResultAlgorithm` 用於調試或互操作性測試。用於生產
應用程序，依賴 `ConnectSession` 和 `ConnectEnvelope`，因此行為與 Rust 和
確切地說是 Android SDK。

## CI 驗證

- 在發布更新的橋接工件或推送 Connect 集成之前，運行：

  ```bash
  make swift-ci
  ```

  目標驗證夾具奇偶性、檢查儀表板源並呈現 CLI
  本地總結。在 Buildkite 中，相同的工作流程取決於元數據鍵，例如
  `ci/xcframework-smoke:<lane>:device_tag`；確認編輯後元數據存在
  管道或代理標籤，以便儀表板可以將結果歸因於正確的模擬器或
  保險箱巷。
- 如果命令失敗，請遵循奇偶校驗手冊 (`docs/source/swift_parity_triage.md`)
  並檢查渲染的 `mobile_ci` 輸出以確定哪個通道需要再生
  或重試之前的事件跟進。