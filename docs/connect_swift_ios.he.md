<!-- Hebrew translation of docs/connect_swift_ios.md -->

---
lang: he
direction: rtl
source: docs/connect_swift_ios.md
status: complete
translator: manual
---

<div dir="rtl">

## זרימת SDK מומלצת (ConnectClient + Norito bridge)

מחפשים מדריך מלא לשילוב ב-Xcode (SPM/Pods, XCFramework, ChaChaPoly)? עיינו ב-
`docs/connect_swift_integration.md` לקבלת הוראות מקצה לקצה לפני שממשיכים עם המקטע הזה.

חבילת Swift כבר כוללת שכבת Connect מבוססת Norito:
- `ConnectClient` מתחזק את חיבור ה-WebSocket (`/v2/connect/ws?...`) מעל `URLSessionWebSocketTask`.
- `ConnectSession` מנהלת את מחזור החיים (open → approve/reject → sign → close) ומפענחת מעטפות מוצפנות לאחר הגדרת מפתחות כיוון.
- `ConnectCrypto` מספקת יצירת מפתח X25519 וגזירת מפתחות כיוון התואמים לנוריטו, כך שאין צורך לממש HKDF/ChaChaPoly ידנית.
- `ConnectEnvelope`/`ConnectControl` מייצגים את המסגרות הטיפוסיות שמספק גשר הנוריטו (`connect_norito_bridge`) ושומרות על פריות עם Android/Rust.

לפני שמתחילים סשן:
1. גזרו מזהה סשן באורך ‎32 בתים (`sid`) לפי הנוסחה המשותפת (`"iroha-connect|sid|" || chain_id || app_pk || nonce16`).
2. צרו זוג מפתחות Connect באמצעות `ConnectCrypto.generateKeyPair()` (או חשבו מפתח ציבורי ממפתח פרטי קיים).
3. בנו את כתובת ה-WebSocket והפעילו את `ConnectClient` בתוך קונטקסט אסינכרוני.

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

    // המתנה לתשובת הארנק (approve/reject/close)
    if case .approve(let approval) = try await connectSession.nextControlFrame() {
        let directionKeys = try ConnectCrypto.deriveDirectionKeys(
            localPrivateKey: keyPair.privateKey,
            peerPublicKey: approval.walletPublicKey,
            sessionID: sessionID
        )
        connectSession.setDirectionKeys(directionKeys)

        // פיענוח מעטפות מוצפנות (תוצאות חתימה, Close מוצפן וכו')
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

`ConnectSession` זורקת `ConnectSessionError.missingDecryptionKeys` אם מתקבלות מעטפות מוצפנות לפני שהוגדרו מפתחות כיוון, לכן יש לגזור אותם מיד אחרי שמתקבלת מסגרת `Approve` (המפתח הציבורי של הארנק מגיע בתוך המטען). אם יש צורך לבדוק מסגרות מוצפנות ידנית, ניתן לקרוא ל-`ConnectEnvelope.decrypt(frame:symmetricKey:)` עם המפתח המתאים לכיוון המסגרת.

> **טיפ:** כאשר גשר הנוריטו לא מקושר (למשל בבניית SwiftPM טהורה) ה-SDK נופל למסגרת JSON פשוטה. פונקציות `ConnectCrypto.*` דורשות את ה-XCFramework ולכן יש לקשר אותו באפליקציות הפקה.

## רפרנס CryptoKit ידני (חלופה/מורשת)

אם הגשר אינו זמין או שנדרש אב־טיפוס ללא ה-SDK המלא, ניתן להשתמש ברפרנס הבא כדי לכתוב את צינור ה-X25519 + ChaChaPoly באופן ידני. הקוד נשאר תואם למפרט Norito, אך אינו מספק את בדיקות הפריות/FFI שמגיעות עם ConnectSession.

קטעי הקוד הבאים מדגימים כיצד לבצע:

המקטעים הבאים מדגימים כיצד לבצע:
- חישוב sid ו-salt (‏BLAKE2b-256; הדוגמה מניחה חבילת BLAKE2b חיצונית או ערכים מחושבים מראש).
- ביצוע החלפת מפתחות X25519 (`Curve25519.KeyAgreement`).
- גזירת מפתח כיוון באמצעות HKDF-SHA256 עם salt ו-info.
- בניית AAD מ-`connect:v1`, ‏sid, ‏dir, ‏seq, ‏kind=Ciphertext וגזירת nonce מ-seq.
- הצפנה ופענוח באמצעות ChaChaPoly עם AAD.
- הצטרפות ל-WS עם טוקן.

נדרש iOS 13 ומעלה (CryptoKit). עבור BLAKE2b ניתן להשתמש בחבילת Swift קטנה (לדוגמה <https://github.com/Frizlab/SwiftBLAKE2>) או לחשב sid/salt בצד השרת; להלן מניחים פונקציה `blake2b(data: Data) -> Data`.

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

// גזירת מפתחות כיוון (אפליקציה→ארנק, ארנק→אפליקציה)
func deriveDirectionKeys(sharedSecret: SharedSecret, sid: Data) -> (SymmetricKey, SymmetricKey) {
    let salt = blake2b(data: Data("iroha-connect|salt|".utf8) + sid)
    let ikm = sharedSecret.hkdfDerivedSymmetricKey(using: SHA256.self, salt: salt, sharedInfo: Data(), outputByteCount: 32)
    let kApp = hkdf(ikm, salt: salt, info: Data("iroha-connect|k_app".utf8))
    let kWallet = hkdf(ikm, salt: salt, info: Data("iroha-connect|k_wallet".utf8))
    return (kApp, kWallet)
}

// הצפנת מעטפה עם AAD ו-nonce שנגזר מ-seq
func sealEnvelopeV1(key: SymmetricKey, sid: Data, dir: UInt8, seq: UInt64, payload: Data) -> Data {
    let aad = aadV1(sid: sid, dir: dir, seq: seq)
    let nonce = nonceFromSeq(seq)
    let sealed = try! ChaChaPoly.seal(payload, using: key, nonce: nonce, authenticating: aad)
    return sealed.combined // ciphertext||tag
}

// פענוח מעטפה ואימות seq
func openEnvelopeV1(key: SymmetricKey, sid: Data, dir: UInt8, seq: UInt64, combined: Data) -> Data {
    let aad = aadV1(sid: sid, dir: dir, seq: seq)
    let nonce = nonceFromSeq(seq)
    let box = try! ChaChaPoly.SealedBox(combined: combined)
    return try! ChaChaPoly.open(box, using: key, authenticating: aad)
}

// קידוד Base64URL ללא ריפוד
func base64url(_ data: Data) -> String {
    let b64 = data.base64EncodedString()
    return b64.replacingOccurrences(of: "+", with: "-")
              .replacingOccurrences(of: "/", with: "_")
              .replacingOccurrences(of: "=", with: "")
}

// יצירת סשן Connect: הלקוח מחשב sid ושולח POST ל-/v2/connect/session
func createConnectSession(node: String, chainId: String, appEphemeralPk: Data, completion: @escaping (Result<(sidB64: String, tokenApp: String, tokenWallet: String), Error>) -> Void) {
    // sid = BLAKE2b-256("iroha-connect|sid|" || chain_id || app_pk || nonce16)
    let nonce16 = (0..<16).map { _ in UInt8.random(in: 0...255) }
    var sidInput = Data("iroha-connect|sid|".utf8)
    sidInput.append(Data(chainId.utf8))
    sidInput.append(appEphemeralPk)
    sidInput.append(Data(nonce16))
    let sid = blake2b(data: sidInput)
    let sidB64 = base64url(sid)

    // שליחת JSON { sid, node }
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

// הצטרפות ל-WS עם טוקן (URLSessionWebSocketTask)
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

// שימוש לדוגמה (צד אפליקציה) לאחר חישוב מפתחות ו-sid
// let frameBinary = noritoEncodeConnectFrameV1(... Ciphertext { aead: sealEnvelopeV1(key: kApp, ...) })
// task.send(.data(frameBinary))
```

הערות:
- הלקוח מחשב sid בצד שלו ואז שולח `/v2/connect/session` עם אותו sid כדי לקבל אסימונים לכל תפקיד. בהצטרפות ל-WS יש להשתמש בטוקן.
- לאחר Approve יש לשלוח Close/Reject כמטענים מוצפנים.
- עטיפת `aead` בתוך `ConnectFrameV1` דורשת מסגור Norito; מימשו בעזרת כריכות Norito שלכם.

### הדגמת מסגור Norito (תצורה זמנית)

הקטע הבא הוא מסגור מינימלי שאינו קנוני, דיו לבדיקות מקומיות בלבד. הוא **לא** תואם Norito ועליו להיות מוחלף במקודד/מפענח Norito אמיתי ברגע שזמין. הוא פשוט משלב שדות בסדר יציב בגישה little-endian כדי לאפשר בדיקת WS מקצה לקצה ב-iOS גם ללא קודק מלא.

תצורה (לדמו בלבד):
- ‎32 בתים: sid
- ‎1 בית: dir (‏0=AppToWallet, ‏1=WalletToApp)
- ‎8 בתים: seq (little-endian)
- ‎1 בית: kind (‏1=Ciphertext)
- ‎1 בית: dir משוכפל בתוכן המוצפן (לשמירת התאמה לסוג המשותף)
- ‎4 בתים: ct_len (little-endian)
- ‎ct_len בתים: ‏aead (פלט ChaChaPoly המשולב)

```swift
func le64(_ x: UInt64) -> Data { var v = x.littleEndian; return withUnsafeBytes(of: &v) { Data($0) } }
func le32(_ x: UInt32) -> Data { var v = x.littleEndian; return withUnsafeBytes(of: &v) { Data($0) } }

// מסגור ConnectFrameV1 מסוג Ciphertext (לדמו)
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

// פירוק מסגרת הדמו חזרה לרכיבים
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

// דוגמת שליחה
// let ct = sealEnvelopeV1(key: kApp, sid: sid, dir: 0, seq: 1, payload: payload)
// let frame = frameCiphertextV1Demo(sid: sid, dir: 0, seq: 1, aead: ct)
// wsTask.send(.data(frame))

// דוגמת קבלה
// case .data(let d): if let f = parseCiphertextV1Demo(d) { let pt = openEnvelopeV1(key: kWallet, sid: f.sid, dir: f.dir, seq: f.seq, combined: f.aead) }
```

### הדגמת Close/Reject מוצפנים (מסגור זמני)

ההדגמה מניחה שכבר גזרתם `kWallet` (מפתח הכיוון wallet→app), ברשותכם `sid` באורך ‎32 בתים ושתהליך `URLSessionWebSocketTask` פעיל, בשם `ws`:

```swift
// שליחת Close מוצפן (wallet → app) ב-seq=1
let closePayload = try! JSONSerialization.data(withJSONObject: [
  "Control": [
    "Close": [
      "who": "Wallet", // לדמו בלבד
      "code": 1000,
      "reason": "done",
      "retryable": false
    ]
  ]
], options: [])
let ctClose = sealEnvelopeV1(key: kWallet, sid: sid, dir: 1, seq: 1, payload: closePayload)
let frameClose = frameCiphertextV1Demo(sid: sid, dir: 1, seq: 1, aead: ctClose)
ws.send(.data(frameClose)) { err in if let err = err { print("ws send close:", err) } }

// שליחת Reject מוצפן (wallet → app) ב-seq=2
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

בצד הקולט השתמשו ב-`parseCiphertextV1Demo` וב-`openEnvelopeV1` כדי לשחזר JSON טקסטואלי, ואז המרוהו לטיפוסים באפליקציה. בשילוב אמיתי החליפו את מסגור הדמו במסגור Norito תקני.

## ולידציה ב-CI

- לפני ביצוע שינויים ב-Connect או בארטיפקטי הגשר, הריצו:

  ```bash
  make swift-ci
  ```

  הפקודה בודקת את פיקסצ'רי Swift, מאמתת את פיד הדשבורד ומרנדרת תקצירים. ב-Buildkite התהליך נשען על מטא-דאטה `ci/xcframework-smoke:<lane>:device_tag`, ולכן אחרי שינויי פייפליין/סוכנים ודאו שהמטא-דאטה קיים כדי לשייך תוצאות לליין הנכון (סימולטור או StrongBox).
- במקרה של כישלון, פעלו לפי `docs/source/swift_parity_triage.md` ובדקו את פלט `mobile_ci` כדי לזהות אילו ריצות דורשות ריג'נרציה או טיפול באינסידנט.

</div>
