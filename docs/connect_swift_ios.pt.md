---
lang: pt
direction: ltr
source: docs/connect_swift_ios.md
status: complete
translator: manual
source_hash: 0f2dbf44a069becc70aee5abfeff66ad22ca2ae04c605cea9e953d4e750dccc6
source_last_modified: "2025-11-02T04:40:28.813100+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Tradução em português de docs/connect_swift_ios.md (Recommended SDK Flow + Manual CryptoKit Reference) -->

## Fluxo recomendado do SDK (ConnectClient + bridge Norito)

Precisa de um passo a passo completo de integração no Xcode (SPM/CocoaPods, ligação do
XCFramework, helpers de ChaChaPoly)?
Veja `docs/connect_swift_integration.md` para o guia de empacotamento ponta a ponta.

O SDK de Swift traz uma stack Connect apoiada por Norito:

- `ConnectClient` mantém o transporte WebSocket (`/v1/connect/ws?...`) sobre
  `URLSessionWebSocketTask`.
- `ConnectSession` orquestra o ciclo de vida (open → approve/reject → sign → close) e
  descriptografa frames de ciphertext depois que as chaves de direção são instaladas.
- `ConnectCrypto` expõe geração de chaves X25519 e derivação de chaves de direção
  compatível com Norito, para que os apps não precisem implementar HKDF/HMAC manualmente.
- `ConnectEnvelope`/`ConnectControl` representam os frames tipados de Norito emitidos pelo
  bridge em Rust (`connect_norito_bridge`); envelopes cifrados são abertos usando os mesmos
  helpers FFI usados em Android/Rust, garantindo paridade.

Antes de iniciar uma sessão:
1. Derive o identificador de sessão de 32 bytes (`sid`) usando a mesma receita de BLAKE2b
   que os demais SDKs (`"iroha-connect|sid|" || chain_id || app_pk || nonce16`).
2. Gere um par de chaves Connect via `ConnectCrypto.generateKeyPair()` ou reutilize uma
   chave privada persistida (a chave pública pode ser recomputada com
   `ConnectCrypto.publicKey(fromPrivateKey:)`).
3. Crie o cliente WebSocket e inicie-o dentro de um contexto assíncrono.

```swift
import IrohaSwift

let connectURL = URL(string: "wss://node.example/v1/connect/ws?sid=\(sidB64)&role=app&token=\(token)")!
let connectClient = ConnectClient(url: connectURL)
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

`ConnectSession` lança `ConnectSessionError.missingDecryptionKeys` se frames de ciphertext
chegarem antes de as chaves de direção estarem instaladas; derive-as imediatamente depois
de processar um controle `Approve` (a chave pública da wallet está incluída no payload).
Para inspecionar frames cifrados manualmente, chame
`ConnectEnvelope.decrypt(frame:symmetricKey:)` com a chave direcional que corresponde ao
sentido do frame.

> **Dica:** Quando o bridge Norito não está presente (por exemplo, builds com Swift
> Package Manager sem o XCFramework), o SDK faz fallback automático para um shim JSON. Os
> helpers de criptografia (`ConnectCrypto.*`) exigem o bridge, então faça o link do
> XCFramework em apps de produção.

## Referência manual de CryptoKit (alternativo / fallback)

Se o bridge Norito não estiver disponível ou você precisar prototipar sem os helpers do
SDK, a referência a seguir mostra como conectar X25519 + ChaChaPoly manualmente. Ela está
alinhada com a especificação de Norito, mas não oferece as garantias de paridade
determinística que o SDK fornece pronto para uso.

Esta seção mostra como:
- Calcular `sid` e `salt` (BLAKE2b-256; aqui usando um pacote BLAKE2b de terceiros ou
  valores pré-computados).
- Realizar acordo de chaves X25519 (`Curve25519.KeyAgreement`).
- Derivar a chave de direção via HKDF-SHA256 com `salt` e `info`.
- Construir o AAD a partir de ("connect:v1", sid, dir, seq, kind=Ciphertext) e a nonce a
  partir de `seq`.
- Selar e abrir payloads via ChaChaPoly com AAD.
- Entrar no WS com um token.

Requer iOS 13+ (CryptoKit). Para BLAKE2b, use um pequeno pacote Swift (por exemplo,
https://github.com/Frizlab/SwiftBLAKE2) ou compute `salt`/`sid` no servidor; o exemplo
abaixo assume que você tem `blake2b(data: Data) -> Data`.

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

// Create Connect session: client computes sid and POSTs to /v1/connect/session
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
    let url = URL(string: node + "/v1/connect/session")!
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
    let wsUrl = node.replacingOccurrences(of: "http", with: "ws") + "/v1/connect/ws?sid=\(sid)&role=\(role)&token=\(token)"
    let task = URLSession.shared.webSocketTask(with: URL(string: wsUrl)!)
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
- Calcule `sid` no lado do cliente e depois faça POST para `/v1/connect/session` com esse
  `sid` para obter tokens por papel; junte-se ao WS com o token.
- Depois de `Approve`, envie `Close`/`Reject` como payloads cifrados.
- É necessário um framing Norito real para encapsular `aead` em `ConnectFrameV1` como
  binário; implemente isso com os seus bindings de Norito.

### Demo de framing Norito (apenas para exemplo)

O que segue é um framing mínimo, não canônico, suficiente para demos locais. NÃO é
compatível com Norito e deve ser substituído por um codificador/decodificador Norito real
quando disponível. Ele apenas concatena campos em uma ordem little‑endian estável para que
você possa testar o transporte WS de ponta a ponta sem trazer um codec completo para iOS.

Layout (somente demo):
- 32 bytes: sid
- 1 byte: dir (0 = AppToWallet, 1 = WalletToApp)
- 8 bytes: seq (LE)
- 1 byte: kind (1 = Ciphertext)
- 1 byte: repetir `dir` dentro do ciphertext (mantido para paridade com o tipo compartilhado)
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

### Demo de Close/Reject criptografados (framing de exemplo)

Supondo que você já derivou `kWallet` (chave de direção wallet→app), possui `sid` (32 bytes)
e um `URLSessionWebSocketTask` conectado chamado `ws`:

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

Ao receber, use `parseCiphertextV1Demo` e `openEnvelopeV1` para obter o JSON em texto puro e
depois faça o decode para os tipos do lado do app. Substitua este framing de demonstração
por uma codificação Norito real ao integrar.

## Validação em CI

- Antes de fazer alterações em Connect ou na integração do bridge, execute:

  ```bash
  make swift-ci
  ```

  Esse comando valida os fixtures Swift, verifica os feeds dos dashboards e gera os
  resumos da CLI. O workflow de CI depende de metadados do Buildkite
  (`ci/xcframework-smoke:<lane>:device_tag`) para mapear resultados de volta para o
  simulador ou para as lanes de StrongBox; após alterar pipelines ou tags de agentes,
  confirme que esses metadados ainda aparecem nos logs.
- Se a execução falhar, siga `docs/source/swift_parity_triage.md` e inspecione a saída de
  `mobile_ci` para identificar qual lane precisa de regeneração ou de investigação
  adicional.

