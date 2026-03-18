---
lang: ar
direction: rtl
source: docs/connect_swift_ios.md
status: complete
translator: manual
source_hash: 0f2dbf44a069becc70aee5abfeff66ad22ca2ae04c605cea9e953d4e750dccc6
source_last_modified: "2025-11-02T04:40:28.813100+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/connect_swift_ios.md (Recommended SDK Flow + Manual CryptoKit Reference) -->

## التدفق الموصى به في الـ SDK (ConnectClient + جسر Norito)

تحتاج إلى شرح كامل لدمج Xcode (SPM / CocoaPods، ربط الـ XCFramework، و helpers
الخاصة بـ ChaChaPoly)؟
اطّلع على `docs/connect_swift_integration.md` للحصول على دليل التغليف من البداية إلى
النهاية.

حزمة Swift SDK توفّر طبقة Connect مبنية على Norito:

- يقوم `ConnectClient` بإدارة نقل WebSocket (`/v1/connect/ws?...`) فوق
  `URLSessionWebSocketTask`.
- يتولّى `ConnectSession` تنسيق دورة الحياة (open → approve/reject → sign → close) و
  يفك تشفير إطارات الـ ciphertext بعد تهيئة مفاتيح الاتجاه.
- يوفّر `ConnectCrypto` توليد مفاتيح X25519 واشتقاق مفاتيح اتجاه متوافقة مع Norito، كي
  لا تضطر التطبيقات إلى تنفيذ HKDF/HMAC يدويًا.
- يمثّل كل من `ConnectEnvelope` و`ConnectControl` إطارات Norito ذات الأنواع المحددة
  الصادرة من bridge Rust (`connect_norito_bridge`)؛ ويتم فك تشفير الأظرف المشفّرة عبر
  نفس الـ helpers الخاصة بـ FFI المستخدمة على Android و Rust، مما يضمن التوافق.

قبل بدء الجلسة:
1. اشتقّ معرّف جلسة بطول 32 بايت (`sid`) باستخدام نفس وصفة BLAKE2b المستخدمة في باقي
   الـ SDKs (`"iroha-connect|sid|" || chain_id || app_pk || nonce16`).
2. ولّد زوج مفاتيح Connect عبر `ConnectCrypto.generateKeyPair()` أو أعد استخدام مفتاح
   خاص مخزَّن (يمكن إعادة حساب المفتاح العام باستخدام
   `ConnectCrypto.publicKey(fromPrivateKey:)`).
3. أنشئ عميل WebSocket وابدأ تشغيله داخل سياق async.

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

يرمي `ConnectSession` الخطأ `ConnectSessionError.missingDecryptionKeys` إذا وصلت إطارات
مشفرّة قبل تثبيت مفاتيح الاتجاه؛ اشتقّ المفاتيح مباشرة بعد معالجة رسالة `Approve`
(يتضمّن الـ payload المفتاح العام للمحفظة). ولتفحص إطارات الـ ciphertext يدويًا، استدع
`ConnectEnvelope.decrypt(frame:symmetricKey:)` مع مفتاح الاتجاه المناسب لاتجاه الإطار.

> **ملاحظة:** عندما يكون جسر Norito غير موجود (مثلًا في بناء Swift Package Manager
> بدون الـ XCFramework)، يرجع الـ SDK تلقائيًا إلى طبقة JSON بسيطة (shim). تحتاج
> دوال التشفير (`ConnectCrypto.*`) إلى وجود الجسر، لذا احرص على ربط الـ XCFramework في
> تطبيقات الإنتاج.

## مرجع CryptoKit اليدوي (وضع قديم / fallback)

إذا لم يكن جسر Norito متوفرًا أو كنت بحاجة إلى نمذجة سريعة دون استخدام helpers الـ
SDK، يوضّح هذا القسم كيفية ربط X25519 مع ChaChaPoly يدويًا. هذا متوافق مع مواصفات
Norito لكنه لا يوفّر ضمانات التماثل (parity) الحتمية التي يقدّمها الـ SDK جاهزًا.

يوضّح هذا القسم كيفية:
- حساب `sid` و`salt` (BLAKE2b‑256؛ باستخدام مكتبة BLAKE2b لطرف ثالث أو قيم مسبقة
  الحساب).
- إجراء اتفاقية مفاتيح X25519 (`Curve25519.KeyAgreement`).
- اشتقاق مفتاح الاتجاه عبر HKDF‑SHA256 باستخدام `salt` و`info`.
- بناء بيانات AAD من ("connect:v1", sid, dir, seq, kind=Ciphertext) واستخدام `seq`
  لاشتقاق الـ nonce.
- تشفير وفتح الـ payload عبر ChaChaPoly مع AAD.
- الانضمام إلى WebSocket باستخدام token.

يتطلّب ذلك iOS 13+ (CryptoKit). بالنسبة لـ BLAKE2b، استخدم حزمة Swift صغيرة (مثل
https://github.com/Frizlab/SwiftBLAKE2) أو احسب `salt`/`sid` على الخادم؛ ويفترض المثال
وجود دالة `blake2b(data: Data) -> Data`.

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

// Example usage (app-side): after computing keys and sid
// let frameBinary = noritoEncodeConnectFrameV1(... Ciphertext { aead: sealEnvelopeV1(key: kApp, ...) })
// task.send(.data(frameBinary))
```

ملاحظات:
- احسب `sid` على جانب العميل ثم أرسل طلب POST إلى `/v1/connect/session` مع هذا `sid`
  للحصول على tokens لكل دور؛ بعد ذلك انضم إلى WebSocket باستخدام الـ token.
- بعد رسالة `Approve`، أرسل رسائل `Close`/`Reject` كـ payloads مشفّرة.
- تحتاج إلى framing Norito حقيقي لاحتواء `aead` داخل `ConnectFrameV1` كـ binary؛ نفّذ
  ذلك باستخدام ربطاتك (bindings) الخاصة بـ Norito.

### ديمو لتغليف Norito (Framing) – لأغراض الاختبار

المثال التالي هو تغليف بسيط وغير معياري يصلح للاختبارات المحلية فقط. هذا التغليف
غير متوافق مع Norito ويجب استبداله بـ encoder/decoder Norito حقيقي عند توفّره. يقوم
فقط بربط الحقول بالتسلسل بطريقة little‑endian ثابتة لكي تتمكن من اختبار النقل عبر
WebSocket من النهاية إلى النهاية دون استحضار codec كامل على iOS.

Layout (للـ demo فقط):
- 32 بايت: sid
- 1 بايت: dir (القيمة 0 = من التطبيق إلى المحفظة AppToWallet، والقيمة 1 = من المحفظة
  إلى التطبيق WalletToApp)
- 8 بايت: seq (ترتيب LE)
- 1 بايت: kind (القيمة 1 = Ciphertext)
- 1 بايت: تكرار `dir` داخل بيانات الـ ciphertext (للحفاظ على التماثل مع النوع المشترك)
- 4 بايت: ct_len (ترتيب LE)
- عدد ct_len من البايتات: `aead` (الناتج الموحّد من ChaChaPoly)

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

### ديمو لإرسال Close/Reject مشفّرين (تغليف تجريبي)

على افتراض أنك اشتققت بالفعل المفتاح `kWallet` (مفتاح اتجاه من المحفظة إلى التطبيق
wallet→app) وتمتلك قيمة `sid` بطول 32 بايت، بالإضافة إلى كائن
`URLSessionWebSocketTask` باسم `ws`:

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

عند الاستقبال، استخدم `parseCiphertextV1Demo` و`openEnvelopeV1` للحصول على JSON
بالنص الصريح، ثم حوِّله (decode) إلى الأنواع المستخدمة في تطبيقك. عند الدمج الفعلي،
استبدل هذا التغليف التجريبي بترميز Norito حقيقي.

## التحقق عبر CI

- قبل إجراء تغييرات على Connect أو على تكامل bridge، نفّذ:

  ```bash
  make swift-ci
  ```

  هذا الأمر يتحقق من الـ fixtures الخاصة بـ Swift، ويفحص التدفقات المستخدمة في
  لوحات المراقبة (dashboards)، ويولّد ملخصات CLI. يعتمد مسار CI على بيانات
  Buildkite الوصفية (`ci/xcframework-smoke:<lane>:device_tag`) لربط النتائج
  بالمحاكي أو بمسارات StrongBox؛ بعد أي تغيير في الـ pipelines أو وسوم الـ agents،
  تأكد من أن هذه البيانات الوصفية ما زالت تظهر في السجلات.
- إذا فشلت العملية، فاتبع `docs/source/swift_parity_triage.md` وراجع مخرجات
  `mobile_ci` لتحديد أي lane يحتاج إلى إعادة توليد أو إلى معالجة إضافية للحادثة.

</div>

