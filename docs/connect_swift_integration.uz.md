---
lang: uz
direction: ltr
source: docs/connect_swift_integration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b937a75e50aa77c02fcab0a11dae1b1cc182f88c179d6f90aa69181afa80d1b
source_last_modified: "2026-01-05T18:22:23.394597+00:00"
translation_last_reviewed: 2026-02-07
---

## Integrating NoritoBridgeKit in an Xcode iOS Project

This guide shows how to integrate the Rust Norito bridge (XCFramework) and the Swift wrappers into an iOS app, then exchange Iroha Connect frames over WebSocket using the same Norito codecs as the Rust host.

Prerequisites
- A NoritoBridge.xcframework zip (built by CI workflow) and the Swift helper `NoritoBridgeKit.swift` (copy the version under `examples/ios/NoritoDemo/Sources` if you are not consuming the demo project directly).
- Xcode 15+, iOS 13+ target.

Option A: Swift Package Manager (recommended)
1) Publish a binary SPM using the `Package.swift.template` in `crates/connect_norito_bridge/` (fill URL and checksum from CI).
2) In Xcode: File → Add Packages… → Enter the SPM repo URL → Add the `NoritoBridge` product to your target.
3) Add `NoritoBridgeKit.swift` to your app target (drag into your project, ensure “Copy if needed” is ticked).

Option B: CocoaPods
1) Create a Podspec from `NoritoBridge.podspec.template` (fill the `s.source` zip URL).
2) `pod trunk push NoritoBridge.podspec`.
3) In your Podfile: `pod 'NoritoBridge'` → `pod install`.
4) Add `NoritoBridgeKit.swift` to your app target.

Imports
```swift
import Foundation
import CryptoKit               // ChaChaPoly / HKDF
import IrohaSwift              // ConnectClient / ConnectSession / ConnectCrypto
import NoritoBridge            // Clang module from the XCFramework
// Ensure NoritoBridgeKit.swift is part of the target
```

### Bootstrapping a Connect session

`ConnectClient` handles the WebSocket, while `ConnectSession` orchestrates control
frames and ciphertext envelopes. The snippet below shows how a dApp would open a session,
derive Connect keys, and wait for an approval response.

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

### Sending ciphertext frames (sign requests, etc.)

When the dApp needs to request a signature it uses the Norito bridge helpers to encode
an envelope, encrypts the payload with ChaChaPoly, and wraps it in a `ConnectFrame`.

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

`ConnectAEAD.header` / `ConnectAEAD.nonce` are convenience helpers (see the snippet in
`docs/connect_swift_ios.md`) built from the shared `connect:v1` header definition. They
are easy to inline if you prefer not to add another utility:

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

### Receiving / decrypting frames

`ConnectSession` already exposes `nextEnvelope()`, which decrypts payloads when direction
keys are configured. If you need manual access (for example to match an existing decoder
pipeline), you can call the lower-level helper:

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

`NoritoBridgeKit` also exposes helpers such as `decodeCiphertextFrame`, `decodeEnvelopeJson`,
and `decodeSignResultAlgorithm` for debugging or interoperability testing. For production
apps, rely on `ConnectSession` and `ConnectEnvelope` so behaviour matches the Rust and
Android SDKs exactly.

## CI validation

- Before publishing updated bridge artifacts or pushing Connect integrations, run:

  ```bash
  make swift-ci
  ```

  The target validates fixture parity, checks the dashboard feeds, and renders the CLI
  summaries locally. In Buildkite the same workflow depends on metadata keys such as
  `ci/xcframework-smoke:<lane>:device_tag`; confirm the metadata is present after editing
  pipelines or agent tags so dashboards can attribute results to the correct simulator or
  StrongBox lane.
- If the command fails, follow the parity playbook (`docs/source/swift_parity_triage.md`)
  and inspect the rendered `mobile_ci` output to identify which lane requires regeneration
  or incident follow-up before retrying.
