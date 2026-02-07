---
lang: hy
direction: ltr
source: docs/connect_swift_integration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b937a75e50aa77c02fcab0a11dae1b1cc182f88c179d6f90aa69181afa80d1b
source_last_modified: "2026-01-05T18:22:23.394597+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## NoritoBridgeKit-ի ինտեգրում Xcode iOS նախագծում

Այս ուղեցույցը ցույց է տալիս, թե ինչպես կարելի է ինտեգրել Rust Norito կամուրջը (XCFramework) և Swift փաթաթվածները iOS հավելվածի մեջ, այնուհետև փոխանակել Iroha Connect շրջանակները WebSocket-ի միջոցով՝ օգտագործելով նույն Norito կոդեկները, ինչպես Rust հոսթը:

Նախադրյալներ
- NoritoBridge.xcframework zip (կառուցված CI աշխատանքային հոսքի կողմից) և Swift օգնական `NoritoBridgeKit.swift` (պատճենեք տարբերակը `examples/ios/NoritoDemo/Sources` տակ, եթե դուք ուղղակիորեն չեք օգտագործում ցուցադրական նախագիծը):
- Xcode 15+, iOS 13+ թիրախ:

Տարբերակ A. Swift փաթեթի կառավարիչ (խորհուրդ է տրվում)
1) Հրապարակեք երկուական SPM՝ օգտագործելով `Package.swift.template` `crates/connect_norito_bridge/`-ում (լրացրեք URL-ը և ստուգման գումարը CI-ից):
2) Xcode-ում՝ Ֆայլ → Ավելացնել փաթեթներ… → Մուտքագրեք SPM ռեպո URL-ը → Ավելացրեք `NoritoBridge` արտադրանքը ձեր թիրախին:
3) Ավելացրեք `NoritoBridgeKit.swift` ձեր հավելվածի թիրախին (քաշեք ձեր նախագիծը, համոզվեք, որ «Պատճենել, եթե անհրաժեշտ է» նշված է):

Տարբերակ B. CocoaPods
1) Ստեղծեք Podspec `NoritoBridge.podspec.template`-ից (լրացրեք `s.source` zip URL-ը):
2) `pod trunk push NoritoBridge.podspec`.
3) Ձեր Podfile-ում՝ `pod 'NoritoBridge'` → `pod install`:
4) Ավելացրեք `NoritoBridgeKit.swift` ձեր հավելվածի թիրախին:

Ներմուծում
```swift
import Foundation
import CryptoKit               // ChaChaPoly / HKDF
import IrohaSwift              // ConnectClient / ConnectSession / ConnectCrypto
import NoritoBridge            // Clang module from the XCFramework
// Ensure NoritoBridgeKit.swift is part of the target
```

### Bootstrapping a Connect նիստը

`ConnectClient`-ը ղեկավարում է WebSocket-ը, մինչդեռ `ConnectSession`-ը կազմակերպում է վերահսկողությունը
շրջանակներ և գաղտնագրված տեքստային ծրարներ: Ստորև բերված հատվածը ցույց է տալիս, թե ինչպես է dApp-ը բացում նիստը,
ստացեք Connect ստեղները և սպասեք հաստատման պատասխանին:

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

### Գաղտնագրված տեքստի շրջանակների ուղարկում (նշանների հարցումներ և այլն)

Երբ dApp-ը ստորագրություն պահանջի, այն կոդավորման համար օգտագործում է Norito կամուրջի օգնականները
ծրար, ծածկագրում է օգտակար բեռը ChaChaPoly-ով և այն փաթաթում `ConnectFrame`-ով:

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

`ConnectAEAD.header` / `ConnectAEAD.nonce` հարմար օգնականներ են (տե՛ս հատվածը.
`docs/connect_swift_ios.md`) կառուցված է ընդհանուր `connect:v1` վերնագրի սահմանումից: Նրանք
հեշտ է ներդնել, եթե նախընտրում եք չավելացնել այլ օգտակար.

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

### Շրջանակների ստացում / վերծանում

`ConnectSession`-ն արդեն բացահայտում է `nextEnvelope()`-ը, որը վերծանում է օգտակար բեռները, երբ ուղղորդվում է
ստեղները կազմաձևված են: Եթե Ձեզ անհրաժեշտ է ձեռքով մուտք գործել (օրինակ՝ գոյություն ունեցող ապակոդավորիչին համապատասխանելու համար
խողովակաշար), կարող եք զանգահարել ստորին մակարդակի օգնականին.

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

`NoritoBridgeKit`-ը նաև բացահայտում է այնպիսի օգնականներ, ինչպիսիք են `decodeCiphertextFrame`, `decodeEnvelopeJson`,
և `decodeSignResultAlgorithm` վրիպազերծման կամ փոխգործունակության փորձարկման համար: Արտադրության համար
հավելվածներին, ապավինեք `ConnectSession`-ին և `ConnectEnvelope`-ին, որպեսզի վարքագիծը համապատասխանի Rust-ին և
Android SDK-ներ հենց.

## CI վավերացում

- Նախքան թարմացված կամուրջի արտեֆակտները հրապարակելը կամ Connect ինտեգրումները սեղմելը, գործարկեք.

  ```bash
  make swift-ci
  ```

  Թիրախը հաստատում է հարմարանքների հավասարությունը, ստուգում է վահանակի թարմացումները և ներկայացնում է CLI-ը
  տեղական ամփոփումներ: Buildkite-ում նույն աշխատանքային հոսքը կախված է մետատվյալների ստեղներից, ինչպիսիք են
  `ci/xcframework-smoke:<lane>:device_tag`; հաստատել, որ մետատվյալներն առկա են խմբագրումից հետո
  խողովակաշարեր կամ գործակալների պիտակներ, որպեսզի վահանակները կարողանան արդյունքները վերագրել ճիշտ սիմուլյատորին կամ
  StrongBox նրբ.
- Եթե հրամանը ձախողվի, հետևեք հավասարության գրքույկին (`docs/source/swift_parity_triage.md`)
  և ստուգեք ստացված `mobile_ci` ելքը՝ պարզելու համար, թե որ գոտին է պահանջում վերականգնում
  կամ միջադեպի հետևում նախքան նորից փորձելը: