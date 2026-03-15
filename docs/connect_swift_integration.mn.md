---
lang: mn
direction: ltr
source: docs/connect_swift_integration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b937a75e50aa77c02fcab0a11dae1b1cc182f88c179d6f90aa69181afa80d1b
source_last_modified: "2026-01-05T18:22:23.394597+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## NoritoBridgeKit-ийг Xcode iOS төсөлд нэгтгэж байна

Энэхүү гарын авлагад Rust Norito гүүр (XCFramework) болон Swift боодолуудыг iOS програмд хэрхэн нэгтгэж, дараа нь Rust хосттой ижил Norito кодлогч ашиглан WebSocket дээр Iroha Connect хүрээг солилцохыг харуулсан.

Урьдчилсан нөхцөл
- NoritoBridge.xcframework зип (CI ажлын урсгалаар бүтээгдсэн) болон Swift туслагч `NoritoBridgeKit.swift` (хэрэв та демо төслийг шууд ашиглахгүй байгаа бол `examples/ios/NoritoDemo/Sources` хувилбарыг хуулна уу).
- Xcode 15+, iOS 13+ зорилтот.

Сонголт А: Swift багц менежер (санал болгож байна)
1) `crates/connect_norito_bridge/` дотор `Package.swift.template` ашиглан хоёртын SPM нийтлэх (CI-ээс URL болон шалгах нийлбэрийг бөглөнө үү).
2) Xcode дээр: Файл → Багц нэмэх... → SPM репо URL-г оруулна уу → `NoritoBridge` бүтээгдэхүүнийг зорилтот хэсэгт нэмнэ үү.
3) `NoritoBridgeKit.swift`-г өөрийн програмын зорилтот хэсэгт нэмнэ үү (төсөлдөө чирж, "Шаардлагатай бол хуулах" гэснийг шалгана уу).

Сонголт B: CocoaPods
1) `NoritoBridge.podspec.template`-ээс Podspec үүсгэнэ үү (`s.source` зип URL-г бөглөнө үү).
2) `pod trunk push NoritoBridge.podspec`.
3) Таны Podfile дотор: `pod 'NoritoBridge'` → `pod install`.
4) Өөрийн програмын зорилтот хэсэгт `NoritoBridgeKit.swift` нэмнэ үү.

Импорт
```swift
import Foundation
import CryptoKit               // ChaChaPoly / HKDF
import IrohaSwift              // ConnectClient / ConnectSession / ConnectCrypto
import NoritoBridge            // Clang module from the XCFramework
// Ensure NoritoBridgeKit.swift is part of the target
```

### Холболтын сессийг ачаалж байна

`ConnectClient` нь WebSocket-ийг зохицуулдаг бол `ConnectSession` нь хяналтыг удирддаг.
хүрээ болон шифр текстийн дугтуйнууд. Доорх хэсэг нь dApp сессийг хэрхэн нээхийг харуулж байна.
Холбох түлхүүрүүдийг гаргаж, зөвшөөрлийн хариуг хүлээнэ үү.

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

### Шифрлэгдсэн текстийн хүрээ илгээх (тэмдэглэх хүсэлт гэх мэт)

dApp гарын үсэг хүсэх шаардлагатай үед кодлохын тулд Norito гүүрний туслахуудыг ашигладаг.
дугтуй хийж, ChaChaPoly ашиглан ачааллыг шифрлэж, `ConnectFrame`-д орооно.

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

`ConnectAEAD.header` / `ConnectAEAD.nonce` нь тав тухтай байдлын туслахууд юм (нэг хэсэг дэх хэсгийг үзнэ үү.
`docs/connect_swift_ios.md`) хуваалцсан `connect:v1` толгойн тодорхойлолтоос бүтээгдсэн. Тэд
Хэрэв та өөр хэрэгсэл нэмэхгүй байхыг хүсвэл шугаманд оруулахад хялбар байдаг:

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

### Хүрээ хүлээн авах / тайлах

`ConnectSession` нь `nextEnvelope()`-г аль хэдийн ил гаргасан бөгөөд энэ нь чиглүүлэх үед ачааллыг тайлдаг.
товчлуурууд тохируулагдсан байна. Хэрэв танд гарын авлага хэрэгтэй бол (жишээ нь одоо байгаа декодчилогчтой тааруулах
дамжуулах хоолой), та доод түвшний туслахыг дуудаж болно:

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

`NoritoBridgeKit` нь мөн `decodeCiphertextFrame`, `decodeEnvelopeJson`,
болон `decodeSignResultAlgorithm` дибаг хийх эсвэл харилцан ажиллах чадварыг шалгах. Үйлдвэрлэлийн хувьд
програмууд нь `ConnectSession` болон `ConnectEnvelope`-д тулгуурладаг тул зан төлөв Rust болон
Яг Android SDK.

## CI баталгаажуулалт

- Шинэчлэгдсэн гүүрний олдворуудыг нийтлэх эсвэл Connect интеграцийг түлхэхээс өмнө дараахийг ажиллуулна уу:

  ```bash
  make swift-ci
  ```

  Зорилтот нь бэхэлгээний тэнцвэрийг баталгаажуулж, хяналтын самбарын тэжээлийг шалгаж, CLI-г гаргадаг
  орон нутгийн хураангуй. Buildkite-д ижил ажлын урсгал нь мета өгөгдлийн түлхүүрүүдээс хамаарна
  `ci/xcframework-smoke:<lane>:device_tag`; засварласны дараа мета өгөгдөл байгаа эсэхийг баталгаажуулна уу
  дамжуулах хоолой эсвэл агент шошготой тул хяналтын самбар нь үр дүнг зөв симулятор эсвэл
  StrongBox эгнээ.
- Хэрэв тушаал амжилтгүй болбол паритын тоглуулах номыг дагана уу (`docs/source/swift_parity_triage.md`)
  аль эгнээг шинэчлэх шаардлагатайг тодорхойлохын тулд үзүүлсэн `mobile_ci` гаралтыг шалгана уу.
  эсвэл дахин оролдохоос өмнө ослын хяналт.