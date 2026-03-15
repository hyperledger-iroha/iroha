---
lang: pt
direction: ltr
source: docs/connect_swift_integration.md
status: complete
translator: manual
source_hash: ebdea5644112eab6e2027a7a4744d0ad3ea37a591abd564175f0a9511654e20a
source_last_modified: "2025-11-02T04:40:28.807763+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Tradução em português de docs/connect_swift_integration.md (Integrating NoritoBridgeKit) -->

## Integração do NoritoBridgeKit em um Projeto iOS (Xcode)

Este guia mostra como integrar o bridge Norito em Rust (XCFramework) e os
wrappers em Swift em um app iOS, e em seguida trocar frames Iroha Connect via
WebSocket usando os mesmos codecs Norito que o host em Rust.

Pré‑requisitos
- Um zip `NoritoBridge.xcframework` (gerado pelo workflow de CI) e o helper
  Swift `NoritoBridgeKit.swift` (copie a versão em
  `examples/ios/NoritoDemo/Sources` se você não estiver consumindo o projeto
  de demo diretamente).
- Xcode 15+ e alvo iOS 13+.

Opção A: Swift Package Manager (recomendada)
1) Publique um pacote binário SPM usando `Package.swift.template` em
   `crates/connect_norito_bridge/` (preencha URL e checksum a partir do CI).
2) No Xcode: File → Add Packages… → informe a URL do repositório SPM → adicione
   o produto `NoritoBridge` ao seu target.
3) Adicione `NoritoBridgeKit.swift` ao target do app (arraste para o projeto
   certificando‑se de marcar “Copy if needed”).

Opção B: CocoaPods
1) Crie um Podspec a partir de `NoritoBridge.podspec.template` (preencha
   `s.source` com a URL do zip).
2) Execute `pod trunk push NoritoBridge.podspec`.
3) No Podfile: adicione `pod 'NoritoBridge'` → `pod install`.
4) Adicione `NoritoBridgeKit.swift` ao target do app.

Imports
```swift
import Foundation
import CryptoKit               // ChaChaPoly / HKDF
import IrohaSwift              // ConnectClient / ConnectSession / ConnectCrypto
import NoritoBridge            // módulo Clang do XCFramework
// Garanta que NoritoBridgeKit.swift faça parte do target
```

### Inicializando uma sessão Connect

`ConnectClient` gerencia o WebSocket, enquanto `ConnectSession` coordena os
frames de controle e os envelopes cifrados. O snippet abaixo ilustra como uma
dApp abre uma sessão, deriva as chaves de Connect e espera por uma resposta
de aprovação.

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
        // Pronto para decifrar envelopes de ciphertext
        let firstEnvelope = try await connectSession.nextEnvelope()
        print("payload:", firstEnvelope.payload)
    }
}
```

### Enviando frames cifrados (solicitações de assinatura etc.)

Quando a dApp precisa solicitar uma assinatura, ela usa os helpers do Norito
bridge para codificar um envelope, cifra a payload com ChaChaPoly e a embala
em um `ConnectFrame`.

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

`ConnectAEAD.header` / `ConnectAEAD.nonce` são helpers de conveniência (ver o
snippet em `docs/connect_swift_ios.md`) construídos a partir da definição de
cabeçalho compartilhada `connect:v1`. Eles podem ser facilmente inlined se
você preferir evitar uma utilidade extra:

```swift
enum ConnectAEAD {
    static func header(sessionID: Data, direction: ConnectDirection, sequence: UInt64) -> Data {
        var buffer = Data()
        buffer.append("connect:v1".data(using: .utf8)!)
        buffer.append(sessionID)
        buffer.append(direction == .appToWallet ? 0 : 1)
        var seq = sequence.littleEndian
        withUnsafeBytes(of: &seq) { buffer.append(contentsOf: $0) }
        buffer.append(1) // Ciphertext
        return buffer
    }

    static func nonce(sequence: UInt64) -> ChaChaPoly.Nonce {
        var seq = sequence.littleEndian
        var bytes = Data(count: 12)
        bytes.replaceSubrange(4..<12, with: withUnsafeBytes(of: &seq) { Data($0) })
        return try! ChaChaPoly.Nonce(data: bytes)
    }
}
```

