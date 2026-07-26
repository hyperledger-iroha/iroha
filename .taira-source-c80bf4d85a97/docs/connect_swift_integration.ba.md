---
lang: ba
direction: ltr
source: docs/connect_swift_integration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b937a75e50aa77c02fcab0a11dae1b1cc182f88c179d6f90aa69181afa80d1b
source_last_modified: "2026-01-05T18:22:23.394597+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Интеграциялау NoritoBridgeKit Xcode iOS проектында

Был ҡулланма күрһәтә, нисек интеграциялау өсөн Rust I18NT0000000000000000000 күпер (XCFramework) һәм Swift roppers iOS ҡушымта, һуңынан алмашыу I18NT00000000000X Connect рамкалар аша WebSocket ҡулланып шул уҡ I18NT0000000000001X codecs кеүек Rust хост.

Тәүшарттар
- NoritoBridge.xcframework zip (CI эш ағымы менән төҙөлгән) һәм Swift ярҙамсыһы I18NI000000010X (I18NI000000011X буйынса версия күсермәһе, әгәр һеҙ туранан-тура демо-проектты ҡулланмайһығыҙ икән).
- Xcode 15+, iOS 13+ маҡсат.

Вариант: Свифт пакет менеджеры (тәҡдим ителгән)
1) `Package.swift.template` ҡулланып бинар СПМ баҫтырып сығарыу `crates/connect_norito_bridge/` (URL-адрес һәм CI-нан чемпионат) ҡулланып.
2) Xcode: Файл → Өҫтәү пакеттар... → SPM repo URL → өҫтәү `NoritoBridge` продукт һеҙҙең маҡсатҡа.
3) өҫтәү `NoritoBridgeKit.swift` һеҙҙең ҡушымта маҡсатлы (һеҙҙең проектҡа һөйрәп, тәьмин итеү “Күсермә булһа, кәрәк булһа” галочкой).

В варианты: КакаоПодтар
1) `NoritoBridge.podspec.template`-тан Podspec булдырыу (`s.source` zip URL-адресын тултырырға).
2) `pod trunk push NoritoBridge.podspec`.
3) Һеҙҙең Подфайлда: `pod 'NoritoBridge'` → `pod install`.
4) өҫтәү I18NI000000021X һеҙҙең ҡушымта маҡсатына.

Импорт
```swift
import Foundation
import CryptoKit               // ChaChaPoly / HKDF
import IrohaSwift              // ConnectClient / ConnectSession / ConnectCrypto
import NoritoBridge            // Clang module from the XCFramework
// Ensure NoritoBridgeKit.swift is part of the target
```

### Bootstrapping тоташтырыу сессияһы

I18NI000000022X WebSocket идара итә, шул уҡ ваҡытта `ConnectSession` идара итә.
кадрҙар һәм шифр текстағы конверттары. Түбәндәге өҙөк күрһәтә, нисек dApp сессия асыр ине,
алыу асҡыстарын тоташтырыу, һәм раҫлау яуап көтөп.

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

### шифрлы кадрҙарҙы ебәреү (билдә үтенестәр һ.б.)

Ҡасан dApp кәрәк, ҡултамға һорап, ул ҡулланыу I18NT00000000002X күпер ярҙамсылары кодлау өсөн .
конверт, ChaChaPoly менән файҙалы йөктө шифрлай һәм уны `ConnectFrame`-ға уратып ала.

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

I18NI000000025X / `ConnectAEAD.nonce` 2012 йылдағы өҙөктәрҙе ҡарағыҙ.
`docs/connect_swift_ios.md`) дөйөм I18NI0000000028X башлыҡ билдәләмәһенән төҙөлгән. Улар
еңел рәткә, әгәр һеҙ өҫтөнлөк бирәһегеҙ, башҡа утилита өҫтәргә өҫтөнлөк бирә:

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

### Ҡабул итеү / расписание

I18NI000000029X инде I18NI0000000300X асыҡлана, был йүнәлеш ҡасан файҙалы йөктәрҙе расшифровка
асҡыстар конфигурациялана. Әгәр һеҙгә ҡул менән инеү кәрәк (мәҫәлән, булған декодер тура килтерергә
торба), түбән кимәлдәге ярҙамсы тип атау мөмкин:

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

I18NI000000031X шулай уҡ I18NI000000032X, I18NI000000033X кеүек ярҙамсыларҙы фашлай,
һәм `decodeSignResultAlgorithm` өсөн отладка йәки үҙ-ара эш итеү һынау. Производство өсөн
ҡушымталар, I18NI000000035X һәм I18NI00000000036X-ға таяна, шуға күрә тәртип Растҡа тап килә һәм
Андроид СДК-лар теүәл.

## CI раҫлау

- Яңыртылған күпер артефакттарын баҫтырыр алдынан йәки Connect интеграцияларын этәрергә, йүгерергә:

  ```bash
  make swift-ci
  ``` X

  Маҡсат раҫлау ҡоролмаһы паритеты, тикшерергә приборҙар таҡтаһы каналдары, һәм CLI-ны күрһәтә.
  йомғаҡ яһай урындағы. Buildkite-ла шул уҡ эш ағымы метамағлүмәттәр асҡыстарына бәйле, мәҫәлән,
  I18NI000000037X; раҫлау метамағлүмәттәр бар, һуңынан мөхәррирләү
  торбалар йәки агент тегтар шулай приборҙар таҡталары һөҙөмтәләрҙе дөрөҫ тренажер йәки
  Көслөбокс һыҙаты.
- Әгәр команда уңышһыҙлыҡҡа осраһа, паритет пьесалары китабын үтәгеҙ (`docs/source/swift_parity_triage.md`)
  һәм тикшерергә күрһәтелгән I18NI00000000039X сығыш асыҡлау өсөн, ниндәй һыҙат регенерация талап итә
  йәки инцидент күҙәтеү алдынан яңынан тырышып.