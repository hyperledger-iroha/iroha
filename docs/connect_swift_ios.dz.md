---
lang: dz
direction: ltr
source: docs/connect_swift_ios.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e3f492c3253124b1066f1ca4389c5ccf4b96a723a2cd9c30ca28ec92775eeaf4
source_last_modified: "2026-01-05T18:22:23.396018+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## གྲོས་འཆར་ཨེསི་ཌི་ཀེ་ ཕོལོ་ (མཐུད་ལམ་ + Norito ཟམ་)

ཨེགསི་ཀོཌི་མཉམ་བསྡོམས་འབད་སའི་ལམ་ཐིག་ཆ་ཚང་དགོཔ་ཨིན་ (SPM/CocoaPods, XCFramework whering, ChaChaPoly གྲོགས་རམ་པ་)?
མཇུག་ལས་མཇུག་ཚུན་ཚོད་ ཐུམ་སྒྲིལ་ལམ་སྟོན་གྱི་དོན་ལུ་ `docs/connect_swift_integration.md` ལུ་བལྟ།

སུའིཕཊི་ཨེསི་ཌི་ཀེ་གིས་ Norito-backed Connect stack:

- I18NI000000011X གིས་ ཝེབ་སོ་ཀེཊི་ (`/v2/connect/ws?...`) འདི་ གུར་ སྐྱེལ་འདྲེན་འབདཝ་ཨིན།
  `URLSessionWebSocketTask`.
- I18NI000000014X གིས་ མི་ཚེ་འཁོར་རིམ་ (ཁ་ཕྱེཔ་ → ཆ་འཇོག་/བཀག་ཆ་ → རྟགས་ → ཉེ་འདབས་) དང་།
  ཁ་ཕྱོགས་ལྡེ་མིག་ཚུ་གཞི་བཙུགས་འབད་ཚར་བའི་ཤུལ་ལས་ སི་ཕར་ཊེགསི་གཞི་ཁྲམ་ཚུ་ གསང་བཟོ་འབདཝ་ཨིན།
- I18NI0000000015X X25519 ལྡེ་མིག་མི་རབས་དང་ Norito-མཐུན་སྒྲིག་ཅན་གྱི་ཁ་ཕྱོགས་ལྡེ་མིག་ཚུ་ ཕྱིར་བཏོན་འབདཝ་ཨིན།
  devation དེ་འབདཝ་ལས་ གློག་རིམ་ཚུ་གིས་ ཨེཆ་ཀེ་ཌི་ཨེཕ་/ཨེཆ་ཨེམ་ཨེ་སི་ཆུ་གཡུར་འདི་ ལག་ཐོག་ལས་ ལག་ལེན་འཐབ་དགོཔ་མེད།
- I18NI0000000016X/`ConnectControl` གིས་བཏོན་མི་ཡིག་དཔར་རྐྱབས་ཡོད་པའི་ I18NT000000003X གིས་ བཏོན་མི་ ཡིག་དཔར་ཅན་གྱི་གཞི་ཁྲམ་ཚུ་ ངོས་འཛིན་འབདཝ་ཨིན།
  རཱསི་ཊི་ཟམ་ (`connect_norito_bridge`); ciphertext ཡིག་ཤུབས་ཚུ་ ༡ ༡ བརྒྱུད་དེ་ གསང་བཟོ་འབདཝ་ཨིན།
  འདྲ་མཚུངས་ FFI གྲོགས་རམ་པ་ Android/Rust, ཆ་སྙོམས་འགན་ལེན་བྱེད་པ།

ལཱ་ཡུན་འགོ་མ་བཙུགས་པའི་ཧེ་མ་:
1. 32-བཱའིཊི་ལཱ་ཡུན་ངོས་འཛིན་པ་ (`sid`) འདི་ BLAKE2b གི་ཐབས་ཤེས་གཅིག་པ་གཞན་བཟུམ་སྦེ་ལག་ལེན་འཐབ་སྟེ་ བཏོན་དགོ།
   ཨེསི་ཌི་ཀེ་ (`"iroha-connect|sid|" || chain_id || app_pk || nonce16`).
2. `ConnectCrypto.generateKeyPair()` བརྒྱུད་དེ་ མཐུད་བྱེད་ལྡེ་མིག་ཆ་གཅིག་བཟོ་བཏོན་འབད་ ཡང་ན་ གསོག་འཇོག་འབད་ཡོད་པའི་ལོག་སྟེ་ལག་ལེན་འཐབ།
   སྒེར་གྱི་ལྡེ་མིག་ (མི་མང་ལྡེ་མིག་ཚུ་ `ConnectCrypto.publicKey(fromPrivateKey:)` དང་གཅིག་ཁར་ ལོག་རྩིས་རྐྱབ་ཚུགས།)
༣ ཝེབ་སོ་ཀེཊི་མཁོ་སྤྲོད་པ་གསར་བསྐྲུན་འབད་ཞིནམ་ལས་ ཨེ་སིན་སྐབས་དོན་ནང་ལུ་འགོ་བཙུགས།

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

`ConnectSession` གིས་ `ConnectSessionError.missingDecryptionKeys` གིས་ སི་ཕར་ཊེགསི་གཞི་ཁྲམ་ཚུ་ བཀོདཔ་ཨིན།
ཁ་ཕྱོགས་ལྡེ་མིག་ཚུ་གཞི་བཙུགས་མ་འབད་བའི་ཧེ་མ་ལས་ལྷོདཔ་ཨིན། ལས་སྦྱོར་འབད་བའི་ཤུལ་ལས་ དེ་འཕྲོ་ལས་ དེ་ཚུ་བཏོན་ནི།
`Approve` ཚད་འཛིན་ (དངུལ་གྱི་མི་མང་ལྡེ་མིག་འདི་ པེ་ལོཌི་ནང་ཚུད་ཡོདཔ་ཨིན།) ཚིག་མཛོད་ཞིབ་དཔྱད་འབད་ནི།
གཞི་ཁྲམ་ཚུ་ ལག་ཐོག་ལས་ འབོད་བརྡ་འབད་དེ་ ཕྱོགས་སྟོན་འབདཝ་ཨིན།
ལྡེ་མིག་འདི་ གཞི་ཁྲམ་གྱི་ཕྱོགས་དང་མཐུན་སྒྲིག་འབདཝ་ཨིན།

> **Tip:** Norito ཟམ་འདི་མེད་པའི་སྐབས་ (དཔེར་ན་ སུའིཕཊི་ཐུམ་སྒྲིལ་འཛིན་སྐྱོང་པ་གིས་ མེད་པར་བཟོ་བསྐྲུན་འབདཝ་ཨིན།
> ཨེགསི་སི་ཕེརེམ་ལཱས), ཨེསི་ཌི་ཀེ་འདི་ རང་བཞིན་གྱིས་ ཇེ་ཨེསི་ཨོ་ཨེན་ ཤིམ་ལུ་ལོག་འགྱོཝ་ཨིན། གསང་བཟོའི་གྲོགས་རམ་པ།
> (`ConnectCrypto.*`) ཟམ་འདི་དགོཔ་ལས་ ཐོན་སྐྱེད་གློག་རིམ་ཚུ་ནང་ XCFramework འདི་འབྲེལ་མཐུད་འབད།

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
I18NF0000008X
## CI བདེན་དཔང་།

- མཐུད་སྦྲེལ་ཡང་ན་ཟམ་མཉམ་བསྡོམས་བསྒྱུར་བཅོས་མ་འབད་བའི་ཧེ་མ་ གཡོག་བཀོལ།

  I18NF0000009X

  བརྡ་བཀོད་འདི་གིས་ སུའིཕཊི་སྒྲིག་ཆས་ཚུ་བདེན་དཔྱད་འབདཝ་ཨིནམ་དང་ ཌེཤ་བོརཌི་ཕིཌི་ཚུ་ཞིབ་དཔྱད་འབདཝ་ཨིནམ་དང་ སི་ཨེལ་ཨའི་འདི་བཀྲམ་སྟོན་འབདཝ་ཨིན།
  བཅུད་བསྡུས། CI ལཱ་གི་རྒྱུན་རིམ་འདི་ Buildkite མེ་ཊ་ཌེ་ཊ་ལུ་ བརྟེན་དོ་ཡོདཔ་ཨིན།
  (`ci/xcframework-smoke:<lane>:device_tag`) ས་ཁྲ་བཟོ་ནི་གི་དོན་ལུ་ དཔེ་སྟོན་འབད་མི་ཡང་ན་ ས་ཁྲ་བཟོ་ནི།
  StrongBox ལམ་ཚུ་—མདོང་ལམ་ཡང་ན་ ལས་ཚབ་ཀྱི་ངོ་རྟགས་ཚུ་བསྒྱུར་བཅོས་འབད་བའི་ཤུལ་ལས་ མེ་ཊ་ཌེ་ཊ་འདི་ད་ལྟོ་ཡང་ ངེས་གཏན་བཟོ།
  དྲན་ཐོ་ཚུ་ནང་འབྱུངམ་ཨིན།
- གཡོག་བཀོལ་མི་འདི་འཐུས་ཤོར་བྱུང་པ་ཅིན་ `docs/source/swift_parity_triage.md` ལུ་རྗེས་སུ་འབྲང་ཞིནམ་ལས་ བརྟག་ཞིབ་འབད།
  I18NI000000030X ཐོན་འབྲས་འདི་ ལམ་གང་བསྐྱར་བཟོ་དགོཔ་ཨིན་ན་ ཡང་ན་ བྱུང་རྐྱེན་གཞན་དགོཔ་ཨིན་ན་ ཤེས་ཚུགས།
  འཛིན་སྐྱོང་པ།