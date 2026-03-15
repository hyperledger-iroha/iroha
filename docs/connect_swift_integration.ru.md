---
lang: ru
direction: ltr
source: docs/connect_swift_integration.md
status: complete
translator: manual
source_hash: ebdea5644112eab6e2027a7a4744d0ad3ea37a591abd564175f0a9511654e20a
source_last_modified: "2025-11-02T04:40:28.807763+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Русский перевод docs/connect_swift_integration.md (Integrating NoritoBridgeKit) -->

## Интеграция NoritoBridgeKit в iOS‑проект Xcode

В этом руководстве показано, как интегрировать Rust‑bridge Norito
(XCFramework) и Swift‑обёртки в iOS‑приложение, а затем обмениваться Iroha
Connect‑фреймами по WebSocket с использованием тех же Norito‑кодеков, что и
на Rust‑хосте.

Предварительные требования
- Архив `NoritoBridge.xcframework` (собранный CI‑workflow) и Swift‑helper
  `NoritoBridgeKit.swift` (скопируйте версию из
  `examples/ios/NoritoDemo/Sources`, если вы не используете демо‑проект
  напрямую).
- Xcode 15+ и целевая платформа iOS 13+.

Вариант A: Swift Package Manager (рекомендуется)
1) Опубликуйте бинарный SPM‑пакет, используя `Package.swift.template` в
   `crates/connect_norito_bridge/` (подставьте URL и checksum из CI).
2) В Xcode: File → Add Packages… → укажите URL репозитория SPM → добавьте
   продукт `NoritoBridge` в свой target.
3) Добавьте `NoritoBridgeKit.swift` в target приложения (перетащите файл в
   проект, убедитесь, что включена опция “Copy if needed”).

Вариант B: CocoaPods
1) Создайте Podspec на основе `NoritoBridge.podspec.template` (заполните
   `s.source` URL архива zip).
2) Выполните `pod trunk push NoritoBridge.podspec`.
3) В Podfile добавьте `pod 'NoritoBridge'` → `pod install`.
4) Добавьте `NoritoBridgeKit.swift` в target приложения.

Импорты
```swift
import Foundation
import CryptoKit               // ChaChaPoly / HKDF
import IrohaSwift              // ConnectClient / ConnectSession / ConnectCrypto
import NoritoBridge            // Clang-модуль из XCFramework
// Убедитесь, что NoritoBridgeKit.swift добавлен в target
```

### Инициализация Connect‑сессии

`ConnectClient` управляет WebSocket‑соединением, а `ConnectSession`
оркестрирует управляющие фреймы и шифрованные конверты. В следующем
сниппете показано, как dApp открывает сессию, выводит Connect‑ключи и ждёт
ответа на approve.

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
        // Теперь можно расшифровывать зашифрованные конверты
        let firstEnvelope = try await connectSession.nextEnvelope()
        print("payload:", firstEnvelope.payload)
    }
}
```

### Отправка шифрованных фреймов (sign‑запросы и т.п.)

Когда dApp нужно запросить подпись, она использует хелперы Norito‑bridge для
кодирования конверта, шифрует payload с помощью ChaChaPoly и оборачивает её в
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

`ConnectAEAD.header` / `ConnectAEAD.nonce` — вспомогательные функции
(см. сниппет в `docs/connect_swift_ios.md`), построенные на общей
структуре заголовка `connect:v1`. При желании их можно заинлайнить, чтобы
избежать ещё одного helper’а:

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

### Приём и расшифровка фреймов

`ConnectSession` уже предоставляет метод `nextEnvelope()`, который
расшифровывает payload’ы при настроенных direction‑ключах. Если нужен более
низкоуровневый контроль (например, чтобы встроиться в существующий pipeline
декодера), можно воспользоваться helper’ом:

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

`NoritoBridgeKit` также предоставляет хелперы `decodeCiphertextFrame`,
`decodeEnvelopeJson` и `decodeSignResultAlgorithm` для отладки и
интероперабельности. В production‑приложениях рекомендуется опираться на
`ConnectSession` и `ConnectEnvelope`, чтобы поведение совпадало с Rust‑ и
Android‑SDK.

## Проверки в CI

- Перед публикацией обновлённых артефактов bridge или добавлением Connect‑
  интеграций выполните:

  ```bash
  make swift-ci
  ```

  Цель проверяет паритет фикстур, состояние dashboard‑фидов и локально
  рендерит CLI‑сводки. В Buildkite тот же workflow зависит от метаданных
  `ci/xcframework-smoke:<lane>:device_tag`; после изменений pipelines или
  тегов агентов убедитесь, что нужные метаданные присутствуют, чтобы
  dashboards могли привязывать результаты к правильному симулятору или
  StrongBox‑lane.
- Если команда завершается с ошибкой, следуйте плейбуку по паритету
  (`docs/source/swift_parity_triage.md`) и изучите рендеринг `mobile_ci`,
  чтобы определить lane, которому требуется регенерация или отдельный
  инцидент и повторный запуск.

