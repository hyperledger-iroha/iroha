---
lang: fr
direction: ltr
source: docs/connect_swift_ios.md
status: complete
translator: manual
source_hash: 0f2dbf44a069becc70aee5abfeff66ad22ca2ae04c605cea9e953d4e750dccc6
source_last_modified: "2025-11-02T04:40:28.813100+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traduction française de docs/connect_swift_ios.md (Recommended SDK Flow + Manual CryptoKit Reference) -->

## Flux SDK recommandé (ConnectClient + bridge Norito)

Besoin d’un tutoriel complet d’intégration Xcode (SPM/CocoaPods, câblage du XCFramework,
helpers ChaChaPoly) ?
Voir `docs/connect_swift_integration.md` pour le guide de packaging de bout en bout.

Le SDK Swift embarque une pile Connect adossée à Norito :

- `ConnectClient` maintient le transport WebSocket (`/v2/connect/ws?...`) au‑dessus de
  `URLSessionWebSocketTask`.
- `ConnectSession` orchestre le cycle de vie (open → approve/reject → sign → close) et
  déchiffre les frames de ciphertext une fois les clés de direction installées.
- `ConnectCrypto` expose la génération de clés X25519 et la dérivation de clés de
  direction conforme à Norito afin que les apps n’aient pas à implémenter HKDF/HMAC à la
  main.
- `ConnectEnvelope`/`ConnectControl` représentent les frames Norito typées émises par le
  bridge Rust (`connect_norito_bridge`) ; les enveloppes chiffrées sont déchiffrées via les
  mêmes helpers FFI utilisés côté Android/Rust, garantissant la parité.

Avant de démarrer une session :
1. Dérivez l’identifiant de session 32 octets (`sid`) à l’aide de la même recette BLAKE2b
   que les autres SDK (`"iroha-connect|sid|" || chain_id || app_pk || nonce16`).
2. Générez une paire de clés Connect via `ConnectCrypto.generateKeyPair()` ou réutilisez
   une clé privée persistée (la clé publique peut être recomputée avec
   `ConnectCrypto.publicKey(fromPrivateKey:)`).
3. Créez le client WebSocket et démarrez‑le dans un contexte asynchrone.

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

`ConnectSession` lève `ConnectSessionError.missingDecryptionKeys` si des frames chiffrées
arrivent avant que les clés de direction ne soient installées ; dérivez‑les immédiatement
après avoir traité un contrôle `Approve` (la clé publique du wallet est incluse dans le
payload). Pour inspecter les frames chiffrées manuellement, appelez
`ConnectEnvelope.decrypt(frame:symmetricKey:)` avec la clé directionnelle correspondant au
sens du frame.

> **Astuce :** Lorsque le bridge Norito est absent (par exemple, build Swift Package
> Manager sans le XCFramework), le SDK revient automatiquement à un shim JSON. Les helpers
> de chiffrement (`ConnectCrypto.*`) nécessitent le bridge, donc liez le XCFramework dans
> les applications de production.

## Référence CryptoKit manuelle

Si le bridge Norito n’est pas disponible ou si vous devez prototyper sans les helpers du
SDK, la référence suivante montre comment câbler X25519 + ChaChaPoly manuellement. Elle est
alignée sur la spécification Norito mais ne fournit pas les garanties de parité
déterministe que le SDK offre prêt à l’emploi.

Cette section montre comment :
- Calculer `sid` et `salt` (BLAKE2b-256 ; ici en utilisant un package BLAKE2b tiers ou des
  valeurs pré‑calculées).
- Effectuer un accord de clés X25519 (`Curve25519.KeyAgreement`).
- Dériver la clé de direction via HKDF‑SHA256 avec `salt` et `info`.
- Construire l’AAD à partir de ("connect:v1", sid, dir, seq, kind=Ciphertext) et la nonce
  à partir de `seq`.
- Chiffrer/déchiffrer des payloads via ChaChaPoly avec AAD.
- Rejoindre le WS avec un token.

Nécessite iOS 13+ (CryptoKit). Pour BLAKE2b, utilisez un petit package Swift (par exemple
https://github.com/Frizlab/SwiftBLAKE2) ou calculez `salt`/`sid` côté serveur ; l’exemple
suppose que vous disposez de `blake2b(data: Data) -> Data`.

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

Notes :
- Calculez `sid` côté client puis faites un POST vers `/v2/connect/session` avec ce `sid`
  pour obtenir les tokens par rôle ; rejoignez le WS avec le token.
- Après `Approve`, envoyez `Close`/`Reject` comme payloads chiffrés.
- Un framing Norito réel est nécessaire pour encapsuler `aead` dans `ConnectFrameV1` en
  binaire ; implémentez‑le avec vos bindings Norito.

### Démo de framing Norito (exemple)

Ce qui suit est un framing minimal, non canonique, suffisant pour des démos locales. Il
n’est PAS au format Norito et doit être remplacé par un encodeur/décodeur Norito correct
lorsqu’il sera disponible. Il concatène simplement les champs dans un ordre little‑endian
stable afin de tester le transport WS de bout en bout sans embarquer un codec complet dans
iOS.

Layout (démo uniquement) :
- 32 octets : sid
- 1 octet : dir (0 = AppToWallet, 1 = WalletToApp)
- 8 octets : seq (LE)
- 1 octet : kind (1 = Ciphertext)
- 1 octet : répétition de `dir` à l’intérieur du ciphertext (conservé pour la parité avec
  le type partagé)
- 4 octets : ct_len (LE)
- ct_len octets : aead (ChaChaPoly combiné)

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

### Démo Close/Reject chiffrés (framing d’exemple)

En supposant que vous avez déjà dérivé `kWallet` (clé de direction wallet→app), disposez
de `sid` (32 octets) et d’une `URLSessionWebSocketTask` connectée nommée `ws` :

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

À la réception, utilisez `parseCiphertextV1Demo` et `openEnvelopeV1` pour obtenir le JSON en
clair, puis décodez‑le dans vos types côté app. Remplacez ce framing de démonstration par
un encodage Norito réel lors de l’intégration.

## Validation CI

- Avant de modifier Connect ou l’intégration du bridge, exécutez :

  ```bash
  make swift-ci
  ```

  Cette commande valide les fixtures Swift, vérifie les flux de dashboards et génère les
  résumés CLI. Le workflow de CI s’appuie sur des métadonnées Buildkite
  (`ci/xcframework-smoke:<lane>:device_tag`) pour faire correspondre les résultats au
  simulateur ou aux lanes StrongBox ; après toute modification de pipelines ou de tags
  d’agents, vérifiez que ces métadonnées apparaissent toujours dans les logs.
- En cas d’échec, suivez `docs/source/swift_parity_triage.md` et inspectez la sortie de
  `mobile_ci` pour déterminer quel lane nécessite une régénération ou un traitement
  d’incident complémentaire.
