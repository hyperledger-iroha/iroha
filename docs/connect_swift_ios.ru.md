---
lang: ru
direction: ltr
source: docs/connect_swift_ios.md
status: complete
translator: manual
source_hash: 0f2dbf44a069becc70aee5abfeff66ad22ca2ae04c605cea9e953d4e750dccc6
source_last_modified: "2025-11-02T04:40:28.813100+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Русский перевод docs/connect_swift_ios.md (Recommended SDK Flow + Manual CryptoKit Reference) -->

## Рекомендуемый поток SDK (ConnectClient + мост Norito)

Нужен полный walkthrough по интеграции в Xcode (SPM/CocoaPods, подключение XCFramework,
helpers для ChaChaPoly)?
См. `docs/connect_swift_integration.md` для сквозного руководства по упаковке.

Swift‑SDK включает стек Connect на базе Norito:

- `ConnectClient` поддерживает транспорт WebSocket (`/v1/connect/ws?...`) поверх
  `URLSessionWebSocketTask`.
- `ConnectSession` управляет жизненным циклом (open → approve/reject → sign → close) и
  расшифровывает фреймы‑ciphertext после установки ключей направления.
- `ConnectCrypto` предоставляет генерацию ключей X25519 и деривацию ключей направления,
  совместимую с Norito, так что приложениям не нужно вручную собирать HKDF/HMAC.
- `ConnectEnvelope`/`ConnectControl` представляют типизированные фреймы Norito, которые
  отдает Rust‑бридж (`connect_norito_bridge`); зашифрованные конверты расшифровываются
  теми же FFI‑helpers, что используются в Android/Rust, что обеспечивает паритет.

Перед запуском сессии:
1. Выведите 32‑байтовый идентификатор сессии (`sid`), используя ту же формулу BLAKE2b,
   что и другие SDK (`"iroha-connect|sid|" || chain_id || app_pk || nonce16`).
2. Сгенерируйте пару ключей Connect через `ConnectCrypto.generateKeyPair()` или
   переиспользуйте сохраненный приватный ключ (публичный ключ можно заново получить
   вызовом `ConnectCrypto.publicKey(fromPrivateKey:)`).
3. Создайте WebSocket‑клиент и запустите его в async‑контексте.

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

`ConnectSession` выбрасывает `ConnectSessionError.missingDecryptionKeys`, если фреймы
ciphertext приходят до установки ключей направления; вычисляйте их сразу после обработки
контроля `Approve` (публичный ключ кошелька включен в payload). Чтобы вручную
проинспектировать зашифрованные фреймы, вызовите
`ConnectEnvelope.decrypt(frame:symmetricKey:)`, передав ключ направления, который
соответствует направлению фрейма.

> **Подсказка.** Когда мост Norito отсутствует (например, сборка через Swift Package
> Manager без XCFramework), SDK автоматически откатывается на JSON‑shim. Helpers шифрования
> (`ConnectCrypto.*`) требуют присутствия моста, поэтому XCFramework должен быть
> подключен в продакшн‑приложениях.

## Справочник по CryptoKit вручную

Если мост Norito недоступен или вам нужно быстро прототипировать без helpers SDK, этот
раздел показывает, как вручную связать X25519 + ChaChaPoly. Подход согласован со
спецификацией Norito, но не дает детерминированных гарантий паритета, которые обеспечивает
SDK “из коробки”.

В этом разделе показано, как:
- вычислить `sid` и `salt` (BLAKE2b‑256; здесь с использованием стороннего пакета BLAKE2b
  или заранее вычисленных значений);
- выполнить согласование ключей X25519 (`Curve25519.KeyAgreement`);
-.derive ключ направления через HKDF‑SHA256 с `salt` и `info`;
- собрать AAD из ("connect:v1", sid, dir, seq, kind=Ciphertext) и nonce из `seq`;
- шифровать/расшифровывать payload’ы с ChaChaPoly и AAD;
- присоединиться к WS с использованием токена.

Требуется iOS 13+ (CryptoKit). Для BLAKE2b используйте небольшой Swift‑пакет (например
https://github.com/Frizlab/SwiftBLAKE2) или вычисляйте `salt`/`sid` на сервере; далее
предполагается, что у вас есть `blake2b(data: Data) -> Data`.

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

Заметки:
- Вычисляйте `sid` на стороне клиента и затем отправляйте POST на `/v1/connect/session`
  с этим `sid`, чтобы получить токены для каждой роли; к WS подключайтесь с токеном.
- После `Approve` отправляйте `Close`/`Reject` как зашифрованные payload’ы.
- Для упаковки `aead` в бинарный `ConnectFrameV1` нужен настоящий Norito‑framing; реализуйте
  его с помощью своих bindings к Norito.

### Демо Norito‑framing (пример)

Далее приведена минимальная, неканоничная схема кадрирования, достаточная для локальных
демо. Она НЕ совместима с Norito и должна быть заменена полноценным Norito‑encoder/decoder
при их появлении. Схема просто конкатенирует поля в стабильном little‑endian порядке, чтобы
можно было протестировать end‑to‑end‑транспорт по WS без полноценного codec’а на iOS.

Layout (только для демо):
- 32 байта: sid
- 1 байт: dir (0 = AppToWallet, 1 = WalletToApp)
- 8 байт: seq (LE)
- 1 байт: kind (1 = Ciphertext)
- 1 байт: повтор `dir` внутри ciphertext (для паритета с общим типом)
- 4 байта: ct_len (LE)
- ct_len байт: aead (ChaChaPoly combined)

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

### Демо зашифрованных Close/Reject (пример framing)

Предположим, что вы уже вывели `kWallet` (ключ направления wallet→app), у вас есть `sid`
(32 байта) и подключенный `URLSessionWebSocketTask` с именем `ws`:

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

При получении используйте `parseCiphertextV1Demo` и `openEnvelopeV1`, чтобы получить JSON
в открытом виде, затем декодируйте его в ваши типы на стороне приложения. При реальной
интеграции замените этот демонстрационный framing на полноценное кодирование Norito.

## Проверка в CI

- Перед изменением Connect или интеграции моста выполните:

  ```bash
  make swift-ci
  ```

  Эта команда проверяет Swift‑fixtures, сверяет потоки dashboards и формирует сводки CLI.
  Workflow CI опирается на метаданные Buildkite
  (`ci/xcframework-smoke:<lane>:device_tag`), чтобы сопоставлять результаты с симулятором
  или StrongBox‑lane; после изменения pipeline’ов или тегов агентов убедитесь, что эти
  метаданные по‑прежнему видны в логах.
- Если запуск завершается неуспешно, следуйте `docs/source/swift_parity_triage.md` и
  изучите вывод `mobile_ci`, чтобы определить, какой lane нуждается в регенерации или
  дополнительной обработке инцидента.
