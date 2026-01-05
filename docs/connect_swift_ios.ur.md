---
lang: ur
direction: rtl
source: docs/connect_swift_ios.md
status: complete
translator: manual
source_hash: 0f2dbf44a069becc70aee5abfeff66ad22ca2ae04c605cea9e953d4e750dccc6
source_last_modified: "2025-11-02T04:40:28.813100+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- اردو ترجمہ: docs/connect_swift_ios.md (Recommended SDK Flow + Manual CryptoKit Reference) -->

## تجویز کردہ SDK فلو (ConnectClient + Norito پل)

اگر آپ کو Xcode میں مکمل انٹیگریشن کا walkthrough چاہیے (SPM/CocoaPods، XCFramework
وائرنگ، ChaChaPoly helpers وغیرہ)،
تو `docs/connect_swift_integration.md` میں موجود اینڈ ٹو اینڈ پیکیجنگ گائیڈ دیکھیں۔

Swift SDK ایک Norito پر مبنی Connect اسٹیک فراہم کرتا ہے:

- `ConnectClient`، `URLSessionWebSocketTask` کے اوپر WebSocket (`/v1/connect/ws?...`)
  ٹرانسپورٹ کو مینٹین کرتا ہے۔
- `ConnectSession` لائف سائیکل کو منظم کرتا ہے (open → approve/reject → sign → close)
  اور جب direction keys سیٹ ہو جائیں تو ciphertext فریمز کو ڈی کرپٹ کرتا ہے۔
- `ConnectCrypto`، X25519 key generation اور Norito‑مطابق direction‑key derivation
  فراہم کرتا ہے تاکہ ایپس کو HKDF/HMAC کی manual وائرنگ نہ کرنی پڑے۔
- `ConnectEnvelope`/`ConnectControl` وہ typed Norito فریمز ہیں جو Rust bridge
  (`connect_norito_bridge`) سے خارج ہوتے ہیں؛ encrypted envelopes کو وہی FFI helpers
  ڈی کرپٹ کرتے ہیں جو Android/Rust میں استعمال ہو رہے ہیں، اس طرح parity برقرار
  رہتی ہے۔

سیشن شروع کرنے سے پہلے:
1. 32‑بائٹ کا سیشن آئی ڈی (`sid`) وہی BLAKE2b ترکیب استعمال کر کے نکالیں جو دوسرے
   SDKs میں ہے (`"iroha-connect|sid|" || chain_id || app_pk || nonce16`)۔
2. `ConnectCrypto.generateKeyPair()` سے Connect key pair جنریٹ کریں، یا محفوظ شدہ
   پرائیویٹ کی کو reuse کریں (پبلک کی کو `ConnectCrypto.publicKey(fromPrivateKey:)`
   کے ذریعے دوبارہ نکالا جا سکتا ہے)۔
3. WebSocket کلائنٹ بنائیں اور اسے async context کے اندر اسٹارٹ کریں۔

```swift
import IrohaSwift

let connectURL = URL(string: "wss://node.example/v1/connect/ws?sid=\(sidB64)&role=app&token=\(token)")!
let connectClient = ConnectClient(url: connectURL)
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

    // Wait for wallet response (approve/reject/close)
    if case .approve(let approval) = try await connectSession.nextControlFrame() {
        let directionKeys = try ConnectCrypto.deriveDirectionKeys(
            localPrivateKey: keyPair.privateKey,
            peerPublicKey: approval.walletPublicKey,
            sessionID: sessionID
        )
        connectSession.setDirectionKeys(directionKeys)

        // Decrypt ciphertext frames (sign results, encrypted controls)
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

اگر ciphertext فریمز direction keys لگنے سے پہلے آ جائیں تو `ConnectSession`
،`ConnectSessionError.missingDecryptionKeys` تھرو کرتا ہے؛ اس لیے `Approve` کنٹرول
(جس کے payload میں wallet کی public key ہوتی ہے) پراسیس کرنے کے فوراً بعد keys مشتق
کریں۔ اگر آپ ciphertext فریمز کو دستی طور پر دیکھنا چاہیں تو
`ConnectEnvelope.decrypt(frame:symmetricKey:)` استعمال کریں اور وہی directional key
دیں جو اس فریم کے direction کے مطابق ہو۔

> **نوٹ:** جب Norito bridge موجود نہ ہو (مثلاً Swift Package Manager سے build کرتے
> وقت XCFramework لنک نہ کیا گیا ہو) تو SDK خود بخود JSON shim پر fallback کر لیتا
> ہے۔ Encryption helpers (`ConnectCrypto.*`) کے لیے bridge کا ہونا ضروری ہے، لہٰذا
> پروڈکشن ایپس میں XCFramework کو لازماً لنک کریں۔

## CryptoKit کا دستی ریفرنس (لیگیسی / fallback)

اگر Norito bridge دستیاب نہ ہو، یا آپ کو بغیر SDK helpers کے تیز prototyping درکار
ہو، تو یہ ریفرنس دکھاتا ہے کہ X25519 + ChaChaPoly کو دستی طور پر کیسے وائر کیا جائے۔
یہ Norito کی specification کے مطابق ہے مگر SDK کی فراہم کردہ deterministic parity
گارنٹیز فراہم نہیں کرتا۔

اس سیکشن میں یہ بتایا گیا ہے کہ کیسے:
- `sid` اور `salt` نکالیں (BLAKE2b‑256؛ یہاں third‑party BLAKE2b پیکیج یا precomputed
  ویلیوز استعمال کی گئی ہیں)۔
- X25519 key agreement کریں (`Curve25519.KeyAgreement`)۔
- HKDF‑SHA256 کے ذریعے `salt` اور `info` کے ساتھ direction key derive کریں۔
- ("connect:v1", sid, dir, seq, kind=Ciphertext) سے AAD بنائیں اور `seq` سے nonce
  نکالیں۔
- ChaChaPoly اور AAD کے ذریعے payload کو seal اور open کریں۔
- token کے ساتھ WebSocket جوائن کریں۔

iOS 13+ (CryptoKit) درکار ہے۔ BLAKE2b کے لیے کوئی چھوٹا سا Swift پیکیج استعمال کریں
(مثلاً https://github.com/Frizlab/SwiftBLAKE2) یا `salt`/`sid` سرور سائیڈ پر compute
کریں؛ ذیل کا کوڈ یہ فرض کرتا ہے کہ آپ کے پاس `blake2b(data: Data) -> Data` دستیاب
ہے۔

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

// Derive direction keys (app→wallet, wallet→app)
func deriveDirectionKeys(sharedSecret: SharedSecret, sid: Data) -> (SymmetricKey, SymmetricKey) {
    let salt = blake2b(data: Data("iroha-connect|salt|".utf8) + sid)
    let ikm = sharedSecret.hkdfDerivedSymmetricKey(using: SHA256.self, salt: salt, sharedInfo: Data(), outputByteCount: 32)
    let kApp = hkdf(ikm, salt: salt, info: Data("iroha-connect|k_app".utf8))
    let kWallet = hkdf(ikm, salt: salt, info: Data("iroha-connect|k_wallet".utf8))
    return (kApp, kWallet)
}

// Seal payload with AAD and seq-derived nonce
func sealEnvelopeV1(key: SymmetricKey, sid: Data, dir: UInt8, seq: UInt64, payload: Data) -> Data {
    let aad = aadV1(sid: sid, dir: dir, seq: seq)
    let nonce = nonceFromSeq(seq)
    let sealed = try! ChaChaPoly.seal(payload, using: key, nonce: nonce, authenticating: aad)
    return sealed.combined // ciphertext||tag
}

// Open payload and validate seq
func openEnvelopeV1(key: SymmetricKey, sid: Data, dir: UInt8, seq: UInt64, combined: Data) -> Data {
    let aad = aadV1(sid: sid, dir: dir, seq: seq)
    let nonce = nonceFromSeq(seq)
    let box = try! ChaChaPoly.SealedBox(combined: combined)
    return try! ChaChaPoly.open(box, using: key, authenticating: aad)
}

// Base64URL (no padding) encode
func base64url(_ data: Data) -> String {
    let b64 = data.base64EncodedString()
    return b64.replacingOccurrences(of: "+", with: "-")
              .replacingOccurrences(of: "/", with: "_")
              .replacingOccurrences(of: "=", with: "")
}

// Create Connect session: client computes sid and POSTs to /v1/connect/session
func createConnectSession(node: String, chainId: String, appEphemeralPk: Data, completion: @escaping (Result<(sidB64: String, tokenApp: String, tokenWallet: String), Error>) -> Void) {
    // Compute sid = BLAKE2b-256("iroha-connect|sid|" || chain_id || app_pk || nonce16)
    let nonce16 = (0..<16).map { _ in UInt8.random(in: 0...255) }
    var sidInput = Data("iroha-connect|sid|".utf8)
    sidInput.append(Data(chainId.utf8))
    sidInput.append(appEphemeralPk)
    sidInput.append(Data(nonce16))
    let sid = blake2b(data: sidInput)
    let sidB64 = base64url(sid)

    // POST JSON { sid, node }
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

// Join WS with token (URLSessionWebSocketTask)
func joinWs(node: String, sid: String, role: String, token: String, onMessage: @escaping (Data)->Void) {
    let wsUrl = node.replacingOccurrences(of: "http", with: "ws") + "/v1/connect/ws?sid=\(sid)&role=\(role)&token=\(token)"
    let task = URLSession.shared.webSocketTask(with: URL(string: wsUrl)!)
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

// Example usage (app-side): after computing keys and sid
// let frameBinary = noritoEncodeConnectFrameV1(... Ciphertext { aead: sealEnvelopeV1(key: kApp, ...) })
// task.send(.data(frameBinary))
```

نوٹس:
- `sid` کلائنٹ کی طرف سے compute کریں، پھر `/v1/connect/session` پر اسی `sid` کے ساتھ
  POST کریں تاکہ ہر رول کے لیے tokens مل سکیں؛ WebSocket سے کنکشن جوڑتے وقت token
  استعمال کریں۔
- `Approve` کے بعد `Close`/`Reject` کو encrypted payloads کے طور پر بھیجیں۔
- `aead` کو بائنری `ConnectFrameV1` میں لپیٹنے کے لیے حقیقی Norito framing درکار ہے؛
  اسے اپنے Norito bindings کے ذریعے implement کریں۔

### Norito فریمنگ ڈیمو (Placeholder)

ذیل میں دیا گیا framing کم سے کم اور غیر canonical ہے، جو صرف مقامی demos کے لیے کافی
ہے۔ یہ Norito format کے مطابق نہیں ہے اور جب آپ کے پاس صحیح Norito encoder/decoder آ جائے
تو اسے ضرور تبدیل کریں۔ اس کا مقصد صرف یہ ہے کہ فیلڈز کو ایک مستقل little‑endian
آرڈر میں جوڑ کر آپ بغیر مکمل codec کے iOS پر end‑to‑end WebSocket transport کو
آزمانے کے قابل ہوں۔

Layout (صرف ڈیمو کے لیے):
- 32 بائٹ: sid
- 1 بائٹ: dir (0 = AppToWallet، 1 = WalletToApp)
- 8 بائٹ: seq (LE)
- 1 بائٹ: kind (1 = Ciphertext)
- 1 بائٹ: ciphertext کے اندر `dir` کی دوبارہ نقل (مشترکہ ٹائپ کے ساتھ parity برقرار
  رکھنے کے لیے)
- 4 بائٹ: ct_len (LE)
- ct_len بائٹ: aead (ChaChaPoly combined)

```swift
func le64(_ x: UInt64) -> Data { var v = x.littleEndian; return withUnsafeBytes(of: &v) { Data($0) } }
func le32(_ x: UInt32) -> Data { var v = x.littleEndian; return withUnsafeBytes(of: &v) { Data($0) } }

// Frame a ciphertext ConnectFrameV1 (demo framing)
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

// Parse demo frame back into components
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

// Example usage: send
// let ct = sealEnvelopeV1(key: kApp, sid: sid, dir: 0, seq: 1, payload: payload)
// let frame = frameCiphertextV1Demo(sid: sid, dir: 0, seq: 1, aead: ct)
// wsTask.send(.data(frame))

// Example usage: receive
// case .data(let d): if let f = parseCiphertextV1Demo(d) { let pt = openEnvelopeV1(key: kWallet, sid: f.sid, dir: f.dir, seq: f.seq, combined: f.aead) }
```

### Close/Reject کے encrypted فریمز کا ڈیمو (مثالی فریمنگ)

یہ فرض کرتے ہوئے کہ آپ نے پہلے ہی `kWallet` (wallet→app ڈائریکشن کی) derive کر لی
ہے، `sid` (32 بائٹ) موجود ہے اور آپ کے پاس کنیکٹڈ `URLSessionWebSocketTask` ہے جس کا
نام `ws` ہے:

```swift
// Send encrypted Close (wallet → app) at seq=1
let closePayload = try! JSONSerialization.data(withJSONObject: [
  "Control": [
    "Close": [
      "who": "Wallet", // for demo logging only
      "code": 1000,
      "reason": "done",
      "retryable": false
    ]
  ]
], options: [])
let ctClose = sealEnvelopeV1(key: kWallet, sid: sid, dir: 1, seq: 1, payload: closePayload)
let frameClose = frameCiphertextV1Demo(sid: sid, dir: 1, seq: 1, aead: ctClose)
ws.send(.data(frameClose)) { err in if let err = err { print("ws send close:", err) } }

// Send encrypted Reject (wallet → app) at seq=2
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

ریسیو کرتے وقت `parseCiphertextV1Demo` اور `openEnvelopeV1` استعمال کریں تاکہ plaintext
JSON نکل آئے، پھر اسے اپنی ایپ کے ٹائپس میں decode کریں۔ اصل انٹیگریشن کرتے وقت اس
ڈیمو فریمنگ کو حقیقی Norito encoding سے بدل دیں۔

## CI کے ذریعے تصدیق

- Connect یا bridge انٹیگریشن میں تبدیلی کرنے سے پہلے یہ کمانڈ چلائیں:

  ```bash
  make swift-ci
  ```

  یہ کمانڈ Swift fixtures کو ویری فائی کرتی ہے، dashboards کے فیڈز چیک کرتی ہے اور CLI
  سمریز رینڈر کرتی ہے۔ CI ورک فلو Buildkite کے میٹا ڈیٹا
  (`ci/xcframework-smoke:<lane>:device_tag`) پر انحصار کرتا ہے تاکہ نتائج کو سیمولیٹر
  یا StrongBox lanes سے میپ کیا جا سکے؛ جب بھی آپ pipelines یا agent tags میں تبدیلی
  کریں، اس کے بعد لاگز میں ان میٹا ڈیٹا keys کی موجودگی کی تصدیق کریں۔
- اگر رن fail ہو جائے تو `docs/source/swift_parity_triage.md` کی ہدایات فالو کریں اور
  `mobile_ci` کا آؤٹ پٹ دیکھیں تاکہ یہ پتہ چل سکے کہ کون سا lane دوبارہ جنریٹ یا
  مزید incident ہینڈلنگ کا محتاج ہے۔

</div>
