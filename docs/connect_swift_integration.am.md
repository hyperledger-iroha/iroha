---
lang: am
direction: ltr
source: docs/connect_swift_integration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b937a75e50aa77c02fcab0a11dae1b1cc182f88c179d6f90aa69181afa80d1b
source_last_modified: "2026-01-05T18:22:23.394597+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## NoritoBridgeKitን በXcode iOS ፕሮጀክት ውስጥ በማዋሃድ ላይ

ይህ መመሪያ የ Rust Norito ድልድይ (XCFramework) እና የስዊፍት መጠቅለያዎችን ወደ iOS መተግበሪያ እንዴት እንደሚያዋህድ ያሳያል፣ ከዚያም Iroha Connect ፍሬሞችን በWebSocket ላይ እንደ Rust host ተመሳሳይ Norito ኮዶችን ይቀይሩ።

ቅድመ-ሁኔታዎች
- NoritoBridge.xcframework ዚፕ (በሲአይ የስራ ፍሰት የተሰራ) እና የስዊፍት አጋዥ `NoritoBridgeKit.swift` (የማሳያ ፕሮጄክቱን በቀጥታ የማይጠቀሙ ከሆነ በ `examples/ios/NoritoDemo/Sources` ስር ያለውን ስሪት ይቅዱ)።
- Xcode 15+፣ iOS 13+ ኢላማ።

አማራጭ ሀ፡ የስዊፍት ጥቅል አስተዳዳሪ (የሚመከር)
1) `Package.swift.template` በ `crates/connect_norito_bridge/` (ዩአርኤል ሙላ እና የ CI ቼክ) በመጠቀም ሁለትዮሽ SPM ያትሙ።
2) በXcode: ፋይል → ፓኬጆችን አክል… → የ SPM ሪፖ ዩአርኤል ያስገቡ → የ`NoritoBridge` ምርት ወደ ዒላማዎ ያክሉ።
3) `NoritoBridgeKit.swift` ወደ የእርስዎ መተግበሪያ ዒላማ ያክሉ (ወደ ፕሮጀክትዎ ይጎትቱ፣ “ከተፈለገ ቅዳ” ምልክት የተደረገበት መሆኑን ያረጋግጡ)።

አማራጭ B: CocoaPods
1) ከ `NoritoBridge.podspec.template` Podspec ይፍጠሩ (የ`s.source` ዚፕ ዩአርኤልን ይሙሉ)።
2) `pod trunk push NoritoBridge.podspec`.
3) በፖድፋይልዎ ውስጥ፡ I18NI0000019X → `pod install`።
4) `NoritoBridgeKit.swift` ወደ የእርስዎ መተግበሪያ ዒላማ ያክሉ።

ያስመጣሉ።
```swift
import Foundation
import CryptoKit               // ChaChaPoly / HKDF
import IrohaSwift              // ConnectClient / ConnectSession / ConnectCrypto
import NoritoBridge            // Clang module from the XCFramework
// Ensure NoritoBridgeKit.swift is part of the target
```

### የግንኙነት ክፍለ ጊዜን በማስነሳት ላይ

`ConnectClient` ዌብሶኬትን ይቆጣጠራል፣ `ConnectSession` ኦርኬስትራዎች ግን ይቆጣጠራል።
ክፈፎች እና የምስጢር ጽሑፍ ፖስታዎች። ከታች ያለው ቅንጣቢ dApp እንዴት ክፍለ ጊዜ እንደሚከፍት ያሳያል።
የግንኙነት ቁልፎችን ያውጡ እና የማጽደቅ ምላሽ ይጠብቁ።

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

### የምስጢር ጽሑፍ ፍሬሞችን በመላክ ላይ (ምልክት ጥያቄዎች፣ ወዘተ)

dApp ፊርማ ሲፈልግ የNorito ድልድይ አጋዥዎችን ይጠቀማል።
ኤንቨሎፕ፣ ክፍያውን በ ChaChaPoly ኢንክሪፕት ያደርጋል፣ እና በ`ConnectFrame` ይጠቀለላል።

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

`ConnectAEAD.header` / `ConnectAEAD.nonce` ምቹ ረዳቶች ናቸው (በ ውስጥ ያለውን ቅንጣቢ ይመልከቱ)
`docs/connect_swift_ios.md`) ከተጋራው I18NI0000028X ራስጌ ትርጉም የተሰራ። እነሱ
ሌላ መገልገያ ላለመጨመር ከመረጡ መስመር ውስጥ ለመግባት ቀላል ናቸው፡

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

### ክፈፎችን መቀበል / መፍታት

`ConnectSession` ቀድሞውንም `nextEnvelope()` አጋልጧል፣ ይህም አቅጣጫ ሲሄድ የሚጫኑ ጭነቶችን ዲክሪፕት ያደርጋል።
ቁልፎች ተዋቅረዋል። በእጅ መድረስ ከፈለጉ (ለምሳሌ አሁን ካለው ዲኮደር ጋር ለማዛመድ
የቧንቧ መስመር) ዝቅተኛ ደረጃ ረዳትን መደወል ይችላሉ-

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

`NoritoBridgeKit` እንዲሁም እንደ `decodeCiphertextFrame`፣ `decodeEnvelopeJson`፣
እና `decodeSignResultAlgorithm` ለማረም ወይም አብሮ ለመስራት መሞከር። ለማምረት
መተግበሪያዎች፣ በ`ConnectSession` እና I18NI0000036X ላይ ተመርኩዘው ባህሪው ከዝገቱ እና
የአንድሮይድ ኤስዲኬዎች በትክክል።

## CI ማረጋገጫ

- የተሻሻሉ የድልድይ ቅርሶችን ከማተምዎ ወይም የግንኙነት ውህደቶችን ከመግፋትዎ በፊት ያሂዱ፡-

  ```bash
  make swift-ci
  ```

  ዒላማው የቋሚነት እኩልነትን ያረጋግጣል፣ የዳሽቦርድ ምግቦችን ይፈትሻል እና CLI ን ይሰጣል
  በአካባቢው ማጠቃለያዎች. በBuildkite ውስጥ ተመሳሳይ የስራ ፍሰት እንደ ሜታዳታ ቁልፎች ይወሰናል
  `ci/xcframework-smoke:<lane>:device_tag`; ከአርትዖት በኋላ ሜታዳታ መኖሩን ያረጋግጡ
  የቧንቧ መስመሮች ወይም ኤጀንት መለያዎች ዳሽቦርዶች ውጤቱን በትክክለኛው ሲሙሌተር ወይም
  StrongBox መስመር።
- ትዕዛዙ ካልተሳካ ፣ተመጣጣኝ ማጫወቻውን (`docs/source/swift_parity_triage.md`) ይከተሉ።
  እና የትኛውን መስመር እድሳት እንደሚያስፈልገው ለመለየት የተሰራውን የI18NI0000039X ውፅዓት ይፈትሹ
  ወይም እንደገና ከመሞከርዎ በፊት የክስተት ክትትል።