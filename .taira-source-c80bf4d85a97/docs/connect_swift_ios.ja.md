<!-- Japanese translation of docs/connect_swift_ios.md -->

lang: ja
direction: ltr
source: docs/connect_swift_ios.md
status: complete
translator: manual
---

## 推奨 SDK フロー（ConnectClient + Norito ブリッジ）

Xcode への統合手順（SPM / CocoaPods、XCFramework、ChaChaPoly ハンドリング）をまとめて確認したい場合は
`docs/connect_swift_integration.md` を参照してください。以下ではフレームワークを組み込んだ後のワークフローに焦点を当てます。

Swift SDK には Norito ベースの Connect スタックが同梱されています。
- `ConnectClient` が `URLSessionWebSocketTask` 上で `/v1/connect/ws?...` の WebSocket を維持します。
- `ConnectSession` が Open→Approve/Reject→Sign→Close のライフサイクルを管理し、方向キー設定後は暗号化フレームを復号します。
- `ConnectCrypto` は X25519 キーペア生成と Norito 仕様準拠の方向キー導出を提供し、HKDF/ChaChaPoly を手書きする必要がありません。
- `ConnectEnvelope` / `ConnectControl` は `connect_norito_bridge` が返す型付きフレームと一致し、Android / Rust と同じワイヤ形式になります。

セッション開始前に:
1. 共通レシピ (`"iroha-connect|sid|" || chain_id || app_pk || nonce16`) で 32 バイトの `sid` を導出します。
2. `ConnectCrypto.generateKeyPair()` で Connect キーを生成（既存秘密鍵がある場合は `ConnectCrypto.publicKey(fromPrivateKey:)` で再計算）。
3. WebSocket の URL を組み立て、非同期コンテキスト内で `ConnectClient` を起動します。

```swift
import IrohaSwift

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
        appMetadata: ConnectAppMetadata(name: "Demo dApp", iconURL: nil, description: "Sample workflow"),
        constraints: ConnectConstraints(chainID: "00000000-0000-0000-0000-000000000000"),
        permissions: ConnectPermissions(methods: ["sign"], events: [])
    )
    try await connectSession.sendOpen(open: open)

    // ウォレットの応答 (approve/reject/close) を待つ
    if case .approve(let approval) = try await connectSession.nextControlFrame() {
        let directionKeys = try ConnectCrypto.deriveDirectionKeys(
            localPrivateKey: keyPair.privateKey,
            peerPublicKey: approval.walletPublicKey,
            sessionID: sessionID
        )
        connectSession.setDirectionKeys(directionKeys)

        // 暗号化フレーム（署名結果や Close など）を復号
        let envelope = try await connectSession.nextEnvelope()
        switch envelope.payload {
        case .signResultOk(let signature):
            print("signature:", signature.signature.base64EncodedString())
        case .controlClose(let close):
            print("session closed:", close.code, close.reason ?? "<none>")
        default:
            break
        }
    }
}
```

`ConnectSession` は方向キー未設定のまま暗号化フレームを受け取ると `ConnectSessionError.missingDecryptionKeys` を投げます。`Approve` フレームにウォレット公開鍵が含まれるので、その直後に `ConnectCrypto.deriveDirectionKeys` を呼んで `setDirectionKeys(_:)` してください。暗号化フレームを直接調べたい場合は `ConnectEnvelope.decrypt(frame:symmetricKey:)` に方向別キーを渡します。

> **メモ:** Norito ブリッジ（XCFramework）がリンクされていない SwiftPM ビルドでは自動的に JSON シムへフォールバックします。`ConnectCrypto.*` はブリッジ依存なので本番アプリでは XCFramework を必ず読み込んでください。

## 手動 CryptoKit リファレンス（レガシー／フォールバック）

Norito ブリッジが使えない場合や軽量な実験を行いたい場合は、以下の CryptoKit 実装を参考にしてください。Norito 仕様には準拠しますが、ConnectSession が提供する FFI 連携やパリティ保証は得られません。

このスニペットでは以下を実装します。
- sid と salt の計算（BLAKE2b-256。ここではサードパーティの BLAKE2b パッケージまたはあらかじめ計算した値を利用）。
- X25519 キー共有（`Curve25519.KeyAgreement`）。
- HKDF-SHA256（salt と info を使用）による方向キー導出。
- 「connect:v1」「sid」「dir」「seq」「kind=Ciphertext」からなる AAD の生成と、`seq` 由来のノンス生成。
- ChaChaPoly + AAD での暗号化／復号。
- トークンを使った WS 参加。

iOS 13 以上（CryptoKit 必須）。BLAKE2b は小さな Swift パッケージ（例: <https://github.com/Frizlab/SwiftBLAKE2>）を使うか、サーバー側で salt/sid を計算してください。以下の例は `blake2b(data: Data) -> Data` を利用できる前提です。

```swift
import Foundation
import CryptoKit

func aadV1(sid: Data, dir: UInt8, seq: UInt64) -> Data {
    var out = Data()
    out.append("connect:v1".data(using: .utf8)!)
    out.append(sid)
    out.append(Data([dir]))
    var le = seq.littleEndian
    withUnsafeBytes(of: &le) { out.append($0) }
    out.append(Data([1])) // kind=Ciphertext
    return out
}

func nonceFromSeq(_ seq: UInt64) -> ChaChaPoly.Nonce {
    var n = Data(count: 12)
    var le = seq.littleEndian
    n.replaceSubrange(4..<12, with: withUnsafeBytes(of: &le) { Data($0) })
    return try! ChaChaPoly.Nonce(data: n)
}

func hkdf(_ ikm: SymmetricKey, salt: Data, info: Data, len: Int = 32) -> SymmetricKey {
    let sk = HKDF<SHA256>.deriveKey(inputKeyMaterial: ikm, salt: SymmetricKey(data: salt), info: info, outputByteCount: len)
    return sk
}

// 方向キー（app→wallet, wallet→app）を導出
func deriveDirectionKeys(sharedSecret: SharedSecret, sid: Data) -> (SymmetricKey, SymmetricKey) {
    let salt = blake2b(data: Data("iroha-connect|salt|".utf8) + sid)
    let ikm = sharedSecret.hkdfDerivedSymmetricKey(using: SHA256.self, salt: salt, sharedInfo: Data(), outputByteCount: 32)
    let kApp = hkdf(ikm, salt: salt, info: Data("iroha-connect|k_app".utf8))
    let kWallet = hkdf(ikm, salt: salt, info: Data("iroha-connect|k_wallet".utf8))
    return (kApp, kWallet)
}

// AAD と seq 由来ノンスを用いて暗号化
func sealEnvelopeV1(key: SymmetricKey, sid: Data, dir: UInt8, seq: UInt64, payload: Data) -> Data {
    let aad = aadV1(sid: sid, dir: dir, seq: seq)
    let nonce = nonceFromSeq(seq)
    let sealed = try! ChaChaPoly.seal(payload, using: key, nonce: nonce, authenticating: aad)
    return sealed.combined // ciphertext||tag
}

// 復号し、seq を検証
func openEnvelopeV1(key: SymmetricKey, sid: Data, dir: UInt8, seq: UInt64, combined: Data) -> Data {
    let aad = aadV1(sid: sid, dir: dir, seq: seq)
    let nonce = nonceFromSeq(seq)
    let box = try! ChaChaPoly.SealedBox(combined: combined)
    return try! ChaChaPoly.open(box, using: key, authenticating: aad)
}

// Base64URL（パディングなし）でエンコード
func base64url(_ data: Data) -> String {
    let b64 = data.base64EncodedString()
    return b64.replacingOccurrences(of: "+", with: "-")
              .replacingOccurrences(of: "/", with: "_")
              .replacingOccurrences(of: "=", with: "")
}

// Connect セッションを作成: クライアントが sid を計算し /v1/connect/session に POST
func createConnectSession(node: String, chainId: String, appEphemeralPk: Data, completion: @escaping (Result<(sidB64: String, tokenApp: String, tokenWallet: String), Error>) -> Void) {
    // sid = BLAKE2b-256("iroha-connect|sid|" || chain_id || app_pk || nonce16)
    let nonce16 = (0..<16).map { _ in UInt8.random(in: 0...255) }
    var sidInput = Data("iroha-connect|sid|".utf8)
    sidInput.append(Data(chainId.utf8))
    sidInput.append(appEphemeralPk)
    sidInput.append(Data(nonce16))
    let sid = blake2b(data: sidInput)
    let sidB64 = base64url(sid)

    // JSON { sid, node } を POST
    let url = URL(string: node + "/v1/connect/session")!
    var req = URLRequest(url: url)
    req.httpMethod = "POST"
    req.setValue("application/json", forHTTPHeaderField: "Content-Type")
    let body = ["sid": sidB64, "node": node]
    req.httpBody = try? JSONSerialization.data(withJSONObject: body, options: [])
    URLSession.shared.dataTask(with: req) { data, resp, err in
        if let err = err { completion(.failure(err)); return }
        guard let http = resp as? HTTPURLResponse, let data = data, http.statusCode == 200,
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let tokenApp = obj["token_app"] as? String,
              let tokenWallet = obj["token_wallet"] as? String,
              let sidEcho = obj["sid"] as? String
        else {
            completion(.failure(NSError(domain: "connect", code: -1)))
            return
        }
        completion(.success((sidEcho, tokenApp, tokenWallet)))
    }.resume()
}

// トークンで WS に参加（URLSessionWebSocketTask）
func joinWs(node: String, sid: String, role: String, token: String, onMessage: @escaping (Data)->Void) {
    let wsUrl = node.replacingOccurrences(of: "http", with: "ws") + "/v1/connect/ws?sid=\(sid)&role=\(role)"
    var request = URLRequest(url: URL(string: wsUrl)!)
    request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
    let task = URLSession.shared.webSocketTask(with: request)
    task.resume()
    func recv() {
        task.receive { result in
            switch result {
            case .failure(let e): print("ws error", e)
            case .success(let msg):
                switch msg {
                case .data(let d): onMessage(d)
                case .string: break
                @unknown default: break
                }
                recv()
            }
        }
    }
    recv()
}

// 利用例（アプリ側）: キーと sid を計算済みの場合
// let frameBinary = noritoEncodeConnectFrameV1(... Ciphertext { aead: sealEnvelopeV1(key: kApp, ...) })
// task.send(.data(frameBinary))
```

備考:
- クライアント側で `sid` を計算し、その `sid` を添えて `/v1/connect/session` に POST するとロールごとのトークンが取得できます。WS 参加時はトークンを使用してください。
- Approve 後は Close/Reject を暗号化ペイロードとして送信します。
- `aead` を `ConnectFrameV1` にパックする際は Norito フレーミングが必要です。Norito バインディングで実装してください。

### Norito フレーミング デモ（暫定版）

以下はローカルデモ向けの最小限で非正規なフレーミングです。Norito 互換ではないため、正式な Norito エンコーダ／デコーダが利用可能になり次第置き換えてください。ここでは単純にフィールドを小端リトルエンディアンで連結し、iOS でもフルコーデックなしに WS 伝送を確認できるようにしています。

レイアウト（デモ専用）:
- 32 バイト: sid
- 1 バイト: dir（0 = AppToWallet、1 = WalletToApp）
- 8 バイト: seq（LE）
- 1 バイト: kind（1 = Ciphertext）
- 1 バイト: ciphertext 内の dir（共有型との整合のため）
- 4 バイト: ct_len（LE）
- ct_len バイト: aead（ChaChaPoly の combined）

```swift
func le64(_ x: UInt64) -> Data { var v = x.littleEndian; return withUnsafeBytes(of: &v) { Data($0) } }
func le32(_ x: UInt32) -> Data { var v = x.littleEndian; return withUnsafeBytes(of: &v) { Data($0) } }

// Ciphertext ConnectFrameV1 をフレーム化（デモ用）
func frameCiphertextV1Demo(sid: Data, dir: UInt8, seq: UInt64, aead: Data) -> Data {
    precondition(sid.count == 32)
    var out = Data(capacity: 32 + 1 + 8 + 1 + 1 + 4 + aead.count)
    out.append(sid)
    out.append(Data([dir]))
    out.append(le64(seq))
    out.append(Data([1])) // kind = Ciphertext
    out.append(Data([dir]))
    out.append(le32(UInt32(aead.count)))
    out.append(aead)
    return out
}

struct ParsedCiphertextV1Demo { let sid: Data; let dir: UInt8; let seq: UInt64; let aead: Data }

// デモフレームを構造体へ戻す
func parseCiphertextV1Demo(_ data: Data) -> ParsedCiphertextV1Demo? {
    if data.count < 32 + 1 + 8 + 1 + 1 + 4 { return nil }
    var o = 0
    let sid = data.subdata(in: o..<(o+32)); o += 32
    let dir = data[o]; o += 1
    let seqLe = data.subdata(in: o..<(o+8)); o += 8
    let seq = seqLe.withUnsafeBytes { $0.load(as: UInt64.self) }.littleEndian
    let kind = data[o]; o += 1
    guard kind == 1 else { return nil }
    let dir2 = data[o]; o += 1
    guard dir2 == dir else { return nil }
    let lenLe = data.subdata(in: o..<(o+4)); o += 4
    let ctLen = lenLe.withUnsafeBytes { $0.load(as: UInt32.self) }.littleEndian
    guard data.count >= o + Int(ctLen) else { return nil }
    let aead = data.subdata(in: o..<(o+Int(ctLen)))
    return ParsedCiphertextV1Demo(sid: sid, dir: dir, seq: seq, aead: aead)
}

// 送信例
// let ct = sealEnvelopeV1(key: kApp, sid: sid, dir: 0, seq: 1, payload: payload)
// let frame = frameCiphertextV1Demo(sid: sid, dir: 0, seq: 1, aead: ct)
// wsTask.send(.data(frame))

// 受信例
// case .data(let d): if let f = parseCiphertextV1Demo(d) { let pt = openEnvelopeV1(key: kWallet, sid: f.sid, dir: f.dir, seq: f.seq, combined: f.aead) }
```

### 暗号化された Close/Reject デモ（暫定フレーミング）

`kWallet`（wallet→app 方向キー）を導出済みで、`sid`（32 バイト）と接続済みの `URLSessionWebSocketTask`（`ws`）がある前提:

```swift
// seq=1 で暗号化 Close（wallet → app）を送信
let closePayload = try! JSONSerialization.data(withJSONObject: [
  "Control": [
    "Close": [
      "who": "Wallet", // デモ用ログのみ
      "code": 1000,
      "reason": "done",
      "retryable": false
    ]
  ]
], options: [])
let ctClose = sealEnvelopeV1(key: kWallet, sid: sid, dir: 1, seq: 1, payload: closePayload)
let frameClose = frameCiphertextV1Demo(sid: sid, dir: 1, seq: 1, aead: ctClose)
ws.send(.data(frameClose)) { err in if let err = err { print("ws send close:", err) } }

// seq=2 で暗号化 Reject（wallet → app）を送信
let rejectPayload = try! JSONSerialization.data(withJSONObject: [
  "Control": [
    "Reject": [
      "code": 401,
      "code_id": "UNAUTHORIZED",
      "reason": "user denied"
    ]
  ]
], options: [])
let ctReject = sealEnvelopeV1(key: kWallet, sid: sid, dir: 1, seq: 2, payload: rejectPayload)
let frameReject = frameCiphertextV1Demo(sid: sid, dir: 1, seq: 2, aead: ctReject)
ws.send(.data(frameReject)) { err in if let err = err { print("ws send reject:", err) } }
```

受信時は `parseCiphertextV1Demo` と `openEnvelopeV1` を利用して平文 JSON を取り出し、アプリ側の型にデコードしてください。実際に統合する際は、このデモフレーミングを正規の Norito エンコードに置き換えます。

## CI バリデーション

- Connect 統合やブリッジ関連の変更を行う際は、事前に以下を実行してください。

  ```bash
  make swift-ci
  ```

  このターゲットは Swift のフィクスチャ検証に加え、ダッシュボード用 JSON のチェックと CLI レンダリングを行います。Buildkite では `ci/xcframework-smoke:<lane>:device_tag` 形式のメタデータに依存しており、`iphone-sim` や `strongbox` などの対象レーンを識別します。パイプラインやエージェントタグを変更した場合は、メタデータが出力されていることを確認してください。
- コマンドが失敗した場合は `docs/source/swift_parity_triage.md` の手順に従い、`mobile_ci` の出力を確認してどのレーンが再生成またはインシデント対応を必要としているか判断します。
