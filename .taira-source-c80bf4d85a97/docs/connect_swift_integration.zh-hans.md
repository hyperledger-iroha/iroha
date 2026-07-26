---
lang: zh-hans
direction: ltr
source: docs/connect_swift_integration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b937a75e50aa77c02fcab0a11dae1b1cc182f88c179d6f90aa69181afa80d1b
source_last_modified: "2026-01-05T18:22:23.394597+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## 在 Xcode iOS 项目中集成 NoritoBridgeKit

本指南介绍如何将 Rust Norito 桥 (XCFramework) 和 Swift 包装器集成到 iOS 应用程序中，然后使用与 Rust 主机相同的 Norito 编解码器通过 WebSocket 交换 Iroha Connect 帧。

先决条件
- NoritoBridge.xcframework zip（由 CI 工作流程构建）和 Swift 助手 `NoritoBridgeKit.swift`（如果您不直接使用演示项目，请复制 `examples/ios/NoritoDemo/Sources` 下的版本）。
- Xcode 15+、iOS 13+ 目标。

选项 A：Swift 包管理器（推荐）
1) 使用 `crates/connect_norito_bridge/` 中的 `Package.swift.template` 发布二进制 SPM（填写来自 CI 的 URL 和校验和）。
2) 在 Xcode 中：文件 → 添加软件包… → 输入 SPM 存储库 URL → 将 `NoritoBridge` 产品添加到您的目标。
3) 将 `NoritoBridgeKit.swift` 添加到您的应用程序目标（拖到您的项目中，确保勾选“如果需要则复制”）。

选项 B：CocoaPods
1) 从 `NoritoBridge.podspec.template` 创建 Podspec（填写 `s.source` zip URL）。
2) `pod trunk push NoritoBridge.podspec`。
3) 在你的 Podfile 中：`pod 'NoritoBridge'` → `pod install`。
4) 将 `NoritoBridgeKit.swift` 添加到您的应用程序目标。

进口
```swift
import Foundation
import CryptoKit               // ChaChaPoly / HKDF
import IrohaSwift              // ConnectClient / ConnectSession / ConnectCrypto
import NoritoBridge            // Clang module from the XCFramework
// Ensure NoritoBridgeKit.swift is part of the target
```

### 引导连接会话

`ConnectClient` 处理 WebSocket，而 `ConnectSession` 协调控制
帧和密文信封。下面的代码片段展示了 dApp 如何打开会话，
派生连接密钥，并等待批准响应。

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

### 发送密文帧（签名请求等）

当 dApp 需要请求签名时，它使用 Norito 桥助手进行编码
一个信封，使用 ChaChaPoly 加密有效负载，并将其包装在 `ConnectFrame` 中。

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

`ConnectAEAD.header` / `ConnectAEAD.nonce` 是方便的助手（请参阅中的片段
`docs/connect_swift_ios.md`) 从共享 `connect:v1` 标头定义构建。他们
如果您不想添加其他实用程序，则很容易内联：

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

### 接收/解密帧

`ConnectSession` 已经暴露了 `nextEnvelope()`，它在定向时解密有效负载
键已配置。如果您需要手动访问（例如匹配现有解码器
pipeline），您可以调用较低级别的帮助程序：

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

`NoritoBridgeKit` 还公开了帮助程序，例如 `decodeCiphertextFrame`、`decodeEnvelopeJson`、
和 `decodeSignResultAlgorithm` 用于调试或互操作性测试。用于生产
应用程序，依赖 `ConnectSession` 和 `ConnectEnvelope`，因此行为与 Rust 和
确切地说是 Android SDK。

## CI 验证

- 在发布更新的桥接工件或推送 Connect 集成之前，运行：

  ```bash
  make swift-ci
  ```

  目标验证夹具奇偶性、检查仪表板源并呈现 CLI
  本地总结。在 Buildkite 中，相同的工作流程取决于元数据键，例如
  `ci/xcframework-smoke:<lane>:device_tag`；确认编辑后元数据存在
  管道或代理标签，以便仪表板可以将结果归因于正确的模拟器或
  保险箱巷。
- 如果命令失败，请遵循奇偶校验手册 (`docs/source/swift_parity_triage.md`)
  并检查渲染的 `mobile_ci` 输出以确定哪个通道需要再生
  或重试之前的事件跟进。