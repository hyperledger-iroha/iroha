---
lang: uz
direction: ltr
source: docs/connect_swift_integration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b937a75e50aa77c02fcab0a11dae1b1cc182f88c179d6f90aa69181afa80d1b
source_last_modified: "2026-01-05T18:22:23.394597+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## NoritoBridgeKitni Xcode iOS loyihasiga integratsiya qilish

Ushbu qo‘llanma Rust Norito ko‘prigi (XCFramework) va Swift o‘ramlarini iOS ilovasiga qanday qilib integratsiya qilishni, so‘ngra Rust xosti bilan bir xil Norito kodeklaridan foydalangan holda WebSocket orqali Iroha Connect freymlarini almashishni ko‘rsatadi.

Old shartlar
- NoritoBridge.xcframework zip (CI ish oqimi tomonidan yaratilgan) va Swift yordamchisi `NoritoBridgeKit.swift` (agar siz demo loyihasini bevosita ishlatmasangiz, `examples/ios/NoritoDemo/Sources` ostidagi versiyadan nusxa oling).
- Xcode 15+, iOS 13+ maqsadi.

Variant A: Swift Package Manager (tavsiya etiladi)
1) `crates/connect_norito_bridge/` da `Package.swift.template` yordamida ikkilik SPMni nashr eting (CI dan URL va nazorat summasini toʻldiring).
2) Xcode-da: Fayl → Paketlarni qo'shish… → SPM repo URL manzilini kiriting → `NoritoBridge` mahsulotini maqsadingizga qo'shing.
3) Ilova maqsadingizga `NoritoBridgeKit.swift` qo'shing (loyihangizga torting, "Agar kerak bo'lsa nusxa ko'chirish" belgisini qo'yganligiga ishonch hosil qiling).

Variant B: CocoaPods
1) `NoritoBridge.podspec.template` dan Podspec yarating (`s.source` zip URL manzilini to'ldiring).
2) `pod trunk push NoritoBridge.podspec`.
3) Podfaylingizda: `pod 'NoritoBridge'` → `pod install`.
4) Ilova maqsadingizga `NoritoBridgeKit.swift` qo'shing.

Import
```swift
import Foundation
import CryptoKit               // ChaChaPoly / HKDF
import IrohaSwift              // ConnectClient / ConnectSession / ConnectCrypto
import NoritoBridge            // Clang module from the XCFramework
// Ensure NoritoBridgeKit.swift is part of the target
```

### Connect seansini yuklash

`ConnectClient` WebSocket-ni boshqaradi, `ConnectSession` boshqaruvni boshqaradi
ramkalar va shifrlangan matn konvertlari. Quyidagi parcha dApp seansni qanday ochishini ko'rsatadi,
Connect tugmachalarini oling va tasdiqlash javobini kuting.

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

### Shifrlangan matn ramkalarini yuborish (imzo so'rovlari va boshqalar)

Agar dApp imzo so'rashi kerak bo'lsa, u kodlash uchun Norito ko'prik yordamchilaridan foydalanadi.
konvert, foydali yukni ChaChaPoly bilan shifrlaydi va uni `ConnectFrame` ga o'radi.

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

`ConnectAEAD.header` / `ConnectAEAD.nonce` qulaylik uchun yordamchilardir (snippetga qarang).
`docs/connect_swift_ios.md`) umumiy `connect:v1` sarlavha ta'rifidan qurilgan. Ular
Agar siz boshqa yordamchi dasturni qo'shmaslikni xohlasangiz, qatorga kiritish oson:

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

### Kadrlarni qabul qilish / shifrini ochish

`ConnectSession` allaqachon `nextEnvelope()` ni ochib beradi, u yo'nalish bo'lganda foydali yuklarni shifrlaydi
kalitlari sozlangan. Agar sizga qo'lda kirish kerak bo'lsa (masalan, mavjud dekoderga mos kelish uchun
quvur liniyasi), siz quyi darajadagi yordamchiga qo'ng'iroq qilishingiz mumkin:

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

`NoritoBridgeKit` shuningdek, `decodeCiphertextFrame`, `decodeEnvelopeJson`,
va disk raskadrovka yoki birgalikda ishlash testi uchun `decodeSignResultAlgorithm`. Ishlab chiqarish uchun
ilovalar uchun `ConnectSession` va `ConnectEnvelope` ga tayaning, shuning uchun xatti-harakatlar Rust va
Android SDK-lar aniq.

## CI tekshiruvi

- Yangilangan ko'prik artefaktlarini nashr etishdan yoki Connect integratsiyasini ishga tushirishdan oldin, quyidagini bajaring:

  ```bash
  make swift-ci
  ```

  Maqsad moslama paritetini tasdiqlaydi, asboblar paneli tasmasi tekshiradi va CLI-ni ko'rsatadi
  mahalliy xulosalar. Buildkite-da bir xil ish jarayoni metadata kalitlariga bog'liq, masalan
  `ci/xcframework-smoke:<lane>:device_tag`; tahrirlangandan keyin metadata mavjudligini tasdiqlang
  quvurlar yoki agent teglari, shuning uchun asboblar paneli natijalarni to'g'ri simulyator yoki
  StrongBox chizig'i.
- Agar buyruq bajarilmasa, paritet o'yin kitobiga amal qiling (`docs/source/swift_parity_triage.md`)
  va qaysi qator regeneratsiyani talab qilishini aniqlash uchun ko'rsatilgan `mobile_ci` chiqishini tekshiring
  yoki qayta urinishdan oldin hodisani kuzatish.