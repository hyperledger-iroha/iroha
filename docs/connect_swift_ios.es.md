---
lang: es
direction: ltr
source: docs/connect_swift_ios.md
status: complete
translator: manual
source_hash: 0f2dbf44a069becc70aee5abfeff66ad22ca2ae04c605cea9e953d4e750dccc6
source_last_modified: "2025-11-02T04:40:28.813100+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traducción al español de docs/connect_swift_ios.md (Recommended SDK Flow + Manual CryptoKit Reference) -->

## Flujo recomendado del SDK (ConnectClient + bridge Norito)

¿Necesitas una guía completa de integración en Xcode (SPM/CocoaPods, conexión del XCFramework, helpers de ChaChaPoly)?
Consulta `docs/connect_swift_integration.md` para la guía de empaquetado extremo a extremo.

El SDK de Swift incluye una pila Connect respaldada por Norito:

- `ConnectClient` mantiene el transporte WebSocket (`/v2/connect/ws?...`) sobre
  `URLSessionWebSocketTask`.
- `ConnectSession` orquesta el ciclo de vida (open → approve/reject → sign → close) y
  descifra los frames de ciphertext cuando las claves de dirección ya están instaladas.
- `ConnectCrypto` expone generación de claves X25519 y derivación de claves de dirección
  según Norito, de modo que las apps no tengan que cablear HKDF/HMAC a mano.
- `ConnectEnvelope`/`ConnectControl` representan los frames tipados de Norito emitidos por
  el bridge en Rust (`connect_norito_bridge`); los sobres cifrados se descifran con los
  mismos helpers FFI usados en Android/Rust, garantizando paridad.

Antes de iniciar una sesión:
1. Deriva el identificador de sesión de 32 bytes (`sid`) usando la misma receta BLAKE2b que
   el resto de SDKs (`"iroha-connect|sid|" || chain_id || app_pk || nonce16`).
2. Genera un par de claves Connect con `ConnectCrypto.generateKeyPair()` o reutiliza una
   clave privada almacenada (las claves públicas se pueden recomputar con
   `ConnectCrypto.publicKey(fromPrivateKey:)`).
3. Crea el cliente WebSocket y arráncalo dentro de un contexto async.

```swift
import IrohaSwift

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
        appMetadata: ConnectAppMetadata(name: "Demo dApp", iconURL: nil, description: "Sample workflow"),
        constraints: ConnectConstraints(chainID: "00000000-0000-0000-0000-000000000000"),
        permissions: ConnectPermissions(methods: ["sign"], events: [])
    )
    try await connectSession.sendOpen(open: open)

    // Wait for wallet response (approve/reject/close)
    if case .approve(let approval) = try await connectSession.nextControlFrame() {
        let directionKeys = try ConnectCrypto.deriveDirectionKeys(
            localPrivateKey: keyPair.privateKey,
            peerPublicKey: approval.walletPublicKey,
            sessionID: sessionID
        )
        connectSession.setDirectionKeys(directionKeys)

        // Decrypt ciphertext frames (sign results, encrypted controls)
        let envelope = try await connectSession.nextEnvelope()
        switch envelope.payload {
        case .signResultOk(let signature):
            print("signature:", signature.signature.base64EncodedString())
        case .controlClose(let close):
            print("session closed:", close.code, close.reason ?? "<none>")
        default:
            break
        }
    }
}
```

`ConnectSession` lanza `ConnectSessionError.missingDecryptionKeys` si llegan frames de
ciphertext antes de que se hayan instalado las claves de dirección; derívalas
inmediatamente después de procesar un control `Approve` (la clave pública de la wallet va
incluida en el payload). Si necesitas inspeccionar los frames cifrados manualmente, llama a
`ConnectEnvelope.decrypt(frame:symmetricKey:)` con la clave direccional que corresponda al
sentido del frame.

> **Consejo:** Cuando el bridge Norito no está disponible (por ejemplo, builds con Swift
> Package Manager sin el XCFramework), el SDK cae automáticamente en un shim JSON. Los
> helpers de cifrado (`ConnectCrypto.*`) requieren el bridge, así que enlaza el XCFramework
> en apps de producción.

## Referencia manual de CryptoKit (alternativo / fallback)

Si el bridge Norito no está disponible o necesitas prototipar sin los helpers del SDK, la
referencia siguiente muestra cómo cablear X25519 + ChaChaPoly de forma manual. Se ajusta a
la especificación de Norito, pero no ofrece las garantías de paridad determinista que el
SDK da “de serie”.

Esta sección muestra cómo:
- Calcular `sid` y `salt` (BLAKE2b-256; aquí usando un paquete BLAKE2b de terceros o
  valores precomputados).
- Realizar acuerdo de claves X25519 (`Curve25519.KeyAgreement`).
- Derivar la clave de dirección mediante HKDF-SHA256 con `salt` e `info`.
- Construir el AAD a partir de ("connect:v1", sid, dir, seq, kind=Ciphertext) y la nonce
  a partir de `seq`.
- Sellar y abrir payloads con ChaChaPoly y AAD.
- Unirse al WS con el token.

Requiere iOS 13+ (CryptoKit). Para BLAKE2b, usa un pequeño paquete Swift (por ejemplo,
https://github.com/Frizlab/SwiftBLAKE2) o calcula `salt`/`sid` en el servidor; el ejemplo
asume que dispones de `blake2b(data: Data) -> Data`.

```swift
import Foundation
import CryptoKit

func aadV1(sid: Data, dir: UInt8, seq: UInt64) -> Data {
    var out = Data()
    out.append("connect:v1".data(using: .utf8)!)
    out.append(sid)
    out.append(Data([dir]))
    var le = seq.littleEndian
    withUnsafeBytes(of: &le) { out.append($0) }
    out.append(Data([1])) // kind=Ciphertext
    return out
}

func nonceFromSeq(_ seq: UInt64) -> ChaChaPoly.Nonce {
    var n = Data(count: 12)
    var le = seq.littleEndian
    n.replaceSubrange(4..<12, with: withUnsafeBytes(of: &le) { Data($0) })
    return try! ChaChaPoly.Nonce(data: n)
}

func hkdf(_ ikm: SymmetricKey, salt: Data, info: Data, len: Int = 32) -> SymmetricKey {
    let sk = HKDF<SHA256>.deriveKey(inputKeyMaterial: ikm, salt: SymmetricKey(data: salt), info: info, outputByteCount: len)
    return sk
}

// Derive direction keys (app→wallet, wallet→app)
func deriveDirectionKeys(sharedSecret: SharedSecret, sid: Data) -> (SymmetricKey, SymmetricKey) {
    let salt = blake2b(data: Data("iroha-connect|salt|".utf8) + sid)
    let ikm = sharedSecret.hkdfDerivedSymmetricKey(using: SHA256.self, salt: salt, sharedInfo: Data(), outputByteCount: 32)
    let kApp = hkdf(ikm, salt: salt, info: Data("iroha-connect|k_app".utf8))
    let kWallet = hkdf(ikm, salt: salt, info: Data("iroha-connect|k_wallet".utf8))
    return (kApp, kWallet)
}

// Seal payload with AAD and seq-derived nonce
func sealEnvelopeV1(key: SymmetricKey, sid: Data, dir: UInt8, seq: UInt64, payload: Data) -> Data {
    let aad = aadV1(sid: sid, dir: dir, seq: seq)
    let nonce = nonceFromSeq(seq)
    let sealed = try! ChaChaPoly.seal(payload, using: key, nonce: nonce, authenticating: aad)
    return sealed.combined // ciphertext||tag
}

// Open payload and validate seq
func openEnvelopeV1(key: SymmetricKey, sid: Data, dir: UInt8, seq: UInt64, combined: Data) -> Data {
    let aad = aadV1(sid: sid, dir: dir, seq: seq)
    let nonce = nonceFromSeq(seq)
    let box = try! ChaChaPoly.SealedBox(combined: combined)
    return try! ChaChaPoly.open(box, using: key, authenticating: aad)
}

// Base64URL (no padding) encode
func base64url(_ data: Data) -> String {
    let b64 = data.base64EncodedString()
    return b64.replacingOccurrences(of: "+", with: "-")
              .replacingOccurrences(of: "/", with: "_")
              .replacingOccurrences(of: "=", with: "")
}

// Create Connect session: client computes sid and POSTs to /v2/connect/session
func createConnectSession(node: String, chainId: String, appEphemeralPk: Data, completion: @escaping (Result<(sidB64: String, tokenApp: String, tokenWallet: String), Error>) -> Void) {
    // Compute sid = BLAKE2b-256("iroha-connect|sid|" || chain_id || app_pk || nonce16)
    let nonce16 = (0..<16).map { _ in UInt8.random(in: 0...255) }
    var sidInput = Data("iroha-connect|sid|".utf8)
    sidInput.append(Data(chainId.utf8))
    sidInput.append(appEphemeralPk)
    sidInput.append(Data(nonce16))
    let sid = blake2b(data: sidInput)
    let sidB64 = base64url(sid)

    // POST JSON { sid, node }
    let url = URL(string: node + "/v2/connect/session")!
    var req = URLRequest(url: url)
    req.httpMethod = "POST"
    req.setValue("application/json", forHTTPHeaderField: "Content-Type")
    let body = ["sid": sidB64, "node": node]
    req.httpBody = try? JSONSerialization.data(withJSONObject: body, options: [])
    URLSession.shared.dataTask(with: req) { data, resp, err in
        if let err = err { completion(.failure(err)); return }
        guard let http = resp as? HTTPURLResponse, let data = data, http.statusCode == 200,
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let tokenApp = obj["token_app"] as? String,
              let tokenWallet = obj["token_wallet"] as? String,
              let sidEcho = obj["sid"] as? String
        else {
            completion(.failure(NSError(domain: "connect", code: -1)))
            return
        }
        completion(.success((sidEcho, tokenApp, tokenWallet)))
    }.resume()
}

// Join WS with token (URLSessionWebSocketTask)
func joinWs(node: String, sid: String, role: String, token: String, onMessage: @escaping (Data)->Void) {
    let wsUrl = node.replacingOccurrences(of: "http", with: "ws") + "/v2/connect/ws?sid=\(sid)&role=\(role)"
    var request = URLRequest(url: URL(string: wsUrl)!)
    request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
    let task = URLSession.shared.webSocketTask(with: request)
    task.resume()
    func recv() {
        task.receive { result in
            switch result {
            case .failure(let e): print("ws error", e)
            case .success(let msg):
                switch msg {
                case .data(let d): onMessage(d)
                case .string: break
                @unknown default: break
                }
                recv()
            }
        }
    }
    recv()
}

// Example usage (app-side): after computing keys and sid
// let frameBinary = noritoEncodeConnectFrameV1(... Ciphertext { aead: sealEnvelopeV1(key: kApp, ...) })
// task.send(.data(frameBinary))
```

Notas:
- Calcula `sid` en el cliente y luego haz POST a `/v2/connect/session` con ese `sid` para
  obtener los tokens por rol; únete al WS con el token.
- Después de `Approve`, envía `Close`/`Reject` como payloads cifrados.
- Necesitas un framing Norito real para envolver `aead` dentro de `ConnectFrameV1` como
  binario; impleméntalo con tus bindings de Norito.

### Demo de framing Norito (solo ejemplo)

Lo siguiente es un framing mínimo y no canónico suficiente para demos locales. NO está
en formato Norito y debe sustituirse por un encoder/decoder Norito real cuando esté
disponible. Simplemente concatena campos en orden little‑endian estable para que puedas
probar transporte WS de extremo a extremo sin traer un codec completo a iOS.

Layout (solo demo):
- 32 bytes: sid
- 1 byte: dir (0 = AppToWallet, 1 = WalletToApp)
- 8 bytes: seq (LE)
- 1 byte: kind (1 = Ciphertext)
- 1 byte: repetir `dir` dentro del ciphertext (mantenido para paridad con el tipo compartido)
- 4 bytes: ct_len (LE)
- ct_len bytes: aead (ChaChaPoly combinado)

```swift
func le64(_ x: UInt64) -> Data { var v = x.littleEndian; return withUnsafeBytes(of: &v) { Data($0) } }
func le32(_ x: UInt32) -> Data { var v = x.littleEndian; return withUnsafeBytes(of: &v) { Data($0) } }

// Frame a ciphertext ConnectFrameV1 (demo framing)
func frameCiphertextV1Demo(sid: Data, dir: UInt8, seq: UInt64, aead: Data) -> Data {
    precondition(sid.count == 32)
    var out = Data(capacity: 32 + 1 + 8 + 1 + 1 + 4 + aead.count)
    out.append(sid)
    out.append(Data([dir]))
    out.append(le64(seq))
    out.append(Data([1])) // kind = Ciphertext
    out.append(Data([dir]))
    out.append(le32(UInt32(aead.count)))
    out.append(aead)
    return out
}

struct ParsedCiphertextV1Demo { let sid: Data; let dir: UInt8; let seq: UInt64; let aead: Data }

// Parse demo frame back into components
func parseCiphertextV1Demo(_ data: Data) -> ParsedCiphertextV1Demo? {
    if data.count < 32 + 1 + 8 + 1 + 1 + 4 { return nil }
    var o = 0
    let sid = data.subdata(in: o..<(o+32)); o += 32
    let dir = data[o]; o += 1
    let seqLe = data.subdata(in: o..<(o+8)); o += 8
    let seq = seqLe.withUnsafeBytes { $0.load(as: UInt64.self) }.littleEndian
    let kind = data[o]; o += 1
    guard kind == 1 else { return nil }
    let dir2 = data[o]; o += 1
    guard dir2 == dir else { return nil }
    let lenLe = data.subdata(in: o..<(o+4)); o += 4
    let ctLen = lenLe.withUnsafeBytes { $0.load(as: UInt32.self) }.littleEndian
    guard data.count >= o + Int(ctLen) else { return nil }
    let aead = data.subdata(in: o..<(o+Int(ctLen)))
    return ParsedCiphertextV1Demo(sid: sid, dir: dir, seq: seq, aead: aead)
}

// Example usage: send
// let ct = sealEnvelopeV1(key: kApp, sid: sid, dir: 0, seq: 1, payload: payload)
// let frame = frameCiphertextV1Demo(sid: sid, dir: 0, seq: 1, aead: ct)
// wsTask.send(.data(frame))

// Example usage: receive
// case .data(let d): if let f = parseCiphertextV1Demo(d) { let pt = openEnvelopeV1(key: kWallet, sid: f.sid, dir: f.dir, seq: f.seq, combined: f.aead) }
```

### Demo de Close/Reject cifrados (framing de ejemplo)

Suponiendo que ya has derivado `kWallet` (clave de dirección wallet→app), dispones de
`sid` (32 bytes) y un `URLSessionWebSocketTask` conectado llamado `ws`:

```swift
// Send encrypted Close (wallet → app) at seq=1
let closePayload = try! JSONSerialization.data(withJSONObject: [
  "Control": [
    "Close": [
      "who": "Wallet", // for demo logging only
      "code": 1000,
      "reason": "done",
      "retryable": false
    ]
  ]
], options: [])
let ctClose = sealEnvelopeV1(key: kWallet, sid: sid, dir: 1, seq: 1, payload: closePayload)
let frameClose = frameCiphertextV1Demo(sid: sid, dir: 1, seq: 1, aead: ctClose)
ws.send(.data(frameClose)) { err in if let err = err { print("ws send close:", err) } }

// Send encrypted Reject (wallet → app) at seq=2
let rejectPayload = try! JSONSerialization.data(withJSONObject: [
  "Control": [
    "Reject": [
      "code": 401,
      "code_id": "UNAUTHORIZED",
      "reason": "user denied"
    ]
  ]
], options: [])
let ctReject = sealEnvelopeV1(key: kWallet, sid: sid, dir: 1, seq: 2, payload: rejectPayload)
let frameReject = frameCiphertextV1Demo(sid: sid, dir: 1, seq: 2, aead: ctReject)
ws.send(.data(frameReject)) { err in if let err = err { print("ws send reject:", err) } }
```

Al recibir, usa `parseCiphertextV1Demo` y `openEnvelopeV1` para obtener el JSON en claro y
decodifícalo en tus tipos del lado de la app. Sustituye este framing de demo por un
encoding Norito real cuando integres.

## Validación en CI

- Antes de hacer cambios en Connect o en la integración del bridge, ejecuta:

  ```bash
  make swift-ci
  ```

  Este comando valida los fixtures de Swift, comprueba los feeds de los dashboards y
  genera los resúmenes de la CLI. El workflow de CI se apoya en metadatos de Buildkite
  (`ci/xcframework-smoke:<lane>:device_tag`) para mapear resultados al simulador o a las
  lanes de StrongBox; tras cambiar pipelines o etiquetas de agentes, confirma que esos
  metadatos siguen apareciendo en los logs.
- Si la ejecución falla, sigue `docs/source/swift_parity_triage.md` e inspecciona la salida
  de `mobile_ci` para determinar qué lane necesita regeneración o más gestión de
  incidencias.
