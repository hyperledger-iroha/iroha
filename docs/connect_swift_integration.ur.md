---
lang: ur
direction: rtl
source: docs/connect_swift_integration.md
status: complete
translator: manual
source_hash: ebdea5644112eab6e2027a7a4744d0ad3ea37a591abd564175f0a9511654e20a
source_last_modified: "2025-11-02T04:40:28.807763+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- docs/connect_swift_integration.md (Integrating NoritoBridgeKit) کا اردو ترجمہ -->

## NoritoBridgeKit کو Xcode iOS پروجیکٹ میں integrate کرنا

یہ گائیڈ بتاتی ہے کہ Rust Norito bridge (XCFramework) اور Swift wrappers کو
iOS ایپ میں کیسے integrate کیا جائے، اور پھر Rust host کے ساتھ یکساں Norito
codecs استعمال کرتے ہوئے WebSocket کے ذریعے Iroha Connect frames کا تبادلہ
کیسے کیا جائے۔

Prerequisites
- `NoritoBridge.xcframework` نامی zip (جو CI workflow سے build ہوتا ہے) اور
  Swift helper `NoritoBridgeKit.swift` (اگر آپ demo پروجیکٹ کو براہِ راست
  استعمال نہیں کر رہے تو `examples/ios/NoritoDemo/Sources` کے نیچے والی
  ورژن کو کاپی کریں)۔
- Xcode 15+ اور iOS 13+ target۔

آپشن A: Swift Package Manager (مُستحسن)
1) `crates/connect_norito_bridge/` میں موجود `Package.swift.template` کو
   استعمال کرتے ہوئے ایک binary SPM پیکج publish کریں (URL اور checksum کو
   CI کی آؤٹ پٹ کے ساتھ پُر کریں)۔
2) Xcode میں: File → Add Packages… → SPM repo کا URL درج کریں → `NoritoBridge`
   پروڈکٹ کو اپنے target میں add کریں۔
3) `NoritoBridgeKit.swift` کو اپنی app target میں add کریں (فائل کو
   project میں drag کریں اور “Copy if needed” کو منتخب رکھیں)۔

آپشن B: CocoaPods
1) `NoritoBridge.podspec.template` سے Podspec بنائیں (فیلڈ `s.source` میں
   zip URL سیٹ کریں)۔
2) `pod trunk push NoritoBridge.podspec` چلائیں۔
3) Podfile میں `pod 'NoritoBridge'` شامل کریں → `pod install` چلائیں۔
4) `NoritoBridgeKit.swift` کو app target میں شامل کریں۔

Imports
```swift
import Foundation
import CryptoKit               // ChaChaPoly / HKDF
import IrohaSwift              // ConnectClient / ConnectSession / ConnectCrypto
import NoritoBridge            // XCFramework سے Clang module
// پکا کریں کہ NoritoBridgeKit.swift target کا حصہ ہو
```

### Connect سیشن کا bootstrap کرنا

`ConnectClient` WebSocket کو handle کرتا ہے، جبکہ `ConnectSession` control
frames اور ciphertext envelopes کو orchestrate کرتا ہے۔ نیچے دیا گیا
snippet یہ دکھاتا ہے کہ dApp ایک session کیسے کھولتی ہے، Connect keys
derive کرتی ہے، اور approval response کا انتظار کرتی ہے۔

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
        // اب ciphertext envelopes کو decrypt کرنے کے لیے تیار
        let firstEnvelope = try await connectSession.nextEnvelope()
        print("payload:", firstEnvelope.payload)
    }
}
```

### Ciphertext فریمز بھیجنا (sign requests وغیرہ)

جب dApp کو signature کی درخواست بھیجنی ہو تو وہ Norito bridge helpers کو
استعمال کر کے envelope encode کرتی ہے، payload کو ChaChaPoly کے ساتھ encrypt
کرتی ہے اور اسے `ConnectFrame` میں wrap کرتی ہے۔

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

`ConnectAEAD.header` / `ConnectAEAD.nonce` convenience helpers ہیں (مزید
دیكھیں `docs/connect_swift_ios.md` میں) جو shared header definition
`connect:v1` سے بنائے گئے ہیں۔ اگر آپ کوئی extra utility add نہیں کرنا چاہتے
تو انہیں آسانی سے inline بھی کیا جا سکتا ہے:

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

### فریمز وصول اور decrypt کرنا

`ConnectSession` پہلے ہی `nextEnvelope()` فراہم کرتا ہے، جو direction keys
configure ہونے کی صورت میں payloads کو decrypt کرتا ہے۔ اگر آپ کو manual
ایکسس چاہیے (مثلاً کسی موجودہ decoder pipeline سے میچ کرنے کے لیے)، تو آپ
نیچے دیا گیا lower‑level helper استعمال کر سکتے ہیں:

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

`NoritoBridgeKit` debugging یا interoperability testing کے لیے helpers
فراہم کرتا ہے جیسے `decodeCiphertextFrame`, `decodeEnvelopeJson`,
`decodeSignResultAlgorithm` وغیرہ۔ production apps میں ترجیح یہ ہے کہ
رفتار و رویّہ Rust اور Android SDKs کے مطابق رہے، اس لیے
`ConnectSession` اور `ConnectEnvelope` پر انحصار کیا جائے۔

## CI validation

- کسی بھی updated bridge artifacts کو publish کرنے یا Connect integration
  میں تبدیلیاں push کرنے سے پہلے یہ کمانڈ چلائیں:

  ```bash
  make swift-ci
  ```

  یہ ٹارگٹ fixture parity، dashboard feeds، اور CLI summaries کو مقامی طور
  پر validate/ render کرتا ہے۔ Buildkite میں یہی workflow عام طور پر
  metadata keys جیسے `ci/xcframework-smoke:<lane>:device_tag` پر منحصر
  ہوتا ہے؛ pipelines یا agent tags میں تبدیلی کے بعد یقینی بنائیں کہ
  متعلقہ metadata موجود ہو، تاکہ dashboards صحیح simulator یا StrongBox
  lane کے ساتھ نتائج کو associate کر سکیں۔
- اگر کمانڈ fail ہو جائے تو parity playbook
  (`docs/source/swift_parity_triage.md`) فالو کریں اور `mobile_ci` output
  کو inspect کریں تاکہ وہ lane سامنے آ سکے جسے regeneration یا
  incident follow‑up کے بعد دوبارہ چلانے کی ضرورت ہو۔

</div>

