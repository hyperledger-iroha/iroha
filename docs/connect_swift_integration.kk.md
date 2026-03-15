---
lang: kk
direction: ltr
source: docs/connect_swift_integration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b937a75e50aa77c02fcab0a11dae1b1cc182f88c179d6f90aa69181afa80d1b
source_last_modified: "2026-01-05T18:22:23.394597+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## NoritoBridgeKit-ті Xcode iOS жобасына біріктіру

Бұл нұсқаулық Rust Norito көпірін (XCFramework) және Swift орауыштарын iOS қолданбасына біріктіруді, содан кейін Rust хосты сияқты Norito кодектерін пайдаланып WebSocket арқылы Iroha Connect жақтауларын алмасу жолын көрсетеді.

Алғы шарттар
- NoritoBridge.xcframework zip (CI жұмыс процесі арқылы құрастырылған) және Swift көмекшісі `NoritoBridgeKit.swift` (демонстрациялық жобаны тікелей пайдаланбасаңыз, `examples/ios/NoritoDemo/Sources` астындағы нұсқаны көшіріңіз).
- Xcode 15+, iOS 13+ мақсаты.

А нұсқасы: Swift пакет менеджері (ұсынылады)
1) `crates/connect_norito_bridge/` ішіндегі `Package.swift.template` көмегімен екілік SPM жариялаңыз (URL және CI-дан бақылау сомасын толтырыңыз).
2) Xcode ішінде: Файл → Пакеттерді қосу… → SPM репо URL мекенжайын енгізіңіз → `NoritoBridge` өнімін мақсатыңызға қосыңыз.
3) `NoritoBridgeKit.swift` қолданбасын мақсатқа қосыңыз (жобаңызға сүйреңіз, «Қажет болса көшіру» құсбелгі қойылғанына көз жеткізіңіз).

В нұсқасы: CocoaPods
1) `NoritoBridge.podspec.template` ішінен Podspec жасаңыз (`s.source` zip URL мекенжайын толтырыңыз).
2) `pod trunk push NoritoBridge.podspec`.
3) Подфайлыңызда: `pod 'NoritoBridge'` → `pod install`.
4) `NoritoBridgeKit.swift` қолданбасын мақсатқа қосыңыз.

Импорттар
```swift
import Foundation
import CryptoKit               // ChaChaPoly / HKDF
import IrohaSwift              // ConnectClient / ConnectSession / ConnectCrypto
import NoritoBridge            // Clang module from the XCFramework
// Ensure NoritoBridgeKit.swift is part of the target
```

### Қосылу сеансын жүктеу

`ConnectClient` WebSocket өңдейді, ал `ConnectSession` басқаруды реттейді
жақтаулар мен шифрлы мәтін конверттері. Төмендегі үзінді dApp сеансты қалай ашатынын көрсетеді,
Connect пернелерін шығарып, мақұлдау жауабын күтіңіз.

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

### Шифрлық мәтін кадрларын жіберу (қол қою сұраулары, т.б.)

dApp қолтаңбаны сұрау қажет болғанда кодтау үшін Norito көпір көмекшілерін пайдаланады.
конверт, пайдалы жүктемені ChaChaPoly арқылы шифрлайды және оны `ConnectFrame` ішіне орады.

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

`ConnectAEAD.header` / `ConnectAEAD.nonce` ыңғайлы көмекшілер (үзіндіні қараңыз).
`docs/connect_swift_ios.md`) ортақ `connect:v1` тақырып анықтамасынан құрастырылған. Олар
басқа қызметтік бағдарламаны қоспауды қаласаңыз, кірістіру оңай:

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

### Фреймдерді қабылдау/шифрын шешу

`ConnectSession` `nextEnvelope()`-ны ашады, ол бағытты анықтау кезінде пайдалы жүктемелердің шифрын ашады.
пернелері конфигурацияланған. Қолмен кіру қажет болса (мысалы, бар декодермен сәйкестендіру үшін
құбыр), сіз төменгі деңгейдегі көмекшіге қоңырау шала аласыз:

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

`NoritoBridgeKit` сонымен қатар `decodeCiphertextFrame`, `decodeEnvelopeJson` сияқты көмекшілерді көрсетеді.
және `decodeSignResultAlgorithm` отладтау немесе өзара әрекеттесу сынағы үшін. Өндіріс үшін
қолданбалар, мінез-құлық Rust және
Android SDK дәл.

## CI валидациясы

- Жаңартылған көпір артефактілерін жарияламас бұрын немесе Connect біріктірулерін баспас бұрын, іске қосыңыз:

  ```bash
  make swift-ci
  ```

  Мақсат бекіту тепе-теңдігін тексереді, бақылау тақтасының арналарын тексереді және CLI көрсетеді
  жергілікті қорытындылар. Buildkite бағдарламасында бірдей жұмыс процесі сияқты метадеректер кілттеріне байланысты
  `ci/xcframework-smoke:<lane>:device_tag`; өңдеуден кейін метадеректер бар екенін растаңыз
  бақылау тақталары нәтижелерді дұрыс симуляторға немесе
  StrongBox жолағы.
- Егер пәрмен орындалмаса, паритеттік ойын кітабын орындаңыз (`docs/source/swift_parity_triage.md`)
  және қай жолақ регенерацияны қажет ететінін анықтау үшін көрсетілген `mobile_ci` шығысын тексеріңіз
  немесе қайталаудан бұрын оқиғаны бақылау.