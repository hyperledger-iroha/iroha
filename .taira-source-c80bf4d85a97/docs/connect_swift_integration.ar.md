---
lang: ar
direction: rtl
source: docs/connect_swift_integration.md
status: complete
translator: manual
source_hash: ebdea5644112eab6e2027a7a4744d0ad3ea37a591abd564175f0a9511654e20a
source_last_modified: "2025-11-02T04:40:28.807763+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/connect_swift_integration.md (Integrating NoritoBridgeKit) -->

## دمج NoritoBridgeKit في مشروع iOS على Xcode

توضح هذه الإرشادات كيفية دمج جسر Norito المكتوب بـ Rust (على شكل
XCFramework) وأغلفة Swift في تطبيق iOS، ثم تبادل إطارات Iroha Connect عبر
WebSocket باستخدام نفس كودكات Norito المستخدَمة في الـ host المكتوب بـ Rust.

المتطلبات المسبقة
- ملف zip باسم `NoritoBridge.xcframework` (تتم تهيئته عبر سير عمل CI) وملف
  Swift helper `NoritoBridgeKit.swift` (انسخ النسخة الموجودة تحت
  `examples/ios/NoritoDemo/Sources` إذا كنت لا تستخدم مشروع الـ demo مباشرة).
- Xcode 15 أو أحدث، وهدف iOS 13 أو أعلى.

الخيار A: Swift Package Manager (مُوصى به)
1) انشر حزمة SPM ثنائية باستخدام `Package.swift.template` في
   `crates/connect_norito_bridge/` (املأ الـ URL وقيمة checksum من الـ CI).
2) في Xcode: من القائمة File → Add Packages… → أدخِل رابط الريبو الخاص بـ
   SPM → أضف المنتج `NoritoBridge` إلى الـ target الخاص بالتطبيق.
3) أضف `NoritoBridgeKit.swift` إلى target التطبيق (اسحب الملف إلى المشروع،
   وتأكد من تفعيل “Copy if needed”).

الخيار B: CocoaPods
1) أنشئ Podspec انطلاقًا من `NoritoBridge.podspec.template` (املأ
   `s.source` بعنوان الـ zip).
2) نفّذ `pod trunk push NoritoBridge.podspec`.
3) في ملف Podfile: أضف `pod 'NoritoBridge'` ثم نفّذ `pod install`.
4) أضف `NoritoBridgeKit.swift` إلى target التطبيق.

Imports
```swift
import Foundation
import CryptoKit               // ChaChaPoly / HKDF
import IrohaSwift              // ConnectClient / ConnectSession / ConnectCrypto
import NoritoBridge            // وحدة Clang المولَّدة من XCFramework
// تأكد من أن NoritoBridgeKit.swift جزء من الـ target
```

### تمهيد جلسة Connect

يتولّى `ConnectClient` إدارة WebSocket، بينما يقوم `ConnectSession`
بتنسيق إطارات التحكم (control frames) وأظرف ciphertext. يوضّح المقطع
التالي كيف تقوم dApp بفتح جلسة، واشتقاق مفاتيح Connect، وانتظار رد
الموافقة (approve).

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
        // جاهز الآن لفك تشفير أظرف الـ ciphertext
        let firstEnvelope = try await connectSession.nextEnvelope()
        print("payload:", firstEnvelope.payload)
    }
}
```

### إرسال إطارات مشفّرة (طلبات توقيع، إلخ.)

عندما تحتاج dApp إلى طلب توقيع، تستخدم helpers الخاصة بجسر Norito لترميز
ظرف (envelope)، ثم تشفير الـ payload باستخدام ChaChaPoly، ولفّها داخل
`ConnectFrame`.

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

تُعتبر `ConnectAEAD.header` و`ConnectAEAD.nonce` وظائف مساعدة (انظر
المقطع في `docs/connect_swift_ios.md`) مبنية على تعريف الترويسة
المشتركة `connect:v1`. من السهل تضمينها inline إذا لم ترغب في إضافة
Utility إضافية:

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

### استقبال وفك تشفير الإطارات

يوفّر `ConnectSession` الدالة `nextEnvelope()`، التي تفك تشفير الـ payload
ما دام أن مفاتيح الاتجاه (direction keys) مهيّأة. إذا احتجت إلى الوصول
اليدوي (مثلاً لمواءمة pipeline decode موجود مسبقًا)، يمكنك استدعاء
helper منخفض المستوى كما يلي:

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

يوفّر `NoritoBridgeKit` أيضًا helpers مثل `decodeCiphertextFrame` و
`decodeEnvelopeJson` و`decodeSignResultAlgorithm` لأغراض debug أو اختبار
interoperability. في تطبيقات الإنتاج، يُفضَّل الاعتماد على
`ConnectSession` و`ConnectEnvelope`، بحيث يبقى السلوك متّسقًا مع Rust و
Android SDK.

## التحقق عبر CI

- قبل نشر حزم bridge محدَّثة أو دفع تغييرات Connect integration، نفّذ:

  ```bash
  make swift-ci
  ```

  يتحقق هذا الهدف من تطابق الـ fixtures، ويُراجع خلاصات dashboards، ويولِّد
  ملخصات CLI محليًا. في Buildkite يعتمد نفس الـ workflow على مفاتيح
  metadata مثل `ci/xcframework-smoke:<lane>:device_tag`؛ بعد تعديل
  pipelines أو وسوم الـ agents تأكد من وجود هذه الـ metadata حتى تتمكن
  لوحات المراقبة من إسناد النتائج إلى الـ simulator أو StrongBox lane
  الصحيح.
- إذا فشل الأمر، فاتبع دليل parity
  (`docs/source/swift_parity_triage.md`) وراجع المخرجات المولَّدة تحت
  `mobile_ci` لتحديد الـ lane الذي يحتاج إلى إعادة توليد أو متابعة incident
  قبل إعادة المحاولة.

</div>

