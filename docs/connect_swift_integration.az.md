---
lang: az
direction: ltr
source: docs/connect_swift_integration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b937a75e50aa77c02fcab0a11dae1b1cc182f88c179d6f90aa69181afa80d1b
source_last_modified: "2026-01-05T18:22:23.394597+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## NoritoBridgeKit-in Xcode iOS Layihəsinə inteqrasiyası

Bu bələdçi Rust Norito körpüsünü (XCFramework) və Swift sarğılarını iOS proqramına necə inteqrasiya edəcəyini, sonra Rust hostu ilə eyni Norito kodeklərindən istifadə edərək WebSocket üzərindən Iroha Connect çərçivələrinin mübadiləsini göstərir.

İlkin şərtlər
- NoritoBridge.xcframework zip (CI iş axını tərəfindən qurulmuşdur) və Swift köməkçisi `NoritoBridgeKit.swift` (birbaşa demo layihəsini istifadə etmirsinizsə, `examples/ios/NoritoDemo/Sources` altındakı versiyanı kopyalayın).
- Xcode 15+, iOS 13+ hədəfi.

Seçim A: Swift Paket Meneceri (tövsiyə olunur)
1) `crates/connect_norito_bridge/`-də `Package.swift.template` istifadə edərək ikili SPM dərc edin (URL və yoxlama məbləğini CI-dən doldurun).
2) Xcode-da: Fayl → Paketlər əlavə et… → SPM repo URL-ni daxil edin → `NoritoBridge` məhsulunu hədəfinizə əlavə edin.
3) Tətbiq hədəfinizə `NoritoBridgeKit.swift` əlavə edin (layihənizə sürükləyin, “Lazım olduqda kopyalayın” işarəsinin seçildiyinə əmin olun).

Seçim B: CocoaPods
1) `NoritoBridge.podspec.template`-dən Podspec yaradın (`s.source` zip URL-ni doldurun).
2) `pod trunk push NoritoBridge.podspec`.
3) Podfaylınızda: `pod 'NoritoBridge'` → `pod install`.
4) Tətbiq hədəfinizə `NoritoBridgeKit.swift` əlavə edin.

İdxal
```swift
import Foundation
import CryptoKit               // ChaChaPoly / HKDF
import IrohaSwift              // ConnectClient / ConnectSession / ConnectCrypto
import NoritoBridge            // Clang module from the XCFramework
// Ensure NoritoBridgeKit.swift is part of the target
```

### Qoşulma sessiyasının yüklənməsi

`ConnectClient` WebSocket-i idarə edir, `ConnectSession` isə idarəetməni təşkil edir
çərçivələr və şifrəli mətn zərfləri. Aşağıdakı fraqment dApp-ın sessiyanı necə açacağını göstərir,
Connect düymələrini əldə edin və təsdiq cavabını gözləyin.

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

### Şifrə mətn çərçivələrinin göndərilməsi (imza sorğuları və s.)

dApp imza tələb etməli olduqda, kodlaşdırmaq üçün Norito körpü köməkçilərindən istifadə edir.
zərf, faydalı yükü ChaChaPoly ilə şifrələyir və onu `ConnectFrame`-ə bükür.

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

`ConnectAEAD.header` / `ConnectAEAD.nonce` rahatlıq köməkçiləridir (snippetə baxın
`docs/connect_swift_ios.md`) paylaşılan `connect:v1` başlıq tərifindən qurulmuşdur. Onlar
başqa bir yardım proqramı əlavə etməməyi üstün tutursunuzsa, daxil etmək asandır:

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

### Çərçivələrin qəbulu/şifrinin açılması

`ConnectSession` artıq `nextEnvelope()`-ni ifşa edir, hansı ki, istiqamət verildikdə faydalı yüklərin şifrəsini açır.
düymələri konfiqurasiya edilmişdir. Əllə girişə ehtiyacınız varsa (məsələn, mövcud dekoderlə uyğunlaşdırmaq üçün
boru kəməri), aşağı səviyyəli köməkçiyə zəng edə bilərsiniz:

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

`NoritoBridgeKit`, həmçinin `decodeCiphertextFrame`, `decodeEnvelopeJson`,
və sazlama və ya qarşılıqlı fəaliyyət testi üçün `decodeSignResultAlgorithm`. İstehsal üçün
tətbiqlər üçün `ConnectSession` və `ConnectEnvelope`-ə etibar edin ki, davranış Rust və
Android SDK tam olaraq.

## CI doğrulaması

- Yenilənmiş körpü artefaktlarını dərc etməzdən və ya Connect inteqrasiyalarına təkan verməzdən əvvəl:

  ```bash
  make swift-ci
  ```

  Hədəf fikstür paritetini təsdiqləyir, tablosuna verilən xəbərləri yoxlayır və CLI-ni göstərir
  yerli xülasələr. Buildkite-də eyni iş axını kimi metadata açarlarından asılıdır
  `ci/xcframework-smoke:<lane>:device_tag`; redaktədən sonra metadatanın mövcud olduğunu təsdiqləyin
  boru kəmərləri və ya agent teqləri beləliklə idarə panelləri nəticələri düzgün simulyatora və ya
  StrongBox zolağı.
- Əgər əmr uğursuz olarsa, paritet kitabçasına əməl edin (`docs/source/swift_parity_triage.md`)
  və regenerasiya tələb edən zolağı müəyyən etmək üçün göstərilən `mobile_ci` çıxışını yoxlayın
  və ya təkrar cəhd etməzdən əvvəl hadisənin təqibi.