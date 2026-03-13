---
lang: es
direction: ltr
source: docs/connect_swift_integration.md
status: complete
translator: manual
source_hash: ebdea5644112eab6e2027a7a4744d0ad3ea37a591abd564175f0a9511654e20a
source_last_modified: "2025-11-02T04:40:28.807763+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traducción al español de docs/connect_swift_integration.md (Integrating NoritoBridgeKit) -->

## Integrar NoritoBridgeKit en un Proyecto iOS de Xcode

Esta guía muestra cómo integrar el bridge Norito en Rust (XCFramework) y los
wrappers en Swift en una app iOS, y luego intercambiar frames de Iroha Connect
por WebSocket usando los mismos codecs Norito que el host en Rust.

Requisitos previos
- Un zip `NoritoBridge.xcframework` (generado por el workflow de CI) y el
  helper Swift `NoritoBridgeKit.swift` (copia la versión bajo
  `examples/ios/NoritoDemo/Sources` si no estás consumiendo el proyecto de
  demo directamente).
- Xcode 15+ y objetivo iOS 13+.

Opción A: Swift Package Manager (recomendada)
1) Publica un paquete binario SPM usando `Package.swift.template` en
   `crates/connect_norito_bridge/` (rellena la URL y checksum a partir del CI).
2) En Xcode: File → Add Packages… → introduce la URL del repo SPM → añade el
   producto `NoritoBridge` a tu target.
3) Añade `NoritoBridgeKit.swift` al target de tu app (arrastra el archivo al
   proyecto y asegúrate de marcar “Copy if needed”).

Opción B: CocoaPods
1) Crea un Podspec a partir de `NoritoBridge.podspec.template` (rellena el
   campo `s.source` con la URL del zip).
2) Ejecuta `pod trunk push NoritoBridge.podspec`.
3) En tu Podfile: añade `pod 'NoritoBridge'` → `pod install`.
4) Añade `NoritoBridgeKit.swift` al target de tu app.

Imports
```swift
import Foundation
import CryptoKit               // ChaChaPoly / HKDF
import IrohaSwift              // ConnectClient / ConnectSession / ConnectCrypto
import NoritoBridge            // módulo Clang del XCFramework
// Asegúrate de que NoritoBridgeKit.swift forma parte del target
```

### Inicializar una sesión de Connect

`ConnectClient` se encarga del WebSocket, mientras que `ConnectSession`
coordina los frames de control y los sobres cifrados. El siguiente snippet
muestra cómo una dApp abre una sesión, deriva las claves de Connect y espera
una respuesta de aprobación.

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
        // Listo para descifrar sobres de ciphertext
        let firstEnvelope = try await connectSession.nextEnvelope()
        print("payload:", firstEnvelope.payload)
    }
}
```

### Enviar frames cifrados (solicitudes de firma, etc.)

Cuando la dApp necesita solicitar una firma, usa los helpers del bridge Norito
para codificar un sobre, cifra la payload con ChaChaPoly y la encapsula en un
`ConnectFrame`.

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

`ConnectAEAD.header` / `ConnectAEAD.nonce` son helpers de conveniencia (ver el
snippet en `docs/connect_swift_ios.md`) construidos a partir de la definición
compartida de cabecera `connect:v1`. Puedes inlinearlos fácilmente si prefieres
no añadir otra utilidad:

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

