<!-- Japanese translation of docs/connect_swift_integration.md -->

---
lang: ja
direction: ltr
source: docs/connect_swift_integration.md
status: complete
translator: manual
---

## iOS プロジェクトでの NoritoBridgeKit 統合

このガイドでは、NoritoBridge.xcframework と Swift ラッパーを iOS アプリに組み込み、`ConnectClient` / `ConnectSession` / `ConnectCrypto` を利用して Rust と同じ Norito コーデックで Connect フレームを扱う手順を説明します。

前提条件
- NoritoBridge.xcframework（CI で生成）と `NoritoBridgeKit.swift`（`examples/ios/NoritoDemo/Sources/NoritoBridgeKit.swift` からコピー可能）。
- Xcode 15 以上、iOS 13 以上をターゲットとするプロジェクト。

### Option A: Swift Package Manager（推奨）
1. `crates/connect_norito_bridge/Package.swift.template` を使ってバイナリ SPM を公開（CI の URL / checksum を設定）。
2. Xcode → `File → Add Packages…` → リポジトリ URL を入力 → `NoritoBridge` プロダクトをターゲットに追加。
3. `NoritoBridgeKit.swift` をターゲットへ追加（ドラッグ＆ドロップし、「Copy if needed」をオン）。

### Option B: CocoaPods
1. `NoritoBridge.podspec.template` から Podspec を作成し、`s.source` に zip URL を設定。
2. `pod trunk push NoritoBridge.podspec`
3. Podfile に `pod 'NoritoBridge'` を追加し、`pod install`
4. `NoritoBridgeKit.swift` をターゲットへ追加。

### Imports
```swift
import Foundation
import CryptoKit               // ChaChaPoly / HKDF
import IrohaSwift              // ConnectClient / ConnectSession / ConnectCrypto
import NoritoBridge            // XCFramework の Clang モジュール
// NoritoBridgeKit.swift も同じターゲットに含めてください
```

### Connect セッションをブートストラップ

```swift
let connectURL = URL(string: "wss://node.example/v2/connect/ws?sid=\(sidB64)&role=app")!
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
        let firstEnvelope = try await connectSession.nextEnvelope()
        print("payload:", firstEnvelope.payload)
    }
}
```

### 暗号化フレームの送信

```swift
let bridge = NoritoBridgeKit()
let seq = nextSequence()
let txBytes = Data([0x01, 0x02, 0x03])
let envelope = try bridge.encodeEnvelopeSignRequestTx(seq: seq, tx: txBytes)

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

`ConnectAEAD`（ヘッダー / nonce 生成ヘルパー）の例:

```swift
enum ConnectAEAD {
    static func header(sessionID: Data, direction: ConnectDirection, sequence: UInt64) -> Data {
        var buffer = Data()
        buffer.append("connect:v1".data(using: .utf8)!)
        buffer.append(sessionID)
        buffer.append(direction == .appToWallet ? 0 : 1)
        var seq = sequence.littleEndian
        withUnsafeBytes(of: &seq) { buffer.append(contentsOf: $0) }
        buffer.append(1) // Ciphertext
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

### 受信と復号

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

`NoritoBridgeKit.decodeCiphertextFrame` や `decodeEnvelopeJson` なども用意されているため、デバッグや相互運用テストで直接ワイヤデータを確認できます。実運用では `ConnectSession` / `ConnectEnvelope` を使用し、Rust / Android SDK と完全に同じ挙動を維持することを推奨します。

## CI バリデーション

- ブリッジ成果物を更新したり Connect 統合を変更する前に、以下を実行してください。

  ```bash
  make swift-ci
  ```

  このターゲットはフィクスチャパリティの確認に加え、ダッシュボード用 JSON の検証と CLI レンダリングを行います。Buildkite では `ci/xcframework-smoke:<lane>:device_tag` 形式のメタデータに依存しており、対象レーン（`iphone-sim` や `strongbox` など）を特定します。パイプラインやエージェントタグを変更した場合は、メタデータが出力されていることを必ず確認してください。
- コマンドが失敗した場合は `docs/source/swift_parity_triage.md` の手順に従い、レンダリングされた `mobile_ci` 出力を確認して、どのレーンに再生成またはインシデント対応が必要か判断してください。
