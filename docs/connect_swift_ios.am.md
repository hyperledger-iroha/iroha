---
lang: am
direction: ltr
source: docs/connect_swift_ios.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e3f492c3253124b1066f1ca4389c5ccf4b96a723a2cd9c30ca28ec92775eeaf4
source_last_modified: "2026-01-05T18:22:23.396018+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## የሚመከር የኤስዲኬ ፍሰት (ConnectClient + I18NT0000000X ድልድይ)

ሙሉ የXcode ውህደት መራመጃ ይፈልጋሉ (SPM/CocoaPods፣ XCFramework wiring፣ ChaChaPoly helpers)?
ከጫፍ እስከ ጫፍ የማሸጊያ መመሪያን ለማግኘት `docs/connect_swift_integration.md` ይመልከቱ።

ስዊፍት ኤስዲኬ በNorito የሚደገፍ የግንኙነት ቁልል ይልካል።

- `ConnectClient` የዌብሶኬትን (`/v1/connect/ws?...`) ማጓጓዣን ከላይ ያቆያል
  `URLSessionWebSocketTask`.
- `ConnectSession` የህይወት ኡደቱን ያቀናጃል (ክፍት → ማጽደቅ/ ውድቅ → ምልክት → መዝጋት) እና
  የአቅጣጫ ቁልፎች ከተጫኑ በኋላ የምስጢር ጽሑፍ ፍሬሞችን ዲክሪፕት ያደርጋል።
- `ConnectCrypto` የ X25519 ቁልፍ ትውልድ እና I18NT0000002X የሚያከብር የአቅጣጫ ቁልፍ ያጋልጣል
  አፕሊኬሽኖች HKDF/HMAC የቧንቧ ስራን በእጅ መተግበር የለባቸውም።
- `ConnectEnvelope`/`ConnectControl` የተተየቡ I18NT0000003X ፍሬሞችን ይወክላሉ
  ዝገት ድልድይ (`connect_norito_bridge`); የምስጢር ጽሁፍ ኤንቨሎፕ ዲክሪፕት የተደረገው በ
  ተመሳሳይ የኤፍኤፍአይ ረዳቶች በአንድሮይድ/ዝገት ላይ ጥቅም ላይ ይውላሉ፣ተመሳሳይነት ዋስትና።

አንድ ክፍለ ጊዜ ከመጀመርዎ በፊት፡-
1. የ32-ባይት ክፍለ ጊዜ መለያውን (`sid`) ልክ እንደሌሎች ተመሳሳይ BLAKE2b የምግብ አሰራርን በመጠቀም ያግኙ።
   ኤስዲኬዎች (`"iroha-connect|sid|" || chain_id || app_pk || nonce16`)።
2. የግንኙነት ቁልፍ ጥንድ በ`ConnectCrypto.generateKeyPair()` በኩል ይፍጠሩ ወይም የተከማቸን እንደገና ይጠቀሙ
   የግል ቁልፍ (የወል ቁልፎች በ `ConnectCrypto.publicKey(fromPrivateKey:)` እንደገና ማስላት ይቻላል)።
3. የዌብሶኬት ደንበኛን ይፍጠሩ እና በተመሳሰል አውድ ውስጥ ይጀምሩት።

```swift
import IrohaSwift

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

`ConnectSession` የምስጥር ፅሁፍ ፍሬሞች ከሆኑ `ConnectSessionError.missingDecryptionKeys` ይጥላል
የአቅጣጫ ቁልፎች ከመጫናቸው በፊት ይድረሱ; ከሂደቱ በኋላ ወዲያውኑ ያገኟቸው
`Approve` መቆጣጠሪያ (የኪስ ቦርሳ የህዝብ ቁልፍ በክፍያ ጭነት ውስጥ ተካትቷል)። ምስጢራዊ ጽሑፍን ለመመርመር
ፍሬሞችን በእጅ፣ ከአቅጣጫው ጋር I18NI0000026X ይደውሉ
ከክፈፉ አቅጣጫ ጋር የሚዛመድ ቁልፍ።

> ** ጠቃሚ ምክር:** የ Norito ድልድይ ሲጠፋ (ለምሳሌ የስዊፍት ፓኬጅ አስተዳዳሪ ያለ ይገነባል)
> XCFramework)፣ ኤስዲኬ ወዲያውኑ ወደ JSON shim ይመለሳል። ምስጠራ ረዳቶች
> (`ConnectCrypto.*`) ድልድዩን ይፈልጋል፣ ስለዚህ XCFrameworkን በምርት መተግበሪያዎች ውስጥ ያገናኙት።

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
    let wsUrl = node.replacingOccurrences(of: "http", with: "ws") + "/v1/connect/ws?sid=\(sid)&role=\(role)"
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
## CI ማረጋገጫ

- የግንኙነት ወይም የድልድይ ውህደት ለውጦችን ከማድረግዎ በፊት ያሂዱ

  ```bash
  make swift-ci
  ```

  ትዕዛዙ Swift fixturesን ያረጋግጣል፣ የዳሽቦርድ ምግቦችን ይፈትሻል እና CLI ን ይሰጣል
  ማጠቃለያ የCI የስራ ፍሰት በBuildkite ሜታዳታ ላይ ይመሰረታል።
  (`ci/xcframework-smoke:<lane>:device_tag`) ውጤቱን ወደ አስመሳዩ ለመመለስ ወይም
  StrongBox መስመሮች—የቧንቧ መስመሮችን ወይም የወኪል መለያዎችን ከቀየሩ በኋላ ሜታዳታውን አሁንም ያረጋግጡ
  በምዝግብ ማስታወሻዎች ውስጥ ይታያል.
- ሩጫው ካልተሳካ, `docs/source/swift_parity_triage.md` ን ይከተሉ እና ይፈትሹ
  የትኛው ሌይን መታደስ ወይም ተጨማሪ ክስተት እንደሚያስፈልገው ለመወሰን `mobile_ci` ውፅዓት
  አያያዝ.