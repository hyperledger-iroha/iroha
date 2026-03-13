---
lang: ka
direction: ltr
source: docs/connect_swift_integration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b937a75e50aa77c02fcab0a11dae1b1cc182f88c179d6f90aa69181afa80d1b
source_last_modified: "2026-01-05T18:22:23.394597+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## NoritoBridgeKit-ის ინტეგრირება Xcode iOS პროექტში

ეს გზამკვლევი გვიჩვენებს, თუ როგორ გავაერთიანოთ Rust Norito ხიდი (XCFramework) და Swift შეფუთვა iOS აპში, შემდეგ გავცვალოთ Iroha Connect ჩარჩოები WebSocket-ის მეშვეობით იგივე Norito კოდეკების გამოყენებით, როგორც Rust ჰოსტ.

წინაპირობები
- NoritoBridge.xcframework zip (აშენებული CI workflow-ის მიერ) და Swift დამხმარე `NoritoBridgeKit.swift` (დააკოპირეთ ვერსია `examples/ios/NoritoDemo/Sources` ქვეშ, თუ პირდაპირ არ მოიხმართ დემო პროექტს).
- Xcode 15+, iOS 13+ სამიზნე.

ვარიანტი A: Swift პაკეტის მენეჯერი (რეკომენდებულია)
1) გამოაქვეყნეთ ბინარული SPM `Package.swift.template`-ის გამოყენებით `crates/connect_norito_bridge/`-ში (შეავსეთ URL და შემოწმების ჯამი CI-დან).
2) Xcode-ში: ფაილი → პაკეტების დამატება… → შეიყვანეთ SPM რეპო URL → დაამატეთ `NoritoBridge` პროდუქტი თქვენს სამიზნეზე.
3) დაამატეთ `NoritoBridgeKit.swift` თქვენს აპლიკაციის სამიზნეს (გადაათრიეთ თქვენს პროექტში, დარწმუნდით, რომ მონიშნეთ „ასლი საჭიროების შემთხვევაში“).

ვარიანტი B: CocoaPods
1) შექმენით Podspec `NoritoBridge.podspec.template`-დან (შეავსეთ `s.source` zip URL).
2) `pod trunk push NoritoBridge.podspec`.
3) თქვენს პოდფაილში: `pod 'NoritoBridge'` → `pod install`.
4) დაამატეთ `NoritoBridgeKit.swift` თქვენს აპლიკაციის სამიზნეს.

იმპორტი
```swift
import Foundation
import CryptoKit               // ChaChaPoly / HKDF
import IrohaSwift              // ConnectClient / ConnectSession / ConnectCrypto
import NoritoBridge            // Clang module from the XCFramework
// Ensure NoritoBridgeKit.swift is part of the target
```

### დაკავშირების სესიის ჩატვირთვა

`ConnectClient` მართავს WebSocket-ს, ხოლო `ConnectSession` აკონტროლებს კონტროლს
ჩარჩოები და შიფრული ტექსტის კონვერტები. ქვემოთ მოყვანილი ფრაგმენტი გვიჩვენებს, თუ როგორ ხსნის dApp სესიას,
მიიღეთ Connect გასაღებები და დაელოდეთ დამტკიცების პასუხს.

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
        // Ready to decrypt ciphertext envelopes
        let firstEnvelope = try await connectSession.nextEnvelope()
        print("payload:", firstEnvelope.payload)
    }
}
```

### შიფრული ტექსტის ჩარჩოების გაგზავნა (ხელმოწერის მოთხოვნები და ა.შ.)

როდესაც dApp-ს სჭირდება ხელმოწერის მოთხოვნა, ის იყენებს Norito ხიდის დამხმარეებს კოდირებისთვის
კონვერტი, დაშიფვრავს დატვირთვას ChaChaPoly-ით და ახვევს მას `ConnectFrame`-ში.

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

`ConnectAEAD.header` / `ConnectAEAD.nonce` არის მოხერხებულობის დამხმარეები (იხილეთ ფრაგმენტი
`docs/connect_swift_ios.md`) აგებულია `connect:v1` სათაურის საერთო განმარტებიდან. მათ
ადვილია ჩასმა, თუ გსურთ არ დაამატოთ სხვა პროგრამა:

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

### ჩარჩოების მიღება/გაშიფვრა

`ConnectSession` უკვე ამჟღავნებს `nextEnvelope()`-ს, რომელიც შიფრავს დატვირთვას მიმართულებისას
გასაღებები კონფიგურირებულია. თუ გჭირდებათ ხელით წვდომა (მაგალითად, არსებული დეკოდერის შესატყვისად
მილსადენი), შეგიძლიათ დარეკოთ ქვედა დონის დამხმარე:

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

`NoritoBridgeKit` ასევე ავლენს დამხმარეებს, როგორიცაა `decodeCiphertextFrame`, `decodeEnvelopeJson`,
და `decodeSignResultAlgorithm` გამართვის ან თავსებადობის ტესტირებისთვის. წარმოებისთვის
აპებს დაეყრდნოთ `ConnectSession` და `ConnectEnvelope`, რათა ქცევა ემთხვეოდეს Rust-ს და
Android SDK-ები ზუსტად.

## CI ვალიდაცია

- განახლებული ხიდის არტეფაქტების გამოქვეყნებამდე ან Connect ინტეგრაციის ჩართვამდე, გაუშვით:

  ```bash
  make swift-ci
  ```

  სამიზნე ამოწმებს მოწყობილობების პარიტეტს, ამოწმებს დაფის არხებს და ახდენს CLI-ს
  შეჯამებები ადგილობრივად. Buildkite-ში იგივე სამუშაო პროცესი დამოკიდებულია მეტამონაცემების გასაღებებზე, როგორიცაა
  `ci/xcframework-smoke:<lane>:device_tag`; დაადასტურეთ, რომ მეტამონაცემები არსებობს რედაქტირების შემდეგ
  მილსადენები ან აგენტის ტეგები, რათა დაფებმა შეძლონ შედეგების სწორ სიმულატორს ან
  StrongBox შესახვევი.
- თუ ბრძანება ვერ მოხერხდა, მიჰყევით პარიტეტის სათამაშო წიგნს (`docs/source/swift_parity_triage.md`)
  და შეამოწმეთ გაწეული `mobile_ci` გამომავალი, რათა დაადგინოთ რომელი ზოლი საჭიროებს რეგენერაციას
  ან ინციდენტის შემდგომი დაკვირვება ხელახლა ცდამდე.