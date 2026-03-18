<!-- Hebrew translation of docs/connect_swift_integration.md -->

---
lang: he
direction: rtl
source: docs/connect_swift_integration.md
status: complete
translator: manual
---

<div dir="rtl">

## שילוב NoritoBridgeKit בפרויקט iOS ב-Xcode

מדריך זה מסביר כיצד לשלב את גשר ה-Rust (NoritoBridge.xcframework) ואת שכבות ה-Swift באפליקציית iOS, ואז להשתמש ב-`ConnectClient`/`ConnectSession` כדי לקבל ולשלוח מסגרות Connect עם אותם קודקים כמו בצד ה-Rust.

דרישות מקדימות
- קובץ ‎NoritoBridge.xcframework (מתוצר ה-CI) וקובץ העזר ‎`NoritoBridgeKit.swift` (ניתן להעתיק מ-`examples/ios/NoritoDemo/Sources/NoritoBridgeKit.swift`).
- ‏Xcode 15 ומעלה, פרויקט המכוון ל-iOS 13 ומעלה.

### אפשרות A: Swift Package Manager (מומלצת)
1. פרסמו חבילת בינארי בעזרת ‎`crates/connect_norito_bridge/Package.swift.template` (מלאו URL + checksum מה-CI).
2. ב-Xcode → ‎`File → Add Packages…` → הזינו את כתובת המאגר → הוסיפו את מוצר ‎`NoritoBridge` ליעד.
3. הוסיפו את ‎`NoritoBridgeKit.swift` ליעד (גרירה לפרויקט + “Copy if needed”).

### אפשרות B: CocoaPods
1. צרו Podspec מתוך ‎`NoritoBridge.podspec.template` (עדכנו את ‎`s.source` עם כתובת הזיפ).
2. `pod trunk push NoritoBridge.podspec`.
3. הוסיפו `pod 'NoritoBridge'` ל-Podfile והריצו `pod install`.
4. צירפו את ‎`NoritoBridgeKit.swift` ליעד האפליקציה.

### ייבוא מודולים
```swift
import Foundation
import CryptoKit               // ChaChaPoly / HKDF
import IrohaSwift              // ConnectClient / ConnectSession / ConnectCrypto
import NoritoBridge            // מודול ה-Clang מתוך ה-XCFramework
// ודאו ש-NoritoBridgeKit.swift נמצא תחת אותו Target
```

### אתחול סשן Connect

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
        let firstEnvelope = try await connectSession.nextEnvelope()
        print("payload:", firstEnvelope.payload)
    }
}
```

### שליחת מעטפות מוצפנות

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

`ConnectAEAD` מייצר את ה-AAD וה-nonce המשותפים:

```swift
enum ConnectAEAD {
    static func header(sessionID: Data, direction: ConnectDirection, sequence: UInt64) -> Data {
        var buffer = Data()
        buffer.append("connect:v1".data(using: .utf8)!)
        buffer.append(sessionID)
        buffer.append(direction == .appToWallet ? 0 : 1)
        var seq = sequence.littleEndian
        withUnsafeBytes(of: &seq) { buffer.append(contentsOf: $0) }
        buffer.append(1)
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

### קבלה ופענוח

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

לצורכי דיבוג ניתן להשתמש גם ב-`NoritoBridgeKit.decodeCiphertextFrame`, ב-`decodeEnvelopeJson` וב-`decodeSignResultAlgorithm`. לשימוש פרודקשן העדיפו את `ConnectSession`/`ConnectEnvelope` כדי לשמור על פריות עם ה-SDK של Rust ו-Android.

## ולידציה ב-CI

- לפני שמפרסמים ארטיפקט חדש של הגשר או דוחפים שינויי Connect, הריצו:

  ```bash
  make swift-ci
  ```

  הפקודה מאמתת את פריות הפיקסצ'רים, בודקת את פידי הדשבורד ומרנדרת את התקציר מקומית. ב-Buildkite אותו תהליך נשען על מטא-דאטה בצורה `ci/xcframework-smoke:<lane>:device_tag`; לאחר שינוי פייפליין או תגי סוכנים ודאו שהמטא-דאטה מופיע כדי שניתן יהיה לשייך תוצאות לליין הנכון (סימולטור או StrongBox).
- במקרה של כשל, פעלו לפי `docs/source/swift_parity_triage.md` ועיינו בפלט `mobile_ci` כדי לזהות איזו ריצה דורשת ריג'נרציה או טיפול באינסידנט לפני נסיון נוסף.

</div>
