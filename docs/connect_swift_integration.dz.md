---
lang: dz
direction: ltr
source: docs/connect_swift_integration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b937a75e50aa77c02fcab0a11dae1b1cc182f88c179d6f90aa69181afa80d1b
source_last_modified: "2026-01-05T18:22:23.394597+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## ཨེགསི་ཀོཌ་ཨའི་ཨོ་ཨེསི་ལས་འགུལ་ནང་ ནོར་རི་ཊོ་བིརིཇ་ཀིཊ་གཅིག་བསྡོམས་འབད་ནི།

ལམ་སྟོན་འདི་གིས་ རསཊ་ I18NT0000000X ཟམ་ (XCFramework) དང་ སུའིཕཊ་གི་ བཀབ་ཆ་ཚུ་ iOS གློག་རིག་ནང་ ག་དེ་སྦེ་ མཉམ་བསྡོམས་འབད་ནི་ཨིན་ན་ སྟོན་ཞིནམ་ལས་ དེ་ལས་ I18NT00000000003X གི་ ཝེབ་སོ་ཀེཊི་གུ་ རསཊི་ཧོསིཊི་དང་གཅིག་ཁར་ Norito གསང་ཡིག་ཚུ་ ལག་ལེན་འཐབ་སྟེ་ གཞི་ཁྲམ་ཚུ་ བརྗེ་སོར་འབད་དགོ།

སྔོན་འགྲོའི་ཆ་རྐྱེན།
- NoritoBridge.xcframework zip (CI ལཱ་གི་རྒྱུན་རིམ་གྱིས་བཟོ་བསྐྲུན་འབད་ཡོདཔ་) དང་ Swift གྲོགས་རམ་པ་ I18NI000000010X (ཁྱོད་ཀྱིས་ བརྡ་སྟོན་ལས་འགུལ་འདི་ཐད་ཀར་དུ་ ཆ་འཇོག་མ་འབད་བ་ཅིན་ ཐོན་རིམ་འདི་ འདྲ་བཤུས་རྐྱབས།)
- Xcode 15+, iOS 13+ དམིགས་ཚད།

གདམ་ཁ་ཀ་: སོར་བསྒྱུར་ཐུམ་སྒྲིལ་འཛིན་སྐྱོང་པ། (གྲོས་འཆར་བཀོད།)
༡༽ `Package.swift.template` ལག་ལེན་འཐབ་སྟེ་ I18NI0000000013X (URL fill དང་ CI) ལག་ལེན་འཐབ་སྟེ་ གཉིས་ལྡན་ཨེསི་པི་ཨེམ་ཅིག་ དཔར་བསྐྲུན་འབད་ཡོདཔ་ཨིན།
༢༽ ཨེགསི་ཀོཌི་ནང་ ཡིག་སྣོད་ → ཐུམ་སྒྲིལ་ཚུ་ཁ་སྐོང་... → ཁྱོད་རའི་དམིགས་གཏད་ལུ་ `NoritoBridge` ཐོན་སྐྱེད་འདི་ ཨེསི་པི་ཨེམ་ repo URL → བཙུགས།
༣༽ ཁྱོད་རའི་གློག་རིམ་དམིགས་གཏད་ལུ་ I18NI0000015X ཁ་སྐོང་རྐྱབས་ (ཁྱོད་རའི་ལས་འགུལ་ནང་ལུ་འདྲུད་དེ་ “དགོས་མཁོ་ཡོད་མེད་འདྲ་བཤུས་” འདི་རྟགས་བཀལ་དགོ།)

གདམ་ཁ།: ཀོ་ཀོ་པོཌ།
༡༽ `NoritoBridge.podspec.template` (`s.source` zip URL བཀང་) ལས་ Podspec ཅིག་གསར་བསྐྲུན་འབད།
༢༽ `pod trunk push NoritoBridge.podspec`.
༣༽ ཁྱོད་ཀྱི་པོད་ཕའིལ་ནང་: `pod 'NoritoBridge'` → I18NI0000020X.
༤༽ ཁྱོད་རའི་གློག་རིམ་དམིགས་གཏད་ལུ་ `NoritoBridgeKit.swift` ཁ་སྐོང་འབད།

ནང་འདྲེན་ཚུ།
I18NF0000004X

### མཐུད་ལམ་ལཱ་ཡུན་ཅིག་བུཊི་སིཊིང་འབད་དོ།

`ConnectClient` གིས་ ཝེབ་སོ་ཀེཊི་འདི་ འཛིན་སྐྱོང་འཐབ་ཨིན་ དེ་ལས་ I18NI000000023X གིས་ ཚད་འཛིན་ཚད་འཛིན་འབདཝ་ཨིན།
གཞི་ཁྲམ་དང་ སི་ཕར་ཊེགསི་ ཡིག་ཤུབས་ཚུ། འོག་གི་ཐིག་ཁྲམ་འདི་གིས་ ཌི་ཨེ་པི་གིས་ ལཱ་ཡུན་ཅིག་ག་དེ་སྦེ་ཁ་ཕྱེ་འོང་ག་སྟོནམ་ཨིན།
འབྲེལ་མཐུན་ལྡེ་མིག་ཚུ་ ཆ་འཇོག་ལན་འདེབས་འབད་ནི་ལུ་སྒུག་སྡོད།

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

### སི་ཕར་ཊེགསི་གཞི་ཁྲམ་ཚུ་གཏང་དོ་ཡོདཔ་ཨིན་ (ཞུ་བ་ཚུ་ ལ་སོགས་པ་ཚུ་)།

ཌི་ཨེ་པི་གིས་ མཚན་རྟགས་ཅིག་ ཞུ་བ་འབད་དགོཔ་ད་ དེ་གིས་ Norito ཟམ་གྱི་གྲོགས་རམ་ཚུ་ ལག་ལེན་འཐབ་ཨིན།
ཡིག་ཤུབས་ཅིག་གིས་ ཆ་ཅ་པོ་ལི་དང་གཅིག་ཁར་ པེ་ལོཌ་འདི་གསང་བཟོ་འབད་དེ་ I18NI000000024X ནང་ལུ་ བཀབ་བཞགཔ་ཨིན།

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

`ConnectAEAD.header` / `ConnectAEAD.nonce` འདི་ སྟབས་བདེ་བའི་གྲོགས་རམ་པ་ཨིན།
`docs/connect_swift_ios.md` གིས་ `connect:v1` མགོ་ཡིག་ངེས་ཚིག་ལས་ བཟོ་བསྐྲུན་འབད་ཡོདཔ་ཨིན། ཁོང
ཁྱོད་ཀྱིས་ གཞན་མི་མཐུན་རྐྱེན་ཁ་སྐོང་མ་འབད་བ་ཅིན་ ནང་ན་འཇམ་ཏོང་ཏོ་ཨིན།

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

### ལེན་པ་ / གཞི་ཁྲམ་ཚུ་ གསང་བཟོ་འབད་དོ།

I18NI000000029X ཧེ་མ་ལས་རང་ I18NI000000030X གསལ་སྟོན་འབདཝ་ཨིན།
ལྡེ་མིག་ཚུ་རིམ་སྒྲིག་འབད་ཡོདཔ་ཨིན། ཁྱོད་ལུ་ལག་དེབ་འཛུལ་སྤྱོད་དགོ་པ་ཅིན་ (དཔེར་ན་ ད་ལྟོ་ཡོད་པའི་ཌི་ཀོ་ཌར་ཅིག་དང་མཐུན་སྒྲིག་འབད་ནི་ལུ་
པའིཔ་ལའིན་), ཁྱོད་ཀྱིས་ དམའ་རིམ་གྱི་གྲོགས་རམ་པ་ལུ་འབོ་ཚུགས།

I18NF0000008X

`NoritoBridgeKit` གིས་ I18NI000000032X, `decodeEnvelopeJson`, བཟུམ་གྱི་གྲོགས་རམ་པ་ཚུ་ཡང་སྟོནམ་ཨིན།
དང་ `decodeSignResultAlgorithm` རྐྱེན་སེལ་ཡང་ན་ ཕན་ཚུན་འབྲེལ་བའི་བརྟག་དཔྱད་ཀྱི་དོན་ལུ་ཨིན། ཐོན་སྐྱེད་ཀྱི་དོན་ལུ།
གློག་རིམ་ཚུ་ I18NI000000035X དང་ I18NI0000000036X ལུ་བརྟེན་ཏེ་ སྤྱོད་ལམ་འདི་གིས་ Rust དང་ མཐུན་སྒྲིག་འབདཝ་ཨིན།
Android SDKs ཏག་ཏག་ཡོད།

## CI བདེན་དཔང་།

- དུས་མཐུན་བཟོ་ཡོད་པའི་ཟམ་གྱི་ཅ་རྙིང་ཚུ་དཔར་བསྐྲུན་མ་འབད་བའི་ཧེ་མ་ ཡང་ན་ མཐུད་སྦྲེལ་མཉམ་བསྡོམས་ཚུ་ ཨེབ་གཏང་མ་འབད་བའི་ཧེ་མ་ གཡོག་བཀོལ།

  I18NF0000009X

  དམིགས་གཏད་འདི་གིས་ བརྟན་བཞུགས་འདྲ་མཉམ་ཚུ་ བདེན་དཔྱད་འབད་དེ་ ཌེཤ་བོརཌི་ཕིཌི་ཚུ་ཞིབ་དཔྱད་འབདཝ་ཨིནམ་དང་ སི་ཨེལ་ཨའི་འདི་ བཀྲམ་སྟོན་འབདཝ་ཨིན།
  ས་གནས་ནང་ བཅུད་བསྡུས། བཱའིལཌི་ཀི་ཊི་ནང་ ལཱ་གི་རྒྱུན་རིམ་ཅོག་འཐདཔ་འདི་ མེ་ཊ་ཌེ་ཊ་ལྡེ་མིག་ཚུ་ལུ་རག་ལསཔ་ཨིན།
  `ci/xcframework-smoke:<lane>:device_tag`; ཞུན་དག་འབད་བའི་ཤུལ་ལས་ མེ་ཊ་ཌེ་ཊ་འདི་ ངེས་གཏན་བཟོ།
  པའིཔ་ལའིན་ཡང་ན་ ལས་ཚབ་ངོ་རྟགས་ཚུ་ དེ་འབདཝ་ལས་ ཌེཤ་བོརཌི་གིས་ གྲུབ་འབྲས་ཚུ་ ངེས་བདེན་ དཔེ་སྟོན་འབད་མི་ལུ་ ཆེད་བརྗོད་འབད་ཚུགས།
  StrongBox ལམ།
- བརྡ་བཀོད་འདི་འཐུས་ཤོར་བྱུང་པ་ཅིན་ ཆ་སྙོམས་རྩེད་དེབ་ (I18NI0000038X) ལུ་རྗེས་སུ་འབྲང་།
  དང་ བཀོད་སྒྲིག་འབད་ཡོད་པའི་ `mobile_ci` ཐོན་འབྲས་འདི་ ལམ་ག་འདི་ བསྐྱར་བཟོ་དགོཔ་ཨིན་ན་ ངོས་འཛིན་འབད་ནི།
  ཡང་ན་ ལོག་འབད་རྩོལ་མ་བསྐྱེད་པའི་ཧེ་མ་ བྱུང་རྐྱེན་རྗེས་འཇུག་འབད་ནི།